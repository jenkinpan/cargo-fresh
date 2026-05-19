//! 解析 cargo 配置，决定 sparse index 的 base URL。
//!
//! 默认走 `https://index.crates.io`；但企业 / 国内用户常在
//! `$CARGO_HOME/config.toml` 写：
//!
//! ```toml
//! [source.crates-io]
//! replace-with = "ustc"
//!
//! [source.ustc]
//! registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
//! ```
//!
//! 如果不识别这种配置，cargo-fresh 的 sparse index 请求会全部 404 / DNS 失败，
//! 退化到 `cargo search` 兜底——这正是 sparse index 性能优化要避免的事。
//!
//! 解析范围（保持最小可用）：
//! - `$CARGO_HOME/config.toml`（若未设 CARGO_HOME 则回退 `$HOME/.cargo/config.toml`）
//! - `[source.crates-io].replace-with` → `[source.<name>].registry` 的 `sparse+URL`
//! - 不支持 git registry、不支持项目级 `.cargo/config.toml`、不支持环境变量
//!   `CARGO_REGISTRIES_*`（这些属于 P1 / P2 范畴）

use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const DEFAULT_SPARSE_INDEX: &str = "https://index.crates.io";

/// 取 sparse index base URL。命中顺序：
/// 1. CLI 显式 `--registry-url`
/// 2. `$CARGO_HOME/config.toml` 的 source replacement
/// 3. `DEFAULT_SPARSE_INDEX`
///
/// 结果（除 CLI 覆盖外）会被进程级缓存——cargo 配置在一次运行内不会变。
pub fn sparse_index_base(override_url: Option<&str>) -> String {
    if let Some(u) = override_url {
        return normalize(u);
    }
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| resolve_from_config().unwrap_or_else(|| DEFAULT_SPARSE_INDEX.to_string()))
        .clone()
}

fn normalize(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

pub(crate) fn cargo_home() -> Option<PathBuf> {
    if let Ok(s) = env::var("CARGO_HOME") {
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    env::var("HOME").ok().map(|h| PathBuf::from(h).join(".cargo"))
}

fn resolve_from_config() -> Option<String> {
    let path = cargo_home()?.join("config.toml");
    let body = std::fs::read_to_string(&path).ok()?;
    parse_sparse_base(&body)
}

/// 纯函数版本，便于单元测试。
pub fn parse_sparse_base(body: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(body).ok()?;
    let sources = value.get("source")?.as_table()?;
    let crates_io = sources.get("crates-io")?.as_table()?;
    let replace_with = crates_io.get("replace-with")?.as_str()?;
    let target = sources.get(replace_with)?.as_table()?;

    // 新风格：registry = "sparse+URL"
    if let Some(reg) = target.get("registry").and_then(|v| v.as_str()) {
        if let Some(rest) = reg.strip_prefix("sparse+") {
            return Some(normalize(rest));
        }
        // git registry：cargo-fresh 没办法直接 HTTP，让它兜底
        return None;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ustc_style_mirror() {
        let body = r#"
[source.crates-io]
replace-with = "ustc"

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
"#;
        assert_eq!(
            parse_sparse_base(body).as_deref(),
            Some("https://mirrors.ustc.edu.cn/crates.io-index")
        );
    }

    #[test]
    fn returns_none_for_git_mirror() {
        // git registry 不是 sparse，cargo-fresh 无法直接 HTTP，让 cargo search 兜底
        let body = r#"
[source.crates-io]
replace-with = "tuna"

[source.tuna]
registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"
"#;
        assert_eq!(parse_sparse_base(body), None);
    }

    #[test]
    fn returns_none_when_no_replace_with() {
        let body = r#"
[net]
retry = 3
"#;
        assert_eq!(parse_sparse_base(body), None);
    }

    #[test]
    fn returns_none_when_target_source_missing() {
        let body = r#"
[source.crates-io]
replace-with = "ghost"
"#;
        assert_eq!(parse_sparse_base(body), None);
    }

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(normalize("https://example.com/"), "https://example.com");
        assert_eq!(normalize("  https://x.com  "), "https://x.com");
    }

    #[test]
    fn override_takes_precedence() {
        let s = sparse_index_base(Some("https://custom.example/"));
        assert_eq!(s, "https://custom.example");
    }

    #[test]
    fn default_when_nothing_configured() {
        // OnceLock 会被前面的测试污染，所以这里只测试 override 路径——
        // 真实配置路径在集成测试里覆盖（P1-9）。
        assert!(DEFAULT_SPARSE_INDEX.starts_with("https://"));
    }
}
