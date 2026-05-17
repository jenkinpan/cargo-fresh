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

# Test (63 unit tests as of v0.9.14)
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
| `src/main.rs` | Top-level orchestration; CLI parsing dispatch (handles both `cargo-fresh` and `cargo fresh` subcommand forms) |
| `src/cli/mod.rs` | Clap argument parsing; shell completion generation for bash/zsh/fish/powershell/elvish/nushell |
| `src/models/mod.rs` | `PackageInfo`, `UpdateResult`, `PackageSource { Crates, Git { url, rev }, Path { dir } }`; constants `MAX_RETRY_ATTEMPTS`, `RETRY_DELAY_MS`, `VERSION_UPDATE_DELAY_MS`, `PROGRESS_TICK_MS`; semver-based `has_update` and `is_prerelease` |
| `src/package/mod.rs` | Parses `cargo install --list`; `fetch_latest_versions` (sparse index primary + cargo search fallback); `filter_packages` / `exclude_packages` (globset); installed version cache with `invalidate_installed_version` for surgical refresh after updates |
| `src/package/sparse_index.rs` | crates.io sparse index client. `index_path` shard rule + `parse_index_body` pure parser + `fetch_latest` async HTTP via shared `reqwest::Client` |
| `src/updater/mod.rs` | Builds cargo args per source, runs commands through per-package spinner, retry loop, dry-run short-circuit; `PbGuard` RAII guard ensures spinner cleanup on every return path; `verify_and_report_update` extracted helper covers both primary and binstall→install fallback paths |
| `src/display/mod.rs` | **All user-facing output goes through `status` / `status_warn` / `status_err` / `status_dim` (and `pb_status*` for progress-bar contexts).** Cargo-style "   Verb message" format: 12-char right-aligned colored bold verb + content. No emojis anywhere |
| `src/locale/` | Auto-detects system language via `LANG`/`LC_ALL`/`LC_CTYPE`; English/Chinese text maps in `texts.rs`. `Language::format_text(key, &[(name, value)])` uses named placeholders to avoid the bug where chained `.replace("{}", x)` would substitute all placeholders at once |

### Key design decisions

- **sparse index over `cargo search`**: `https://index.crates.io/{shard}/{name}` — ~50–100ms per request, single shared `reqwest::Client` with connection pool. `cargo search` retained as fallback for environments where sparse index is blocked
- **Concurrency limit**: `Semaphore(16)` on version-check tasks. Prevents fd exhaustion / crates.io rate-limit with 100+ packages
- **Single-pass stable + prerelease**: sparse index responses include all historical versions, so we always get both candidates in one fetch. `check_package_updates` picks `latest_version` based on update direction — stable preferred, prerelease shown when no stable update available
- **`cargo install --list` cache**: `OnceLock<Mutex<HashMap>>` populated once by `get_installed_packages`; `invalidate_installed_version(pkg)` removes a single entry after a successful update so the next read picks up the new version. Without this, N package updates would invoke `cargo install --list` N+1 times
- **semver-based comparison**: `PackageInfo::has_update` and `is_prerelease` use `semver::Version`. Yank rollbacks (current > latest) no longer flag as "needs update". Note: the semver crate's `Ord` compares build metadata for total ordering even though the SemVer spec says it shouldn't — cargo-fresh leans into this since a re-published artifact with new build metadata is usually worth reinstalling
- **binstall fallback**: only attempted for `PackageSource::Crates`. `is_binstall_available()` is the read-only probe (used in dry-run); `ensure_binstall_available()` will install cargo-binstall on first miss
- **Source-aware update strategy**: `updater::build_args` switches on `PackageSource` — crates uses `binstall`/`install`, git uses `install --git URL [--rev REV]`, path uses `install --path DIR`
- **`PbGuard` RAII for spinner cleanup**: `update_package` wraps the per-package `ProgressBar` in a `PbGuard` immediately after creation. Its `Drop` impl calls `finish_and_clear()`, guaranteeing no spinner frames are left on screen regardless of which return path is taken (success, failure, retry exhausted, dry-run)
- **No main progress bar**: The overall N/M progress is shown as a plain `   Package 3/18 cargo-fresh` status line (only when updating more than one package). This avoids two concurrent spinners conflicting in the terminal
- **i18n named placeholders**: templates use `{name}` / `{old}` / `{new}` / `{code}` / `{error}` / `{attempt}`. Each variable substituted via `format_text(key, args)` — never chain `.replace("{}", x)` on multi-placeholder templates (single-value templates may still use `{}` with `.replace("{}", val)`)
- **Bilingual UI**: all user-facing strings live in `src/locale/texts.rs` as enum-mapped pairs. Adding a language means adding a `match` arm and updating the consistency test list in `texts.rs::tests`

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

**Exception**: `--verbose` output in `check_package_updates` (package-level) still uses raw `println!` — this is a known inconsistency, not an intentional pattern to follow.

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
| `Failed` | red | Package update failed |
| `Finished` | green/red | Final summary line (red if any failures) |

### Adding a new locale string

1. Add the key + English text to `get_english_text` in `src/locale/texts.rs`
2. Add the key + Chinese text to `get_chinese_text` in the same file
3. Add the key to the consistency test array in `tests::test_text_consistency`
4. Use `language.get_text("key")` for single-value or positional strings; use `language.format_text("key", &[("name", val)])` for named-placeholder templates

## Release process

Releases are fully automated via GitHub Actions:
- **`crate.yml`**: triggered on `v*` tag push; publishes to crates.io using OIDC, creates a GitHub release
- **`release.yml`**: triggered by crate.yml completion; builds for macOS ARM64/x86_64 and Linux x86_64, uploads binaries

To release: bump version in `Cargo.toml`, add a CHANGELOG entry, commit, then `git tag -a vX.Y.Z -m "..."` and push both the commit and the tag.

## Roadmap to 1.0

See `plan.md` (gitignored, locally generated) for the full path. Status as of v0.9.14:

- ✅ **0.9.10** — Cargo.lock committed, semver comparison, prerelease detection, 29 unit tests added
- ✅ **0.9.11** — globset filter, git/path source support, `--dry-run`, `--exclude`
- ✅ **0.9.12** — sparse index, Semaphore(16) concurrency limit, install-list cache
- ✅ **0.9.13** — cargo-style CLI output, no emojis
- ✅ **0.9.14** — PbGuard RAII spinner cleanup, removed conflicting main progress bar
- ⏭ **1.0.0-rc.1** — issue/PR templates, CONTRIBUTING.md, SECURITY.md, README "Stability Guarantees" section, comparison table
- ⏭ **1.0.0** — gather rc.1 feedback, finalize

Open questions before 1.0: README bilingual sync, MSRV declaration, whether `--json` ships in 1.0 or 1.1, whether to keep `cargo search` fallback long-term, migrate verbose `println!` in `check_package_updates` to the `status` family.
