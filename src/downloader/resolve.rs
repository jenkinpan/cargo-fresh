//! 把 (包名, 版本, 仓库 URL, target triple) 推导成一组候选 GitHub Release URL。
//!
//! 纯函数, 不做 HTTP——HEAD 探测在 fetch.rs。
//!
//! 文件名/路径模板列表借鉴自 cargo-binstall 的
//! `crates/binstalk-fetchers/src/gh_crate_meta/hosting.rs`
//! (Apache-2.0 OR MIT, https://github.com/cargo-bins/cargo-binstall)。
//! 这里只保留 GitHub Release 路径, 不解析 `package.metadata.binstall` 自定义模板。
//! 10 个文件名模板 × 2 个 tag 前缀 (v{version} / {version}) × 2 个归档格式
//! (.tar.gz / .zip) × N 个 target 别名 = 40N 个候选 URL。

use crate::downloader::events::{DownloaderError, UnsupportedReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateUrl {
    pub url: String,
    /// 归档扩展名: "tar.gz" / "zip" / "bin"——给 archive.rs 分派用。
    pub archive_fmt: ArchiveFmt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFmt {
    TarGz,
    Zip,
    Bin,
}

/// 文件名模板 (借鉴自 cargo-binstall FULL_FILENAMES + NOVERSION_FILENAMES)。
/// 占位符: {name} {version} {target} {ext}
const FILENAME_TEMPLATES: &[&str] = &[
    // FULL_FILENAMES — 8 条
    "{name}-{target}-v{version}.{ext}",
    "{name}-{target}-{version}.{ext}",
    "{name}-{version}-{target}.{ext}",
    "{name}-v{version}-{target}.{ext}",
    "{name}_{target}_v{version}.{ext}",
    "{name}_{target}_{version}.{ext}",
    "{name}_{version}_{target}.{ext}",
    "{name}_v{version}_{target}.{ext}",
    // NOVERSION_FILENAMES — 2 条
    "{name}-{target}.{ext}",
    "{name}_{target}.{ext}",
];

const ARCHIVE_EXTS: &[(ArchiveFmt, &str)] = &[(ArchiveFmt::TarGz, "tar.gz"), (ArchiveFmt::Zip, "zip")];

/// 推导候选 URL 列表。第一个返回 2xx 的胜出。
///
/// `repo_url` 形如 "https://github.com/owner/repo" (尾随 / 容忍)。
/// 非 github.com 域返回 `Unsupported(NoMetadataAndNoConvention)`。
///
/// `targets` 是一组等价的 target 别名 (例如 macOS aarch64 通常发布为
/// `aarch64-apple-darwin` 也可能是 `arm64-apple-darwin` 或 `darwin-arm64`)。
/// 输出长度 = 10 文件名 × 2 tag 前缀 × 2 归档 × N 别名 = 40N。
/// `name_candidates` 是一组要试的 `{name}` 替换值: 通常包含 package 名 +
/// binary 名 (例如 tauri-cli 包的 binary 是 cargo-tauri, 而文件名形如
/// `cargo-tauri-aarch64-apple-darwin.zip` ——必须用 binary 名作 {name}
/// 才能命中)。第一个 (canonical, 通常是 package 名) 在前。
pub fn candidate_urls(
    name_candidates: &[String],
    version: &str,
    repo_url: &str,
    targets: &[String],
) -> Result<Vec<CandidateUrl>, DownloaderError> {
    let repo = repo_url.trim_end_matches('/');
    if !repo.starts_with("https://github.com/") && !repo.starts_with("http://github.com/") {
        return Err(DownloaderError::Unsupported(
            UnsupportedReason::NoMetadataAndNoConvention,
        ));
    }
    if targets.is_empty() || name_candidates.is_empty() {
        return Err(DownloaderError::Unsupported(
            UnsupportedReason::NoMetadataAndNoConvention,
        ));
    }

    // tag 路径段:
    // - "v{version}" / "{version}"            通用 (大多数 Rust 单 crate 项目)
    // - "{pkg}-v{version}" / "{pkg}-{version}" monorepo 带前缀 (tauri-cli 用 `tauri-cli-v2.11.2`)
    // - "{pkg}/v{version}" / "{pkg}/{version}" 斜杠分隔 monorepo (URL 里 `/` 不用 %2F, GitHub 会接受)
    // name_candidates[0] 是 canonical package 名 (caller 保证); 子 crate 前缀只用它,
    // 不用 binary 名 (因为 tag 跟着 crate, 不跟着 binary)
    let pkg = &name_candidates[0];
    let tag_paths: Vec<String> = vec![
        format!("v{version}"),
        format!("{version}"),
        format!("{pkg}-v{version}"),
        format!("{pkg}-{version}"),
        format!("{pkg}/v{version}"),
        format!("{pkg}/{version}"),
    ];
    let mut out = Vec::with_capacity(40 * targets.len() * name_candidates.len() * tag_paths.len() / 2);
    let mut seen = std::collections::HashSet::new();
    for name in name_candidates {
        for target in targets {
            for tag_path in &tag_paths {
                let base = format!("{repo}/releases/download/{tag_path}");
                for tmpl in FILENAME_TEMPLATES {
                    for (fmt, ext) in ARCHIVE_EXTS {
                        let filename = tmpl
                            .replace("{name}", name)
                            .replace("{version}", version)
                            .replace("{target}", target)
                            .replace("{ext}", ext);
                        let url = format!("{base}/{filename}");
                        // 同一 name (如 package == binary 时) 会产生重复
                        // 候选, 这里去重避免做无效 HEAD
                        if seen.insert(url.clone()) {
                            out.push(CandidateUrl { url, archive_fmt: *fmt });
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// 当前进程的 target triple 别名列表 (canonical 在最前)。
///
/// 不同发布者命名约定不一 (Rust triple vs Go/npm 风格 vs Apple 简写),
/// 所以同一 (arch, os) 给出多个等价候选, 由 fetch 阶段 HEAD 探测决定哪个真实存在。
pub fn current_targets() -> Vec<String> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match (arch, os) {
        ("aarch64", "macos") => vec![
            "aarch64-apple-darwin".into(),
            "arm64-apple-darwin".into(),
            "darwin-arm64".into(),
        ],
        ("x86_64", "macos") => vec![
            "x86_64-apple-darwin".into(),
            "x64-apple-darwin".into(),
            "darwin-amd64".into(),
            "darwin-x64".into(),
        ],
        ("aarch64", "linux") => vec![
            "aarch64-unknown-linux-gnu".into(),
            "aarch64-unknown-linux-musl".into(),
            "arm64-unknown-linux-gnu".into(),
            "linux-arm64".into(),
        ],
        ("x86_64", "linux") => vec![
            "x86_64-unknown-linux-gnu".into(),
            "x86_64-unknown-linux-musl".into(),
            "linux-amd64".into(),
            "linux-x64".into(),
        ],
        _ => Vec::new(),
    }
}

/// 纯函数:本地枚举一个包在给定版本下"如果有预编译,文件名最可能长什么样"。
///
/// 不带 tag 路径段 —— 这是 asset 文件名,不是 URL。给 github_api::match_winning_asset
/// 用,做集合交。和 `candidate_urls` 的 URL 拼装是同一份模板,通过分离这块逻辑
/// 实现「同一份候选,两个消费方:本地匹配 vs 盲探 URL 拼装」。
pub(crate) fn expected_filenames(
    name_candidates: &[String],
    version: &str,
    targets: &[String],
) -> Vec<String> {
    let mut out = Vec::with_capacity(
        FILENAME_TEMPLATES.len() * ARCHIVE_EXTS.len() * name_candidates.len() * targets.len(),
    );
    let mut seen = std::collections::HashSet::new();
    for name in name_candidates {
        for target in targets {
            for tmpl in FILENAME_TEMPLATES {
                for (_fmt, ext) in ARCHIVE_EXTS {
                    let filename = tmpl
                        .replace("{name}", name)
                        .replace("{version}", version)
                        .replace("{target}", target)
                        .replace("{ext}", ext);
                    if seen.insert(filename.clone()) {
                        out.push(filename);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(target: &str) -> Vec<String> {
        vec![target.into()]
    }

    #[test]
    fn non_github_repo_is_unsupported() {
        let err = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://gitlab.com/x/y",
            &one("x86_64-apple-darwin"),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DownloaderError::Unsupported(UnsupportedReason::NoMetadataAndNoConvention)
        ));
    }

    #[test]
    fn empty_targets_is_unsupported() {
        let err = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &[],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DownloaderError::Unsupported(UnsupportedReason::NoMetadataAndNoConvention)
        ));
    }

    #[test]
    fn single_target_yields_expected_candidate_count() {
        let cands = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        // 10 filenames × 6 tag paths × 2 archives = 120
        assert_eq!(cands.len(), 120);
    }

    #[test]
    fn multiple_targets_yield_proportional_candidates() {
        let cands = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &[
                "aarch64-apple-darwin".into(),
                "arm64-apple-darwin".into(),
                "darwin-arm64".into(),
            ],
        )
        .unwrap();
        // 120 per target × 3 targets = 360
        assert_eq!(cands.len(), 360);
    }

    #[test]
    fn includes_tauri_style_subcrate_prefix_tag() {
        // tauri 用 tag `tauri-cli-v2.11.2`, 文件名 `cargo-tauri-aarch64-apple-darwin.zip`
        let cands = candidate_urls(
            &["tauri-cli".into(), "cargo-tauri".into()],
            "2.11.2",
            "https://github.com/tauri-apps/tauri",
            &one("aarch64-apple-darwin"),
        )
        .unwrap();
        let urls: Vec<&str> = cands.iter().map(|c| c.url.as_str()).collect();
        assert!(urls.iter().any(|u| u == &"https://github.com/tauri-apps/tauri/releases/download/tauri-cli-v2.11.2/cargo-tauri-aarch64-apple-darwin.zip"));
    }

    #[test]
    fn includes_ripgrep_style_v_prefix_name_version_target() {
        let cands = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        let urls: Vec<&str> = cands.iter().map(|c| c.url.as_str()).collect();
        assert!(urls.iter().any(|u| u == &"https://github.com/BurntSushi/ripgrep/releases/download/v14.1.2/ripgrep-14.1.2-x86_64-apple-darwin.tar.gz"));
    }

    #[test]
    fn includes_mdbook_style_v_in_filename() {
        // mdbook 用 `mdbook-v0.5.3-x86_64-apple-darwin.tar.gz`
        let cands = candidate_urls(
            &["mdbook".into()],
            "0.5.3",
            "https://github.com/rust-lang/mdBook",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        let urls: Vec<&str> = cands.iter().map(|c| c.url.as_str()).collect();
        assert!(urls.iter().any(|u| u.contains("mdbook-v0.5.3-x86_64-apple-darwin.tar.gz")));
    }

    #[test]
    fn includes_cargo_deny_style_bare_version_tag() {
        // cargo-deny 用 tag `0.19.7` (无 v), 文件名 `cargo-deny-0.19.7-x86_64-apple-darwin`
        let cands = candidate_urls(
            &["cargo-deny".into()],
            "0.19.7",
            "https://github.com/EmbarkStudios/cargo-deny",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        let urls: Vec<&str> = cands.iter().map(|c| c.url.as_str()).collect();
        assert!(urls.iter().any(|u| u == &"https://github.com/EmbarkStudios/cargo-deny/releases/download/0.19.7/cargo-deny-0.19.7-x86_64-apple-darwin.tar.gz"));
    }

    #[test]
    fn includes_underscore_separated_variant() {
        let cands = candidate_urls(
            &["somepkg".into()],
            "1.0.0",
            "https://github.com/x/y",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        let urls: Vec<&str> = cands.iter().map(|c| c.url.as_str()).collect();
        assert!(urls.iter().any(|u| u.contains("somepkg_x86_64-apple-darwin_1.0.0")));
    }

    #[test]
    fn trailing_slash_in_repo_url_is_tolerated() {
        let a = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep/",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        let b = candidate_urls(
            &["ripgrep".into()],
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn current_targets_returns_nonempty_on_supported_unix() {
        if std::env::consts::OS == "macos" || std::env::consts::OS == "linux" {
            let ts = current_targets();
            assert!(!ts.is_empty());
            assert!(ts[0].contains('-'));
        }
    }

    #[test]
    fn expected_filenames_includes_canonical_ripgrep() {
        let names = vec!["ripgrep".to_string()];
        let targets = vec!["aarch64-apple-darwin".to_string()];
        let filenames = expected_filenames(&names, "15.1.0", &targets);
        assert!(filenames.iter().any(|f| f == "ripgrep-15.1.0-aarch64-apple-darwin.tar.gz"));
    }
}
