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
pub mod github_api;
pub mod install;
pub mod probe;
pub mod resolve;
pub mod token;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::downloader::events::{DownloaderError, ProgressEvent, UnsupportedReason};

/// 调度器传给 downloader 的输入。
pub struct InstallSpec {
    pub name: String,
    pub version: String,
    pub repo_url: Option<String>,
    /// binary 名候选——`.crates2.json` 的 bins[] 直接搬过来。
    /// 包名 != binary 名时 (ripgrep -> rg) 必须填, 否则解压找不到文件。
    /// 空 Vec → fallback 到 `name` 自身。
    pub bins: Vec<String>,
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

    // {name} 候选: package 名先 (canonical, 多数包文件名沿用), 再加 binary 名
    // (覆盖 tauri-cli 这种情况: 包名 tauri-cli, 二进制 cargo-tauri,
    //  release 文件名是 `cargo-tauri-aarch64-apple-darwin.zip`)
    let mut name_candidates: Vec<String> = vec![spec.name.clone()];
    for b in &spec.bins {
        if !name_candidates.contains(b) {
            name_candidates.push(b.clone());
        }
    }
    let candidates = resolve::candidate_urls(&name_candidates, &spec.version, repo_url, &targets)?;

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

    // 包名 != binary 名时 (ripgrep -> rg), spec.bins 来自 .crates2.json;
    // 空时 fallback 到 spec.name (单 binary 普通包路径)
    let mut bin_candidates: Vec<String> = spec.bins.clone();
    if bin_candidates.is_empty() {
        bin_candidates.push(spec.name.clone());
    }
    let extracted = archive::extract(&fetched.archive_path, fmt, &bin_candidates)?;

    if cancel.load(Ordering::SeqCst) {
        return Err(DownloaderError::Cancelled);
    }

    let _ = events.send(ProgressEvent::Installing {
        name: spec.name.clone(),
    });

    // 实际找到的 binary 名 (可能是 "rg" 而非 "ripgrep") —— install_binary 用这个
    // 当作目标文件名, .crates*.json 写入器会通过 bins[] 找到对应包条目
    let _installed_path = install::install_binary(
        &extracted.binary_path,
        &extracted.binary_name,
        &spec.version,
    )?;

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
