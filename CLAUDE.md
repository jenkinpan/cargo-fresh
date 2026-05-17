# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build --release

# Run in development
cargo run -- [flags]

# Lint (zero warnings policy)
cargo clippy

# Install locally for testing
cargo install --path .
```

There are no automated tests in this project.

## Architecture

**cargo-fresh** is a Rust CLI tool that checks and updates globally installed Cargo packages. It runs as a cargo subcommand (`cargo fresh`).

### Core workflow

1. **Discover** — parse `cargo install --list` output to get installed packages
2. **Check** — concurrently query `cargo search <pkg>` for latest versions (Tokio tasks)
3. **Display** — show update candidates via interactive prompts (dialoguer MultiSelect/Confirm)
4. **Update** — use `cargo binstall` (fast, pre-compiled) with automatic fallback to `cargo install`
5. **Verify** — confirm installed version post-update

### Module responsibilities

| Module | Role |
|--------|------|
| `src/main.rs` | Top-level orchestration of the workflow |
| `src/cli/mod.rs` | Clap argument parsing; shell completion generation |
| `src/models/mod.rs` | `PackageInfo`, `UpdateResult` structs; shared constants |
| `src/package/mod.rs` | Parses installed packages; queries `cargo search`; version comparison and prerelease detection |
| `src/updater/mod.rs` | Executes binstall/install; retry logic (3 attempts, 2s backoff); `OnceLock`-cached binstall availability check |
| `src/display/mod.rs` | Colored terminal output; progress bars (indicatif); interactive selection prompts |
| `src/locale/` | Auto-detects system language via `LANG`/`LC_ALL`/`LC_CTYPE` env vars; English/Chinese text maps |

### Key design decisions

- **binstall fallback**: `updater` checks for `cargo binstall` once (via `OnceLock`) then uses it when available; falls back to `cargo install` on failure
- **Prerelease handling**: versions containing "alpha", "beta", or "rc" are separated from stable updates and presented with distinct prompts
- **Glob filter**: `--filter` supports simple `*` wildcards for package name matching
- **Bilingual UI**: all user-facing strings live in `src/locale/texts.rs` as enum-mapped pairs

## Release process

Releases are fully automated via GitHub Actions:
- **`crate.yml`**: triggered on `v*` tag push; publishes to crates.io using OIDC
- **`release.yml`**: triggered by crate.yml; builds for macOS ARM64/x86_64 and Linux x86_64, uploads binaries, updates Homebrew tap formula

To release: bump the version in `Cargo.toml` and push a `v*` tag.
