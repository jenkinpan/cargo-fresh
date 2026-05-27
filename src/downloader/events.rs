//! ProgressEvent + DownloaderError 公共定义。这些类型横跨调度器 /
//! downloader 内部 / UI 层, 单独放一个模块避免循环依赖。

use thiserror::Error;

/// 下载进度事件——downloader 在每个阶段切换时通过 mpsc 发出, UI 层订阅。
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// 开始为这个包推导候选 URL。
    Resolving { name: String },
    /// 找到一个候选 URL, 即将 HEAD 探测。
    UrlCandidate { name: String, url: String },
    /// 下载流进度。`total` 在 server 没返回 Content-Length 时为 None。
    Downloading {
        name: String,
        got: u64,
        total: Option<u64>,
    },
    /// 下载完成, 正在算 sha256。
    Verifying { name: String },
    /// 校验通过, 正在解压。
    Extracting { name: String },
    /// 解压完成, 正在 atomic rename 到 ~/.cargo/bin。
    Installing { name: String },
    /// 整条管道完成。
    Done { name: String, version: String },
    /// 这个包失败了——UI 层用来打 Fallback / Skip 提示, 调度器据此推 Phase 2。
    Failed { name: String, reason: String },
}

/// downloader 返回的错误。三个变体决定调度器后续行为:
/// - `Unsupported`: 不试都知道搞不定——调度器立刻推 Phase 2, UI 显示 "Skip"。
/// - `Failed`:     试过但栽了——调度器推 Phase 2, UI 显示 "Fallback"。
/// - `Cancelled`:  Ctrl-C 命中 await 点——调度器丢弃此包, 主循环退出。
#[derive(Debug, Error)]
pub enum DownloaderError {
    #[error("downloader unsupported: {0:?}")]
    Unsupported(UnsupportedReason),
    #[error("downloader failed ({kind:?}): {source}")]
    Failed {
        kind: FailureKind,
        #[source]
        source: anyhow::Error,
    },
    #[error("cancelled by user")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedReason {
    NoMetadataAndNoConvention,
    UnknownArchiveFormat,
    UnsupportedPlatform,
    GitSource,
    PathSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    AllUrlsFailed,
    DownloadInterrupted,
    ChecksumMismatch,
    ExtractFailed,
    InstallFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_display_includes_reason() {
        let e = DownloaderError::Unsupported(UnsupportedReason::GitSource);
        let s = format!("{e}");
        assert!(s.contains("GitSource"), "got: {s}");
    }

    #[test]
    fn failed_chains_source() {
        let inner = anyhow::anyhow!("connect refused");
        let e = DownloaderError::Failed {
            kind: FailureKind::AllUrlsFailed,
            source: inner,
        };
        let s = format!("{e}");
        assert!(s.contains("AllUrlsFailed"), "got: {s}");
    }
}
