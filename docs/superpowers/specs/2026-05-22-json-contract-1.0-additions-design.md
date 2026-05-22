# Design — JSON contract additions for 1.0

**Date:** 2026-05-22
**Status:** Approved (pending spec review)
**Origin:** Feedback on the 1.0-contract meta issue (#3). A reviewer doing a
CLI/JSON contract review (not a field report) raised four points; the user
selected three to land before `1.0.0-rc.1` and deferred the fourth.

## Goal

Make `--format=json` output safe to script against before the
`schema_version=1` contract is frozen at 1.0. Three additive field additions,
no version bump, no BREAKING/BEHAVIOR change.

## Scope

In scope (this spec):

1. **`reason_code` on `skipped[]`** — a stable, branchable enum so scripts
   never branch on the free-form `reason` prose.
2. **`version_check_errors[]` + `fresh[]` fix** — surface registry lookup
   failures so a CI job can tell "all fresh" from "could not prove freshness."
3. **`selected` / `attempted` counts in `summary`** — let a script tell
   "updates existed but policy did not apply them" from "nothing actionable"
   without reconstructing it from arrays.

Out of scope (deferred to 1.x, deliberately):

- `prerelease_policy` field. The schema is additive-only, so a speculative
  field that only mirrors the existing `include_prerelease` boolean costs
  nothing to defer. Added only if/when an `rc-only` variant actually exists.

## Background — the bug behind item 2

A crates.io package whose sparse-index lookup fails ends up with
`latest_version = None`. `PackageInfo::has_update()` returns `false` when
`latest_version` is `None`, and `emit_report` builds `fresh[]` as
`!p.has_update() && p.source.is_crates()`. **Result: a package whose version
check failed is reported as `fresh`.** A CI job reading `updates_available: []`
cannot distinguish "everything is up to date" from "three lookups timed out."
This is a correctness defect in an existing 1.0-contract field, not just a
missing nicety — which is why item 2 is the priority of the three.

## Design

All three changes are additive to the structs in `src/models/mod.rs` and the
two report builders in `src/main.rs` (`emit_report`, `emit_empty_report`).

### Item 1 — `reason_code` on `JsonSkipped`

- `JsonSkipped` gains `reason_code: &'static str`.
- Values: `path_source` | `git_source` | `unknown_source`. Exactly the
  non-crates `PackageSource` variants — `skipped[]` is filtered to
  `!source.is_crates()` so no other value can occur.
- New method `PackageSource::skip_reason_code(&self) -> &'static str`, placed
  next to the existing `kind_str()`. The `Crates` arm returns `"crates_source"`
  and is never emitted (filtered out upstream).
- `reason` (prose) stays and remains free to reword in any release.

Note: `registry_error` is **not** a `reason_code` value. `skipped[]` means
*deterministic, by-design* skips (a source cargo-fresh structurally cannot
version-check). Transient registry failures live in `version_check_errors[]`
(item 2). Keeping the two arrays disjoint makes each one mean one thing.

### Item 2 — `version_check_errors[]` and the `fresh[]` fix

**New domain types** in `src/models/mod.rs`:

```rust
pub enum CheckErrorKind { NotFound, Unavailable }

pub struct CheckError {
    pub kind: CheckErrorKind,
    pub message: String,   // free-form, human-readable; not for branching
}
```

`CheckErrorKind` gets a `kind_str()` → `"not_found"` | `"unavailable"`,
mirroring `PackageSource::kind_str()`.

- `not_found` — sparse index returned a 4xx (today `fetch_latest` `bail!`s on
  `status.is_client_error()`). Permanent: the crate is not in this registry —
  a renamed/removed crate, or a misconfigured `--registry-url`. Retry will not
  help.
- `unavailable` — network error, timeout, HTTP 5xx after the retry is
  exhausted, or a body read/parse failure. Transient: a retried CI job may
  succeed.

**`PackageInfo`** gains `check_error: Option<CheckError>`, defaulting to `None`
in both constructors.

**Plumbing — `src/package/sparse_index.rs`:**
`fetch_latest` currently returns `Result<LatestVersions>` with `anyhow::Error`.
Introduce a typed error so the kind survives:

```rust
pub enum SparseIndexError {
    NotFound,                    // 4xx
    Unavailable(anyhow::Error),  // network / 5xx-exhausted / parse
}
```

`fetch_latest` returns `Result<LatestVersions, SparseIndexError>`:
- 4xx branch → `Err(SparseIndexError::NotFound)`
- 5xx / network / `resp.text()` failure / retries-exhausted →
  `Err(SparseIndexError::Unavailable(..))`

**Plumbing — `src/package/mod.rs`:**
`fetch_latest_versions` currently returns a bare `LatestVersions`, swallowing
all errors to `default()`. Change it to also return an optional error:

```rust
pub struct VersionLookup {
    pub versions: LatestVersions,
    pub error: Option<CheckError>,
}
```

Rules:
- sparse `Ok` → `{ versions, error: None }`.
- sparse `Err` + cargo-search fallback succeeds → `{ versions, error: None }`
  (we got an answer; the failure was recovered).
- sparse `Err` + fallback fails, or `no_fallback` → `{ versions: default,
  error: Some(CheckError) }`. The `CheckError` is classified from the *sparse*
  error (`NotFound` → `not_found`, `Unavailable` → `unavailable`); the
  cargo-search failure is not separately reported.

`get_latest_version` (the `#[allow(dead_code)]` compat wrapper) keeps its
current signature — it adapts to the new `VersionLookup` internally.

`check_package_updates` sets `packages[index].check_error` from the lookup's
`error` field after each fetch.

**`JsonReport`** in `src/models/mod.rs`:

```rust
pub struct JsonCheckError<'a> {
    pub name: &'a str,
    pub kind: &'static str,   // "not_found" | "unavailable"
    pub error: &'a str,       // prose message
}
```

`JsonReport` gains `version_check_errors: Vec<JsonCheckError<'a>>`.

**`emit_report`:**
- Build `version_check_errors` from packages with `check_error.is_some()`.
- Fix the `fresh` filter: `!p.has_update() && p.source.is_crates() &&
  p.check_error.is_none()` — `fresh[]` now means what it says.
- `emit_empty_report` sets `version_check_errors: vec![]`.

**Exit code:** unchanged. The array is purely additive visibility. A
non-empty `version_check_errors` does not by itself change the exit code in
this release (could be revisited in 1.x; out of scope here).

### Item 3 — `selected` / `attempted` in `JsonSummary`

- `JsonSummary` gains `selected: usize` and `attempted: usize`.
- `selected` = number of packages chosen for update this run. Source:
  `selections.len()` in `run()`. `0` when the update block never ran (JSON
  mode without `--batch`; `--no-interactive` with no picks).
- `attempted` = `update_results.len()` (equal to `succeeded + failed`;
  surfaced explicitly so consumers need not know the identity).
- `emit_report` gains a `selected: usize` parameter, threaded from `run()`.
  `emit_empty_report` uses `0`.
- `summary` also gains `check_errors: usize` (= `version_check_errors.len()`),
  for symmetry with the existing `available` / `skipped` count fields.

## Resulting JSON shape (illustrative)

```json
{
  "schema_version": 1,
  "format": "cargo-fresh-v1",
  "updates_available": [],
  "fresh": ["ripgrep"],
  "skipped": [
    { "name": "my-tool", "source": "git",
      "reason_code": "git_source",
      "reason": "non-crates source: version check skipped" }
  ],
  "version_check_errors": [
    { "name": "bat", "kind": "unavailable",
      "error": "sparse index HTTP 503" }
  ],
  "results": [],
  "summary": {
    "checked": 3, "available": 0, "selected": 0, "attempted": 0,
    "succeeded": 0, "failed": 0, "skipped": 1, "check_errors": 1,
    "duration_ms": 412
  },
  "aborted": false
}
```

## Documentation (same commit)

- `docs/json-schema.json` — add `reason_code`, `version_check_errors`
  (with the `kind` enum), and the new `summary` properties. CLAUDE.md
  mandates updating the schema file in the same commit as `JsonReport`
  changes.
- `README.md` — JSON section: document the three additions.
- `CHANGELOG.md` — `[Unreleased]`: note the additive fields.

## Testing

- **`src/package/sparse_index.rs`** unit/wiremock: `tests/sparse_index_http.rs`
  already covers 404 and 5xx — extend those cases to assert `fetch_latest`
  returns `SparseIndexError::NotFound` on 404 and `Unavailable` on
  5xx-exhausted.
- **`src/models/mod.rs`** unit: `PackageSource::skip_reason_code()` mapping;
  `CheckErrorKind::kind_str()` mapping.
- **`fresh[]` exclusion** — extract the `fresh`/`skipped`/`version_check_errors`
  partitioning into a pure function if it eases testing, and unit-test that a
  package with `check_error: Some(..)` is excluded from `fresh` and present in
  `version_check_errors`.
- **`tests/cli.rs`** contract test: assert the JSON output object carries the
  `version_check_errors` key and `summary.selected` / `summary.attempted`
  keys, verifying the external contract shape.

## Non-goals / deferred

- `prerelease_policy` — deferred to 1.x (see Scope).
- Changing exit codes on version-check failure — deferred.
- Per-fallback (cargo-search) error reporting — only the sparse error is
  classified and reported.
