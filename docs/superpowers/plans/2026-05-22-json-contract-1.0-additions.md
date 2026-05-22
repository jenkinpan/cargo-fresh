# JSON Contract Additions for 1.0 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three additive `--format=json` fields — `reason_code` on `skipped[]`, a `version_check_errors[]` array (with a `fresh[]` bug fix), and `selected`/`attempted` summary counts — before the `schema_version=1` contract is frozen at 1.0.

**Architecture:** All changes are additive field additions to the structs in `src/models/mod.rs` and the report builders in `src/main.rs`. No `schema_version` bump, no BREAKING/BEHAVIOR change, exit codes unchanged. Task 1 first extracts a pure `build_report()` function so the report logic becomes unit-testable; later tasks build on it. Version-check failures are surfaced by threading a typed error from `sparse_index::fetch_latest` through `fetch_latest_versions` into a new `PackageInfo.check_error` field.

**Tech Stack:** Rust, `serde` (JSON serialization), `tokio` (async), `wiremock` (HTTP test doubles), `assert_cmd` (CLI integration tests).

**Spec:** `docs/superpowers/specs/2026-05-22-json-contract-1.0-additions-design.md`

---

## File Structure

| File | Change |
|------|--------|
| `src/models/mod.rs` | New types `CheckErrorKind`, `CheckError`, `JsonCheckError`; new fields on `PackageInfo`, `JsonReport`, `JsonSummary`, `JsonSkipped`; new methods `PackageSource::skip_reason_code`, `CheckErrorKind::kind_str` |
| `src/package/sparse_index.rs` | New `SparseIndexError` enum; `fetch_latest` return type changes to `Result<LatestVersions, SparseIndexError>` |
| `src/package/mod.rs` | New `VersionLookup` struct; `fetch_latest_versions` returns `VersionLookup`; `check_package_updates` populates `check_error` |
| `src/main.rs` | Extract pure `build_report()`; build `version_check_errors`, fix `fresh[]` filter, set `reason_code` + new summary fields; thread `selected` count; `#[cfg(test)]` unit tests |
| `tests/sparse_index_http.rs` | Update 404/5xx assertions for the typed error; new test for `check_package_updates` setting `check_error` |
| `tests/cli.rs` | New contract test asserting the new JSON keys are present |
| `docs/json-schema.json` | Add the three additions to the schema |
| `README.md`, `CHANGELOG.md` | Document the additions |

---

## Task 1: Extract a pure `build_report()` function

Refactor only — no behavior change. `emit_report` and `emit_empty_report` both construct a `JsonReport`; unify them into one pure `build_report()` so later tasks can unit-test report logic.

**Files:**
- Modify: `src/main.rs` (lines 354-466: `emit_empty_report`, `emit_report`, `print_json`; call sites at lines 117, 131, 175, 326)

- [ ] **Step 1: Write the failing test**

Add to the bottom of `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use cargo_fresh::models::{PackageInfo, PackageSource};

    fn empty_cli() -> Cli {
        Cli::parse_from(["cargo-fresh"])
    }

    #[test]
    fn build_report_counts_packages_and_sets_format() {
        let cli = empty_cli();
        let packages = vec![
            PackageInfo::with_source("ripgrep".into(), Some("14.1.1".into()), PackageSource::Crates),
        ];
        let report = build_report(&cli, &packages, &[], &[], false, std::time::Instant::now());
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.format, "cargo-fresh-v1");
        assert_eq!(report.summary.checked, 1);
        assert_eq!(report.fresh, vec!["ripgrep"]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin cargo-fresh build_report_counts_packages -- --exact`
Expected: FAIL — compile error, `build_report` not found.

- [ ] **Step 3: Replace `emit_empty_report` and `emit_report` with `build_report` + thin wrapper**

In `src/main.rs`, delete the entire `emit_empty_report` function (lines 354-377) and replace the `emit_report` function (lines 379-454) with:

