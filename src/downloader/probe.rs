//! 检查阶段的预编译可用性探测。
//!
//! 替代 0.11 之前调 `cargo binstall --dry-run` 的方案。改用 cargo-fresh 自己
//! 的 downloader resolve + HEAD probe,和真正的 update 路径用同一份 URL
//! 模板,结果不再"check 说 prebuilt 但实际 cargo install"。
//!
//! 三元结论:
//! - `Prebuilt`  至少一个候选 URL HEAD 返回 2xx
//! - `Source`    全部 4xx (典型 404) —— downloader 会回退 cargo install
//! - `Unknown`   全部 5xx / 超时 / 网络异常 —— 探测拉胯,下次再试

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::sync::Semaphore;

use crate::downloader::resolve::candidate_urls;
use crate::models::{PackageInfo, PackageSource, PrebuiltAvailability};

/// 单包最多同时在飞的 HEAD 请求数。
const PROBE_CONCURRENCY: usize = 16;
/// 单 HEAD 超时,和真正的 downloader 一致 (fetch.rs 也用 5s)。
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// 用注入的 URL 列表跑 HEAD 探测——主要给测试用,生产路径走
/// `probe_prebuilt` 它会先 resolve 出候选再调本函数。
pub async fn probe_with_candidates(
    client: &reqwest::Client,
    urls: &[String],
) -> PrebuiltAvailability {
    if urls.is_empty() {
        return PrebuiltAvailability::Source;
    }

    let sem = Arc::new(Semaphore::new(PROBE_CONCURRENCY));
    let cancel = Arc::new(AtomicBool::new(false));
    let mut tasks: FuturesUnordered<_> = urls
        .iter()
        .cloned()
        .map(|url| {
            let sem = sem.clone();
            let client = client.clone();
            let cancel = cancel.clone();
            async move {
                let _permit = sem.acquire_owned().await.ok()?;
                if cancel.load(Ordering::SeqCst) {
                    return None;
                }
                let fut = client.head(&url).send();
                match tokio::time::timeout(PROBE_TIMEOUT, fut).await {
                    Ok(Ok(r)) if r.status().is_success() => Some(ProbeOutcome::Hit),
                    Ok(Ok(r)) if r.status().is_client_error() => Some(ProbeOutcome::NotFound),
                    Ok(Ok(_)) => Some(ProbeOutcome::ServerError),
                    Ok(Err(_)) => Some(ProbeOutcome::NetworkError),
                    Err(_) => Some(ProbeOutcome::Timeout),
                }
            }
        })
        .collect();

    let mut saw_not_found = false;
    let mut saw_uncertain = false;
    while let Some(res) = tasks.next().await {
        match res {
            Some(ProbeOutcome::Hit) => {
                cancel.store(true, Ordering::SeqCst);
                return PrebuiltAvailability::Prebuilt;
            }
            Some(ProbeOutcome::NotFound) => saw_not_found = true,
            Some(ProbeOutcome::ServerError)
            | Some(ProbeOutcome::NetworkError)
            | Some(ProbeOutcome::Timeout) => saw_uncertain = true,
            None => saw_uncertain = true,
        }
    }

    if saw_not_found && !saw_uncertain {
        PrebuiltAvailability::Source
    } else {
        PrebuiltAvailability::Unknown
    }
}

enum ProbeOutcome {
    Hit,
    NotFound,
    ServerError,
    NetworkError,
    Timeout,
}

/// 对单个 crates.io 包跑预编译可用性探测。
///
/// 用"默认 GitHub 仓库猜" (`https://github.com/<name>/<name>`) 生成候选,
/// 不查 crates.io meta 也不读 .crates2.json bins ——check 阶段要快。
/// 漏报 (monorepo / 重命名 binary) 时 caller 看到 `Source`,真正 update
/// 时 downloader 会再做一次全量 resolve,该回去拿预编译的还是会拿到。
pub async fn probe_prebuilt(
    client: &reqwest::Client,
    name: &str,
    version: &str,
) -> PrebuiltAvailability {
    let targets = crate::downloader::resolve::current_targets();
    if targets.is_empty() {
        return PrebuiltAvailability::Unknown;
    }
    let repo_guess = format!("https://github.com/{name}/{name}");
    let name_candidates = vec![name.to_string()];
    let urls: Vec<String> = match candidate_urls(&name_candidates, version, &repo_guess, &targets) {
        Ok(cands) => cands.into_iter().map(|c| c.url).collect(),
        Err(_) => return PrebuiltAvailability::Source,
    };
    probe_with_candidates(client, &urls).await
}

/// 并发对一组 `PackageInfo` 跑预检,只覆盖"有更新且来自 crates.io"的那些,
/// 把结果写回 `pkg.prebuilt`。
pub async fn annotate_updates(packages: &mut [PackageInfo]) {
    let client = crate::package::http_client();
    let targets: Vec<(usize, String, String)> = packages
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(p.source, PackageSource::Crates) && p.has_update())
        .filter_map(|(i, p)| p.latest_version.clone().map(|v| (i, p.name.clone(), v)))
        .collect();

    if targets.is_empty() {
        return;
    }

    let mut tasks: FuturesUnordered<_> = targets
        .into_iter()
        .map(|(i, name, ver)| async move {
            (i, probe_prebuilt(client, &name, &ver).await)
        })
        .collect();

    while let Some((i, kind)) = tasks.next().await {
        packages[i].prebuilt = Some(kind);
    }
}
