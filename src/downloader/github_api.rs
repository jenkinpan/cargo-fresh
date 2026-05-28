//! GitHub Releases API client. 一次 API 拿一个 release 的所有 assets,
//! 替代盲探 360 个 HEAD 的方案。
//!
//! Endpoint: `GET https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}`
//! Auth:    `Authorization: Bearer <token>` (可选;匿名 60/hr,认证 5000/hr)
//!
//! 设计要点:
//! - `base_url` 显式注入,wiremock 测试不需要打真实 github
//! - 401/403/429 统一映射为 `RateLimited` —— 调用方都做同一件事(fallback)
//! - 404 单独映射为 `NotFound`,让调用方区分"该 tag 不存在"(试下一个 tag)
//!   和"网络坏了"(直接 fallback)

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    #[allow(dead_code)]
    tag_name: Option<String>,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug)]
pub enum GithubApiError {
    NotFound,
    RateLimited,
    Network(reqwest::Error),
    Parse(String),
}

impl std::fmt::Display for GithubApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GithubApiError::NotFound => write!(f, "release not found"),
            GithubApiError::RateLimited => write!(f, "GitHub API rate limited or unauthorized"),
            GithubApiError::Network(e) => write!(f, "network error: {e}"),
            GithubApiError::Parse(s) => write!(f, "JSON parse error: {s}"),
        }
    }
}
impl std::error::Error for GithubApiError {}

pub async fn fetch_release_assets(
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    tag: &str,
    token: Option<&str>,
) -> Result<Vec<ReleaseAsset>, GithubApiError> {
    let url = format!("{base_url}/repos/{owner}/{repo}/releases/tags/{tag}");
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(
            "User-Agent",
            "cargo-fresh (https://github.com/jenkinpan/cargo-fresh)",
        );
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = req.send().await.map_err(GithubApiError::Network)?;
    match resp.status().as_u16() {
        200 => {
            let body: ReleaseResponse = resp
                .json()
                .await
                .map_err(|e| GithubApiError::Parse(e.to_string()))?;
            Ok(body.assets)
        }
        404 => Err(GithubApiError::NotFound),
        401 | 403 | 429 => Err(GithubApiError::RateLimited),
        other => Err(GithubApiError::Parse(format!("unexpected status {other}"))),
    }
}

/// 从 crates.io 拿到的 repo URL 抠 (owner, repo)。仅支持 github.com;其他
/// 平台返回 None,让调用方走 fallback。
///
/// 接受形式: `https://github.com/<owner>/<repo>`, 可带 `.git` 后缀或 `/` 结尾。
pub fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim_end_matches('/');
    let stripped = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))?;
    let stripped = stripped.strip_suffix(".git").unwrap_or(stripped);
    let mut parts = stripped.split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

/// 在 API 返回的 assets 里找第一个文件名能匹配本地 expected_filenames 的。
///
/// O(N×M) 但 N 一般 < 30 (常见 release asset 数), M 一般 < 100 (filename
/// 模板交叉),不需要 HashSet 优化。返回 &ReleaseAsset 让调用方拿到
/// `browser_download_url` 直接 stream GET。
pub fn match_winning_asset<'a>(
    assets: &'a [ReleaseAsset],
    expected: &[String],
) -> Option<&'a ReleaseAsset> {
    assets.iter().find(|a| expected.iter().any(|e| e == &a.name))
}
