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
/// 走和真正 update 路径相同的 resolve 输入 —— crates.io API 的 `repository`
/// 字段 + `.crates2.json` 的 `bins[]`。早期版本试过 `{name}/{name}` 启发式
/// 兜底,但 ripgrep/cargo-deny/mdbook/tauri-cli 这些热门包没一个是这个形状
/// (BurntSushi/ripgrep、EmbarkStudios/cargo-deny ...),启发式探出的 `Source`
/// 比 `Unknown` 更具误导性,所以 API 失败直接报 `Unknown`,让用户知道这次没探明。
pub async fn probe_prebuilt(
    client: &reqwest::Client,
    name: &str,
    version: &str,
) -> PrebuiltAvailability {
    let targets = crate::downloader::resolve::current_targets();
    if targets.is_empty() {
        return PrebuiltAvailability::Unknown;
    }
    let repo = match crate::package::crates_api::fetch_repo_url(client, name).await {
        Some(r) => r,
        None => return PrebuiltAvailability::Unknown,
    };

    // bins 让 monorepo + binary 名 ≠ package 名的情况能命中
    // (tauri-cli 的 `cargo-tauri-aarch64-apple-darwin.zip`)
    let mut name_candidates = vec![name.to_string()];
    if let Some(home) = crate::package::registry::cargo_home() {
        for b in crate::package::crates2::lookup_bins(&home, name) {
            if !name_candidates.contains(&b) {
                name_candidates.push(b);
            }
        }
    }

    // --- API path (1-6 requests instead of 360 HEADs, now concurrent) -----
    if let Some((owner, repo_name)) = crate::downloader::github_api::parse_owner_repo(&repo) {
        let token = crate::downloader::token::discover_token();
        let expected = Arc::new(crate::downloader::resolve::expected_filenames(
            &name_candidates,
            version,
            &targets,
        ));
        let candidate_tags = tag_candidates(&name_candidates[0], version);

        #[derive(Debug)]
        enum ApiTagResult {
            Hit,
            NoMatch,
            NotFound,
        }

        let sem = Arc::new(Semaphore::new(2));
        let mut tasks = FuturesUnordered::new();

        for tag in candidate_tags {
            let sem = sem.clone();
            let client = client.clone();
            let owner = owner.clone();
            let repo_name = repo_name.clone();
            let expected = expected.clone();
            tasks.push(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                match crate::downloader::github_api::fetch_release_assets(
                    &client,
                    "https://api.github.com",
                    &owner,
                    &repo_name,
                    &tag,
                    token,
                )
                .await
                {
                    Ok(assets) => {
                        if crate::downloader::github_api::match_winning_asset(&assets, &expected)
                            .is_some()
                        {
                            return Some(ApiTagResult::Hit);
                        }
                        Some(ApiTagResult::NoMatch)
                    }
                    Err(crate::downloader::github_api::GithubApiError::NotFound) => {
                        Some(ApiTagResult::NotFound)
                    }
                    Err(_) => None, // RateLimited / Network / Parse — fatal
                }
            });
        }

        let mut hit = false;
        let mut hit_api = false;

        while let Some(res) = tasks.next().await {
            match res {
                Some(ApiTagResult::Hit) => {
                    hit = true;
                    break;
                }
                Some(ApiTagResult::NoMatch) | Some(ApiTagResult::NotFound) => {
                    hit_api = true;
                }
                None => {
                    // fatal: API unreachable — will fall through to HEAD
                }
            }
        }
        drop(tasks);

        if hit {
            return PrebuiltAvailability::Prebuilt;
        }
        if hit_api {
            // 至少一个 tag 200 但 asset 全不匹配,或所有 tag 都 404 —— 都算 Source
            return PrebuiltAvailability::Source;
        }
        // API 一次都没成 —— 走 fallback 兜底
    }

    // --- Fallback: 旧的 HEAD 盲探 (API 不可达 / repo 不是 github.com) -------
    let urls: Vec<String> = match candidate_urls(&name_candidates, version, &repo, &targets) {
        Ok(cands) => cands.into_iter().map(|c| c.url).collect(),
        Err(_) => return PrebuiltAvailability::Source,
    };
    probe_with_candidates(client, &urls).await
}

/// 生成 tag 名候选, 沿用 resolve::candidate_urls 的 6 个模板形状。
fn tag_candidates(pkg: &str, version: &str) -> Vec<String> {
    vec![
        format!("v{version}"),
        version.to_string(),
        format!("{pkg}-v{version}"),
        format!("{pkg}-{version}"),
        format!("{pkg}/v{version}"),
        format!("{pkg}/{version}"),
    ]
}

/// 顺序对一组 `PackageInfo` 跑预检,只覆盖"有更新且来自 crates.io"的那些,
/// 把结果写回 `pkg.prebuilt`。
///
/// 故意串行而非并发。每个 probe_prebuilt 内部已经把 GitHub HEAD 探测吃到
/// `Semaphore(16)` 上限了;若再让 N 个 probe 并发,等于同时压 16×N 条到
/// github.com,触发匿名限流后报 `[probe failed]`,接着 update 阶段的
/// HEAD 也连带 throttle 失败。串行后单包 ~1-2s,4 个候选最多 ~10s,值得。
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

    for (i, name, ver) in targets {
        let kind = probe_prebuilt(client, &name, &ver).await;
        packages[i].prebuilt = Some(kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_candidates_covers_common_shapes() {
        let tags = tag_candidates("ripgrep", "15.1.0");
        assert!(tags.contains(&"v15.1.0".to_string()));
        assert!(tags.contains(&"15.1.0".to_string()));
        assert!(tags.contains(&"ripgrep-v15.1.0".to_string()));
    }
}
