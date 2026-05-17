use anyhow::Result;
use colored::*;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::process::Command;
use std::sync::OnceLock;

use semver::Version;

use crate::locale::detection::detect_language;
use crate::models::{PackageInfo, PackageSource};

// 缓存 cargo binstall 的可用性状态
static BINSTALL_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// 检查 cargo binstall 是否可用（使用缓存）
pub fn is_binstall_available() -> bool {
    *BINSTALL_AVAILABLE.get_or_init(|| {
        Command::new("cargo")
            .args(["binstall", "--help"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

/// 安装 cargo binstall
pub async fn install_binstall() -> Result<bool> {
    let language = detect_language();
    println!("{}", language.get_text("installing_binstall").yellow());

    let output = Command::new("cargo")
        .args(["install", "cargo-binstall"])
        .output()?;

    if output.status.success() {
        println!(
            "✅ {}",
            language.get_text("binstall_installed_successfully").green()
        );
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "❌ {}: {}",
            language.get_text("binstall_install_failed").red(),
            stderr
        );
        Ok(false)
    }
}

/// 确保 cargo binstall 可用，如果不可用则尝试安装
pub async fn ensure_binstall_available() -> Result<bool> {
    // 首先检查 cargo binstall 是否已经可用
    if is_binstall_available() {
        return Ok(true);
    }

    let language = detect_language();

    // 只有在 cargo binstall 确实不可用时才显示安装提示
    println!("🔍 {}", language.get_text("binstall_not_found").yellow());
    println!(
        "⚡ {}",
        language.get_text("attempting_to_install_binstall").cyan()
    );

    let result = install_binstall().await?;

    // 如果安装成功，更新缓存
    if result {
        let _ = BINSTALL_AVAILABLE.set(true);
    }

    Ok(result)
}

/// 把模式编译成 case-insensitive 的 GlobSet。
///
/// 单条模式被规范化：不含 `*`/`?`/`[` 等 glob 通配符时，自动包裹为 `*pattern*`
/// 做子串匹配——保留旧版"模糊匹配"的友好行为，同时让真正的 glob 模式
/// （如 `cargo-*`、`*update`）按预期工作。
fn build_globset(patterns: &[&str]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for raw in patterns {
        let p = raw.trim();
        if p.is_empty() {
            continue;
        }
        let normalized = if p.contains(['*', '?', '[']) {
            p.to_string()
        } else {
            format!("*{}*", p)
        };
        let glob = GlobBuilder::new(&normalized)
            .case_insensitive(true)
            .build()?;
        builder.add(glob);
    }
    Ok(builder.build()?)
}

/// 用 glob 模式保留匹配的包；空模式不过滤。
pub fn filter_packages(packages: &mut Vec<PackageInfo>, pattern: &str) -> Result<()> {
    if pattern.trim().is_empty() {
        return Ok(());
    }
    let set = build_globset(&[pattern])?;
    packages.retain(|p| set.is_match(&p.name));
    Ok(())
}

/// 用 glob 模式列表剔除包；空列表无操作。
///
/// 与 `filter_packages` 相反：匹配任一模式的包会被移除。
pub fn exclude_packages(packages: &mut Vec<PackageInfo>, patterns: &[String]) -> Result<()> {
    if patterns.is_empty() {
        return Ok(());
    }
    let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
    let set = build_globset(&refs)?;
    packages.retain(|p| !set.is_match(&p.name));
    Ok(())
}

pub async fn get_installed_packages() -> Result<Vec<PackageInfo>> {
    let output = Command::new("cargo").args(["install", "--list"]).output()?;

    if !output.status.success() {
        let language = detect_language();
        anyhow::bail!("{}", language.get_text("cargo_install_list_failed"));
    }

    let output_str = String::from_utf8(output.stdout)?;
    let mut packages = Vec::new();
    let mut seen_packages = HashSet::new();

    for line in output_str.lines() {
        if let Some((name, version, source)) = parse_package_line(line) {
            if !name.is_empty() && !version.is_empty() && seen_packages.insert(name.to_string()) {
                packages.push(PackageInfo::with_source(
                    name.to_string(),
                    Some(version.to_string()),
                    source,
                ));
            }
        }
    }

    Ok(packages)
}

/// 解析 `cargo install --list` 的包行。
///
/// 输入示例：
/// - `cargo-update v15.0.1 (registry+https://github.com/rust-lang/crates.io-index):`
/// - `some-tool v0.1.0 (git+https://github.com/foo/bar#abcdef):`
/// - `local-tool v0.1.0 (path+file:///Users/me/repos/local-tool):`
/// - `ripgrep v14.1.1:`（旧 cargo 或省略来源时）
///
/// 返回 `(name, version, source)`。无法解析时返回 `None`。
pub fn parse_package_line(line: &str) -> Option<(&str, &str, PackageSource)> {
    // 包行总是以 ":" 结尾；二进制子行（"    rg"）和空行都不会满足
    let line_no_colon = line.trim_end().strip_suffix(':')?;

    // 名字与版本之间用 " v" 分隔。先剥掉行尾的 ":"，
    // 避免 URL 里的 "://" 被误识别为字段分隔符。
    let (name_part, rest) = line_no_colon.split_once(" v")?;
    let name = name_part.trim();
    let rest = rest.trim();
    if name.is_empty() || rest.is_empty() {
        return None;
    }

    // rest 形如 "1.0.0 (git+URL#sha)" 或 "1.0.0 (registry+...)" 或 "1.0.0"
    let (version, source) = match rest.find('(') {
        Some(paren_idx) => {
            let version = rest[..paren_idx].trim();
            let source_str = rest[paren_idx + 1..].trim_end_matches(')').trim();
            (version, parse_source(source_str))
        }
        None => (rest, PackageSource::Crates),
    };

    if version.is_empty() {
        return None;
    }
    Some((name, version, source))
}

/// 解析括号内的来源字串。
///
/// - `registry+URL` → Crates
/// - `git+URL` 或 `git+URL#rev` → Git
/// - `path+file:///DIR` 或 `path+DIR` → Path
/// - 其他未知格式默认归类为 Crates（保守，不丢失包）
fn parse_source(s: &str) -> PackageSource {
    if let Some(rest) = s.strip_prefix("git+") {
        let (url, rev) = match rest.split_once('#') {
            Some((u, r)) => (u.to_string(), Some(r.to_string())),
            None => (rest.to_string(), None),
        };
        PackageSource::Git { url, rev }
    } else if let Some(rest) = s.strip_prefix("path+file://") {
        PackageSource::Path { dir: rest.to_string() }
    } else if let Some(rest) = s.strip_prefix("path+") {
        PackageSource::Path { dir: rest.to_string() }
    } else {
        // registry+... 或缺省时归为 crates.io
        PackageSource::Crates
    }
}

pub async fn get_installed_version(package_name: &str) -> Result<Option<String>> {
    let output = Command::new("cargo").args(["install", "--list"]).output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let output_str = String::from_utf8(output.stdout)?;

    for line in output_str.lines() {
        if line.contains(package_name) {
            if let Some((name, version, _)) = parse_package_line(line) {
                if name == package_name {
                    return Ok(Some(version.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn extract_version_from_line(line: &str) -> Option<String> {
    line.find("= \"").and_then(|start| {
        line[start + 3..]
            .find("\"")
            .map(|end| line[start + 3..start + 3 + end].to_string())
    })
}

/// 判断版本字符串是否为稳定版（无预发布标签）。
///
/// 使用 semver 标准：稳定版 = `Version.pre.is_empty()`。
/// 解析失败时保守返回 true（视为稳定版），避免把无法解析的合法版本号误归为预发布。
pub fn is_stable_version(version: &str) -> bool {
    Version::parse(version)
        .map(|v| v.pre.is_empty())
        .unwrap_or(true)
}

pub async fn get_latest_version(
    package_name: &str,
    include_prerelease: bool,
) -> Result<Option<String>> {
    let output = Command::new("cargo")
        .args(["search", package_name, "--limit", "10"])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let output_str = String::from_utf8(output.stdout)?;
    let package_prefix = format!("{} =", package_name);

    // 查找精确匹配的包名
    for line in output_str.lines() {
        if line.starts_with(&package_prefix) && line.contains("\"") {
            if let Some(version) = extract_version_from_line(line) {
                if include_prerelease || is_stable_version(&version) {
                    return Ok(Some(version));
                }
            }
        }
    }

    // 如果没有找到稳定版本且不包含预发布版本，返回None
    if !include_prerelease {
        return Ok(None);
    }

    // 如果包含预发布版本但没有找到精确匹配，返回第一个匹配的版本
    for line in output_str.lines() {
        if line.starts_with(&package_prefix) && line.contains("\"") {
            if let Some(version) = extract_version_from_line(line) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

pub async fn check_package_updates(
    packages: &mut [PackageInfo],
    verbose: bool,
    include_prerelease: bool,
) -> Result<()> {
    let language = detect_language();

    // 创建并发任务来检查所有包
    let mut handles = Vec::new();

    for (index, package) in packages.iter().enumerate() {
        // Git / Path 源的"最新版本"在 crates.io 上无意义——跳过查询，
        // 用户若想升级需要重新跑 cargo install --git / --path（updater 会用对应命令）
        if !package.source.is_crates() {
            continue;
        }
        let package_name = package.name.clone();
        let handle = tokio::spawn(async move {
            if verbose {
                println!(
                    "{} {}...",
                    language.get_text("checking_package"),
                    package_name.cyan()
                );
            }

            let result = get_latest_version(&package_name, include_prerelease).await;
            (index, package_name, result)
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        match handle.await {
            Ok((index, package_name, result)) => match result {
                Ok(Some(version)) => {
                    packages[index].latest_version = Some(version.clone());
                    if verbose {
                        println!(
                            "  {} {}: {}",
                            package_name,
                            language.get_text("latest_version"),
                            version.green()
                        );
                    }
                }
                Ok(None) => {
                    if verbose {
                        println!(
                            "  {} {}",
                            package_name.red(),
                            language.get_text("unable_to_get_latest_version")
                        );
                    }
                }
                Err(e) => {
                    if verbose {
                        println!(
                            "  {} {}: {}",
                            package_name.red(),
                            language.get_text("check_failed"),
                            e
                        );
                    }
                }
            },
            Err(e) => {
                if verbose {
                    println!("Task failed: {}", e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- parse_package_line ----------

    fn parse_nv(line: &str) -> Option<(&str, &str)> {
        parse_package_line(line).map(|(n, v, _)| (n, v))
    }

    #[test]
    fn parse_package_line_standard() {
        let line = "ripgrep v14.1.1:";
        assert_eq!(parse_nv(line), Some(("ripgrep", "14.1.1")));
        assert_eq!(parse_package_line(line).unwrap().2, PackageSource::Crates);
    }

    #[test]
    fn parse_package_line_with_build_metadata() {
        // semver 允许 +build 元数据，解析必须保留完整版本号
        let line = "some-tool v1.2.3+arch64:";
        assert_eq!(parse_nv(line), Some(("some-tool", "1.2.3+arch64")));
    }

    #[test]
    fn parse_package_line_with_prerelease() {
        let line = "cargo-fresh v0.9.10-rc.1:";
        assert_eq!(parse_nv(line), Some(("cargo-fresh", "0.9.10-rc.1")));
    }

    #[test]
    fn parse_package_line_empty() {
        assert_eq!(parse_package_line(""), None);
    }

    #[test]
    fn parse_package_line_missing_colon() {
        assert_eq!(parse_package_line("ripgrep v14.1.1"), None);
    }

    #[test]
    fn parse_package_line_missing_v_prefix() {
        assert_eq!(parse_package_line("ripgrep 14.1.1:"), None);
    }

    #[test]
    fn parse_package_line_binary_subline_returns_none() {
        // cargo install --list 的第二行通常是缩进的二进制名，没有 " v" 也没有 ":"
        assert_eq!(parse_package_line("    rg"), None);
    }

    #[test]
    fn parse_package_line_registry_source() {
        let line = "cargo-update v15.0.1 (registry+https://github.com/rust-lang/crates.io-index):";
        let (name, version, source) = parse_package_line(line).unwrap();
        assert_eq!(name, "cargo-update");
        assert_eq!(version, "15.0.1");
        assert_eq!(source, PackageSource::Crates);
    }

    #[test]
    fn parse_package_line_git_source_with_rev() {
        let line = "some-tool v0.1.0 (git+https://github.com/foo/bar#abc123):";
        let (name, version, source) = parse_package_line(line).unwrap();
        assert_eq!(name, "some-tool");
        assert_eq!(version, "0.1.0");
        assert_eq!(
            source,
            PackageSource::Git {
                url: "https://github.com/foo/bar".to_string(),
                rev: Some("abc123".to_string()),
            }
        );
    }

    #[test]
    fn parse_package_line_git_source_without_rev() {
        let line = "some-tool v0.1.0 (git+https://github.com/foo/bar):";
        let source = parse_package_line(line).unwrap().2;
        assert_eq!(
            source,
            PackageSource::Git {
                url: "https://github.com/foo/bar".to_string(),
                rev: None,
            }
        );
    }

    #[test]
    fn parse_package_line_path_source() {
        let line = "local-tool v0.1.0 (path+file:///Users/me/repos/local-tool):";
        let source = parse_package_line(line).unwrap().2;
        assert_eq!(
            source,
            PackageSource::Path {
                dir: "/Users/me/repos/local-tool".to_string(),
            }
        );
    }

    // ---------- extract_version_from_line ----------

    #[test]
    fn extract_version_from_line_standard() {
        let line = r#"ripgrep = "14.1.1"    # ripgrep recursively searches..."#;
        assert_eq!(extract_version_from_line(line), Some("14.1.1".to_string()));
    }

    #[test]
    fn extract_version_from_line_missing_quotes() {
        let line = "ripgrep = 14.1.1";
        assert_eq!(extract_version_from_line(line), None);
    }

    #[test]
    fn extract_version_from_line_prerelease() {
        let line = r#"my-crate = "1.0.0-beta.2"  # description"#;
        assert_eq!(extract_version_from_line(line), Some("1.0.0-beta.2".to_string()));
    }

    // ---------- is_stable_version ----------

    #[test]
    fn is_stable_version_stable() {
        assert!(is_stable_version("1.0.0"));
        assert!(is_stable_version("14.1.1"));
        assert!(is_stable_version("0.9.10"));
    }

    #[test]
    fn is_stable_version_prerelease() {
        assert!(!is_stable_version("1.0.0-alpha"));
        assert!(!is_stable_version("1.0.0-beta.2"));
        assert!(!is_stable_version("2.0.0-rc.1"));
    }

    #[test]
    fn is_stable_version_with_build_metadata_is_stable() {
        // +build 元数据不是预发布
        assert!(is_stable_version("1.0.0+arch64"));
        assert!(is_stable_version("2.3.4+20240101"));
    }

    #[test]
    fn is_stable_version_substring_rc_is_not_misidentified() {
        // 关键回归测试：旧实现用 contains("rc") 会把这些误判为预发布
        // semver 标准下它们都是 valid 的稳定版本（pre 段为空）
        assert!(is_stable_version("1.0.0+arc-build"));
        assert!(is_stable_version("1.0.0+rc-meta"));
    }

    // ---------- filter_packages / exclude_packages ----------

    fn pkg(name: &str) -> PackageInfo {
        PackageInfo::new(name.to_string(), Some("1.0.0".to_string()))
    }

    #[test]
    fn filter_packages_empty_pattern_no_op() {
        let mut pkgs = vec![pkg("cargo-edit"), pkg("ripgrep")];
        filter_packages(&mut pkgs, "").unwrap();
        assert_eq!(pkgs.len(), 2);
    }

    #[test]
    fn filter_packages_plain_word_is_substring() {
        // 无 glob 字符的模式自动包裹为 *p*，保留旧版友好行为
        let mut pkgs = vec![pkg("cargo-edit"), pkg("ripgrep"), pkg("cargo-update")];
        filter_packages(&mut pkgs, "cargo").unwrap();
        assert_eq!(pkgs.len(), 2);
        assert!(pkgs.iter().all(|p| p.name.contains("cargo")));
    }

    #[test]
    fn filter_packages_glob_prefix() {
        // cargo-* 只匹配以 cargo- 开头的，不再像旧版那样把 *cargo* 和 cargo* 当一回事
        let mut pkgs = vec![pkg("cargo-edit"), pkg("ripcargo"), pkg("cargo-update")];
        filter_packages(&mut pkgs, "cargo-*").unwrap();
        assert_eq!(pkgs.len(), 2);
        assert!(pkgs.iter().all(|p| p.name.starts_with("cargo-")));
    }

    #[test]
    fn filter_packages_glob_suffix() {
        let mut pkgs = vec![pkg("cargo-update"), pkg("cargo-edit"), pkg("topgrade")];
        filter_packages(&mut pkgs, "*update").unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "cargo-update");
    }

    #[test]
    fn filter_packages_no_match_clears_list() {
        let mut pkgs = vec![pkg("ripgrep"), pkg("tokei")];
        filter_packages(&mut pkgs, "nonexistent").unwrap();
        assert!(pkgs.is_empty());
    }

    #[test]
    fn filter_packages_case_insensitive() {
        let mut pkgs = vec![pkg("CargoEdit"), pkg("RIPGREP")];
        filter_packages(&mut pkgs, "cargo").unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "CargoEdit");
    }

    #[test]
    fn exclude_packages_empty_list_no_op() {
        let mut pkgs = vec![pkg("cargo-edit"), pkg("ripgrep")];
        exclude_packages(&mut pkgs, &[]).unwrap();
        assert_eq!(pkgs.len(), 2);
    }

    #[test]
    fn exclude_packages_removes_matches() {
        let mut pkgs = vec![pkg("cargo-edit"), pkg("ripgrep"), pkg("cargo-update")];
        exclude_packages(&mut pkgs, &["cargo-*".to_string()]).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "ripgrep");
    }

    #[test]
    fn exclude_packages_multiple_patterns() {
        let mut pkgs = vec![pkg("cargo-edit"), pkg("ripgrep"), pkg("tokei")];
        exclude_packages(
            &mut pkgs,
            &["cargo*".to_string(), "tokei".to_string()],
        )
        .unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "ripgrep");
    }

    // ---------- PackageInfo::has_update ----------

    fn pkg_with_latest(name: &str, current: &str, latest: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            current_version: Some(current.to_string()),
            latest_version: Some(latest.to_string()),
            source: PackageSource::Crates,
        }
    }

    #[test]
    fn has_update_normal_upgrade() {
        let p = pkg_with_latest("x", "1.0.0", "1.1.0");
        assert!(p.has_update());
    }

    #[test]
    fn has_update_same_version() {
        let p = pkg_with_latest("x", "1.0.0", "1.0.0");
        assert!(!p.has_update());
    }

    #[test]
    fn has_update_rollback_returns_false() {
        // 关键回归测试：current > latest（yank 回滚场景）必须返回 false，
        // 旧实现用字符串 != 会误报需要更新
        let p = pkg_with_latest("x", "2.0.0", "1.9.0");
        assert!(!p.has_update());
    }

    #[test]
    fn has_update_major_upgrade() {
        let p = pkg_with_latest("x", "1.9.0", "2.0.0");
        assert!(p.has_update());
    }

    #[test]
    fn has_update_prerelease_to_stable() {
        // semver: 1.0.0-rc.1 < 1.0.0
        let p = pkg_with_latest("x", "1.0.0-rc.1", "1.0.0");
        assert!(p.has_update());
    }

    #[test]
    fn has_update_build_metadata_differs_is_treated_as_update() {
        // semver 规范说 build metadata "不参与版本优先级判断"，但 semver crate 的
        // Ord 为了提供全序仍会比较 build 段。对 cargo-fresh 来说这恰好对路：
        // 同语义版本但 build 不同通常意味着上游重新发布了 artifact，值得 `cargo install`。
        let p = pkg_with_latest("x", "1.0.0", "1.0.0+xyz");
        assert!(p.has_update());
    }

    #[test]
    fn has_update_unparseable_falls_back_to_string_compare() {
        // 任一版本无法解析时，fallback 到字符串 != 比较
        let p = pkg_with_latest("x", "git-abc123", "git-def456");
        assert!(p.has_update());
        let same = pkg_with_latest("x", "git-abc123", "git-abc123");
        assert!(!same.has_update());
    }

    #[test]
    fn has_update_missing_versions_returns_false() {
        let p = PackageInfo::new("x".to_string(), Some("1.0.0".to_string()));
        assert!(!p.has_update());

        let p2 = PackageInfo::new("x".to_string(), None);
        assert!(!p2.has_update());
    }

    // ---------- PackageInfo::is_prerelease ----------

    #[test]
    fn is_prerelease_detects_pre_segment() {
        let p = pkg_with_latest("x", "1.0.0", "2.0.0-alpha.1");
        assert!(p.is_prerelease());
    }

    #[test]
    fn is_prerelease_stable_returns_false() {
        let p = pkg_with_latest("x", "1.0.0", "2.0.0");
        assert!(!p.is_prerelease());
    }

    #[test]
    fn is_prerelease_build_metadata_is_not_prerelease() {
        // 关键回归测试：含 "rc" 字面量但 pre 段为空的稳定版不再误判
        let p = pkg_with_latest("x", "1.0.0", "1.0.0+rc-meta");
        assert!(!p.is_prerelease());
    }
}
