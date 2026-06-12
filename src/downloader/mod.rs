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
use crate::downloader::resolve::CandidateUrl;

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

/// 在 download_and_install 之前先调一次 GitHub Releases API,如果命中
/// 直接返回单元素的候选列表;fetch::fetch 拿到只跑 1 个 HEAD 就胜出,
/// 跳过 360 候选盲探。
///
/// 返回 None 的两种情况调用方都走 fallback:
/// - repo_url 不在 github.com 上 (e.g. gitlab.com 自托管)
/// - API 任一形态失败 (RateLimited / Network / Parse)
async fn try_api_winning_url(
    client: &reqwest::Client,
    spec: &InstallSpec,
    repo_url: &str,
    targets: &[String],
    name_candidates: &[String],
) -> Option<CandidateUrl> {
    let (owner, repo) = github_api::parse_owner_repo(repo_url)?;
    let token = token::discover_token();
    let expected = Arc::new(resolve::expected_filenames(
        name_candidates,
        &spec.version,
        targets,
    ));
    let pkg = name_candidates.first()?;
    let tag_strings: Vec<String> = vec![
        format!("v{}", spec.version),
        spec.version.clone(),
        format!("{pkg}-v{}", spec.version),
        format!("{pkg}-{}", spec.version),
        format!("{pkg}/v{}", spec.version),
        format!("{pkg}/{}", spec.version),
    ];
    crate::display::status_debug(
        "downloader",
        &format!(
            "{}: github={}/{} token={} tags={} expected={}",
            spec.name,
            owner,
            repo,
            token::discover_token_source(),
            tag_strings.len(),
            expected.len()
        ),
    );

    let tag_count = tag_strings.len();

    // 并发探测 6 个 tag, 最多 2 个同时在飞 (5000/hr 认证限额下安全)。
    use futures_util::stream::{FuturesUnordered, StreamExt};
    let sem = Arc::new(tokio::sync::Semaphore::new(2));
    let mut tasks = FuturesUnordered::new();

    for tag in tag_strings {
        let sem = sem.clone();
        let client = client.clone();
        let owner = owner.clone();
        let repo = repo.clone();
        let expected = expected.clone();
        let spec_name = spec.name.clone();
        tasks.push(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            match github_api::fetch_release_assets(
                &client,
                "https://api.github.com",
                &owner,
                &repo,
                &tag,
                token,
            )
            .await
            {
                Ok(assets) => {
                    if let Some(asset) = github_api::match_winning_asset(&assets, &expected) {
                        let asset = asset.clone();
                        crate::display::status_debug(
                            "downloader",
                            &format!(
                                "{}: API tag={} matched asset={}",
                                spec_name, tag, asset.name
                            ),
                        );
                        return Some(Ok((asset, tag)));
                    }
                    crate::display::status_debug(
                        "downloader",
                        &format!(
                            "{}: API tag={} 200 but none of {} assets matched",
                            spec_name,
                            tag,
                            assets.len()
                        ),
                    );
                    None
                }
                Err(github_api::GithubApiError::NotFound) => {
                    crate::display::status_debug(
                        "downloader",
                        &format!("{}: API tag={} 404", spec_name, tag),
                    );
                    None
                }
                Err(e) => {
                    crate::display::status_debug(
                        "downloader",
                        &format!(
                            "{}: API tag={} error={}, falling back to URL enumeration",
                            spec_name, tag, e
                        ),
                    );
                    Some(Err(e))
                }
            }
        });
    }

    let mut result: Option<github_api::ReleaseAsset> = None;
    while let Some(res) = tasks.next().await {
        match res {
            Some(Ok((asset, _tag))) => {
                result = Some(asset);
                break; // first hit wins — drop(tasks) cancels the rest
            }
            Some(Err(_)) => {
                return None; // fatal — fall back to URL enumeration
            }
            None => {} // NotFound or no-match, keep waiting
        }
    }
    drop(tasks);

    match result {
        Some(asset) => {
            let archive_fmt = if asset.name.ends_with(".zip") {
                resolve::ArchiveFmt::Zip
            } else if asset.name.ends_with(".tar.gz") || asset.name.ends_with(".tgz") {
                resolve::ArchiveFmt::TarGz
            } else {
                resolve::ArchiveFmt::Bin
            };
            Some(CandidateUrl {
                url: asset.browser_download_url.clone(),
                archive_fmt,
            })
        }
        None => {
            crate::display::status_debug(
                "downloader",
                &format!(
                    "{}: API exhausted {} tags with no match, falling back to URL enumeration",
                    spec.name, tag_count
                ),
            );
            None
        }
    }
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
        return Err(DownloaderError::Unsupported(
            UnsupportedReason::UnsupportedPlatform,
        ));
    }

    let repo_url = spec
        .repo_url
        .as_deref()
        .ok_or(DownloaderError::Unsupported(
            UnsupportedReason::NoMetadataAndNoConvention,
        ))?;

    // {name} 候选: package 名先 (canonical, 多数包文件名沿用), 再加 binary 名
    // (覆盖 tauri-cli 这种情况: 包名 tauri-cli, 二进制 cargo-tauri,
    //  release 文件名是 `cargo-tauri-aarch64-apple-darwin.zip`)
    let mut name_candidates: Vec<String> = vec![spec.name.clone()];
    for b in &spec.bins {
        if !name_candidates.contains(b) {
            name_candidates.push(b.clone());
        }
    }
    // API-first: 1 GitHub API request -> single-URL candidate list -> fetch
    // does 1 HEAD + stream GET. Fallback to full 360-URL candidate enumeration
    // when API is unreachable / rate-limited / repo isn't on github.com.
    let candidates =
        match try_api_winning_url(client, &spec, repo_url, &targets, &name_candidates).await {
            Some(winner) => {
                crate::display::status_debug(
                    "downloader",
                    &format!("{}: 1 candidate (API winner)", spec.name),
                );
                vec![winner]
            }
            None => {
                let urls =
                    resolve::candidate_urls(&name_candidates, &spec.version, repo_url, &targets)?;
                crate::display::status_debug(
                    "downloader",
                    &format!(
                        "{}: {} candidates (URL enumeration fallback)",
                        spec.name,
                        urls.len()
                    ),
                );
                urls
            }
        };

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
