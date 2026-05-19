# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build --release

# Run in development
cargo run -- [flags]

# Lint (zero warnings policy enforced)
cargo clippy --all-targets -- -D warnings

# Test (84 unit + 10 integration as of v0.10.1)
cargo test

# Install locally for testing
cargo install --path .
```

`Cargo.lock` is committed — this is a binary crate, so we want reproducible builds.

## Architecture

**cargo-fresh** is a Rust CLI tool that checks and updates globally installed Cargo packages. It runs as a cargo subcommand (`cargo fresh`).

### Core workflow

1. **Discover** — parse `cargo install --list` output to get installed packages (name, version, source)
2. **Filter** — apply `--filter PATTERN` (keep matches), then `--exclude PATTERN` (drop matches); both support globset glob syntax
3. **Check** — for crates.io-sourced packages, concurrently fetch latest stable + prerelease in a single round-trip; git/path sources are skipped (no crates.io concept of "latest" for them)
4. **Display** — show update candidates in cargo-style status lines (`   Updating pkg 0.9.8 -> 0.9.12`); interactive prompts via dialoguer when not in `--batch` or `--no-interactive` mode
5. **Update** — per package: pick install command based on source (crates → binstall/install, git → `install --git URL`, path → `install --path DIR`); retry 3× with 2s backoff
6. **Verify** — read installed version after update via cached `cargo install --list`; invalidate just the updated entry to avoid N+1 list invocations

### Module responsibilities

| Module | Role |
|--------|------|
| `src/lib.rs` | Module re-exports so `tests/` can call internals directly (`cargo_fresh::package::sparse_index::fetch_latest`). Not a downstream API — `bin` and `lib` share one module tree |
| `src/main.rs` | Top-level orchestration; CLI parsing dispatch (handles both `cargo-fresh` and `cargo fresh` subcommand forms); `run() -> Result<i32>` returns exit codes, `main()` prints errors and applies `errors::hint_for` |
| `src/cli/mod.rs` | Clap argument parsing via `CommandFactory` derive (one source of truth for help, completion, and man page); shell completion generation for bash/zsh/fish/powershell/elvish/nushell; `cargo fresh man` renders roff via `clap_mangen` to stdout — same plumbing as `completion`, no build.rs needed |
| `src/models/mod.rs` | `PackageInfo`, `UpdateResult`, `PackageSource { Crates, Git { url, rev }, Path { dir }, Unknown(String) }`; `JsonReport` + friends with `schema_version = 1`; semver-based `has_update` and `is_prerelease`; `kind_str()` for JSON serialization |
| `src/errors.rs` | `thiserror`-derived `CargoFreshError` variants for the few failures that can surface actionable hints (currently `CargoListFailed`); `hint_for(&anyhow::Error)` walks the error chain and emits a `Hint:` line via `main` |
| `src/package/mod.rs` | Parses `cargo install --list`; `fetch_latest_versions(name, include_prerelease, registry_override, no_fallback, verbose)`; `choose_latest` pure function for `--include-prerelease` semantics; `filter_packages` / `exclude_packages` (globset); installed version cache uses `OnceLock::get_or_init` + `lock/clear/extend` to refresh safely across calls |
| `src/package/sparse_index.rs` | crates.io sparse index client. `index_path` shard rule + `parse_index_body` pure parser + `fetch_latest(client, base_url, name)` async HTTP via shared `reqwest::Client`; 1 retry on network/5xx with 500ms backoff, 4xx not retried |
| `src/package/registry.rs` | Resolves the sparse index base URL: explicit `--registry-url` wins; otherwise parses `$CARGO_HOME/config.toml` for `[source.crates-io] replace-with = "..." → [source.<n>].registry = "sparse+URL"`; falls back to `https://index.crates.io` |
| `src/updater/mod.rs` | Builds cargo args per source via async `tokio::process::Command`, runs commands through per-package spinner, retry loop, dry-run short-circuit; `PbGuard` RAII guards spinner cleanup; `SlowGuard` aborts the 30s slow-warning watchdog; `verify_and_report_update` covers both primary and binstall→install fallback paths |
| `src/display/mod.rs` | **All user-facing output goes through `status` / `status_warn` / `status_err` / `status_dim` (and `pb_status*` for progress-bar contexts).** Cargo-style "   Verb message" format: 12-char right-aligned colored bold verb + content. No emojis. **All status output is routed to stderr via `eprintln!`** — stdout is reserved for machine-readable JSON (`--format=json`). `JSON_MODE` atomic short-circuits everything when `--format=json`. Non-TTY (`!stderr.is_terminal()`) disables spinners but keeps `pb.println` working (indicatif draws to stderr by default) |
| `src/locale/` | Auto-detects system language via `LANG`/`LC_ALL`/`LC_CTYPE`; pure function `detect_from_locale(&str)` is what tests target (no `env::set_var` races); English/Chinese text maps in `texts.rs`. `Language::format_text(key, &[(name, value)])` uses named placeholders to avoid the bug where chained `.replace("{}", x)` would substitute all placeholders at once |
| `tests/cli.rs` | `assert_cmd` integration tests for `--version` / `--help` flag inventory / `cargo fresh` subcommand form / bash & fish completion. Verifies external contract, not byte-for-byte output |
| `tests/sparse_index_http.rs` | `wiremock` tests covering 200 / 404 (no retry) / 5xx (retry once) / 5xx-then-200 (recovery) / empty body. Offline — no network needed |

