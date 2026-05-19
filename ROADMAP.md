# ROADMAP

**Current**: 0.10.1 shipped. Both P0 (1.0 blockers) and P1 (1.0.0-rc.1 prereqs) are done. We're in the **1.0 feedback window** (until **2026-06-30**) — see pinned issue [#3](https://github.com/jenkinpan/cargo-fresh/issues/3).

This file is the durable plan. CLAUDE.md tracks the high-level status; CHANGELOG.md tracks what shipped and when.

## ✅ P0 — 1.0 blockers (shipped in 0.10.0)

| # | Item | Note |
|---|------|------|
| P0-1 | Unify completion generation with `CommandFactory` | `Cli::command()` is the single source of truth |
| P0-2 | Handle SIGINT, abort update loop cleanly | `Arc<AtomicBool>` + `tokio::signal::ctrl_c`; exit 130 |
| P0-3 | Respect cargo sparse mirror | `$CARGO_HOME/config.toml` `[source.crates-io] replace-with` → `sparse+URL` |
| P0-4 | Stop surfacing prereleases when `--include-prerelease` is off (BREAKING) | `choose_latest` pure function |
| P0-5 | `--format=json` + documented exit codes | `JsonReport schema_version=1` |
| P0-6 | Declare MSRV and enforce in CI | Currently 1.86 (bumped in 0.10.1 for `clap_derive`/`icu`) |

## ✅ P1 — 1.0.0-rc.1 prereqs (shipped in 0.10.1)

| # | Item | Note |
|---|------|------|
| P1-1 | Push blocking IO out of async fns | `tokio::process::Command` everywhere; tokio features trimmed |
| P1-2 | `cargo search` fallback opt-out | `--no-cargo-search-fallback` / `CARGO_FRESH_NO_FALLBACK=1` |
| P1-3 | `--verbose` → `status_*` | `Check` / `Latest` verbs; non-TTY auto-downgrades spinner |
| P1-4 | binstall auto-install opt-in (BEHAVIOR) | `--install-binstall` to restore old behavior |
| P1-5 | Long-tail package visualization | spinner shows elapsed; `Slow` after 30s via `SlowGuard` |
| P1-6 | sparse index retry with backoff | 1 retry, 500ms; 4xx not retried |
| P1-7 | `PackageSource::Unknown(String)` | Explicit skip with `Skip [unknown source]` |
| P1-8 | `INSTALLED_VERSION_CACHE` get_or_init + lock/clear/extend | Future `--watch` will re-read correctly |
| P1-9 | Integration tests | `tests/cli.rs` (`assert_cmd`), `tests/sparse_index_http.rs` (`wiremock`); `src/lib.rs` exposes module tree |
| P1-10 | thiserror + actionable hints | `CargoFreshError::CargoListFailed` + `reqwest::Error` chain match in `errors::hint_for` |
| P1-11 | 1.0 doc suite | ISSUE/PR/CONTRIBUTING/SECURITY, README Stability + comparison |

## 🔄 Now — 1.0 feedback window

Window closes **2026-06-30**.

- Meta issue [#3](https://github.com/jenkinpan/cargo-fresh/issues/3) is pinned with concrete asks (CLI shape, JSON schema, exit codes, hint coverage, source-aware install, mirror/private registry)
- README top banner (English + Chinese) points to it
- If 0.10.x picks up BREAKING-class feedback → cut `1.0.0-rc.1` from master after addressing it
- If feedback is purely additive → skip rc, ship `1.0.0` directly after window closes

What 1.0 cements:
- Exit codes `0` / `1` / `2` / `130`
- `--format=json` `schema_version=1` field shape (additive-only after)
- CLI flag inventory (deprecations get one minor cycle of warning)

What 1.0 does **not** cement:
- Status verb wording / colors / locale strings
- Internal `cargo_fresh::*` API (lib exists for tests, not as a downstream API)

## ⏭ P2 — 1.x evolution

1. **P2-1** `--locked` / `--frozen` pass-through to `cargo install`
2. **P2-2** `--include <pattern>` (repeatable) symmetric with `--exclude`
3. **P2-3** `cargo fresh outdated` subcommand (check-only, no prompts)
4. **P2-4** Config file `~/.config/cargo-fresh/config.toml` (default flags, registry overrides, exclude list)
5. **P2-5** Windows + Linux aarch64 release matrix
6. **P2-6** `cargo fresh self update` (don't self-replace; delegate to cargo)
7. **P2-7** `tracing` + `--log-level` / `RUST_LOG` — replace `--verbose` `status_dim` with structured logging
8. **P2-8** dialoguer multi-select: visually separate stable / prerelease groups
9. **P2-9** `--watch` mode — re-scan on interval; tests `INSTALLED_VERSION_CACHE` refresh

Most of these can land as `1.1.0`, `1.2.0`. Anything in this list that becomes a BREAKING change before 1.0 ships gets promoted into rc.1 scope.

## Remaining "Modern Rust CLI" gaps

The list at this point is mostly nice-to-have; the ones below are still meaningfully open:

1. `concolor` / `anstream` instead of `colored` (color detection is still implicit; stdout/stderr split is already explicit as of post-0.10.1)
2. `cargo-deny` + `cargo-audit` in CI
3. `cargo-dist` to replace handwritten release matrix (`crate.yml` + `release.yml`)
4. `etcetera` / `xdg` for config dir resolution (→ P2-4 prerequisite)

Items already closed post-0.10.1: stdout/stderr routing (all `status*` go to stderr via `eprintln!`; stdout reserved for JSON), `docs/json-schema.json` (Draft 2020-12 schema for the v1 JSON contract), `cargo fresh man` subcommand (clap_mangen-rendered roff to stdout, mirrors `completion` subcommand).

Items already closed in 0.10.x: `CommandFactory` derive, `is_terminal` non-TTY downgrade, `tokio` feature pruning, `assert_cmd` + integration tests, MSRV in CI.

## Open question for 1.x

Whether to keep the `cargo search` fallback long-term. Today it's the safety net for environments where sparse index is blocked. If 1.0 feedback shows nobody depends on it, drop it in `1.1.0` to slim the code path.

## Differentiation vs `cargo-update`

The README now has a dedicated comparison table in the "How cargo-fresh differs from cargo-update / cargo-install-update" section — that's the public-facing version. Internally the differentiators are:

1. sparse index default (50–100ms per package, 16-way concurrent)
2. Source-aware install (crates / git+rev / path)
3. globset filtering with auto `*pattern*` wrapping
4. `--format=json` with versioned schema
5. Bilingual UI + locale auto-detection
6. CI-friendly exit code contract + Ctrl-C handling
