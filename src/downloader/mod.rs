//! 自实现的二进制下载器——替代 cargo binstall 子进程。
//!
//! 单元拆分:
//! - `events`:  ProgressEvent / DownloaderError 类型 (无逻辑)
//! - `resolve`: 候选 URL 推导 (纯函数)
//! - `fetch`:   HTTP 流式下载 + sha256
//! - `archive`: tar.gz / zip 解压
//! - `install`: atomic rename + .crates2.json 写

pub mod archive;
pub mod events;
pub mod fetch;
pub mod install;
pub mod resolve;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::downloader::events::{DownloaderError, ProgressEvent, UnsupportedReason};

/// 调度器传给 downloader 的输入。
pub struct InstallSpec {
    pub name: String,
    pub version: String,
    pub repo_url: Option<String>,
}

pub struct InstallOutcome {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: String,
}

/// 主入口——把 (spec, events_tx, cancel) 串成完整流水线。
pub async fn download_and_install(
    client: &reqwest::Client,
    spec: InstallSpec,
    old_version: Option<String>,
    events: UnboundedSender<ProgressEvent>,
    cancel: Arc<AtomicBool>,
) -> Result<InstallOutcome, DownloaderError> {
    let _ = events.send(ProgressEvent::Resolving {
        name: spec.name.clone(),
    });
    if cancel.load(Ordering::SeqCst) {
        return Err(DownloaderError::Cancelled);
    }

    let targets = resolve::current_targets();
    if targets.is_empty() {
        return Err(DownloaderError::Unsupported(UnsupportedReason::UnsupportedPlatform));
    }

    let repo_url = spec
        .repo_url
        .as_deref()
        .ok_or(DownloaderError::Unsupported(UnsupportedReason::NoMetadataAndNoConvention))?;

    let candidates = resolve::candidate_urls(&spec.name, &spec.version, repo_url, &targets)?;

    let fetched = fetch::fetch(client, &spec.name, &candidates, &events, cancel.clone()).await?;

    if cancel.load(Ordering::SeqCst) {
        return Err(DownloaderError::Cancelled);
    }

    let _ = events.send(ProgressEvent::Extracting {
        name: spec.name.clone(),
    });

    // archive_fmt 从 fetched URL 的扩展名复算——更鲁棒
    let fmt = if fetched.winning_url.ends_with(".zip") {
        resolve::ArchiveFmt::Zip
    } else {
        resolve::ArchiveFmt::TarGz
    };

    // binary_name 通常等于 spec.name, 但有 cargo subcommand 包 (cargo-deny 装 cargo-deny binary)
    // 这里直接用 spec.name 作为搜索关键字——edge case 留给 follow-up
    let extracted = archive::extract(&fetched.archive_path, fmt, &spec.name)?;

    if cancel.load(Ordering::SeqCst) {
        return Err(DownloaderError::Cancelled);
    }

    let _ = events.send(ProgressEvent::Installing {
        name: spec.name.clone(),
    });

    let _installed_path =
        install::install_binary(&extracted.binary_path, &spec.name, &spec.version)?;

    let _ = events.send(ProgressEvent::Done {
        name: spec.name.clone(),
        version: spec.version.clone(),
    });

    Ok(InstallOutcome {
        name: spec.name,
        old_version,
        new_version: spec.version,
    })
}