```rust
/// 纯函数：把整次运行的快照组装成 JSON 报告结构。
/// 不做 I/O，便于单元测试；`emit_report` 负责真正写 stdout。
fn build_report<'a>(
    cli: &'a Cli,
    packages: &'a [PackageInfo],
    all_updates: &[&'a PackageInfo],
    update_results: &'a [UpdateResult],
    aborted: bool,
    start: std::time::Instant,
) -> JsonReport<'a> {
    let updates_available: Vec<JsonUpdateCandidate> = all_updates
        .iter()
        .filter_map(|p| {
            p.latest_version.as_deref().map(|latest| JsonUpdateCandidate {
                name: p.name.as_str(),
                current: p.current_version.as_deref(),
                latest,
                source: p.source.kind_str(),
                prerelease: p.is_prerelease(),
            })
        })
        .collect();

    let fresh: Vec<&str> = packages
        .iter()
        .filter(|p| !p.has_update() && p.source.is_crates())
        .map(|p| p.name.as_str())
        .collect();

    let skipped: Vec<JsonSkipped> = packages
        .iter()
        .filter(|p| !p.source.is_crates())
        .map(|p| JsonSkipped {
            name: p.name.as_str(),
            source: p.source.kind_str(),
            reason: "non-crates source: version check skipped",
        })
        .collect();

    let results: Vec<JsonResult> = update_results
        .iter()
        .map(|r| JsonResult {
            name: r.package_name.as_str(),
            old_version: r.old_version.as_deref(),
            new_version: r.new_version.as_deref(),
            success: r.success,
        })
        .collect();

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    let summary = JsonSummary {
        checked: packages.len(),
        available: updates_available.len(),
        succeeded,
        failed,
        skipped: skipped.len(),
        duration_ms: start.elapsed().as_millis(),
    };

    JsonReport {
        schema_version: 1,
        format: "cargo-fresh-v1",
        include_prerelease: cli.include_prerelease,
        dry_run: cli.dry_run,
        registry_url: cli.registry_url.as_deref(),
        updates_available,
        fresh,
        skipped,
        results,
        summary,
        aborted,
    }
}

fn emit_report(
    cli: &Cli,
    packages: &[PackageInfo],
    all_updates: &[&PackageInfo],
    update_results: &[UpdateResult],
    aborted: bool,
    start: std::time::Instant,
) {
    print_json(&build_report(
        cli,
        packages,
        all_updates,
        update_results,
        aborted,
        start,
    ));
}
```

- [ ] **Step 4: Update the `emit_empty_report` call sites**

In `src/main.rs` line 117 and line 131, replace `emit_empty_report(&cli, run_start);` (both occurrences) with:

```rust
emit_report(&cli, &[], &[], &[], false, run_start);
```

- [ ] **Step 5: Run tests and clippy to verify the refactor is clean**

Run: `cargo test --bin cargo-fresh build_report_counts_packages -- --exact`
Expected: PASS

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, zero warnings.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "refactor: extract pure build_report() from emit_report

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Add `reason_code` to `skipped[]` (spec item 1)

A stable, branchable enum so scripts never branch on the free-form `reason` prose.

**Files:**
- Modify: `src/models/mod.rs` (`PackageSource` impl ~line 37-52; `JsonSkipped` ~line 173-178)
- Modify: `src/main.rs` (`build_report`, the `skipped` builder)

- [ ] **Step 1: Write the failing test for `skip_reason_code`**

Add to the `tests` module in `src/models/mod.rs` (before the closing `}`):

```rust
#[test]
fn skip_reason_code_maps_each_source() {
    assert_eq!(
        PackageSource::Path { dir: "/x".into() }.skip_reason_code(),
        "path_source"
    );
    assert_eq!(
        PackageSource::Git { url: "u".into(), rev: None }.skip_reason_code(),
        "git_source"
    );
    assert_eq!(
        PackageSource::Unknown("weird".into()).skip_reason_code(),
        "unknown_source"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib skip_reason_code_maps_each_source -- --exact`
Expected: FAIL — `skip_reason_code` not found.

- [ ] **Step 3: Add the `skip_reason_code` method**

In `src/models/mod.rs`, inside `impl PackageSource` (the block at lines 37-52), add this method after `marker`:

```rust
    /// JSON `skipped[].reason_code`——稳定可判别的枚举字符串。
    /// `skipped[]` 只收非 crates 源，`Crates` 分支不会被实际输出。
    pub fn skip_reason_code(&self) -> &'static str {
        match self {
            PackageSource::Crates => "crates_source",
            PackageSource::Git { .. } => "git_source",
            PackageSource::Path { .. } => "path_source",
            PackageSource::Unknown(_) => "unknown_source",
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib skip_reason_code_maps_each_source -- --exact`
Expected: PASS

- [ ] **Step 5: Add the `reason_code` field to `JsonSkipped`**

In `src/models/mod.rs`, change the `JsonSkipped` struct (lines 173-178) to:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct JsonSkipped<'a> {
    pub name: &'a str,
    pub source: &'static str,
    pub reason_code: &'static str,
    pub reason: &'static str,
}
```

- [ ] **Step 6: Set `reason_code` in `build_report`**

In `src/main.rs`, in the `skipped` builder inside `build_report`, change the `JsonSkipped { .. }` construction to include `reason_code`:

```rust
        .map(|p| JsonSkipped {
            name: p.name.as_str(),
            source: p.source.kind_str(),
            reason_code: p.source.skip_reason_code(),
            reason: "non-crates source: version check skipped",
        })