### Key design decisions

- **sparse index over `cargo search`**: `https://index.crates.io/{shard}/{name}` — ~50–100ms per request, single shared `reqwest::Client` with connection pool. `cargo search` retained as fallback for environments where sparse index is blocked
- **Concurrency limit**: `Semaphore(16)` on version-check tasks. Prevents fd exhaustion / crates.io rate-limit with 100+ packages
- **Single-pass stable + prerelease**: sparse index responses include all historical versions, so we always get both candidates in one fetch. `check_package_updates` picks `latest_version` based on update direction — stable preferred, prerelease shown when no stable update available
- **`cargo install --list` cache**: `OnceLock<Mutex<HashMap>>` populated once by `get_installed_packages`; `invalidate_installed_version(pkg)` removes a single entry after a successful update so the next read picks up the new version. Without this, N package updates would invoke `cargo install --list` N+1 times
- **semver-based comparison**: `PackageInfo::has_update` and `is_prerelease` use `semver::Version`. Yank rollbacks (current > latest) no longer flag as "needs update". Note: the semver crate's `Ord` compares build metadata for total ordering even though the SemVer spec says it shouldn't — cargo-fresh leans into this since a re-published artifact with new build metadata is usually worth reinstalling
- **binstall opt-in (BEHAVIOR change in 0.10.1)**: `is_binstall_available()` is the read-only probe; if binstall is missing, default behavior is just a `Hint:` line and falling back to `cargo install`. Set `--install-binstall` to restore the old auto-install behavior. Rationale: silently invoking `cargo install cargo-binstall` modifies the user's toolchain
- **Source-aware update strategy**: `updater::build_args` switches on `PackageSource` — crates uses `binstall`/`install`, git uses `install --git URL [--rev REV]`, path uses `install --path DIR`, `Unknown(raw)` is explicitly skipped with a `Skip [unknown source]` line so cargo-fresh never tries to do something silly with a registry it doesn't understand
- **`PbGuard` RAII for spinner cleanup**: `update_package` wraps the per-package `ProgressBar` in a `PbGuard` immediately after creation. Its `Drop` impl calls `finish_and_clear()`, guaranteeing no spinner frames are left on screen regardless of which return path is taken (success, failure, retry exhausted, dry-run)
- **No main progress bar**: The overall N/M progress is shown as a plain `   Package 3/18 cargo-fresh` status line (only when updating more than one package). This avoids two concurrent spinners conflicting in the terminal
- **i18n named placeholders**: templates use `{name}` / `{old}` / `{new}` / `{code}` / `{error}` / `{attempt}`. Each variable substituted via `format_text(key, args)` — never chain `.replace("{}", x)` on multi-placeholder templates (single-value templates may still use `{}` with `.replace("{}", val)`)
- **Bilingual UI**: all user-facing strings live in `src/locale/texts.rs` as enum-mapped pairs. Adding a language means adding a `match` arm and updating the consistency test list in `texts.rs::tests`
- **All cargo subprocess calls are async (`tokio::process::Command`)**: `get_installed_packages` / `cargo_search_fallback` / `install_binstall` / `run_cargo` no longer block the runtime. Only `is_binstall_available()` stays sync — it's gated by `OnceLock`, runs at most once, and being sync lets dry-run probe it without an async context. Tokio features trimmed from `"full"` to the minimal set needed: `["macros", "rt-multi-thread", "signal", "process", "time", "sync"]`
- **Exit code contract**: `0` no updates or all succeeded, `1` updates available but not applied (JSON mode without `--batch`; `--no-interactive` with no selection), `2` at least one update failed, `130` SIGINT. Stable as of 1.0. `run() -> Result<i32>` keeps this isolated from the `?`-propagation paths
- **`--format=json` JSON_MODE**: a global `AtomicBool` in `display::mod.rs`. All `status*` / `pb_status*` / `print_results` / `print_update_summary` short-circuit when set, and main emits one `JsonReport` line at the end. `schema_version = 1` is the 1.0 commitment — additive only. Schema is documented in `docs/json-schema.json` (Draft 2020-12) — when adding fields to `JsonReport`, update the schema file in the same commit
- **stdout vs stderr split**: machine-readable output (the JSON report) goes to **stdout**; everything else — status lines, prompts, spinners, errors — goes to **stderr**. This is verified by `tests/cli.rs::json_mode_keeps_stdout_clean` and `non_json_mode_keeps_status_off_stdout`. When adding new user-facing output, use the `status*` helpers (which `eprintln!`) — never `println!` outside of `print_json`
- **Actionable errors via `errors::hint_for`**: keep most paths on `anyhow::Result`; only define `CargoFreshError` variants for failures where we can give a user a concrete next step ("`cargo --version` to verify the toolchain"). Network connect/timeout matches `reqwest::Error` directly without an enum variant

