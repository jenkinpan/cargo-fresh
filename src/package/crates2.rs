//! `$CARGO_HOME/.crates2.json` 解析：还原一个包安装时用的 Cargo 特性选项。
//!
//! 尽力而为（best-effort）——文件缺失 / 损坏 / 无匹配条目，一律静默回退到
//! 默认行为，绝不让它成为更新失败的原因。只提取 features 三项。

use std::collections::HashMap;

use crate::models::InstallOpts;

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
}