```

- [ ] **Step 7: Add a `build_report` test for `reason_code`**

Add to the `tests` module in `src/main.rs`:

```rust
#[test]
fn build_report_sets_skip_reason_code() {
    let cli = empty_cli();
    let packages = vec![PackageInfo::with_source(
        "my-tool".into(),
        Some("0.1.0".into()),
        PackageSource::Git { url: "u".into(), rev: None },
    )];
    let report = build_report(&cli, &packages, &[], &[], false, std::time::Instant::now());
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].reason_code, "git_source");
}
```

- [ ] **Step 8: Run tests and clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, zero warnings.

- [ ] **Step 9: Commit**

```bash
git add src/models/mod.rs src/main.rs
git commit -m "feat(json): add reason_code enum to skipped[]

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Add `selected` / `attempted` to `summary` (spec item 3)

Let a script tell "updates existed but policy did not apply them" from "nothing actionable" without reconstructing it from arrays.

**Files:**
- Modify: `src/models/mod.rs` (`JsonSummary` ~line 188-196)
- Modify: `src/main.rs` (`build_report`, `emit_report`, call sites at lines 175 & 326, and the now-117/131 empty calls)

- [ ] **Step 1: Add `selected` and `attempted` to `JsonSummary`**

In `src/models/mod.rs`, change `JsonSummary` (lines 188-196) to:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct JsonSummary {
    pub checked: usize,
    pub available: usize,
    pub selected: usize,
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u128,
}
```

- [ ] **Step 2: Add a `selected` parameter to `build_report` and `emit_report`**

In `src/main.rs`, change `build_report`'s signature to add `selected: usize` as the last parameter:

```rust
fn build_report<'a>(
    cli: &'a Cli,
    packages: &'a [PackageInfo],
    all_updates: &[&'a PackageInfo],
    update_results: &'a [UpdateResult],
    aborted: bool,
    start: std::time::Instant,
    selected: usize,
) -> JsonReport<'a> {
```

In the same function, change the `summary` construction to:

```rust
    let summary = JsonSummary {
        checked: packages.len(),
        available: updates_available.len(),
        selected,
        attempted: results.len(),
        succeeded,
        failed,
        skipped: skipped.len(),
        duration_ms: start.elapsed().as_millis(),
    };
```

Change `emit_report`'s signature to add `selected: usize` last, and forward it:

```rust
fn emit_report(
    cli: &Cli,
    packages: &[PackageInfo],
    all_updates: &[&PackageInfo],
    update_results: &[UpdateResult],
    aborted: bool,
    start: std::time::Instant,
    selected: usize,
) {
    print_json(&build_report(
        cli,
        packages,
        all_updates,
        update_results,
        aborted,
        start,
        selected,
    ));
}
```

- [ ] **Step 3: Update the three `emit_report` call sites**

In `src/main.rs`:

- The two empty-report calls (currently lines 117 & 131) become:
  ```rust
  emit_report(&cli, &[], &[], &[], false, run_start, 0);
  ```
- The `all_updates.is_empty()` call (currently line 175) becomes:
  ```rust
  emit_report(&cli, &packages, &[], &[], false, run_start, 0);
  ```
- The main call (currently line 326) becomes — note `selections.len()` is in scope here:
  ```rust
  emit_report(
      &cli,
      &packages,
      &all_updates,
      &update_results,
      aborted,
      run_start,
      selections.len(),
  );
  ```

- [ ] **Step 4: Update the existing `build_report` tests for the new parameter**

In `src/main.rs` `tests` module, both `build_report(...)` calls now need the trailing `0` argument. Update `build_report_counts_packages_and_sets_format` and `build_report_sets_skip_reason_code` so each call ends with `..., false, std::time::Instant::now(), 0)`.

- [ ] **Step 5: Write the failing test for the new summary fields**

Add to the `tests` module in `src/main.rs`:

```rust
#[test]
fn build_report_summary_has_selection_counts() {
    let cli = empty_cli();
    let report = build_report(
        &cli,
        &[],
        &[],
        &[],
        false,
        std::time::Instant::now(),
        3,
    );
    assert_eq!(report.summary.selected, 3);
    assert_eq!(report.summary.attempted, 0);
}
```

- [ ] **Step 6: Run tests and clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add src/models/mod.rs src/main.rs
git commit -m "feat(json): add selected/attempted counts to summary

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Typed `SparseIndexError` for the sparse index client (spec item 2, part A)

`fetch_latest` currently returns `anyhow::Error`, collapsing 4xx (permanent) and 5xx/network (transient). Introduce a typed error so the kind survives.

**Files:**
- Modify: `src/package/sparse_index.rs` (`fetch_latest` lines 98-136)
- Modify: `tests/sparse_index_http.rs` (the 404 and 5xx assertions, lines 41-69)

- [ ] **Step 1: Update the 404 and 5xx tests to expect the typed error**

In `tests/sparse_index_http.rs`, change the import on line 12 to also bring in the error type:

```rust
use cargo_fresh::package::sparse_index::{fetch_latest, SparseIndexError};
```

Replace `not_found_is_not_retried` (lines 41-55) body's final two statements with:

```rust
    let err = fetch_latest(&client(), &server.uri(), "cargo-nonexistent")
        .await
        .unwrap_err();
    assert!(matches!(err, SparseIndexError::NotFound), "err = {err:?}");
