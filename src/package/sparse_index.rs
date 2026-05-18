//! crates.io sparse index 客户端。
//!
//! 替代 `cargo search` 的旗舰路径：直接 HTTP GET sparse index 的索引文件，
//! 按行解析 JSON 取最新版本。相比 `cargo search` 启动 cargo 子进程的方式：
//!
//! - 单次请求约 50–100ms，N 个包并发只需 1 个连接（不用启动 N 个子进程）
//! - 没有 cargo 解析输出格式变化的风险
//! - 失败时由 `package::get_latest_version` 自动回退到 `cargo search`
//!
//! 参考：<https://doc.rust-lang.org/cargo/reference/registry-index.html#index-files>

use anyhow::Result;
use semver::Version;
use serde::Deserialize;

/// sparse index 每一行的 JSON 结构（只取我们需要的字段）。
#[derive(Debug, Deserialize)]
struct IndexEntry {
    vers: String,
    #[serde(default)]
    yanked: bool,
}

/// 同时返回稳定版与预发布版的最新版本号。
///
/// 把"是否含预发布"的策略上移到调用方，避免对同一 crate 两次请求。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LatestVersions {
    pub stable: Option<String>,
    pub prerelease: Option<String>,
}

/// 按 crates.io 规则计算包名在 sparse index 中的相对路径。
///
/// 规则（小写化包名后）：
/// - 1 字符 → `1/{name}`
/// - 2 字符 → `2/{name}`
/// - 3 字符 → `3/{first_char}/{name}`
/// - 4+ 字符 → `{c1c2}/{c3c4}/{name}`
pub fn index_path(name: &str) -> String {
    let lower = name.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    match chars.len() {
        0 => String::new(),
        1 => format!("1/{}", lower),
        2 => format!("2/{}", lower),
        3 => format!("3/{}/{}", chars[0], lower),
        _ => format!(
            "{}{}/{}{}/{}",
            chars[0], chars[1], chars[2], chars[3], lower
        ),
    }
}

/// 解析 sparse index 响应文本，按 semver 找出最新稳定版和最新预发布版。
///
/// 纯函数——逻辑可在不联网情况下做单元测试。
pub fn parse_index_body(body: &str) -> LatestVersions {
    let mut max_stable: Option<Version> = None;
    let mut max_prerelease: Option<Version> = None;

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<IndexEntry>(line) else {
            continue;
        };
        if entry.yanked {
            continue;
        }
        let Ok(version) = Version::parse(&entry.vers) else {
            continue;
        };
        if version.pre.is_empty() {
            if max_stable.as_ref().is_none_or(|cur| version > *cur) {
                max_stable = Some(version);
            }
        } else if max_prerelease.as_ref().is_none_or(|cur| version > *cur) {
            max_prerelease = Some(version);
        }
    }

    LatestVersions {
        stable: max_stable.map(|v| v.to_string()),
        prerelease: max_prerelease.map(|v| v.to_string()),
    }
}

/// 网络拉取并解析 sparse index。
///
/// `base_url` 一般是 `https://index.crates.io`，企业 / 国内镜像下从
/// `package::registry::sparse_index_base` 解析得来。包名为空、网络错误、
/// HTTP 5xx 时会进行一次快速重试（500ms 退避）；HTTP 4xx 不重试
/// （404 通常表示包名错误或镜像缺失，重试浪费时间）。所有尝试都
/// 失败时返回 Err，调用方应回退到 `cargo search`（除非 `--no-cargo-search-fallback`）。
pub async fn fetch_latest(
    client: &reqwest::Client,
    base_url: &str,
    name: &str,
) -> Result<LatestVersions> {
    const MAX_ATTEMPTS: u32 = 2;
    const RETRY_DELAY_MS: u64 = 500;

    let path = index_path(name);
    if path.is_empty() {
        anyhow::bail!("empty package name");
    }
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path);

    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let body = resp.text().await?;
                    return Ok(parse_index_body(&body));
                }
                // 4xx 不重试——通常是真的没这个包，再请求一次浪费时间
                if status.is_client_error() {
                    anyhow::bail!("sparse index HTTP {}", status);
                }
                last_err = Some(anyhow::anyhow!("sparse index HTTP {}", status));
            }
            Err(e) => {
                last_err = Some(anyhow::Error::new(e));
            }
        }
        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("sparse index: all retries exhausted")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_path_one_char() {
        assert_eq!(index_path("a"), "1/a");
    }

    #[test]
    fn index_path_two_chars() {
        assert_eq!(index_path("ab"), "2/ab");
    }

    #[test]
    fn index_path_three_chars() {
        assert_eq!(index_path("abc"), "3/a/abc");
    }

    #[test]
    fn index_path_four_plus_chars() {
        assert_eq!(index_path("cargo"), "ca/rg/cargo");
        assert_eq!(index_path("cargo-fresh"), "ca/rg/cargo-fresh");
        assert_eq!(index_path("ripgrep"), "ri/pg/ripgrep");
    }

    #[test]
    fn index_path_lowercases() {
        // crates.io 索引路径用小写
        assert_eq!(index_path("Cargo-Fresh"), "ca/rg/cargo-fresh");
    }

    #[test]
    fn parse_picks_max_stable() {
        let body = r#"
{"name":"ripgrep","vers":"13.0.0","yanked":false}
{"name":"ripgrep","vers":"14.1.1","yanked":false}
{"name":"ripgrep","vers":"14.0.3","yanked":false}
"#;
        let v = parse_index_body(body);
        assert_eq!(v.stable.as_deref(), Some("14.1.1"));
        assert_eq!(v.prerelease, None);
    }

    #[test]
    fn parse_separates_stable_and_prerelease() {
        let body = r#"
{"name":"foo","vers":"1.0.0","yanked":false}
{"name":"foo","vers":"2.0.0-rc.1","yanked":false}
{"name":"foo","vers":"2.0.0-beta.5","yanked":false}
"#;
        let v = parse_index_body(body);
        assert_eq!(v.stable.as_deref(), Some("1.0.0"));
        // semver 排序：rc.1 > beta.5
        assert_eq!(v.prerelease.as_deref(), Some("2.0.0-rc.1"));
    }

    #[test]
    fn parse_skips_yanked() {
        let body = r#"
{"name":"foo","vers":"1.0.0","yanked":false}
{"name":"foo","vers":"2.0.0","yanked":true}
"#;
        let v = parse_index_body(body);
        // 2.0.0 被 yank，不应该被选中
        assert_eq!(v.stable.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn parse_skips_unparseable_lines() {
        let body = r#"
not valid json
{"name":"foo","vers":"1.0.0","yanked":false}
{"name":"foo","vers":"not.a.version","yanked":false}
{"name":"foo","vers":"2.0.0","yanked":false}
"#;
        let v = parse_index_body(body);
        assert_eq!(v.stable.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn parse_empty_body_returns_empty() {
        let v = parse_index_body("");
        assert_eq!(v, LatestVersions::default());
    }

    #[test]
    fn parse_only_yanked_returns_empty() {
        let body = r#"
{"name":"foo","vers":"1.0.0","yanked":true}
{"name":"foo","vers":"2.0.0","yanked":true}
"#;
        let v = parse_index_body(body);
        assert_eq!(v, LatestVersions::default());
    }
}
