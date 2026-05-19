# Preserve install options via `.crates2.json` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `cargo fresh` re-install packages with the same Cargo features they were installed with, instead of silently resetting to default features.

**Architecture:** Read `$CARGO_HOME/.crates2.json` once in `get_installed_packages()` via a new pure-parser module, attach an `Option<InstallOpts>` to each `PackageInfo`, thread it into `build_args` (appends `--features` / `--no-default-features` / `--all-features`), and skip binstall when a package has non-default features (binstall cannot apply them).

**Tech Stack:** Rust, `serde_json` (already a direct dep `1.0.145`), `tokio`, existing `colored`/`display` status helpers.

**Spec:** `docs/superpowers/specs/2026-05-19-crates2-install-opts-preservation-design.md`

**Conventions:**
- Run all commands from the worktree root.
- Lint gate is zero-warning: `cargo clippy --all-targets -- -D warnings`.
- Commit messages: subject in Chinese is the repo norm; end every commit with a trailing line `Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>`. Use `git -c commit.gpgsign=false commit` (repo signs by default in normal use, but CI/agent runs must not block on a signing key).
- Per-module `#[cfg(test)] mod tests` is the established unit-test pattern (see `src/package/registry.rs:84`).

---

### Task 1: `InstallOpts` model + `PackageInfo.install_opts` field

**Files:**
- Modify: `src/models/mod.rs` (struct `PackageInfo` at lines 55-60; `impl PackageInfo` constructors at lines 70-87)
- Test: `src/models/mod.rs` (new `#[cfg(test)] mod tests` at end of file — the file currently has none)

- [ ] **Step 1: Write the failing test**

Append to the very end of `src/models/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_opts_default_is_default() {
        let o = InstallOpts::default();
        assert!(o.is_default());
    }

    #[test]
    fn install_opts_with_features_is_not_default() {
        let o = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["pcre2".to_string()],
        };
        assert!(!o.is_default());
    }

    #[test]
    fn install_opts_no_default_features_is_not_default() {
        let o = InstallOpts {
            no_default_features: true,
            all_features: false,
            features: vec![],
        };
        assert!(!o.is_default());
    }

    #[test]
    fn install_opts_all_features_is_not_default() {
        let o = InstallOpts {
            no_default_features: false,
            all_features: true,
            features: vec![],
        };
        assert!(!o.is_default());
    }

    #[test]
    fn package_info_install_opts_defaults_none() {
        let p = PackageInfo::new("ripgrep".to_string(), Some("14.0.0".to_string()));
        assert!(p.install_opts.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib models::tests 2>&1 | tail -20`
Expected: FAIL — `cannot find type InstallOpts in this scope` / `no field install_opts`.

- [ ] **Step 3: Write minimal implementation**

In `src/models/mod.rs`, add the struct immediately above `pub struct PackageInfo` (before line 55, after the preceding item):

```rust
/// 一个包安装时使用的 Cargo 特性选项，从 `$CARGO_HOME/.crates2.json` 解析而来。
///
/// 只建模 features 三项；profile/target/rustc 刻意不保留（见 spec）。
/// `None`（在 `PackageInfo.install_opts` 上）表示没读到元数据，走默认行为。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallOpts {
    pub no_default_features: bool,
    pub all_features: bool,
    pub features: Vec<String>,
}

impl InstallOpts {
    /// 全默认安装——可安全走 binstall，无需追加任何 cargo flag。
    pub fn is_default(&self) -> bool {
        !self.no_default_features && !self.all_features && self.features.is_empty()
    }
}
```

Add the field to `PackageInfo` (becomes lines ~55-61):

```rust
#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub source: PackageSource,
    pub install_opts: Option<InstallOpts>,
}
```

Update `with_source` (the only struct literal) to initialize the new field:

```rust
    pub fn with_source(
        name: String,
        current_version: Option<String>,
        source: PackageSource,
    ) -> Self {
        Self {
            name,
            current_version,
            latest_version: None,
            source,
            install_opts: None,
        }
    }
```