```

Replace `server_error_is_retried_once_then_fails` (lines 57-69) body's final two statements with:

```rust
    let err = fetch_latest(&client(), &server.uri(), "ripgrep")
        .await
        .unwrap_err();
    match err {
        SparseIndexError::Unavailable(e) => {
            assert!(e.to_string().contains("503"), "inner = {e}");
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test sparse_index_http`
Expected: FAIL — `SparseIndexError` not found / type mismatch.

- [ ] **Step 3: Add the `SparseIndexError` enum**

In `src/package/sparse_index.rs`, after the `LatestVersions` struct (after line 31), add:

```rust
/// `fetch_latest` 的失败分类——决定 JSON `version_check_errors[].kind`。
#[derive(Debug)]
pub enum SparseIndexError {
    /// sparse index 返回 4xx——包不在该 registry，重试无意义。
    NotFound,
    /// 网络错误 / 超时 / 5xx 重试耗尽 / 响应体读取失败——可能是瞬时故障。
    Unavailable(anyhow::Error),
}
```

- [ ] **Step 4: Change `fetch_latest` to return the typed error**

In `src/package/sparse_index.rs`, replace the `fetch_latest` function (lines 98-136) with:

```rust
pub async fn fetch_latest(
    client: &reqwest::Client,
    base_url: &str,
    name: &str,
) -> std::result::Result<LatestVersions, SparseIndexError> {
    const MAX_ATTEMPTS: u32 = 2;
    const RETRY_DELAY_MS: u64 = 500;

    let path = index_path(name);
    if path.is_empty() {
        return Err(SparseIndexError::Unavailable(anyhow::anyhow!(
            "empty package name"
        )));
    }
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path);

    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let body = resp.text().await.map_err(|e| {
                        SparseIndexError::Unavailable(anyhow::Error::new(e))
                    })?;
                    return Ok(parse_index_body(&body));
                }
                // 4xx 不重试——通常是真的没这个包，再请求一次浪费时间
                if status.is_client_error() {
                    return Err(SparseIndexError::NotFound);
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
    Err(SparseIndexError::Unavailable(last_err.unwrap_or_else(|| {
        anyhow::anyhow!("sparse index: all retries exhausted")
    })))
}
```

Note: the `use anyhow::Result;` import at line 12 may now be unused — if `cargo clippy` flags it, remove that line.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test sparse_index_http`
Expected: PASS — all 5 tests.

Run: `cargo build`
Expected: builds (the `Err(_)` arms in `fetch_latest_versions` still match the new error type).

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: zero warnings (remove the unused `use anyhow::Result;` if flagged).

- [ ] **Step 7: Commit**

```bash
git add src/package/sparse_index.rs tests/sparse_index_http.rs
git commit -m "feat: typed SparseIndexError for sparse index client

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: `CheckError` types, `VersionLookup`, and `check_package_updates` plumbing (spec item 2, part B)

Thread the failure classification from `fetch_latest` into a new `PackageInfo.check_error` field.

**Files:**
- Modify: `src/models/mod.rs` (new types; `PackageInfo`)
- Modify: `src/package/mod.rs` (`VersionLookup`, `fetch_latest_versions`, `get_latest_version`, `check_package_updates`)
- Modify: `tests/sparse_index_http.rs` (new test)

- [ ] **Step 1: Write the failing test for `CheckErrorKind::kind_str`**

Add to the `tests` module in `src/models/mod.rs`:

```rust
#[test]
fn check_error_kind_str_maps_both_variants() {
    assert_eq!(CheckErrorKind::NotFound.kind_str(), "not_found");
    assert_eq!(CheckErrorKind::Unavailable.kind_str(), "unavailable");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib check_error_kind_str_maps_both_variants -- --exact`
Expected: FAIL — `CheckErrorKind` not found.

- [ ] **Step 3: Add `CheckErrorKind` and `CheckError` to models**

In `src/models/mod.rs`, after the `InstallOpts` impl (after line 70), add:

```rust
/// 版本检查失败的可判别分类。决定 JSON `version_check_errors[].kind`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckErrorKind {
    /// registry index 里没有这个包（4xx）——重试无意义，可能是改名 / 配错 registry。
    NotFound,
    /// 网络 / 超时 / 5xx / 解析失败——瞬时故障，重试 CI 作业可能恢复。
    Unavailable,
}

impl CheckErrorKind {
    /// JSON 里用 "not_found" / "unavailable" 短串表示。
    pub fn kind_str(&self) -> &'static str {
        match self {
            CheckErrorKind::NotFound => "not_found",
            CheckErrorKind::Unavailable => "unavailable",
        }
    }
}

