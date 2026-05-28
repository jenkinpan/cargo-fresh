//! GitHub token discovery for API authentication.
//!
//! 优先级: `$GITHUB_TOKEN` > `$GH_TOKEN` > `gh auth token` 子进程
//!
//! - 匿名调 GitHub API 是 60/hr,带 token 是 5000/hr。CI / 频繁手动跑都
//!   只能靠认证。
//! - 不持久化 token 到磁盘;只读 env / spawn 子进程
//! - `gh auth token` 是 sync `std::process::Command` —— 一次性,有 OnceLock
//!   cache 兜底,至多 50ms 阻塞,简化调用方,允许在 async 路径里同步调
//!
//! Test seam: `discover_token_uncached` 是裸函数(不走 OnceLock),
//! 测试可以每次拿到新鲜决策;生产路径走 `discover_token` (OnceLock)。

use std::sync::OnceLock;

static TOKEN_CACHE: OnceLock<Option<String>> = OnceLock::new();

/// 生产路径:once-per-process discover + cache。
pub fn discover_token() -> Option<&'static str> {
    TOKEN_CACHE
        .get_or_init(discover_token_uncached)
        .as_deref()
}

/// 测试/重新查的路径:每次都走全套流程。
pub fn discover_token_uncached() -> Option<String> {
    if let Ok(t) = std::env::var("GITHUB_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(t) = std::env::var("GH_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    // `gh auth token` 失败/不存在静默兜底为 None —— 用户没装 gh CLI 是常态
    let out = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