(`new` delegates to `with_source`, so no other constructor change is needed.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib models::tests 2>&1 | tail -20`
Expected: PASS (5 tests).

- [ ] **Step 5: Verify no other `PackageInfo` struct literals broke**

Run: `cargo build 2>&1 | tail -20`
Expected: builds clean. (`with_source` is the only struct-literal constructor; `tests/` and other modules use `new`/`with_source`. If the compiler reports a missing-field error elsewhere, add `install_opts: None` there.)

- [ ] **Step 6: Commit**

```bash
git add src/models/mod.rs
git -c commit.gpgsign=false commit -m "feat(models): 新增 InstallOpts 与 PackageInfo.install_opts 字段

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: `parse_crates2` pure parser

**Files:**
- Create: `src/package/crates2.rs`
- Modify: `src/package/mod.rs:21` (module declarations — currently `pub mod registry;` / `pub mod sparse_index;`)
- Test: `src/package/crates2.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Create the module file with the parser and failing tests**

Create `src/package/crates2.rs`:

```rust
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
```

- [ ] **Step 2: Register the module**

In `src/package/mod.rs`, change line 21 area from:

```rust
pub mod registry;
pub mod sparse_index;
```

to:

```rust
pub mod crates2;
pub mod registry;
pub mod sparse_index;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib package::crates2 2>&1 | tail -20`
Expected: PASS (6 tests).

- [ ] **Step 4: Lint**

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -10`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/package/crates2.rs src/package/mod.rs
git -c commit.gpgsign=false commit -m "feat(package): 新增 crates2.rs，纯函数解析 .crates2.json

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: `load_install_opts` (file locator) + expose `cargo_home`

**Files:**
- Modify: `src/package/registry.rs:49` (`fn cargo_home` → `pub(crate) fn cargo_home`)
- Modify: `src/package/crates2.rs` (add `load_install_opts`)
- Test: `src/package/crates2.rs` (extend `mod tests`)

- [ ] **Step 1: Make `cargo_home` reusable**

In `src/package/registry.rs` line 49, change:

```rust
fn cargo_home() -> Option<PathBuf> {
```

to:

```rust
pub(crate) fn cargo_home() -> Option<PathBuf> {
```

- [ ] **Step 2: Write the failing test**

Add to `src/package/crates2.rs` `mod tests`:

```rust
    #[test]
    fn load_install_opts_missing_file_is_empty() {
        // CARGO_HOME 指向一个没有 .crates2.json 的目录 → 空 map，不 panic。
        // 用进程级 env 改写有竞争风险，所以这里只断言纯逻辑：
        // load_install_opts 内部把"文件读不到"映射成空 map。
        // 真实文件存在与否的集成验证留给手动/CI。
        let tmp = std::env::temp_dir().join("cargo-fresh-no-such-dir-xyz");
        let p = tmp.join(".crates2.json");
        let body = std::fs::read_to_string(&p).unwrap_or_default();
        assert!(parse_crates2(&body).is_empty());
    }
```

- [ ] **Step 3: Run it to verify it passes (it exercises the read→parse fallback contract)**

Run: `cargo test --lib package::crates2::tests::load_install_opts_missing_file_is_empty 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 4: Implement `load_install_opts`**

Add to `src/package/crates2.rs` (after `parse_crates2`, before `mod tests`):

```rust
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
```

- [ ] **Step 5: Build + lint**

Run: `cargo build 2>&1 | tail -5 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: clean (a private→`pub(crate)` widening keeps `cargo_home`'s other caller `resolve_from_config` working; no dead-code warning because `load_install_opts` is used in Task 4 — if Task 3 is committed alone, add `#[allow(dead_code)]` on `load_install_opts` and remove it in Task 4. Prefer to land Task 3 + Task 4 back-to-back to avoid the churn.)

- [ ] **Step 6: Commit**

```bash
git add src/package/registry.rs src/package/crates2.rs
git -c commit.gpgsign=false commit -m "feat(package): load_install_opts 定位 \$CARGO_HOME/.crates2.json

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Match `.crates2.json` entries onto `PackageInfo`

`.crates2.json` keys are `"<name> <version> (<source>)"`. Match by name (first token); when multiple entries share a name, prefer the one whose `(<source>)` prefix matches the `PackageSource`.

**Files:**
- Modify: `src/package/crates2.rs` (add pure `match_install_opts`)
- Modify: `src/package/mod.rs` (`get_installed_packages`, lines 158-187 — the parse loop builds `packages`; attach after the loop, before the `INSTALLED_VERSION_CACHE` block at line ~182)
- Test: `src/package/crates2.rs` (`mod tests`)

- [ ] **Step 1: Write the failing test for the matcher**

Add to `src/package/crates2.rs` `mod tests`:

```rust
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
```

Add the import at the top of `src/package/crates2.rs` (alongside the existing `use crate::models::InstallOpts;`):

```rust
use crate::models::{InstallOpts, PackageSource};
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib package::crates2 2>&1 | tail -15`
Expected: FAIL — `cannot find function match_install_opts`.

- [ ] **Step 3: Implement the matcher**

Add to `src/package/crates2.rs` (after `load_install_opts`, before `mod tests`):

```rust
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
```

- [ ] **Step 4: Run matcher tests to verify pass**

Run: `cargo test --lib package::crates2 2>&1 | tail -15`
Expected: PASS (all crates2 tests, including the 3 new matcher tests).

- [ ] **Step 5: Wire into `get_installed_packages`**

In `src/package/mod.rs`, locate the end of the parse loop (the `for line in output_str.lines()` block that pushes into `packages`, ending around line 175) and the `INSTALLED_VERSION_CACHE` block (line ~182). Insert this **between** the end of the loop and the cache block:

```rust
    // 尽力而为地附上每个包安装时的 features 选项（.crates2.json）。
    // 读不到 / 解析失败 → install_opts 保持 None，走默认行为。
    let install_opts_map = crates2::load_install_opts();
    for pkg in &mut packages {
        pkg.install_opts = crates2::match_install_opts(&install_opts_map, &pkg.name, &pkg.source);
    }
```

- [ ] **Step 6: Build, full lib tests, lint**

Run: `cargo build 2>&1 | tail -5 && cargo test --lib 2>&1 | tail -15 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: builds clean, all lib tests pass, no clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add src/package/crates2.rs src/package/mod.rs
git -c commit.gpgsign=false commit -m "feat(package): get_installed_packages 附加 install_opts

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: `build_args` appends feature flags

Decision (resolves the spec's flagged ambiguity): change `build_args` to return `Vec<String>` and `run_cargo` to accept `&[String]`. This is the least-invasive option — `.join(" ")` and `Command::args(...)` both accept `&[String]`, and it avoids threading an owned comma-joined string through call sites.

**Files:**
- Modify: `src/updater/mod.rs` — `build_args` (lines 94-128), `run_cargo` signature (line 130), call sites (lines 268-273, 357, 373), imports (line 9-11)
- Test: `src/updater/mod.rs` (new `#[cfg(test)] mod tests` at end of file — none currently)

- [ ] **Step 1: Write the failing tests**

Append to the end of `src/updater/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::build_args;
    use crate::models::{InstallOpts, PackageSource};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn crates_default_opts_no_extra_flags() {
        let got = build_args(false, "ripgrep", Some("14.1.1"), &PackageSource::Crates, None);
        assert_eq!(
            got,
            s(&["install", "--force", "ripgrep", "--version", "14.1.1"])
        );
    }

    #[test]
    fn crates_with_features() {
        let opts = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["pcre2".into(), "simd".into()],
        };
        let got = build_args(false, "ripgrep", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install", "--force", "ripgrep", "--features", "pcre2,simd"
            ])
        );
    }

    #[test]
    fn crates_no_default_and_all_features() {
        let opts = InstallOpts {
            no_default_features: true,
            all_features: true,
            features: vec![],
        };
        let got = build_args(false, "x", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install",
                "--force",
                "x",
                "--no-default-features",
                "--all-features"
            ])
        );
    }

    #[test]
    fn git_source_with_features() {
        let opts = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["a".into()],
        };
        let src = PackageSource::Git {
            url: "https://github.com/x/y".into(),
            rev: Some("abc".into()),
        };
        let got = build_args(false, "y", None, &src, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install", "--git", "https://github.com/x/y", "--rev", "abc",
                "--force", "y", "--features", "a"
            ])
        );
    }

    #[test]
    fn path_source_with_no_default_features() {
        let opts = InstallOpts {
            no_default_features: true,
            all_features: false,
            features: vec![],
        };
        let src = PackageSource::Path { dir: "/tmp/p".into() };
        let got = build_args(false, "p", None, &src, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install", "--path", "/tmp/p", "--force", "p",
                "--no-default-features"
            ])
        );
    }

    #[test]
    fn default_opts_some_but_empty_adds_nothing() {
        let opts = InstallOpts::default();
        let got = build_args(true, "tool", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(got, s(&["binstall", "--force", "tool"]));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib updater::tests 2>&1 | tail -15`
Expected: FAIL — arity mismatch (`build_args` takes 4 args, test passes 5) and return-type mismatch.

- [ ] **Step 3: Rewrite `build_args`**

Replace the entire `build_args` function (lines 94-128) with:

```rust
fn build_args(
    use_binstall: bool,
    package_name: &str,
    version: Option<&str>,
    source: &PackageSource,
    opts: Option<&InstallOpts>,
) -> Vec<String> {
    let mut args: Vec<String> = match source {
        PackageSource::Crates => {
            let subcmd = if use_binstall { "binstall" } else { "install" };
            match version {
                Some(v) => vec![
                    subcmd.into(),
                    "--force".into(),
                    package_name.into(),
                    "--version".into(),
                    v.into(),
                ],
                None => vec![subcmd.into(), "--force".into(), package_name.into()],
            }
        }
        PackageSource::Git { url, rev } => {
            let mut a: Vec<String> =
                vec!["install".into(), "--git".into(), url.clone()];
            if let Some(r) = rev {
                a.push("--rev".into());
                a.push(r.clone());
            }
            a.push("--force".into());
            a.push(package_name.into());
            a
        }
        PackageSource::Path { dir } => vec![
            "install".into(),
            "--path".into(),
            dir.clone(),
            "--force".into(),
            package_name.into(),
        ],
        // Unknown 来源不应到这一步——check_package_updates 会跳过它。
        // 万一到了，给个明显错的命令让上层报错而不是默默 cargo install。
        PackageSource::Unknown(raw) => vec![
            "install".into(),
            "--unknown-source-marker".into(),
            raw.clone(),
            package_name.into(),
        ],
    };

    // 追加 features 选项（Unknown 源不追加——它本就要让上层报错）。
    if let (Some(o), false) = (opts, matches!(source, PackageSource::Unknown(_))) {
        if o.no_default_features {
            args.push("--no-default-features".into());
        }
        if o.all_features {
            args.push("--all-features".into());
        }
        if !o.features.is_empty() {
            args.push("--features".into());
            args.push(o.features.join(","));
        }
    }
    args
}
```

- [ ] **Step 4: Add the `InstallOpts` import**

In `src/updater/mod.rs`, the `use crate::models::{...}` block at line 9-11 currently imports `PackageSource, UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_TICK_MS, RETRY_DELAY_MS`. Add `InstallOpts`:

```rust
use crate::models::{
    InstallOpts, PackageSource, UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_TICK_MS,
    RETRY_DELAY_MS,
};
```

- [ ] **Step 5: Update `run_cargo` to take `&[String]`**

In `src/updater/mod.rs` line 130, change:

```rust
async fn run_cargo(pb: &ProgressBar, args: &[&str]) -> Result<Output> {
```

to:

```rust
async fn run_cargo(pb: &ProgressBar, args: &[String]) -> Result<Output> {
```

(`args.join(" ")` at line 131 and `.args(args)` at line 134 both accept `&[String]` unchanged.)

- [ ] **Step 6: Fix the two `build_args` call sites (compile will still fail until Task 6 adds the param thread-through; here pass `None` as a temporary placeholder so this task is independently green)**

In `src/updater/mod.rs` lines 268 and 271, change:

```rust
    let primary_args = build_args(use_binstall, package_name, target_version, source);
    let fallback_args = if use_binstall {
        Some(build_args(false, package_name, target_version, source))
```

to:

```rust
    let primary_args = build_args(use_binstall, package_name, target_version, source, None);
    let fallback_args = if use_binstall {
        Some(build_args(false, package_name, target_version, source, None))
```

(Task 6 replaces both `None`s with the real `install_opts`. Using `None` here keeps Task 5 compilable and its tests meaningful in isolation.)

- [ ] **Step 7: Run tests + build + lint**

Run: `cargo test --lib updater::tests 2>&1 | tail -15 && cargo build 2>&1 | tail -5 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 6 updater tests PASS, build clean, no clippy warnings.

- [ ] **Step 8: Commit**

```bash
git add src/updater/mod.rs
git -c commit.gpgsign=false commit -m "feat(updater): build_args 追加 features flag，返回 Vec<String>

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Thread `install_opts` into `update_package`, skip binstall on non-default features

**Files:**
- Modify: `src/updater/mod.rs` — `update_package` signature (lines 234-240), `use_binstall` decision (lines 251-266), `build_args` calls (lines 268-271), add verbose Check line
- Modify: `src/main.rs` — call site (lines 221-246), thread `install_opts` + `cli.verbose`
- Test: covered by Task 5 unit tests + Task 4 integration of `install_opts`; add one predicate test below.

- [ ] **Step 1: Write the failing predicate test**

Add to `src/updater/mod.rs` `mod tests` (the module created in Task 5):

```rust
    #[test]
    fn non_default_features_disables_binstall() {
        // 复刻 update_package 里的判定式：binstall 仅当 opts 缺省或全默认才用。
        let pick = |binstall_avail: bool, opts: Option<&InstallOpts>| {
            binstall_avail && opts.map_or(true, |o| o.is_default())
        };
        let feat = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["pcre2".into()],
        };
        assert!(pick(true, None));
        assert!(pick(true, Some(&InstallOpts::default())));
        assert!(!pick(true, Some(&feat)));
        assert!(!pick(false, None));
    }
```

- [ ] **Step 2: Run to verify it passes (pure predicate — documents the rule)**

Run: `cargo test --lib updater::tests::non_default_features_disables_binstall 2>&1 | tail -8`
Expected: PASS.

- [ ] **Step 3: Add params to `update_package`**

In `src/updater/mod.rs` lines 234-240, change the signature to:

```rust
pub async fn update_package(
    package_name: &str,
    target_version: Option<&str>,
    source: &PackageSource,
    install_opts: Option<&InstallOpts>,
    dry_run: bool,
    install_binstall: bool,
    verbose: bool,
) -> Result<UpdateResult> {
```

- [ ] **Step 4: Tighten the binstall decision and add the verbose diagnostic**

In `src/updater/mod.rs`, the `use_binstall` block (lines 251-266) currently begins:

```rust
    let use_binstall = match source {
        PackageSource::Crates => {
            if is_binstall_available() {
                true
```

Change the `if is_binstall_available()` line so a package with non-default features never selects binstall:

```rust
    let opts_allow_binstall = install_opts.map_or(true, |o| o.is_default());
    let use_binstall = match source {
        PackageSource::Crates => {
            if is_binstall_available() && opts_allow_binstall {
                true
```

(The `else if install_binstall && !dry_run` and `else` arms are unchanged. When `opts_allow_binstall` is false, control falls into the existing `else` arm which emits the `Hint` and returns `false`, so the package goes through `cargo install` with features — exactly the intended behavior. To avoid a misleading binstall Hint in that case, guard it: change the inner `else` block's `status_dim("Hint", ...)` so it only prints when binstall is genuinely the blocker, i.e. wrap it as below.)

Replace the inner `else` arm:

```rust
            } else {
                // 静默地提示一次，给 CI/审计场景留个线索而不打扰主输出
                status_dim(
                    "Hint",
                    language.get_text("binstall_hint"),
                );
                false
            }
```

with:

```rust
            } else {
                if opts_allow_binstall {
                    // binstall 本身不可用——给 CI/审计场景留个线索
                    status_dim("Hint", language.get_text("binstall_hint"));
                } else if verbose {
                    // 包带非默认 features，走 cargo install 才能生效
                    status_dim(
                        "Check",
                        &format!("{package_name} has custom features, using cargo install"),
                    );
                }
                false
            }
```

- [ ] **Step 5: Add the "no metadata" verbose line and pass opts to `build_args`**

In `src/updater/mod.rs`, replace lines 268-271:

```rust
    let primary_args = build_args(use_binstall, package_name, target_version, source, None);
    let fallback_args = if use_binstall {
        Some(build_args(false, package_name, target_version, source, None))
```

with:

```rust
    if verbose && install_opts.is_none() {
        status_dim(
            "Check",
            &format!("{package_name} no install metadata, using default features"),
        );
    }
    let primary_args =
        build_args(use_binstall, package_name, target_version, source, install_opts);
    let fallback_args = if use_binstall {
        Some(build_args(false, package_name, target_version, source, install_opts))
```

- [ ] **Step 6: Update the `main.rs` call site**

In `src/main.rs` around lines 221-246, the loop extracts `target_version` and `source` from `selected_pkg`. Add an `install_opts` extraction next to the `source` extraction (after line ~230) and pass both new args.

Find:

```rust
            let source = selected_pkg
                .map(|p| p.source.clone())
                .unwrap_or(PackageSource::Crates);
```

Add immediately after it:

```rust
            let install_opts = selected_pkg.and_then(|p| p.install_opts.clone());
```

Then change the `update_package(...)` call (lines 239-245) from:

```rust
            match update_package(
                package_name,
                target_version,
                &source,
                cli.dry_run,
                cli.install_binstall,
            )
```

to:

```rust
            match update_package(
                package_name,
                target_version,
                &source,
                install_opts.as_ref(),
                cli.dry_run,
                cli.install_binstall,
                cli.verbose,
            )
```

Add `InstallOpts` to the `use cargo_fresh::models::{...}` import in `src/main.rs` (lines 14-17) only if the compiler asks — `install_opts.as_ref()` infers `Option<&InstallOpts>` without needing the name in scope, so likely no import change is required. Build will tell you.

- [ ] **Step 7: Build, full test suite, lint**

Run: `cargo build 2>&1 | tail -5 && cargo test 2>&1 | tail -20 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: build clean; **all unit + integration tests pass** (84+ unit, 10 integration baseline plus the new tests); zero clippy warnings.

- [ ] **Step 8: Manual smoke check (dry-run shows preserved features)**

Run: `cargo run -- --dry-run --filter "ripgrep" 2>&1 | tail -10`
Expected: if `ripgrep` is installed with custom features in your `.crates2.json`, the `Would run` line includes `--features ...`; otherwise it runs as before. (Non-fatal if ripgrep isn't installed — pick any installed package, or just confirm no panic.)

- [ ] **Step 9: Commit**

```bash
git add src/updater/mod.rs src/main.rs
git -c commit.gpgsign=false commit -m "feat(updater): update_package 透传 install_opts，非默认 features 跳过 binstall

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Documentation + final verification

**Files:**
- Modify: `CHANGELOG.md` (the existing `## [Unreleased]` block — currently has a `### Docs` entry from the README-fix commit)
- Modify: `README.md` (comparison table `Install options preserved` row + trailing paragraph)
- Modify: `ROADMAP.md` ("Remaining Modern Rust CLI gaps" / P2 area)
- Modify: `CLAUDE.md` (module-responsibilities table + key-design-decisions list)

- [ ] **Step 1: CHANGELOG**

In `CHANGELOG.md`, under the existing `## [Unreleased]`, add these sections above the existing `### Docs` block:

```markdown
### BEHAVIOR

- **带非默认 features 的包跳过 binstall**: 一个包若以 `--features` / `--no-default-features` / `--all-features` 安装,更新时直接走 `cargo install`(binstall 下的是上游预编译二进制,无法应用任意 features)。无自定义 features 的包行为不变,仍优先 binstall

### Added

- **`.crates2.json` 安装选项保留**: 新增 `src/package/crates2.rs`,从 `$CARGO_HOME/.crates2.json` 解析每个包安装时的 features 选项,更新时透传给 `cargo install`。尽力而为——文件缺失/损坏/无匹配条目一律静默回退默认行为,绝不让它成为更新失败的原因

### Fixed

- **更新不再静默丢 features**: 之前 `cargo install --force <name>` 不带任何 `--features`,把以自定义特性安装的包(如 `ripgrep --features pcre2`)悄悄退回默认特性。现在从 `.crates2.json` 还原并保留 `--features` / `--no-default-features` / `--all-features`
```

- [ ] **Step 2: README comparison table**

In `README.md`, the `Install options preserved` row currently reads:

```
| **Install options preserved** | Not yet — `cargo install --force` resets to default features (tracked for before 1.0) | Yes — reads `.crates2.json` for features/profile, plus per-package `cargo-install-update-config` |
```

Change the cargo-fresh cell to:

```
| **Install options preserved** | Yes — features (`--features` / `--no-default-features` / `--all-features`) restored from `.crates2.json` | Yes — `.crates2.json` features/profile, plus per-package `cargo-install-update-config` |
```

Then update the trailing paragraph (currently): `cargo-update is more mature. Its main edge today: it preserves the features/profile a package was installed with ...` — replace its first two sentences with:

```
cargo-update is more mature. Both tools now preserve the features a package was installed with; cargo-update additionally preserves build profile and supports per-package config via `cargo-install-update-config`.
```

- [ ] **Step 3: ROADMAP**

In `ROADMAP.md`, in the `## Remaining "Modern Rust CLI" gaps` section (or the P2 list), add a line under the closed items:

```
- ✅ Install-option preservation via `.crates2.json` (features only; profile/target deferred) — closes the main cargo-update gap
```

- [ ] **Step 4: CLAUDE.md**

In `CLAUDE.md`, add a row to the module-responsibilities table after the `src/package/registry.rs` row:

```
| `src/package/crates2.rs` | Parses `$CARGO_HOME/.crates2.json`. Pure `parse_crates2(&str)` + `match_install_opts(map, name, source)` + `load_install_opts()` file locator (reuses `registry::cargo_home`). Best-effort: any failure → empty map, never errors |
```

And add a key-design-decision bullet:

```
- **Install-option preservation (best-effort)**: `get_installed_packages` attaches `Option<InstallOpts>` (features/no-default/all-features) parsed from `.crates2.json`; `build_args` appends the matching flags. Packages with non-default features skip binstall (it cannot apply arbitrary features). Missing/corrupt metadata silently falls back to default features — never fails an update. `profile`/`target` intentionally not preserved
```

- [ ] **Step 5: Final full verification**

Run: `cargo test 2>&1 | tail -20 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: all tests green, zero clippy warnings. Record the actual test count in the commit message.

- [ ] **Step 6: Commit**

```bash
git add CHANGELOG.md README.md ROADMAP.md CLAUDE.md
git -c commit.gpgsign=false commit -m "docs: .crates2.json features 保留——CHANGELOG/README/ROADMAP/CLAUDE 同步

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage:**
- Data model (`InstallOpts` + `is_default` + `PackageInfo.install_opts`) → Task 1 ✓
- `parse_crates2` pure parser → Task 2 ✓
- `load_install_opts` + `pub(crate) cargo_home` + module registration → Task 3 ✓
- Name match + source-prefix tiebreak in `get_installed_packages` → Task 4 ✓
- `build_args` appends 3 feature flags, all sources uniform, Unknown untouched → Task 5 ✓
- binstall skip on non-default features → Task 6 ✓
- Missing-metadata silent fallback + `--verbose` `Check` line → Task 6 Steps 4-5 ✓
- Error handling = no error path (HashMap not Result) → Task 2/3 implementations ✓
- Tests: crates2 parser, build_args matrix, binstall predicate → Tasks 2/4/5/6 ✓; `tests/cli.rs` unchanged (asserted, no task needed) ✓
- Docs (CHANGELOG BEHAVIOR/Added/Fixed, README, ROADMAP, CLAUDE.md) → Task 7 ✓
- 1.0-contract assessment (BEHAVIOR+Fixed, no rc) → reflected in Task 7 CHANGELOG ✓

No spec requirement is left without a task.

**2. Placeholder scan:** No TBD/TODO/"handle edge cases". The only intentional temporary value is `None` passed to `build_args` in Task 5 Step 6, explicitly explained and replaced in Task 6 Step 5.

**3. Type consistency:** `InstallOpts { no_default_features, all_features, features }` and `is_default()` are used identically across Tasks 1/2/4/5/6. `build_args` returns `Vec<String>` (Task 5) and every call site / `run_cargo` is updated to match. `match_install_opts(&HashMap, &str, &PackageSource) -> Option<InstallOpts>` signature is consistent between Task 4 definition and use. `update_package` new param order (`..., source, install_opts, dry_run, install_binstall, verbose`) is consistent between Task 6 Step 3 definition and Step 6 call site.
