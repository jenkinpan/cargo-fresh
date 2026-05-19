//! `$CARGO_HOME/.crates2.json` 解析：还原一个包安装时用的 Cargo 特性选项。
//!
//! 尽力而为（best-effort）——文件缺失 / 损坏 / 无匹配条目，一律静默回退到
//! 默认行为，绝不让它成为更新失败的原因。只提取 features 三项。

use std::collections::HashMap;

use crate::models::{InstallOpts, PackageSource};

/// 解析 `.crates2.json` 文本，返回 `key -> InstallOpts`。
///
/// key 是 cargo 的 PackageId 原文：`"<name> <version> (<source>)"`。
/// 顶层 JSON 损坏 / 无 `installs` 对象 → 返回空 map（绝不 panic / 报错）。
pub fn parse_crates2(json: &str) -> HashMap<String, InstallOpts> {
    let mut out = HashMap::new();
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let installs = match value.get("installs").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return out,
    };
    for (key, entry) in installs {
        let no_default_features = entry
            .get("no_default_features")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let all_features = entry
            .get("all_features")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let features = entry
            .get("features")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| f.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        out.insert(
            key.clone(),
            InstallOpts {
                no_default_features,
                all_features,
                features,
            },
        );
    }
    out
}

/// 定位并解析 `$CARGO_HOME/.crates2.json`。
///
/// 复用 `registry::cargo_home()`（`CARGO_HOME` env，回退 `$HOME/.cargo`）。
/// 文件不存在 / 不可读 / 解析失败 → 空 map。永不报错、永不打印告警。
pub fn load_install_opts() -> HashMap<String, InstallOpts> {
    let path = match crate::package::registry::cargo_home() {
        Some(p) => p.join(".crates2.json"),
        None => return HashMap::new(),
    };
    let body = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };
    parse_crates2(&body)
}

/// 从已解析的 `.crates2.json` map 里挑出 `name` 对应的安装选项。
///
/// key 形如 `"<name> <version> (<source>)"`。按 name 匹配（key 第一个空格前的
/// 子串）。同名多条时，优先 `(<source>)` 前缀与 `PackageSource` 一致的那条：
/// `registry+` ↔ Crates，`git+` ↔ Git，`path+` ↔ Path。仍无法区分则取首个。
/// 返回 `InstallOpts` 的克隆（调用方需要 owned 值挂到 PackageInfo 上）。
pub fn match_install_opts(
    map: &HashMap<String, InstallOpts>,
    name: &str,
    source: &PackageSource,
) -> Option<InstallOpts> {
    let candidates: Vec<(&String, &InstallOpts)> = map
        .iter()
        .filter(|(k, _)| k.split(' ').next() == Some(name))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let want_prefix = match source {
        PackageSource::Crates => "registry+",
        PackageSource::Git { .. } => "git+",
        PackageSource::Path { .. } => "path+",
        PackageSource::Unknown(_) => "",
    };
    if !want_prefix.is_empty() {
        if let Some((_, opts)) = candidates.iter().find(|(k, _)| {
            k.find('(')
                .and_then(|i| k.get(i + 1..))
                .map(|s| s.starts_with(want_prefix))
                .unwrap_or(false)
        }) {
            return Some((*opts).clone());
        }
    }
    Some(candidates[0].1.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "installs": {
        "ripgrep 14.1.1 (registry+https://github.com/rust-lang/crates.io-index)": {
          "version_req": null, "bins": ["rg"], "features": ["pcre2"],
          "all_features": false, "no_default_features": false,
          "profile": "release", "target": null, "rustc": null
        },
        "cargo-binstall 1.19.1 (registry+https://github.com/rust-lang/crates.io-index)": {
          "version_req": null, "bins": ["cargo-binstall"], "features": [],
          "all_features": false, "no_default_features": false,
          "profile": "release", "target": null, "rustc": null
        },
        "fat 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)": {
          "features": [], "all_features": true, "no_default_features": false
        },
        "slim 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)": {
          "features": [], "all_features": false, "no_default_features": true
        }
      }
    }"#;

    #[test]
    fn parses_features_list() {
        let m = parse_crates2(SAMPLE);
        let o = m
            .get("ripgrep 14.1.1 (registry+https://github.com/rust-lang/crates.io-index)")
            .unwrap();
        assert_eq!(o.features, vec!["pcre2".to_string()]);
        assert!(!o.all_features);
        assert!(!o.no_default_features);
    }

    #[test]
    fn default_package_is_default() {
        let m = parse_crates2(SAMPLE);
        let o = m
            .get("cargo-binstall 1.19.1 (registry+https://github.com/rust-lang/crates.io-index)")
            .unwrap();
        assert!(o.is_default());
    }

    #[test]
    fn parses_all_features_and_no_default() {
        let m = parse_crates2(SAMPLE);
        assert!(
            m.get("fat 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)")
                .unwrap()
                .all_features
        );
        assert!(
            m.get("slim 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)")
                .unwrap()
                .no_default_features
        );
    }

    #[test]
    fn malformed_json_yields_empty_map() {
        assert!(parse_crates2("{not valid json").is_empty());
        assert!(parse_crates2("").is_empty());
    }

    #[test]
    fn missing_installs_key_yields_empty_map() {
        assert!(parse_crates2(r#"{"v1":{}}"#).is_empty());
    }

    #[test]
    fn missing_fields_default_to_false_empty() {
        let m = parse_crates2(
            r#"{"installs":{"x 1.0.0 (registry+https://example)":{"bins":["x"]}}}"#,
        );
        let o = m.get("x 1.0.0 (registry+https://example)").unwrap();
        assert!(o.is_default());
    }

    #[test]
    fn match_by_name_simple() {
        let m = parse_crates2(SAMPLE);
        let o = match_install_opts(&m, "ripgrep", &PackageSource::Crates).unwrap();
        assert_eq!(o.features, vec!["pcre2".to_string()]);
    }

    #[test]
    fn match_no_entry_returns_none() {
        let m = parse_crates2(SAMPLE);
        assert!(match_install_opts(&m, "does-not-exist", &PackageSource::Crates).is_none());
    }

    #[test]
    fn match_prefers_source_consistent_entry() {
        let json = r#"{"installs":{
          "dup 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)": {"features":["reg"]},
          "dup 0.9.0 (git+https://github.com/x/dup#abc)": {"features":["git"]}
        }}"#;
        let m = parse_crates2(json);
        assert_eq!(
            match_install_opts(&m, "dup", &PackageSource::Crates).unwrap().features,
            vec!["reg".to_string()]
        );
        assert_eq!(
            match_install_opts(
                &m,
                "dup",
                &PackageSource::Git { url: "https://github.com/x/dup".into(), rev: None }
            )
            .unwrap()
            .features,
            vec!["git".to_string()]
        );
    }
}
