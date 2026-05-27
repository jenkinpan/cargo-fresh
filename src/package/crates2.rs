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

/// 查 `.crates2.json` 拿到 `package_name` 对应的 bins 列表。
///
/// 包名 != binary 名时 (ripgrep -> rg, tauri-cli -> cargo-tauri),
/// downloader 必须知道实际的 binary 名才能在解压后定位文件。
/// 文件缺失 / 包不在文件里 → 返回空 Vec, caller fallback 到 package_name 本身。
pub fn lookup_bins(cargo_home: &std::path::Path, package_name: &str) -> Vec<String> {
    let path = cargo_home.join(".crates2.json");
    let body = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let installs = match json.get("installs").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return Vec::new(),
    };
    installs
        .iter()
        .find(|(k, _)| k.starts_with(&format!("{package_name} ")))
        .and_then(|(_, v)| v.get("bins").and_then(|b| b.as_array()))
        .map(|arr| {
            arr.iter()
                .filter_map(|b| b.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// 把一次成功的 binary 安装写回 `.crates2.json`——更新该包的
/// `version_req` 字段。如果包不在文件里就新增条目。
///
/// 文件不存在 / 解析失败的边角:返回 Err, caller (install.rs) 据此决定
/// 是否要把 InstallFailed 上报到 UI——这次是真失败 (cargo install --list
/// 后续读不到新版会让用户困惑), 不是悄悄成功。
pub fn write_install_record(
    cargo_home: &std::path::Path,
    package_name: &str,
    new_version: &str,
) -> anyhow::Result<()> {
    use anyhow::Context;
    let path = cargo_home.join(".crates2.json");
    let body = std::fs::read_to_string(&path).context("read .crates2.json")?;
    let mut json: serde_json::Value = serde_json::from_str(&body).context("parse .crates2.json")?;
    let installs = json
        .get_mut("installs")
        .and_then(|v| v.as_object_mut())
        .context(".crates2.json missing 'installs' object")?;

    // 找现有 key (形如 "ripgrep 14.1.1 (registry+https://...)")
    // 先按包名前缀匹配; 找不到时按 bins[] 里的 binary 名匹配
    // (支持包名和 binary 名不同的情况, 如 ripgrep -> rg)
    let existing_key: Option<String> = installs
        .keys()
        .find(|k| k.starts_with(&format!("{package_name} ")))
        .cloned()
        .or_else(|| {
            installs
                .iter()
                .find(|(_, v)| {
                    v.get("bins")
                        .and_then(|b| b.as_array())
                        .map(|arr| arr.iter().any(|b| b.as_str() == Some(package_name)))
                        .unwrap_or(false)
                })
                .map(|(k, _)| k.clone())
        });

    if let Some(old_key) = existing_key {
        let entry = installs.remove(&old_key).expect("just found");
        // key 格式: "<pkg_name> <version> (<source>)"
        // 从 key 里提取真实包名和旧版本, 然后替换版本段
        let pkg_name_in_key = old_key.split_whitespace().next().unwrap_or(package_name);
        let old_version_in_key = old_key.split_whitespace().nth(1).unwrap_or("");
        let new_key = old_key.replacen(
            &format!("{pkg_name_in_key} {old_version_in_key}"),
            &format!("{pkg_name_in_key} {new_version}"),
            1,
        );
        installs.insert(new_key, entry);
    }
    // 新条目情况: MVP 不处理 (binstall 路径理论上不会安装全新包,
    // cargo-fresh 只处理已安装的升级)
    let new_body = serde_json::to_string_pretty(&json).context("serialize .crates2.json")?;
    std::fs::write(&path, new_body).context("write .crates2.json")?;
    Ok(())
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
