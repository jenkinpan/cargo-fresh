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

# Test (143 unit + 42 integration as of 0.11.0)
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
| `src/package/binstall_probe.rs` | `--check-binstall` implementation. Concurrently runs `cargo binstall --dry-run` against each update candidate to classify it as prebuilt-binary (fast) or compile-from-source (slow); writes the result onto `PackageInfo.binstall_kind` so display and JSON reports can show the marker. Off by default — each probe spawns cargo and hits the network |
| `src/package/sparse_index.rs` | crates.io sparse index client. `index_path` shard rule + `parse_index_body` pure parser + `fetch_latest(client, base_url, name)` async HTTP via shared `reqwest::Client`; 1 retry on network/5xx with 500ms backoff, 4xx not retried |
| `src/package/crates2.rs` | Parses `$CARGO_HOME/.crates2.json`. Pure `parse_crates2(&str)` + `match_install_opts(map, name, source)` + `load_install_opts()` file locator + `lookup_bins(cargo_home, package_name)` for downloader binary-name candidates + `write_install_record` to rewrite the version segment of an existing `installs` key. Best-effort reads: any failure → empty map; writes return Err so install.rs can surface InstallFailed |
| `src/package/crates_toml.rs` | `$CARGO_HOME/.crates.toml` line-based writer. `cargo install --list` reads `.crates.toml`, not `.crates2.json`, so the downloader path MUST update both or the listing reports the old version. Pure `update_record(body, binary_name, new_version)` does the textual edit (find line matching `"<pkg> <ver> (<src>)" = [..."<binary>"...]`, replace `<ver>`), preserving indentation/order |
| `src/package/registry.rs` | Resolves the sparse index base URL: explicit `--registry-url` wins; otherwise parses `$CARGO_HOME/config.toml` for `[source.crates-io] replace-with = "..." → [source.<n>].registry = "sparse+URL"`; falls back to `https://index.crates.io` |
| `src/package/crates_api.rs` | Minimal crates.io REST client. `fetch_repo_url(client, name) -> Option<String>` pulls `crate.repository` for the downloader's URL resolution. All failures return None — never errors the update flow |
| `src/downloader/events.rs` | `ProgressEvent` (Resolving / UrlCandidate / Downloading / Verifying / Extracting / Installing / Done / Failed) + `DownloaderError` (Unsupported / Failed / Cancelled three-way split is load-bearing for caller fallback routing) + `UnsupportedReason` / `FailureKind` enums |
| `src/downloader/resolve.rs` | Pure URL-candidate generator. `candidate_urls(name_candidates: &[String], version, repo, &[targets])` cross-products **N names × 6 tag paths × 10 filename templates × 2 archive formats × M target aliases**, deduped. `name_candidates` is `[package_name, ...bins[]]` so tauri-cli's `cargo-tauri-aarch64-apple-darwin.zip` resolves. Tag paths cover plain `v{ver}` + monorepo `{pkg}-v{ver}` / `{pkg}/v{ver}` variants. `current_targets()` returns alias array per (arch, os) — macOS aarch64 gets 3, Linux x86_64 gets 4 |
| `src/downloader/fetch.rs` | HTTP fetch. `head_probe_concurrent` fires HEADs through `Semaphore(16)` + `FuturesUnordered` with 5s per-request timeout, returns the first 2xx (drops the rest, cancelling in-flight). "No prebuilt" detection: ~1-2s typical, 5s worst (vs 6-20s serial). Winning URL streams via `reqwest::bytes_stream()` to `TempDir/archive.{ext}` with cancel-check between chunks. Best-effort `.sha256` sidecar (404 tolerated; mismatch → `Failed(ChecksumMismatch)`) |
| `src/downloader/archive.rs` | Extract tar.gz / zip / raw bin into a fresh `TempDir`. `extract(path, fmt, bin_candidates: &[String])` tries each candidate name in order, returns `ExtractResult { temp_dir, binary_path, binary_name }` so install.rs writes to `~/.cargo/bin/<actual_binary_name>`. `find_binary` BFS depth-3 handles both "binary at archive root" (mdbook) and "binary in versioned subdir" (ripgrep, cargo-deny) layouts |
| `src/downloader/install.rs` | Atomic install to `~/.cargo/bin`. Copy to `.cargo-fresh-{name}-{uuid}.tmp` → chmod 0o755 → fsync → `fs::rename`. Rename failure paths explicitly `remove_file` the tmp. Updates **both** `.crates2.json` (via `crates2::write_install_record`) **and** `.crates.toml` (via `crates_toml::write_install_record`) — the second one is what `cargo install --list` actually reads |
| `src/downloader/mod.rs` | `download_and_install(client, spec, old_version, events_tx, cancel) -> Result<InstallOutcome, DownloaderError>` wires the 4 stages with cancel checks at every async boundary. `InstallSpec { name, version, repo_url, bins }` — caller (updater) fills `repo_url` from `crates_api::fetch_repo_url` and `bins` from `crates2::lookup_bins` |
| `src/updater/mod.rs` | Builds cargo args per source via async `tokio::process::Command`, runs commands through a per-package row in the shared `MultiProgress`, retry loop, dry-run short-circuit. `UpdatePlan::new(names)` pre-registers all selected packages as right-aligned `pending` rows (rustup-style stacked list). Per-package state machine: `pending` → `resolving` → `[bar]` → `verifying` / `extracting` / `installing` → `installed X.XX MiB` (or `compiling from source` → `installed` for fallback path). `finalize_installed/_failed/_aborted` swap the row to a static line via `pb.finish()` so the screen accumulates the full history. `PbGuard` RAII is a safety net for panic/abort paths. `SlowGuard` aborts the 30s slow-warning watchdog. `verify_and_report_update` no longer prints `Updated X → Y` (the row's final state + summary cover it) |
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
- **Install-option preservation (best-effort)**: `get_installed_packages` attaches `Option<InstallOpts>` (features/no-default/all-features) parsed from `.crates2.json`; `build_args` appends the matching flags. Packages with non-default features skip binstall (it cannot apply arbitrary features). Missing/corrupt metadata silently falls back to default features — never fails an update. `profile`/`target` intentionally not preserved
- **Source-aware update strategy**: `updater::build_args` switches on `PackageSource` — crates uses `binstall`/`install`, git uses `install --git URL [--rev REV]`, path uses `install --path DIR`, `Unknown(raw)` is explicitly skipped with a `Skip [unknown source]` line so cargo-fresh never tries to do something silly with a registry it doesn't understand
- **Sticky binstall→install fallback**: the retry loop's command is owned by `CommandSelector`. `current()` returns the active cargo args; the first `switch_to_fallback()` after a binstall failure permanently swaps in the `cargo install` args, so attempts 2 and 3 retry `install` — never `binstall`. Retrying binstall after it has already failed in this environment just repeats the slow build-from-source path; worst-case command sequence is `binstall, install, install, install`
- **rustup-style stacked rows via indicatif `MultiProgress`**: `UpdatePlan::new(names)` pre-registers all selected packages as right-aligned `pending` rows in the shared `MultiProgress` singleton (`updater::multi_progress()`). Each row's `pb.finish_with_message(...)` (via `finalize_installed/_failed/_aborted`) leaves a static line on screen instead of clearing, so the screen accumulates `cargo-deny installed 4.21 MiB` / `tauri-cli installed 7.22 MiB` / etc. Same code path works for 0.12.0 concurrent scheduler — just replace the for loop with `JoinSet`. `PbGuard` is now a panic safety net, not the primary cleanup path
- **Multi-name + monorepo tag URL candidates**: `resolve::candidate_urls` accepts `name_candidates: &[String]` (package name + `bins[]` from `.crates2.json`) so packages where the GitHub Release filename uses the binary name (`cargo-tauri-*.zip` for tauri-cli, `rg-*` would-be for ripgrep though ripgrep uses package name) resolve. Tag paths include `{pkg}-v{ver}` / `{pkg}/v{ver}` monorepo variants — tauri-apps/tauri tags as `tauri-cli-v2.11.2/cargo-tauri-aarch64-apple-darwin.zip` and this is the path that makes it work
- **Concurrent HEAD probing with timeout** (`fetch::head_probe_concurrent`): `FuturesUnordered` + `Semaphore(16)` + 5s per-HEAD timeout. First 2xx wins, remaining tasks drop (cancel in-flight). "No prebuilt available" detection drops from 6-20s (360 candidates × 150ms serial × reqwest 30s default timeout) to ~1-2s typical / 5s worst — this is the user-visible cost of any package that has to fall back to `cargo install`
- **Write both `.crates.toml` AND `.crates2.json`**: `cargo install --list` reads `.crates.toml`, not `.crates2.json`. Pre-fix, downloader only wrote the second file → second `cargo fresh` run would see the old version and report "Unchanged" / "needs update again". `install::install_binary` now updates both; `crates_toml::update_record` does a textual line-based rewrite (find line where key starts with `"<pkg> ..."` OR bracket list contains `"<binary>"`, replace `<ver>` segment), preserving format/indentation
- **`InstallMethod` per result + grouped summary**: `UpdateResult::install_method` tracks Downloader / CargoInstall / Unknown via `.with_install_method()` setter at each success path. End-of-run summary appends `Prebuilt: pkg-a, pkg-b` (cyan) / `Compiled: pkg-c` (yellow) after the per-package `Updated X 旧 -> 新` lines, omitting empty groups. Per-package `Using ...` scroll-above lines removed (4 lines × N packages of noise)
- **i18n named placeholders**: templates use `{name}` / `{old}` / `{new}` / `{code}` / `{error}` / `{attempt}`. Each variable substituted via `format_text(key, args)` — never chain `.replace("{}", x)` on multi-placeholder templates (single-value templates may still use `{}` with `.replace("{}", val)`)
- **Bilingual UI**: all user-facing strings live in `src/locale/texts.rs` as enum-mapped pairs. Adding a language means adding a `match` arm and updating the consistency test list in `texts.rs::tests`
- **All cargo subprocess calls are async (`tokio::process::Command`)**: `get_installed_packages` / `cargo_search_fallback` / `install_binstall` / `run_cargo` no longer block the runtime. Only `is_binstall_available()` stays sync — it's gated by `OnceLock`, runs at most once, and being sync lets dry-run probe it without an async context. Tokio features trimmed from `"full"` to the minimal set needed: `["macros", "rt-multi-thread", "signal", "process", "time", "sync"]`
- **Exit code contract**: `0` no updates or all succeeded, `1` updates available but not applied (JSON mode without `--batch`; `--no-interactive` with no selection), `2` at least one update failed, `130` SIGINT. Stable as of 1.0. `run() -> Result<i32>` keeps this isolated from the `?`-propagation paths
- **`--format=json` JSON_MODE**: a global `AtomicBool` in `display::mod.rs`. All `status*` / `pb_status*` / `print_results` / `print_update_summary` short-circuit when set, and main emits one `JsonReport` line at the end. `schema_version` bumps on rename/remove; additive within a major. 0.12.0 bumps to 2 (rename `updates_available[].binstall` → `prebuilt`, `source_build` → `source`); 1.0 will freeze whatever `schema_version` is current at that release. Schema is documented in `docs/json-schema.json` (Draft 2020-12) — when adding/renaming fields on `JsonReport`, update the schema file in the same commit. Additive history under `schema_version=1`: `skipped[].reason_code` (enum), `version_check_errors[]`, `summary.selected`/`attempted`/`check_errors`. `updates_available[].prebuilt` is populated only when `--check-prebuilt` is set; `null` otherwise.
- **`--check-binstall` pre-flight probe**: opt-in flag that runs `cargo binstall --dry-run` concurrently across all update candidates during the check phase to mark each as "would fetch prebuilt" vs "would compile from source". Off by default because each probe spawns cargo + hits the network (~10s per package, run concurrently). Surfaces in display output and as `updates_available[].binstall` in JSON. If cargo-binstall isn't installed the run prints a `Hint` and proceeds without markers — does not auto-install (consistent with the 0.10.1 binstall opt-in change)
- **stdout vs stderr split**: machine-readable output (the JSON report) goes to **stdout**; everything else — status lines, prompts, spinners, errors — goes to **stderr**. This is verified by `tests/cli.rs::json_mode_keeps_stdout_clean` and `non_json_mode_keeps_status_off_stdout`. When adding new user-facing output, use the `status*` helpers (which use `anstream::eprintln!`) — never `println!` outside of `print_json`
- **Color detection via `anstream`**: `main::run` calls `anstream::AutoStream::choice(&io::stderr())` once at startup and forwards the decision to `colored::control::set_override`. `colored` still owns the ergonomic `.green().bold()` API, but the actual yes/no on emitting ANSI is centralized in `anstream`'s logic — `NO_COLOR`, `CLICOLOR_FORCE`, `TERM=dumb`, and TTY detection all flow through one place. Every status write goes through `anstream::eprintln!`, which strips ANSI at the syscall boundary when the destination doesn't support it. Verified by `tests/cli.rs::no_color_env_strips_ansi_from_stderr` and `clicolor_force_keeps_ansi_when_redirected`
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
| `Prebuilt` | dim | End-of-run group: packages that took the downloader (prebuilt binary) path |
| `Compiled` | dim | End-of-run group: packages that took the cargo install (compile-from-source) path |

Per-package row phase prefixes (live, inside the `MultiProgress` row — not scrolled above):
`pending` (dim) → `resolving` / `verifying` / `extracting` / `installing` (green bold) → `downloading X.X MiB` (green bold, when content-length unknown) or bar style (when known) → `compiling from source` (yellow bold, on cargo install fallback) → terminal `installed X.XX MiB` / `installed` / `failed ...` / `aborted` (set by `finalize_*`).

### Adding a new locale string

1. Add the key + English text to `get_english_text` in `src/locale/texts.rs`
2. Add the key + Chinese text to `get_chinese_text` in the same file
3. Add the key to the consistency test array in `tests::test_text_consistency`
4. Use `language.get_text("key")` for single-value or positional strings; use `language.format_text("key", &[("name", val)])` for named-placeholder templates

## Release process

Releases are fully automated via GitHub Actions:
- **`ci.yml`**: every push/PR to master runs `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on stable; separate job runs `cargo check --locked --lib --bins` on the MSRV toolchain (1.86 today). MSRV job intentionally excludes `--all-targets` so dev-deps' MSRV drift doesn't break it
- **`audit.yml`**: runs `cargo-deny check advisories licenses sources bans` and `cargo-audit` against `Cargo.lock`. Triggers: push/PR that touches manifests or `deny.toml`, plus a weekly cron (Mon 06:00 UTC) so new RustSec advisories show up even without code changes. Config in `deny.toml`: license allowlist mirrors the actual dep-tree footprint, sources locked to crates.io, multiple-versions is `warn` (zero-duplicates is unrealistic)
- **`crate.yml`**: triggered on `v*` tag push; publishes to crates.io using OIDC, creates a GitHub release
- **`release.yml`**: triggered by crate.yml completion; builds for macOS ARM64/x86_64 and Linux x86_64, uploads binaries, updates Homebrew tap. **Version extraction is strict**: `workflow_run.head_branch` is the bare tag (e.g. `v0.10.2`), not `refs/tags/v0.10.2` — the parser strips the `v` prefix and validates semver shape. Any parse failure errors out instead of falling back to a default (a prior bug defaulted to `0.1.0`, leading to a ghost release; the fix is `exit 1` on any unresolvable trigger)

To release: bump version in `Cargo.toml`, move `[Unreleased]` content to a dated `[X.Y.Z]` heading in `CHANGELOG.md`, run `cargo build --release` (to bump `Cargo.lock`), commit, then `git tag -a vX.Y.Z -m "..."` and push both the commit and the tag.

## Roadmap to 1.0

Detailed item-by-item plan lives in `ROADMAP.md`. Status as of **v0.10.6**:

- ✅ **0.9.10–0.9.14** — Cargo.lock, semver, sparse index, cargo-style output, PbGuard
- ✅ **0.10.0** — `--include-prerelease` strict (BREAKING), `--registry-url`, mirror auto-detect, Ctrl-C cancel, `--format=json` + exit code contract
- ✅ **0.10.1** — async cargo subprocess, `--no-cargo-search-fallback`, `--install-binstall` (BEHAVIOR), non-TTY downgrade, `SlowGuard` 30s watchdog, `PackageSource::Unknown`, `errors::hint_for`, `tests/` integration suite, MSRV 1.86, ISSUE/PR/CONTRIBUTING/SECURITY, README Stability + cargo-update comparison
- ✅ **0.10.2** — 1.0 contract polish (no BREAKING / no BEHAVIOR): explicit stdout/stderr split via `anstream::eprintln!`, `docs/json-schema.json` (Draft 2020-12 for `schema_version=1`), `cargo fresh man` subcommand (clap_mangen), `anstream` color pipeline (`NO_COLOR`/`CLICOLOR_FORCE`/TTY detection centralized), `audit.yml` CI (cargo-deny + cargo-audit + weekly cron), strict `release.yml` version parsing
- ✅ **0.10.3** — `.crates2.json` install-option preservation (`src/package/crates2.rs`, features passthrough), binstall-skip for non-default-features packages (BEHAVIOR), sticky binstall→install fallback fix (`CommandSelector`), `--format=json` additions under `schema_version=1` (`skipped[].reason_code`, `version_check_errors[]`, `summary.selected`/`attempted`/`check_errors`)
- ✅ **0.10.4** — `--check-binstall` pre-flight probe (`src/package/binstall_probe.rs`, concurrent `cargo binstall --dry-run` per candidate, marks prebuilt vs source-build; `updates_available[].binstall` added under `schema_version=1`); root-cause fix for `cargo binstall` hanging on interactive `Do you wish to continue?` prompt (always pass `--no-confirm`, plus `run_cargo` redirects child stdin to `/dev/null` as second layer); Ctrl-C no longer misreported as update failure (cancel flag threaded through `update_package`'s retry loop — aborted packages marked `Aborted`, not `Failed`, and excluded from failure count). No BREAKING, no BEHAVIOR
- ✅ **0.10.5** — `cargo fresh man` auto-renders through system `man` when stdout is a TTY (writes roff to tempfile, invokes `man <tmpfile>`; redirected stdout still emits raw roff so `> cargo-fresh.1` and `| mandoc` unchanged); fish `--cargo-fresh` completion install prints a hint pointing at the correct path (`completions/cargo.fish` or `conf.d/cargo-fresh.fish`) since `cargo-fresh.fish` only autoloads on `cargo-fresh<TAB>`, not `cargo fresh<TAB>`. No BREAKING, no BEHAVIOR
- ✅ **0.10.6** — release-process polish: GitHub Release body now extracted from the matching `## [X.Y.Z]` section in `CHANGELOG.md` (via awk in `crate.yml`, with a `Full Changelog` compare link) instead of the generic template; new `changelog-sync` job in `ci.yml` fails PRs whose `Cargo.toml` version has no corresponding `CHANGELOG.md` heading, so the "forgot to update changelog" mistake gets caught at PR time rather than at tag push. No code changes, no BREAKING, no BEHAVIOR
- ✅ **0.10.7** — test-and-docs hardening: `tests/json_schema.rs` validates real `JsonReport` shapes against `docs/json-schema.json`; `tests/cli_snapshots.rs` locks 8 core verb line formats via `insta`. `display::format_status_line` extracted as the single render path. No BREAKING, no BEHAVIOR
- ✅ **0.11.0** — **self-hosted binary downloader (BEHAVIOR)**: replaces `cargo binstall` subprocess with in-process `src/downloader/` (events / resolve / fetch / archive / install). HEAD-probes 6–24 candidate URLs per package across multi-arch aliases (`aarch64-apple-darwin` + `arm64-apple-darwin` + `darwin-arm64` etc), streams the winning GitHub Release tarball with cancel-check between chunks, verifies optional `.sha256` sidecar, extracts via `tar`/`zip`, atomically renames into `~/.cargo/bin`, updates `.crates2.json`. All tempfiles owned by `tempfile::TempDir` — guaranteed `/tmp` cleanup. `update_package` now takes `Arc<AtomicBool>` for live Ctrl-C signaling. `--install-binstall` flag deprecated (no-op + warning, removal in 0.12.0). JSON `results[].phase = "binstall"` now means "downloader path succeeded" (semantics shift, schema unchanged). `src/ui/download_view.rs` (crossterm region renderer) shipped but dormant — wired in 0.12.0 concurrent scheduler
- 🔄 **0.11.0 follow-up (unreleased)** — coverage + UX polish from the verification loop. (1) `.crates.toml` writer + `bins[]` lookup + multi-name URL candidates + monorepo tag paths (`{pkg}-v{ver}`, `{pkg}/v{ver}`) fix ripgrep / tauri-cli / `cargo install --list` "Unchanged" bug; (2) HEAD probing concurrent with 5s timeout via `Semaphore(16)` + `FuturesUnordered` — fallback detection 6-20s → 1-2s; (3) rustup-style stacked rows via `MultiProgress` + `UpdatePlan` with `pending` → `[bar]` → `installed X.XX MiB` state machine + grouped `Prebuilt:` / `Compiled:` summary; `compiling from source` phase prefix for fallback path. **`src/ui/download_view.rs` + `crossterm` dep deleted** — `MultiProgress` covers serial AND the planned concurrent 0.12.0 layout, so the second renderer was redundant before it ever shipped. No BREAKING / no schema change
- 🔄 **Feedback window** — pinned meta issue #3 collecting 1.0-contract feedback. Window closes **2026-06-30**, then `1.0.0-rc.1` cut from master
- ⏭ **1.0.0-rc.1** — only ships if 0.10.x picks up BREAKING-class feedback that needs to bake before 1.0
- ⏭ **1.0.0** — promote `schema_version=1`, exit codes, CLI flag inventory to permanent contract

Remaining open question: whether to keep the `cargo search` fallback long-term. Today it's the safety net for environments where sparse index is blocked; if 1.0 feedback shows nobody relies on it, we can drop it in 1.x.
