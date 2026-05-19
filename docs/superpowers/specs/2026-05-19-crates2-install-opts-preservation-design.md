# Design: Preserve install options via `.crates2.json`

**Date:** 2026-05-19
**Status:** Approved (brainstorming) — pending spec review
**Topic:** Make `cargo fresh` re-install packages with the same Cargo features they were originally installed with, instead of silently resetting to default features.

## Problem

cargo-fresh updates a crates.io package by running `cargo install --force <name> [--version V]`. It never passes `--features` / `--no-default-features` / `--all-features`. So a package installed as `cargo install ripgrep --features pcre2` is silently downgraded to default features on the next `cargo fresh`.

This is a correctness bug, not a missing nicety. It also undermines the README's "source-aware updates" positioning: cargo-update preserves these options by reading `~/.cargo/.crates2.json`; cargo-fresh currently does not. Closing this gap is a pre-1.0 priority.

`cargo binstall` compounds the bug: it downloads prebuilt binaries whose feature set was fixed by the upstream release author, so `--features` cannot be applied through binstall at all. When binstall is used, any non-default features are lost even if we constructed the right `cargo install` arguments.

## Scope

**In scope:** preserve the three feature-related options only:

- `no_default_features: bool`
- `all_features: bool`
- `features: Vec<String>`

**Explicitly out of scope (YAGNI):** `profile`, `target`, `rustc`, `version_req`, `bins`. `target` in particular would cause update failures when the host toolchain or machine changes; it is deferred to a future iteration if a real need appears.

## `.crates2.json` format (verified against a real file)

Located at `$CARGO_HOME/.crates2.json` (default `~/.cargo/.crates2.json`). Shape:

```json
{
  "installs": {
    "ripgrep 14.1.1 (registry+https://github.com/rust-lang/crates.io-index)": {
      "version_req": null,
      "bins": ["rg"],
      "features": ["pcre2"],
      "all_features": false,
      "no_default_features": false,
      "profile": "release",
      "target": null,
      "rustc": null
    }
  }
}
```

- Top-level `installs` object.
- Key format: `"<name> <version> (<source>)"` — cargo's PackageId. `<source>` is `registry+https://...` for crates.io, `git+URL[#rev]` for git, `path+file://...` for path installs.
- Only `features`, `all_features`, `no_default_features` are consumed.

## Architecture