/// 一个包版本检查失败的记录。`message` 是人读的文案，不保证稳定、不要据此分支。
#[derive(Debug, Clone)]
pub struct CheckError {
    pub kind: CheckErrorKind,
    pub message: String,
}
```

- [ ] **Step 4: Add the `check_error` field to `PackageInfo`**

In `src/models/mod.rs`, add the field to the `PackageInfo` struct (lines 72-79):

```rust
#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub source: PackageSource,
    pub install_opts: Option<InstallOpts>,
    pub check_error: Option<CheckError>,
}
```

And initialize it in `with_source` (the constructor at lines 96-108) — add `check_error: None,` to the struct literal:

```rust
        Self {
            name,
            current_version,
            latest_version: None,
            source,
            install_opts: None,
            check_error: None,
        }
```

- [ ] **Step 5: Run the `CheckErrorKind` test to verify it passes**

Run: `cargo test --lib check_error_kind_str_maps_both_variants -- --exact`
Expected: PASS

- [ ] **Step 6: Add the `VersionLookup` struct and update `fetch_latest_versions`**

In `src/package/mod.rs`, add this struct just before `fetch_latest_versions` (before line 405):

```rust
/// `fetch_latest_versions` 的返回值：版本号 + 可选的检查失败信息。
/// `error` 为 `Some` 时表示既没拿到 sparse index 结果，兜底也失败了。
pub struct VersionLookup {
    pub versions: sparse_index::LatestVersions,
    pub error: Option<crate::models::CheckError>,
}

