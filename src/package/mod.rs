use anyhow::Result;
use colored::*;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

use semver::Version;

use crate::locale::detection::detect_language;
use crate::models::{PackageInfo, PackageSource};

pub mod registry;
pub mod sparse_index;

/// 单进程共享的 HTTP 客户端，启用 connection pool。
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("cargo-fresh/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("build reqwest client")
    })
}

/// 同时拿稳定版与最新预发布版的并发上限，避免 crates.io 限流 / 本地 fd 耗尽
const MAX_CONCURRENT_INDEX_REQUESTS: usize = 16;

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
    use crate::display::{status, status_err};
    let language = detect_language();
    status("Installing", language.get_text("installing_binstall"));

    let output = Command::new("cargo")
        .args(["install", "cargo-binstall"])
        .output()?;

    if output.status.success() {
        status("Installed", language.get_text("binstall_installed_successfully"));
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        status_err(
            "Failed",
            &format!("{}: {}", language.get_text("binstall_install_failed"), stderr.trim()),
        );
        Ok(false)
    }
}

/// 确保 cargo binstall 可用，如果不可用则尝试安装
pub async fn ensure_binstall_available() -> Result<bool> {
    use crate::display::status_dim;
    if is_binstall_available() {
        return Ok(true);
    }
    let language = detect_language();
    status_dim("Note", language.get_text("binstall_not_found"));
    status_dim("Installing", language.get_text("attempting_to_install_binstall"));

    let result = install_binstall().await?;
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
    let mut version_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut seen_packages = HashSet::new();

    for line in output_str.lines() {
        if let Some((name, version, source)) = parse_package_line(line) {
            if !name.is_empty() && !version.is_empty() && seen_packages.insert(name.to_string()) {
                version_map.insert(name.to_string(), version.to_string());
                packages.push(PackageInfo::with_source(
                    name.to_string(),
                    Some(version.to_string()),
                    source,
                ));
            }
        }
    }

    // 缓存版本表，供 get_installed_version 复用，避免每次升级后 N+1 次 `cargo install --list`
    let _ = INSTALLED_VERSION_CACHE.set(std::sync::Mutex::new(version_map));

    Ok(packages)
}

/// 缓存 `cargo install --list` 解析结果的版本表（name -> version）。
///
/// 在 `get_installed_packages` 首次运行时填充；`get_installed_version` 优先读缓存。
/// 升级后通过 `invalidate_installed_version` 让对应条目下次重新查询。
static INSTALLED_VERSION_CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
    OnceLock::new();

