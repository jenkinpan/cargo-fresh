//! 把 (包名, 版本, 仓库 URL, target triple) 推导成一组候选 GitHub Release URL。
//!
//! 纯函数, 不做 HTTP——HEAD 探测在 fetch.rs。
//!
//! 设计权衡: 不解析 `package.metadata.binstall` 自定义模板。MVP 只识别 3 条
//! 内置 GitHub Release 约定 (cargo-binstall 的默认探测模板), 能覆盖主流包
//! ~80%。自定义模板留给 follow-up——加上去后这里再返回模板渲染结果即可。

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

/// 推导候选 URL 列表。第一个返回 2xx 的胜出。
///
/// `repo_url` 形如 "https://github.com/owner/repo" (尾随 / 容忍)。
/// 非 github.com 域返回 `Unsupported(NoMetadataAndNoConvention)`。
///
/// `targets` 是一组等价的 target 别名 (例如 macOS aarch64 通常发布为
/// `aarch64-apple-darwin` 也可能是 `arm64-apple-darwin` 或 `darwin-arm64`)。
/// 调用方传入 `current_targets()` 的结果即可——这里只负责笛卡尔展开。
/// 输出长度 = 3 约定 × 2 归档 × N 别名。
pub fn candidate_urls(
    name: &str,
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
    if targets.is_empty() {
        return Err(DownloaderError::Unsupported(
            UnsupportedReason::NoMetadataAndNoConvention,
        ));
    }

    // 6 约定 × 2 归档 × N 别名 = 12N 个候选。
    // 两种 tag 前缀 (v{version} / {version}) × 三种文件名排列。
    // EmbarkStudios/cargo-deny 用裸版本号当 tag，BurntSushi/ripgrep 用 v 前缀，所以都要试。
    // 别名外层循环——更"像样"的 triple (canonical 在 current_targets 中放在前面) 优先。
    let mut out = Vec::with_capacity(12 * targets.len());
    for target in targets {
        let conventions = [
            // v 前缀 tag (绝大多数 Rust 项目: ripgrep, mdbook, fd, bat …)
            format!("{repo}/releases/download/v{version}/{name}-{version}-{target}"),
            format!("{repo}/releases/download/v{version}/{name}-{target}-{version}"),
            format!("{repo}/releases/download/v{version}/{name}-{target}"),
            // 裸版本号 tag (cargo-deny, 部分 Embark/Google 项目)
            format!("{repo}/releases/download/{version}/{name}-{version}-{target}"),
            format!("{repo}/releases/download/{version}/{name}-{target}-{version}"),
            format!("{repo}/releases/download/{version}/{name}-{target}"),
        ];
        for base in &conventions {
            out.push(CandidateUrl {
                url: format!("{base}.tar.gz"),
                archive_fmt: ArchiveFmt::TarGz,
            });
            out.push(CandidateUrl {
                url: format!("{base}.zip"),
                archive_fmt: ArchiveFmt::Zip,
            });
        }
    }
    Ok(out)
}

/// 当前进程的 target triple 别名列表 (canonical 在最前)。
///
/// 不同发布者命名约定不一 (Rust triple vs Go/npm 风格 vs Apple 简写),
/// 所以同一 (arch, os) 给出多个等价候选, 由 fetch 阶段 HEAD 探测决定哪个真实存在。
/// - macOS aarch64: `aarch64-apple-darwin`, `arm64-apple-darwin`, `darwin-arm64`
/// - macOS x86_64:  `x86_64-apple-darwin`, `x64-apple-darwin`, `darwin-amd64`, `darwin-x64`
/// - Linux aarch64: `aarch64-unknown-linux-gnu`, `aarch64-unknown-linux-musl`,
///   `arm64-unknown-linux-gnu`, `linux-arm64`
/// - Linux x86_64:  `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`,
///   `linux-amd64`, `linux-x64`
/// - Windows / 其它平台: 返回空 Vec——MVP 不支持
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

#[cfg(test)]
mod tests {
    use super::*;

    fn one(target: &str) -> Vec<String> {
        vec![target.into()]
    }

    #[test]
    fn non_github_repo_is_unsupported() {
        let err = candidate_urls(
            "ripgrep",
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
            "ripgrep",
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
    fn single_target_yields_twelve_candidates() {
        let cands = candidate_urls(
            "ripgrep",
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        assert_eq!(cands.len(), 12);
    }

    #[test]
    fn multiple_targets_yield_twelve_times_n_candidates() {
        let cands = candidate_urls(
            "ripgrep",
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &[
            "aarch64-apple-darwin".into(),
            "arm64-apple-darwin".into(),
            "darwin-arm64".into(),
        ],
        )
        .unwrap();
        assert_eq!(cands.len(), 36);
    }

    #[test]
    fn canonical_target_yields_first_candidates() {
        let cands = candidate_urls(
            "ripgrep",
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &["aarch64-apple-darwin".into(), "arm64-apple-darwin".into()],
        )
        .unwrap();
        assert!(cands[0].url.contains("aarch64-apple-darwin"));
        // 第一个 alias 占满前 12 个 (6 约定 × 2 归档)
        assert!(cands[11].url.contains("aarch64-apple-darwin"));
        assert!(cands[12].url.contains("arm64-apple-darwin"));
    }

    #[test]
    fn first_candidate_is_convention_a_targz() {
        let cands = candidate_urls(
            "ripgrep",
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        assert_eq!(
            cands[0].url,
            "https://github.com/BurntSushi/ripgrep/releases/download/v14.1.2/ripgrep-14.1.2-x86_64-apple-darwin.tar.gz"
        );
        assert_eq!(cands[0].archive_fmt, ArchiveFmt::TarGz);
    }

    #[test]
    fn second_candidate_is_convention_a_zip() {
        let cands = candidate_urls(
            "ripgrep",
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        assert_eq!(cands[1].archive_fmt, ArchiveFmt::Zip);
        assert!(cands[1].url.ends_with(".zip"));
    }

    #[test]
    fn trailing_slash_in_repo_url_is_tolerated() {
        let a = candidate_urls(
            "ripgrep",
            "14.1.2",
            "https://github.com/BurntSushi/ripgrep/",
            &one("x86_64-apple-darwin"),
        )
        .unwrap();
        let b = candidate_urls(
            "ripgrep",
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
            assert!(!ts.is_empty(), "supported platform must yield aliases");
            assert!(ts[0].contains('-'), "canonical triple expected, got: {}", ts[0]);
        }
    }

    #[test]
    fn current_targets_macos_aarch64_includes_known_aliases() {
        if std::env::consts::OS == "macos" && std::env::consts::ARCH == "aarch64" {
            let ts = current_targets();
            assert!(ts.iter().any(|t| t == "aarch64-apple-darwin"));
            assert!(ts.iter().any(|t| t == "arm64-apple-darwin"));
            assert!(ts.iter().any(|t| t == "darwin-arm64"));
        }
    }
}