/// 把 sparse index 的失败分类成对外的 `CheckError`。
fn classify_sparse_error(e: &sparse_index::SparseIndexError) -> crate::models::CheckError {
    use crate::models::{CheckError, CheckErrorKind};
    match e {
        sparse_index::SparseIndexError::NotFound => CheckError {
            kind: CheckErrorKind::NotFound,
            message: "package not found in registry index".to_string(),
        },
        sparse_index::SparseIndexError::Unavailable(err) => CheckError {
            kind: CheckErrorKind::Unavailable,
            message: err.to_string(),
        },
    }
}
```

Then replace the body of `fetch_latest_versions` (lines 405-442 — keep the doc comment and signature, but change the return type from `sparse_index::LatestVersions` to `VersionLookup`):

```rust
pub async fn fetch_latest_versions(
    package_name: &str,
    include_prerelease: bool,
    registry_override: Option<&str>,
    no_fallback: bool,
    verbose: bool,
) -> VersionLookup {
    let base = registry::sparse_index_base(registry_override);
    let sparse_err = match sparse_index::fetch_latest(http_client(), &base, package_name).await {
        Ok(v) => {
            return VersionLookup {
                versions: v,
                error: None,
            };
        }
        Err(e) => e,
    };

    if no_fallback {
        return VersionLookup {
            versions: sparse_index::LatestVersions::default(),
            error: Some(classify_sparse_error(&sparse_err)),
        };
    }

    if verbose {
        crate::display::status_dim(
            "Fallback",
            &format!("cargo search (slow path) for {}", package_name.cyan()),
        );
    }
    // 回退到 cargo search——只能拿一个版本，根据需求填入对应字段
    match cargo_search_fallback(package_name, include_prerelease).await {
        Ok(Some(v)) => {
            let versions = if is_stable_version(&v) {
                sparse_index::LatestVersions {
                    stable: Some(v),
                    prerelease: None,
                }
            } else {
                sparse_index::LatestVersions {
                    stable: None,
                    prerelease: Some(v),
                }
            };
            VersionLookup {
                versions,
                error: None,
            }
        }
        _ => VersionLookup {
            versions: sparse_index::LatestVersions::default(),
            error: Some(classify_sparse_error(&sparse_err)),
        },
    }
}
```

- [ ] **Step 7: Update `get_latest_version` for the new return type**

In `src/package/mod.rs`, replace the body of `get_latest_version` (lines 446-457) — the function keeps its signature:

```rust
pub async fn get_latest_version(
    package_name: &str,
    include_prerelease: bool,
) -> Result<Option<String>> {
    let latest =
        fetch_latest_versions(package_name, include_prerelease, None, false, false)
            .await
            .versions;
    Ok(if include_prerelease {
        latest.prerelease.or(latest.stable)
    } else {
        latest.stable
    })
}
```

- [ ] **Step 8: Update `check_package_updates` to record `check_error`**

In `src/package/mod.rs` `check_package_updates`, the spawned task currently returns `(index, package_name, latest)` where `latest` is the fetch result. After this change `fetch_latest_versions` returns a `VersionLookup`, so the tuple's third element is now a `VersionLookup`. In the result-collecting loop (lines 542-580), replace the body between the `let Ok(...) else { ... }` and the closing of the loop with:

```rust
    for handle in handles {
        let Ok((index, package_name, lookup)) = handle.await else {
            if verbose {
                crate::display::status_warn("Check", language.get_text("check_failed"));
            }
            continue;
        };

        let current = packages[index].current_version.clone();
        let chosen = choose_latest(
            lookup.versions.stable.as_deref(),
            lookup.versions.prerelease.as_deref(),
            current.as_deref(),
            include_prerelease,
        );

        if verbose {
            match &chosen {
                Some(v) => crate::display::status_dim(
                    "Latest",
                    &format!(
                        "{} {}: {}",
                        package_name.cyan(),
                        language.get_text("latest_version"),
                        v.green()
                    ),
                ),
                None => crate::display::status_warn(
                    "Check",
                    &format!(
                        "{} {}",
                        package_name.red(),
                        language.get_text("unable_to_get_latest_version")
                    ),
                ),
            }
        }
        packages[index].latest_version = chosen;
        packages[index].check_error = lookup.error;
    }
```

(The `tokio::spawn` closure at lines 516-538 needs no change — it already binds `let latest = fetch_latest_versions(...).await;` and returns `(index, package_name, latest)`; only the type flowing through changes. Optionally rename the closure's `latest` binding to `lookup` for clarity.)

- [ ] **Step 9: Write the failing wiremock test for `check_error`**

Add to `tests/sparse_index_http.rs`:

```rust
#[tokio::test]
async fn check_package_updates_records_unavailable_error() {
    use cargo_fresh::models::{CheckErrorKind, PackageInfo, PackageSource};
    use cargo_fresh::package::check_package_updates;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let mut packages = vec![PackageInfo::with_source(
        "ripgrep".into(),
        Some("14.0.0".into()),
        PackageSource::Crates,
    )];
    // no_fallback = true 跳过 cargo search 慢路径，保证离线确定性
    check_package_updates(&mut packages, false, false, Some(server.uri()), true)
        .await
        .unwrap();

    let err = packages[0].check_error.as_ref().expect("check_error set");
    assert_eq!(err.kind, CheckErrorKind::Unavailable);
    assert!(packages[0].latest_version.is_none());
}
```

- [ ] **Step 10: Run the new test and full suite**

Run: `cargo test --test sparse_index_http check_package_updates_records_unavailable_error -- --exact`
Expected: PASS

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, zero warnings.

- [ ] **Step 11: Commit**

```bash
git add src/models/mod.rs src/package/mod.rs tests/sparse_index_http.rs
git commit -m "feat: record version-check failures on PackageInfo.check_error

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: `version_check_errors[]`, the `fresh[]` fix, and `check_errors` count (spec item 2, part C)

Surface the recorded errors in JSON and stop reporting failed-lookup packages as `fresh`.

**Files:**
- Modify: `src/models/mod.rs` (`JsonCheckError`, `JsonReport`, `JsonSummary`)
- Modify: `src/main.rs` (`build_report`)

- [ ] **Step 1: Add the `JsonCheckError` struct**

In `src/models/mod.rs`, after the `JsonSkipped` struct (after line 178), add:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct JsonCheckError<'a> {
    pub name: &'a str,
    pub kind: &'static str,
    pub error: &'a str,
}
```

- [ ] **Step 2: Add `version_check_errors` to `JsonReport` and `check_errors` to `JsonSummary`**

In `src/models/mod.rs`, add the field to `JsonReport` (the struct at lines 149-162) — insert after `skipped`:

```rust
    pub skipped: Vec<JsonSkipped<'a>>,
    pub version_check_errors: Vec<JsonCheckError<'a>>,
    pub results: Vec<JsonResult<'a>>,