### CLI output style

Every `println!` for user-facing output is forbidden — use the `status` family instead. This guarantees a consistent cargo-aesthetic:

```
    Checking for updates to globally installed packages
       Found 18 installed package(s)
       Fresh ripgrep 14.1.1
    Updating cargo-fresh 0.9.8 -> 0.9.13
   Would run cargo-fresh: cargo binstall --force cargo-fresh --version 0.9.13
    Fallback cargo install --force cargo-fresh --version 0.9.13
    Finished 1 succeeded, 0 failed, in 4.2s
```

Colors carry the semantic load: green (success), yellow (warning), red (failure), dim (secondary). Verbs are 12-char right-aligned and bold.

No exceptions remain — `--verbose` package-level output was migrated to `status_dim("Check", ...)` / `status_warn` / `status_dim("Latest", ...)` in 0.10.1.

### Status verb dictionary

| Verb | Color | Usage |
|------|-------|-------|
| `Checking` | green | Starting version check |
| `Found` | green | Packages discovered count |
| `Fresh` | dim | Package already up to date |
| `Updating` | green | Package has an available update (display) or update loop starting |
| `Updated` | green | Package successfully updated |
| `Installing` | green | Installing cargo-binstall |
| `Installed` | green | cargo-binstall installed successfully |
| `Would run` | green | Dry-run command preview |
| `Fallback` | yellow | binstall failed, falling back to cargo install |
| `Unchanged` | yellow | Update ran but version didn't change |
| `Package` | dim | N/M counter for multi-package update |
| `Note` | yellow | Non-fatal informational message |
| `Hint` | dim | Actionable suggestion after a failure (from `errors::hint_for`) |
| `Check` | dim | `--verbose` per-package check progress |
| `Latest` | dim | `--verbose` reports the latest version resolved for a package |
| `Skip` | yellow | Package skipped (`PackageSource::Unknown`) |
| `Slow` | yellow | 30s elapsed on a single package — emitted by `SlowGuard` watchdog |
| `Aborted` | yellow | User pressed Ctrl-C; partial progress line |
| `Failed` | red | Package update failed |
| `Finished` | green/red | Final summary line (red if any failures) |

### Adding a new locale string

1. Add the key + English text to `get_english_text` in `src/locale/texts.rs`
2. Add the key + Chinese text to `get_chinese_text` in the same file
3. Add the key to the consistency test array in `tests::test_text_consistency`
4. Use `language.get_text("key")` for single-value or positional strings; use `language.format_text("key", &[("name", val)])` for named-placeholder templates

## Release process

Releases are fully automated via GitHub Actions:
- **`ci.yml`**: every push/PR to master runs `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on stable; separate job runs `cargo check --locked --lib --bins` on the MSRV toolchain (1.86 today). MSRV job intentionally excludes `--all-targets` so dev-deps' MSRV drift doesn't break it
- **`crate.yml`**: triggered on `v*` tag push; publishes to crates.io using OIDC, creates a GitHub release
- **`release.yml`**: triggered by crate.yml completion; builds for macOS ARM64/x86_64 and Linux x86_64, uploads binaries, updates Homebrew tap

To release: bump version in `Cargo.toml`, move `[Unreleased]` content to a dated `[X.Y.Z]` heading in `CHANGELOG.md`, run `cargo build --release` (to bump `Cargo.lock`), commit, then `git tag -a vX.Y.Z -m "..."` and push both the commit and the tag.

## Roadmap to 1.0

Detailed item-by-item plan lives in `ROADMAP.md`. Status as of **v0.10.1**:

- ✅ **0.9.10–0.9.14** — Cargo.lock, semver, sparse index, cargo-style output, PbGuard
- ✅ **0.10.0** — `--include-prerelease` strict (BREAKING), `--registry-url`, mirror auto-detect, Ctrl-C cancel, `--format=json` + exit code contract
- ✅ **0.10.1** — async cargo subprocess, `--no-cargo-search-fallback`, `--install-binstall` (BEHAVIOR), non-TTY downgrade, `SlowGuard` 30s watchdog, `PackageSource::Unknown`, `errors::hint_for`, `tests/` integration suite, MSRV 1.86, ISSUE/PR/CONTRIBUTING/SECURITY, README Stability + cargo-update comparison
- 🔄 **Feedback window** — pinned meta issue #3 collecting 1.0-contract feedback. Window closes **2026-06-30**, then `1.0.0-rc.1` cut from master
- ⏭ **1.0.0-rc.1** — only ships if 0.10.x picks up BREAKING-class feedback that needs to bake before 1.0
- ⏭ **1.0.0** — promote `schema_version=1`, exit codes, CLI flag inventory to permanent contract

Remaining open question: whether to keep the `cargo search` fallback long-term. Today it's the safety net for environments where sparse index is blocked; if 1.0 feedback shows nobody relies on it, we can drop it in 1.x.