/// 升级一个包后调用：从版本表里移除该条目，强制下次重新查询真实安装版本。
pub fn invalidate_installed_version(package_name: &str) {
    if let Some(mutex) = INSTALLED_VERSION_CACHE.get() {
        if let Ok(mut map) = mutex.lock() {
            map.remove(package_name);
        }
    }
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
    // 命中缓存直接返回，避免重复 `cargo install --list`（一次启动里通常调用 N+1 次）
    if let Some(mutex) = INSTALLED_VERSION_CACHE.get() {
        if let Ok(map) = mutex.lock() {
            if let Some(v) = map.get(package_name) {
                return Ok(Some(v.clone()));
            }
        }
    }

    // 缓存未命中：cache 被 invalidate 后的查询走这里，去读真实状态并回填
    let output = Command::new("cargo").args(["install", "--list"]).output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let output_str = String::from_utf8(output.stdout)?;
    for line in output_str.lines() {
        if line.contains(package_name) {
            if let Some((name, version, _)) = parse_package_line(line) {
                if name == package_name {
                    // 回填缓存
                    if let Some(mutex) = INSTALLED_VERSION_CACHE.get() {
                        if let Ok(mut map) = mutex.lock() {
                            map.insert(name.to_string(), version.to_string());
                        }
                    }
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

/// 用 `cargo search` 回退路径取最新版本。
///
/// 仅在 sparse index 失败时使用——`cargo search` 慢（启动 cargo 子进程 + 联网），
/// 且解析依赖输出格式不变。
async fn cargo_search_fallback(
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
        if line.starts_with(&package_prefix) && line.contains('"') {
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
        if line.starts_with(&package_prefix) && line.contains('"') {
            if let Some(version) = extract_version_from_line(line) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

/// 拉取一个包的稳定 + 预发布最新版本。
///
/// 主路径走 sparse index（快、并发友好）；任何失败时回退到 `cargo search`。
/// 回退路径无法一次拿两个版本，只能按 `include_prerelease` 拿一个。
///
/// `registry_override`：CLI `--registry-url` 优先生效；否则从
/// `$CARGO_HOME/config.toml` 读 sparse mirror，再退默认 `index.crates.io`。
pub async fn fetch_latest_versions(
    package_name: &str,
    include_prerelease: bool,
    registry_override: Option<&str>,
) -> sparse_index::LatestVersions {
    let base = registry::sparse_index_base(registry_override);
    match sparse_index::fetch_latest(http_client(), &base, package_name).await {
        Ok(v) => v,
        Err(_) => {
            // 回退到 cargo search——只能拿一个版本，根据需求填入对应字段
            match cargo_search_fallback(package_name, include_prerelease).await {
                Ok(Some(v)) => {
                    if is_stable_version(&v) {
                        sparse_index::LatestVersions {
                            stable: Some(v),
                            prerelease: None,
                        }
                    } else {
                        sparse_index::LatestVersions {
                            stable: None,
                            prerelease: Some(v),
                        }
                    }
                }
                _ => sparse_index::LatestVersions::default(),
            }
        }
    }
}

/// 公开 API：只取最新版本（兼容老调用方）。
#[allow(dead_code)]
pub async fn get_latest_version(
    package_name: &str,
    include_prerelease: bool,
) -> Result<Option<String>> {
    let latest = fetch_latest_versions(package_name, include_prerelease, None).await;
    Ok(if include_prerelease {
        latest.prerelease.or(latest.stable)
    } else {
        latest.stable
    })
}

/// 在已抓到的稳定 + 预发布版本中，选择应写入 `latest_version` 的候选。
///
/// 规则（BREAKING since 0.10.0）：
/// - 优先 stable：只要 stable 存在且 > current（或 current 缺失），就选 stable。
/// - 仅当 `include_prerelease == true` 时，stable 已是最新（或不存在）时才看预发布。
/// - 都没有更新候选时返回 stable（用于"已是最新"的展示）。
/// - 旧版会在不加 `--include-prerelease` 时也把预发布塞进 `latest_version`，
///   触发 dialoguer 的 prerelease bucket——0.10.0 起明确不再这样做（BREAKING）。
///
/// `has_update` 会再用 semver 严格比较一次，保证 yank 回滚不被误判。
pub fn choose_latest(
    stable: Option<&str>,
    prerelease: Option<&str>,
    current: Option<&str>,
    include_prerelease: bool,
) -> Option<String> {
    if let Some(s) = stable {
        if current.map(|c| c != s).unwrap_or(true) {
            return Some(s.to_string());
        }
    }
    if include_prerelease {
        if let Some(pre) = prerelease {
            if current.map(|c| c != pre).unwrap_or(true) {
                return Some(pre.to_string());
            }
        }
    }
    // 没有更新候选：stable 存在就返回它（展示"已是最新"），否则 None
    stable.map(|s| s.to_string())
}

/// 并发查询所有 crates.io 源包的最新版本（稳定 + 预发布一次拿齐）。
///
/// 行为：
/// - Git / Path 源跳过（crates.io 上没有它们的"最新版本"概念）。
/// - 同时拿 stable 和 prerelease，避免旧实现的两次串行扫描。
/// - 用 `Semaphore` 限制 ≤ 16 个并发请求，防止 fd 耗尽 / 触发 crates.io 限流。
/// - 选择逻辑见 `choose_latest`——`include_prerelease=false` 时绝不写入预发布。
pub async fn check_package_updates(
    packages: &mut [PackageInfo],
    verbose: bool,
    include_prerelease: bool,
    registry_override: Option<String>,
) -> Result<()> {
    let language = detect_language();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_INDEX_REQUESTS));
    let mut handles = Vec::new();

    for (index, package) in packages.iter().enumerate() {
        if !package.source.is_crates() {
            continue;
        }
        let package_name = package.name.clone();
        let sem = semaphore.clone();
        let override_clone = registry_override.clone();
        let handle = tokio::spawn(async move {
            // 持有 permit 直到任务结束，自动释放
            let _permit = sem.acquire_owned().await.ok();
            if verbose {
                println!(
                    "{} {}...",
                    language.get_text("checking_package"),
                    package_name.cyan()
                );
            }
            let latest =
                fetch_latest_versions(&package_name, true, override_clone.as_deref()).await;
            (index, package_name, latest)
        });
        handles.push(handle);
    }

    for handle in handles {
        let Ok((index, package_name, latest)) = handle.await else {
            if verbose {
                println!("{}", language.get_text("check_failed").red());
            }
            continue;
        };

        let current = packages[index].current_version.clone();
        let chosen = choose_latest(
            latest.stable.as_deref(),
            latest.prerelease.as_deref(),
            current.as_deref(),
            include_prerelease,
        );

        if verbose {
            match &chosen {
                Some(v) => println!(
                    "  {} {}: {}",
                    package_name,
                    language.get_text("latest_version"),
                    v.green()
                ),
                None => println!(
                    "  {} {}",
                    package_name.red(),
                    language.get_text("unable_to_get_latest_version")
                ),
            }
        }
        packages[index].latest_version = chosen;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- choose_latest ----------

    #[test]
    fn choose_latest_picks_stable_when_newer() {
        assert_eq!(
            choose_latest(Some("1.2.0"), Some("2.0.0-rc.1"), Some("1.1.0"), false),
            Some("1.2.0".to_string())
        );
    }

    #[test]
    fn choose_latest_omits_prerelease_when_flag_off() {
        // 关键 BREAKING 用例：stable 已经是最新，但有更新的预发布——
        // 不加 --include-prerelease 时绝对不能让预发布泄漏到 latest_version
        assert_eq!(
            choose_latest(Some("1.1.0"), Some("2.0.0-rc.1"), Some("1.1.0"), false),
            Some("1.1.0".to_string())
        );
    }

    #[test]
    fn choose_latest_picks_prerelease_when_flag_on() {
        assert_eq!(
            choose_latest(Some("1.1.0"), Some("2.0.0-rc.1"), Some("1.1.0"), true),
            Some("2.0.0-rc.1".to_string())
        );
    }

    #[test]
    fn choose_latest_no_stable_no_prerelease_when_flag_off() {
        // 只有预发布版本但用户没要求看预发布：返回 None（不强升）
        assert_eq!(
            choose_latest(None, Some("0.1.0-alpha.1"), Some("0.0.1"), false),
            None
        );
    }

    #[test]
    fn choose_latest_no_stable_picks_prerelease_when_flag_on() {
        assert_eq!(
            choose_latest(None, Some("0.1.0-alpha.1"), Some("0.0.1"), true),
            Some("0.1.0-alpha.1".to_string())
        );
    }

    #[test]
    fn choose_latest_returns_stable_when_already_latest() {
        // current == stable，预发布无关；返回 stable 让上层展示"Fresh"
        assert_eq!(
            choose_latest(Some("1.0.0"), None, Some("1.0.0"), false),
            Some("1.0.0".to_string())
        );
    }

    #[test]
    fn choose_latest_empty_returns_none() {
        assert_eq!(choose_latest(None, None, Some("1.0.0"), true), None);
        assert_eq!(choose_latest(None, None, None, false), None);
    }

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