Chosen approach: a new pure-parser module mirroring the existing "read `cargo install --list` once, attach to `PackageInfo`" pattern. This keeps CARGO_HOME resolution in one place and makes the parser unit-testable offline (consistent with the project's wiremock / pure-function testing philosophy).

### 1. Data model — `src/models/mod.rs`

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstallOpts {
    pub no_default_features: bool,
    pub all_features: bool,
    pub features: Vec<String>,
}

impl InstallOpts {
    /// All-default install — safe to use binstall, no extra cargo flags needed.
    pub fn is_default(&self) -> bool {
        !self.no_default_features && !self.all_features && self.features.is_empty()
    }
}
```

`PackageInfo` gains one field:

```rust
pub install_opts: Option<InstallOpts>,
```

`None` means "no metadata found" → current default behavior (default features). `PackageInfo::with_source` / `new` constructors default this to `None`; a builder or direct field set is used where opts are known. Existing call sites that construct `PackageInfo` are updated to set `install_opts: None`.

### 2. New module — `src/package/crates2.rs`

- `pub fn parse_crates2(json: &str) -> HashMap<String, InstallOpts>` — pure. Parses the top-level `installs` object via `serde_json` (already a direct dependency: `serde_json = "1.0.145"`). Each entry's key is kept verbatim; value maps to `InstallOpts` reading only the three fields (missing fields default: bools `false`, `features` `[]`). Malformed top-level JSON → empty `HashMap` (never panics, never errors).
- `pub fn load_install_opts() -> HashMap<String, InstallOpts>` — locates `$CARGO_HOME/.crates2.json` via `registry::cargo_home()`. File missing or unreadable or unparseable → empty map. No error propagation, no warning printed.
- `registry::cargo_home` is changed from private to `pub(crate)` so `crates2.rs` reuses the exact same CARGO_HOME resolution (env `CARGO_HOME`, fallback `$HOME/.cargo`).
- `src/package/mod.rs` adds `pub mod crates2;` and `src/lib.rs` re-export follows the existing module-tree pattern so integration/unit tests can call `cargo_fresh::package::crates2::parse_crates2`.

### 3. Assembly & matching — `get_installed_packages()`

After building the package list, call `load_install_opts()` once. For each `PackageInfo`, match by **package name** = the first whitespace-delimited token of the `.crates2.json` key. On a name match, populate `install_opts`. If multiple entries share a name, prefer the one whose `(<source>)` prefix is consistent with the package's `PackageSource` (`registry+` ↔ `Crates`, `git+` ↔ `Git`, `path+` ↔ `Path`); if still ambiguous, take the first and do not error. The single read happens alongside the existing install-list parse — no extra `cargo` subprocess, no N+1.

### 4. Command construction — `build_args`

Signature gains `opts: Option<&InstallOpts>`. For `Crates`, `Git`, and `Path` sources uniformly (all accept these flags via `cargo install`), append after the existing args:

- `no_default_features == true` → `--no-default-features`
- `all_features == true` → `--all-features`
- non-empty `features` → `--features` then a single comma-joined `a,b,c` argument

`Unknown` source: unchanged. Because dry-run preview routes through the same `build_args`, the previewed command automatically reflects the preserved features.

Argument-lifetime note: `build_args` currently returns `Vec<&'a str>` borrowing its inputs. The comma-joined features string is owned and constructed per call; `build_args` changes to return `Vec<String>` (or the call sites own the joined string and pass `&str`). Implementer picks the least-invasive of the two; the plan must call this out because it touches every `build_args` call site and the two existing call sites in `update_package` (primary + binstall→install fallback).

### 5. binstall conflict

In `update_package`, the existing decision that selects binstall is tightened to:

```
use_binstall && opts.map_or(true, |o| o.is_default())
```

So a package with non-default features goes straight to `cargo install` even when binstall is available — the only path on which features actually take effect. The binstall→install fallback path is unaffected (it already uses `cargo install`, and now with features).

### 6. Missing metadata / old cargo / no entry

All silently fall back to current behavior (default features, binstall eligible). Under `--verbose` only, emit one line reusing the existing `Check` verb: `Check <pkg> no install metadata, using default features`. No new status verb, no default-mode output, no warning.

## Error handling

There is no error path. Every failure mode (no file, unreadable, malformed JSON, no matching entry, unknown source) degrades to "behave as today". `load_install_opts` returns `HashMap` not `Result`. This is deliberate: install-option preservation is best-effort enrichment, never a reason to fail an update the user asked for.

## Testing

- `src/package/crates2.rs` unit tests with inline fixtures: default package (all false/empty), package with `features`, `all_features: true`, `no_default_features: true`, empty `installs`, missing `installs` key, malformed JSON, multiple same-name entries with different sources.
- `build_args` unit tests: `Crates` / `Git` / `Path` × { default opts (no extra flags), features list, all-features, no-default-features, combination }. Assert exact arg vectors.
- `update_package` binstall-selection: a non-default-features package does not select binstall even when binstall is "available" (test the pure selection predicate, not a real subprocess).
- `tests/cli.rs`: no change. No external CLI contract changes — flag inventory, exit codes, and `--format=json` `schema_version=1` are all untouched.

## Documentation changes (same PR)

- `CHANGELOG.md` `[Unreleased]`:
  - `BEHAVIOR`: packages with non-default features now skip binstall and use `cargo install` (so features take effect).
  - `Fixed`: updates no longer silently drop `--features` / `--no-default-features` / `--all-features`.
  - `Added`: `.crates2.json` parsing to preserve install features.
- `README.md` comparison table: change the `Install options preserved` row for cargo-fresh from "Not yet …" to "Yes — features via `.crates2.json`"; adjust the trailing paragraph that frames this as cargo-update's main edge.
- `ROADMAP.md`: note the gap as closed.
- `CLAUDE.md`: add a `src/package/crates2.rs` row to the module-responsibilities table and a key-design-decision bullet (best-effort, silent fallback, binstall-skip rule).

## 1.0-contract assessment

The binstall-skip is a `BEHAVIOR`-class change landing inside the 1.0 feedback window (closes 2026-06-30). Judgment: it fixes a correctness bug (silently discarding user-specified features), so it is recorded as `BEHAVIOR` + `Fixed` per the project's CHANGELOG convention and does **not** require cutting `1.0.0-rc.1`. No `schema_version`, exit-code, or CLI-flag-inventory contract is touched.

## Out of scope / future

- Preserving `profile` / `target` / `rustc`.
- Per-package config file overrides (cargo-update's `cargo-install-update-config` analogue) — relates to ROADMAP P2-4.
- Installing missing packages from a declarative list.
