# ROADMAP

**Current**: 0.10.2 shipped. P0 (1.0 blockers) + P1 (1.0.0-rc.1 prereqs) + P1-ext (1.0 contract polish) all done. We're in the **1.0 feedback window** (until **2026-06-30**) — see pinned issue [#3](https://github.com/jenkinpan/cargo-fresh/issues/3).

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

## ✅ P1-ext — 1.0 contract polish (shipped in 0.10.2)

| # | Item | Note |
|---|------|------|
| P1e-1 | Explicit stdout / stderr split | `status*` 全部走 `anstream::eprintln!` → stderr；`--format=json` 报告独占 stdout 一行；两条回归测试 (`json_mode_keeps_stdout_clean` / `non_json_mode_keeps_status_off_stdout`) 钉合约 |
| P1e-2 | `docs/json-schema.json` | JSON Schema Draft 2020-12 描述 `schema_version=1` 字段形状；README 加 jq 用例 |
| P1e-3 | `cargo fresh man` | `clap_mangen` 把 `Cli::command()` 渲染成 roff 到 stdout，镜像 `completion` 子命令；`man_subcommand_emits_roff` 测试覆盖 |
| P1e-4 | Color detection via `anstream` | `NO_COLOR` / `CLICOLOR_FORCE` / `TERM=dumb` / TTY 检测集中到一处；`colored` 仍提供 `.green().bold()` 人体工学 API，是否真的输出 ANSI 由 anstream 决定。两条回归测试覆盖 |
| P1e-5 | `audit.yml` CI workflow | `cargo-deny check advisories licenses sources bans` + `cargo-audit`；push/PR + 每周一 06:00 UTC cron；`deny.toml` allowlist 基于 `cargo license` 实盘点 |
| P1e-6 | `release.yml` 版本解析硬失败 | 之前 `workflow_run.head_branch` 解析漏洞会 fall back 到默认 `0.1.0`，污染 ghost release。新逻辑：解析失败 `::error::` + exit 1，绝不带默认值继续 |

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

1. `cargo-dist` to replace handwritten release matrix (`crate.yml` + `release.yml`)
2. `etcetera` / `xdg` for config dir resolution (→ P2-4 prerequisite)

Closed across 0.10.x: `CommandFactory` derive, `is_terminal` non-TTY downgrade, `tokio` feature pruning, `assert_cmd` + integration tests, MSRV in CI, stdout/stderr routing via `anstream::eprintln!`, `docs/json-schema.json`, `cargo fresh man` (clap_mangen), `audit.yml` (cargo-deny + cargo-audit), `anstream` color pipeline (centralizes `NO_COLOR`/`CLICOLOR_FORCE`/TTY detection), strict version parsing in `release.yml`.

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