```

And add `check_errors` to `JsonSummary` — insert after `skipped`:

```rust
    pub skipped: usize,
    pub check_errors: usize,
    pub duration_ms: u128,
```

- [ ] **Step 3: Write the failing test for the `fresh[]` exclusion**

Add to the `tests` module in `src/main.rs`:

```rust
#[test]
fn build_report_excludes_check_error_packages_from_fresh() {
    use cargo_fresh::models::{CheckError, CheckErrorKind};

    let cli = empty_cli();
    let mut errored = PackageInfo::with_source(
        "bat".into(),
        Some("0.24.0".into()),
        PackageSource::Crates,
    );
    errored.check_error = Some(CheckError {
        kind: CheckErrorKind::Unavailable,
        message: "sparse index HTTP 503".into(),
    });
    let fresh_pkg = PackageInfo::with_source(
        "ripgrep".into(),
        Some("14.1.1".into()),
        PackageSource::Crates,
    );
    let packages = vec![errored, fresh_pkg];

    let report = build_report(&cli, &packages, &[], &[], false, std::time::Instant::now(), 0);

    assert_eq!(report.fresh, vec!["ripgrep"]);
    assert_eq!(report.version_check_errors.len(), 1);
    assert_eq!(report.version_check_errors[0].name, "bat");
    assert_eq!(report.version_check_errors[0].kind, "unavailable");
    assert_eq!(report.summary.check_errors, 1);
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test --bin cargo-fresh build_report_excludes_check_error -- --exact`
Expected: FAIL — compile error, `version_check_errors` field missing on `JsonReport`.

- [ ] **Step 5: Build `version_check_errors`, fix the `fresh` filter, set `check_errors` in `build_report`**

In `src/main.rs` `build_report`:

Change the `fresh` builder to exclude packages with a check error:

```rust
    let fresh: Vec<&str> = packages
        .iter()
        .filter(|p| !p.has_update() && p.source.is_crates() && p.check_error.is_none())
        .map(|p| p.name.as_str())
        .collect();
```

After the `skipped` builder, add the `version_check_errors` builder:

```rust
    let version_check_errors: Vec<JsonCheckError> = packages
        .iter()
        .filter_map(|p| {
            p.check_error.as_ref().map(|e| JsonCheckError {
                name: p.name.as_str(),
                kind: e.kind.kind_str(),
                error: e.message.as_str(),
            })
        })
        .collect();
```

Change the `summary` construction to set `check_errors`:

```rust
    let summary = JsonSummary {
        checked: packages.len(),
        available: updates_available.len(),
        selected,
        attempted: results.len(),
        succeeded,
        failed,
        skipped: skipped.len(),
        check_errors: version_check_errors.len(),
        duration_ms: start.elapsed().as_millis(),
    };
```

Add `version_check_errors` to the returned `JsonReport` literal (after `skipped`):

```rust
        skipped,
        version_check_errors,
        results,
```

Add the import — change the `use cargo_fresh::models::{...}` line near the top of `src/main.rs` (line 15) to include `JsonCheckError`.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test --bin cargo-fresh build_report_excludes_check_error -- --exact`
Expected: PASS

- [ ] **Step 7: Run the full suite and clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass, zero warnings.

- [ ] **Step 8: Commit**

```bash
git add src/models/mod.rs src/main.rs
git commit -m "feat(json): add version_check_errors[] and fix fresh[] silent failure

A crates.io package whose sparse-index lookup failed previously landed
in fresh[]. It is now excluded and reported in version_check_errors[].

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Documentation — JSON schema, README, CHANGELOG

**Files:**
- Modify: `docs/json-schema.json`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Update `docs/json-schema.json`**

Read `docs/json-schema.json`. It is a Draft 2020-12 schema for the `JsonReport`. Apply these additive changes, matching the file's existing structure and indentation:

1. In the `skipped` array's item schema (the object describing one `JsonSkipped`), add a `reason_code` property between `source` and `reason`:
   ```json
   "reason_code": {
     "type": "string",
     "enum": ["path_source", "git_source", "unknown_source"],
     "description": "Stable machine-readable skip reason."
   }
   ```
   Add `"reason_code"` to that object's `required` array.

2. Add a new top-level `version_check_errors` property alongside `skipped`:
   ```json
   "version_check_errors": {
     "type": "array",
     "description": "crates.io packages whose latest-version lookup failed; fresh[] excludes these.",
     "items": {
       "type": "object",
       "properties": {
         "name": { "type": "string" },
         "kind": {
           "type": "string",
           "enum": ["not_found", "unavailable"],
           "description": "not_found = absent from registry index (permanent); unavailable = network/5xx/parse (transient)."
         },
         "error": { "type": "string", "description": "Human-readable message; not stable, do not branch on it." }
       },
       "required": ["name", "kind", "error"],
       "additionalProperties": false
     }
   }
   ```
   Add `"version_check_errors"` to the top-level `required` array.

3. In the `summary` object schema, add three integer properties — `selected`, `attempted`, `check_errors`:
   ```json
   "selected": { "type": "integer", "description": "Packages chosen for update this run." },
   "attempted": { "type": "integer", "description": "Packages an install command was run for (succeeded + failed)." },
   "check_errors": { "type": "integer", "description": "Length of version_check_errors[]." }
   ```
   Add `"selected"`, `"attempted"`, `"check_errors"` to the `summary` object's `required` array.

- [ ] **Step 2: Validate the schema file is still valid JSON**

Run: `python3 -c "import json; json.load(open('docs/json-schema.json'))"`
Expected: no output (exit 0) — the file parses.

- [ ] **Step 3: Update `README.md`**

Find the section documenting `--format=json` output. Add a short note describing the three additions: the `reason_code` enum on `skipped[]`, the `version_check_errors[]` array (and that `fresh[]` now excludes packages whose check failed), and the `selected`/`attempted`/`check_errors` summary counts. Match the surrounding prose style; keep it to a few sentences.

- [ ] **Step 4: Update `CHANGELOG.md`**

Under the `[Unreleased]` heading, add entries (under an `### Added` subsection, creating it if absent):

```markdown
### Added

- `--format=json`: `skipped[].reason_code` — a stable enum (`path_source` / `git_source` / `unknown_source`) so scripts need not branch on the prose `reason`.
- `--format=json`: `version_check_errors[]` — crates.io packages whose latest-version lookup failed, each with a `kind` (`not_found` / `unavailable`). `fresh[]` now excludes these instead of silently reporting them as up to date.
- `--format=json`: `summary.selected`, `summary.attempted`, `summary.check_errors` counts.
```

All additive under `schema_version=1` — no schema version bump.

- [ ] **Step 5: Commit**

```bash
git add docs/json-schema.json README.md CHANGELOG.md
git commit -m "docs: document JSON contract additions for 1.0

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Integration contract test and final verification

**Files:**
- Modify: `tests/cli.rs` (after `json_mode_keeps_stdout_clean`, around line 106)

- [ ] **Step 1: Write the failing contract test**

Add to `tests/cli.rs`:

```rust
#[test]
fn json_mode_emits_new_contract_fields() {
    // --format=json 始终带上 1.0 新增的契约字段，与本机装了哪些包无关
    let out = bin()
        .args(["--batch", "--dry-run", "--format=json", "--filter=__nonexistent_pkg_xyz__"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    for key in [
        "\"version_check_errors\"",
        "\"selected\"",
        "\"attempted\"",
        "\"check_errors\"",
    ] {
        assert!(out.contains(key), "JSON missing {key}:\n{out}");
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --test cli json_mode_emits_new_contract_fields -- --exact`
Expected: PASS (all four keys are unconditionally present in every report).

- [ ] **Step 3: Full verification**

Run: `cargo test`
Expected: all unit + integration tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: zero warnings.

Run: `cargo build --release`
Expected: builds clean (also refreshes `Cargo.lock` if needed).

- [ ] **Step 4: Manual smoke check of the JSON output**

Run: `cargo run -- --batch --dry-run --format=json --filter=__nonexistent_pkg_xyz__`
Expected: a single JSON line on stdout containing `"version_check_errors":[]`, `"selected":0`, `"attempted":0`, `"check_errors":0`.

- [ ] **Step 5: Commit**

```bash
git add tests/cli.rs Cargo.lock
git commit -m "test: assert JSON contract additions are present

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Done

All three spec items implemented:
- **Item 1** — `reason_code` on `skipped[]` (Task 2)
- **Item 2** — `version_check_errors[]` + `fresh[]` fix (Tasks 4, 5, 6)
- **Item 3** — `selected` / `attempted` in `summary` (Task 3)

Item 4 (`prerelease_policy`) deliberately deferred to 1.x per the spec.

After the branch is green, the remaining manual step is to post the reply on the 1.0 meta issue and (when ready) move the `CHANGELOG.md` `[Unreleased]` entries under a dated release heading per the release process in `CLAUDE.md`.
