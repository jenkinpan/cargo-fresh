//! `$CARGO_HOME/.crates.toml` 维护器。
//!
//! 这是 cargo 内部记录"哪些 binary 由哪个包安装"的索引文件,
//! 格式简单——`[v1]` 段下每行一个条目:
//!
//! ```toml
//! [v1]
//! "ripgrep 14.1.1 (registry+https://github.com/rust-lang/crates.io-index)" = ["rg"]
//! ```
//!
//! `cargo install --list` 直接读这个文件来报告"installed version",
//! 所以下载器装完 binary 后必须同步更新这里, 否则下次 cargo-fresh 仍会
//! 看到旧版本。`.crates2.json` 是 cargo 的"扩展元数据" (features 等),
//! 两者并存——下载器需要同时写。

use anyhow::Context;

/// 把一次成功的 binary 安装写回 `.crates.toml`——更新该包的版本段。
///
/// 查找策略和 `crates2::write_install_record` 一致:
/// 1. 先按包名前缀匹配 (`"<binary_name> ..."`);
/// 2. 找不到再扫 bins 列表 (支持 `package_name != binary_name`, 如 ripgrep -> rg)。
///
/// 文件缺失 / 解析失败 → 返回 Err, caller 据此把 InstallFailed 上报。
/// 找不到匹配条目 → 返回 Ok(()), 静默跳过 (新装的包应走 `cargo install` 路径)。
pub fn write_install_record(
    cargo_home: &std::path::Path,
    binary_name: &str,
    new_version: &str,
) -> anyhow::Result<()> {
    let path = cargo_home.join(".crates.toml");
    let body = std::fs::read_to_string(&path).context("read .crates.toml")?;

    let new_body = update_record(&body, binary_name, new_version)?;
    std::fs::write(&path, new_body).context("write .crates.toml")?;
    Ok(())
}

/// 纯函数: 把 `.crates.toml` 文本里 `binary_name` 对应的版本替换成 `new_version`。
///
/// 找不到匹配条目时原样返回 (不报错), caller 不应假设新条目场景。
pub fn update_record(body: &str, binary_name: &str, new_version: &str) -> anyhow::Result<String> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut matched = false;

    for line in lines {
        if matched {
            out.push(line.to_string());
            continue;
        }
        if let Some(new_line) = try_rewrite_line(line, binary_name, new_version) {
            out.push(new_line);
            matched = true;
        } else {
            out.push(line.to_string());
        }
    }

    let mut joined = out.join("\n");
    if body.ends_with('\n') {
        joined.push('\n');
    }
    Ok(joined)
}

/// 如果 `line` 是 `binary_name` 对应的条目, 返回替换版本后的新行; 否则 None。
fn try_rewrite_line(line: &str, binary_name: &str, new_version: &str) -> Option<String> {
    // 格式: "<pkg_name> <version> (<source>)" = ["<bin1>", "<bin2>", ...]
    let trimmed = line.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }

    // 找到 key 段闭合的 `"`
    let after_open_quote = &trimmed[1..];
    let close_quote_idx = after_open_quote.find('"')?;
    let key = &after_open_quote[..close_quote_idx];

    // key = "<pkg_name> <version> (<source>)"
    let mut parts = key.splitn(3, ' ');
    let pkg_name = parts.next()?;
    let version = parts.next()?;
    let rest = parts.next()?; // "(<source>)"

    let bracket_part = &after_open_quote[close_quote_idx + 1..];

    let is_match = pkg_name == binary_name || bracket_contains_binary(bracket_part, binary_name);
    if !is_match {
        return None;
    }

    // 保持原始缩进
    let indent_len = line.len() - trimmed.len();
    let indent = &line[..indent_len];

    let _ = version; // 显式忽略——直接覆盖
    let _ = rest;

    let new_key = format!("\"{pkg_name} {new_version} {rest}\"");
    // bracket_part 形如 ` = ["rg"]` —— 整体保留
    Some(format!("{indent}{new_key}{bracket_part}"))
}

/// 检查 ` = [..."binary_name"...]` 里是否含目标 binary。
fn bracket_contains_binary(bracket_part: &str, binary_name: &str) -> bool {
    let needle = format!("\"{binary_name}\"");
    bracket_part.contains(&needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[v1]
"cargo-deny 0.16.0 (registry+https://github.com/rust-lang/crates.io-index)" = ["cargo-deny"]
"ripgrep 14.1.1 (registry+https://github.com/rust-lang/crates.io-index)" = ["rg"]
"mdbook 0.4.40 (registry+https://github.com/rust-lang/crates.io-index)" = ["mdbook"]
"#;

    #[test]
    fn updates_version_for_matching_package_name() {
        let out = update_record(SAMPLE, "cargo-deny", "0.19.7").unwrap();
        assert!(out.contains("\"cargo-deny 0.19.7 (registry+"));
        assert!(!out.contains("\"cargo-deny 0.16.0"));
    }

    #[test]
    fn updates_version_via_binary_name_lookup() {
        // ripgrep 包名 != binary 名 (rg)
        let out = update_record(SAMPLE, "rg", "15.1.0").unwrap();
        assert!(out.contains("\"ripgrep 15.1.0 (registry+"));
        assert!(!out.contains("\"ripgrep 14.1.1"));
    }

    #[test]
    fn unmatched_binary_leaves_body_unchanged() {
        let out = update_record(SAMPLE, "nonexistent", "1.0.0").unwrap();
        assert_eq!(out, SAMPLE);
    }

    #[test]
    fn preserves_trailing_newline() {
        let out = update_record(SAMPLE, "cargo-deny", "0.19.7").unwrap();
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn preserves_v1_header() {
        let out = update_record(SAMPLE, "cargo-deny", "0.19.7").unwrap();
        assert!(out.starts_with("[v1]\n"));
    }

    #[test]
    fn only_updates_first_matching_line() {
        let body = r#"[v1]
"foo 1.0.0 (registry+x)" = ["foo"]
"foo 2.0.0 (registry+x)" = ["foo"]
"#;
        let out = update_record(body, "foo", "9.9.9").unwrap();
        // 第一行替换, 第二行原样
        assert!(out.contains("\"foo 9.9.9 (registry+x)\" = [\"foo\"]"));
        assert!(out.contains("\"foo 2.0.0 (registry+x)\" = [\"foo\"]"));
    }

    #[test]
    fn preserves_multi_binary_list() {
        let body = r#"[v1]
"multi 1.0.0 (registry+x)" = ["bin-a", "bin-b"]
"#;
        let out = update_record(body, "multi", "2.0.0").unwrap();
        assert!(out.contains("\"multi 2.0.0 (registry+x)\" = [\"bin-a\", \"bin-b\"]"));
    }
}
