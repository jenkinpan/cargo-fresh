# Roadmap to 1.0

This is the detailed, item-by-item plan referenced from `CLAUDE.md`. `CLAUDE.md`
keeps the one-line-per-release summary; this file carries the rationale, the
remaining 1.0 checklist, and the deliberately-deferred items.

**Status as of v0.12.6.** The code is feature-complete for 1.0; what remains is a
feedback-bake window and freezing the public contract.

## What 1.0 freezes

1.0 promotes three things from "current behavior" to "permanent, semver-protected
contract". After 1.0, changing any of these is a breaking change requiring 2.0:

| Contract               | Frozen value                                                            | Source of truth                                          |
| ---------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------- |
| JSON output schema     | `schema_version = 2`                                                    | `docs/json-schema.json`, `src/models/mod.rs::JsonReport` |
| Exit codes             | `0` ok / `1` updates-available-not-applied / `2` failure / `130` SIGINT | `src/main.rs` (`EXIT_*`), `run() -> Result<i32>`         |
| CLI flag inventory     | the `--help` flag set + `completion` / `man` subcommands                | `src/cli/mod.rs`, locked by `tests/cli.rs`               |
| Status verb dictionary | the human-output verbs                                                  | `CLAUDE.md` → "Status verb dictionary"                   |

The public checklist for users and issue triage lives in
[`docs/1.0-contract.md`](docs/1.0-contract.md).

Additive changes (new JSON fields, new flags with defaults that preserve behavior)
remain allowed within 1.x.

## Shipped

- ✅ **0.9.10–0.9.14** — `Cargo.lock` committed, semver-based comparison, sparse
  index client, cargo-style status output, `PbGuard` progress cleanup.
- ✅ **0.10.0** — `--include-prerelease` strict (BREAKING), `--registry-url`,
  mirror auto-detect from `$CARGO_HOME/config.toml`, Ctrl-C cancel, `--format=json`
  - the exit-code contract.
- ✅ **0.10.1** — async cargo subprocess, `--no-cargo-search-fallback`,
  `--install-binstall` (BEHAVIOR; later removed), non-TTY downgrade, `SlowGuard`
  30s watchdog, `PackageSource::Unknown`, `errors::hint_for`, `tests/` integration
  suite, MSRV 1.86, ISSUE/PR/CONTRIBUTING/SECURITY docs.
- ✅ **0.10.2** — 1.0-contract polish: explicit stdout/stderr split via
  `anstream::eprintln!`, `docs/json-schema.json`, `cargo fresh man`
  (clap_mangen), `anstream` color pipeline, `audit.yml` CI, strict `release.yml`
  version parsing.
- ✅ **0.10.3** — `.crates2.json` install-option preservation (features
  passthrough), skip prebuilt path for non-default-features packages (BEHAVIOR),
  `CommandSelector` fallback fix, JSON additions under `schema_version=1`.
- ✅ **0.10.4** — pre-flight prebuilt probe (then `--check-binstall`), `--no-confirm`
  hardening, Ctrl-C no longer misreported as failure.
- ✅ **0.10.5** — `cargo fresh man` auto-renders through system `man` on a TTY;
  fish completion install hint.
- ✅ **0.10.6** — GitHub Release body from the matching `CHANGELOG.md` section;
  `changelog-sync` CI job.
- ✅ **0.10.7** — `tests/json_schema.rs`, `tests/cli_snapshots.rs` (insta),
  `display::format_status_line` single render path.
- ✅ **0.11.0** — **self-hosted binary downloader (BEHAVIOR)**: in-process
  `src/downloader/` replaces the `cargo binstall` subprocess. HEAD-probes
  candidate URLs across multi-arch aliases, streams the winning GitHub Release
  archive with cancel-checks, verifies optional `.sha256`, extracts, atomically
  installs into `~/.cargo/bin`, updates `.crates2.json` **and** `.crates.toml`.
  `--install-binstall` deprecated.
- ✅ **0.11.0 follow-up** — `.crates.toml` writer + `bins[]` + multi-name +
  monorepo tag paths (fixes ripgrep / tauri-cli / "Unchanged"); concurrent HEAD
  probing (5s timeout); rustup-style `MultiProgress` stacked rows + grouped
  `Prebuilt:` / `Compiled:` summary. `download_view.rs` + `crossterm` deleted.
- ✅ **0.12.0** — **concurrent scheduler (BEHAVIOR)**: `--jobs N` / `-j N`
  (default 4, `0` unlimited, `1` serial) via `JoinSet` + owned-permit `Semaphore`,
  results re-sorted to input order. **GitHub Releases API client**
  (`downloader/github_api.rs` + `token.rs`): API-first resolution (1–6 requests
  vs 360 HEADs); token discovery `$GITHUB_TOKEN` > `$GH_TOKEN` > `gh auth token`.
  `--check-binstall` → **`--check-prebuilt`**, rewritten on the real downloader
  resolve logic (`downloader/probe.rs`). **`schema_version` 1 → 2** (`binstall`
  → `prebuilt`, `source_build` → `source`).
- ✅ **0.12.1** — cross-major dep bumps (anstream 1.0, clap_mangen 0.3, zip 8,
  reqwest 0.13, toml 1.1, sha2 0.11). reqwest 0.13 moves TLS roots to the
  platform verifier and crypto to aws-lc-rs.
- ✅ **0.12.2** — MSRV 1.86 → 1.88 (forced by `zip 8.x`).
- ✅ **0.12.3** — interactive multi-shell `completion --install` (MultiSelect
  picker, six shells, XDG-aware, `--yes`); README rewrite; removed stale
  `completion/` dir + `COMPLETION.md`.
- ✅ **0.12.4** — fish completion install fix: `completion fish --install` no
  longer shadows fish's built-in `cargo.fish`; added `--debug` downloader
  decision tracing for issue reports. `--debug` is explicitly outside the 1.0
  stable contract.
- ✅ **0.12.5** — code cleanup: removed `CommandSelector` dead code, unreachable
  `cargo_search_fallback` loop, redundant `.iter().find()` in `main.rs`.
- ✅ **0.12.6** — perf: concurrent GitHub API tag probing in downloader
  (`try_api_winning_url`, `FuturesUnordered` + `Semaphore(2)`, cuts 3–5 RTT
  for monorepo packages) and `--check-prebuilt` probe (`probe_prebuilt`, same
  pattern); `registry_override` wrapped in `Arc` in `check_package_updates`
  (one heap alloc instead of N). Plus the pre-freeze **contract audit** below
  (additive / docs-only, `schema_version` stays `2`):
  the frozen CLI flag set (`schema_version` stays `2`, all changes additive /
  docs-only, zero BREAKING):
  - Removed the lone `additionalProperties: false` on `checkError` in
    `docs/json-schema.json` — it conflicted with the 1.x additive-only promise
    (a closed object rejects valid future field additions).
  - `tests/cli.rs::help_lists_core_flags` now asserts the **full** 14-flag
    inventory + short names `-v` / `-u` / `-j`, so accidental removal/rename of
    any frozen flag fails CI (it previously covered only 10).
  - Added JSON `version` (top level — the producing cargo-fresh release, for
    self-describing reports) and `results[].install_method` (`prebuilt` /
    `source` / `null`, sharing vocabulary with `updates_available[].prebuilt`
    so scripts can compare the `--check-prebuilt` prediction vs the real
    outcome).
  - Decided **not** to rename `--no-cargo-search-fallback` (the precise name is
    correct; `--no-fallback` would falsely imply it also disables the
    downloader → `cargo install` install fallback). Clarified the flag's scope
    in help text + both READMEs instead.

## In progress

- 🔄 **Feedback window** — pinned meta issue
  [#3 "Towards 1.0 — Feedback Wanted"](https://github.com/jenkinpan/cargo-fresh/issues/3).
  Window closes **2026-06-30**. Collecting BREAKING-class feedback that should
  bake before the contract freezes.

## Planned

- ⏭ **1.0.0-rc.1** — cut from master after the feedback window closes. Only ships
  as a distinct RC if 0.12.x picks up BREAKING-class feedback that needs to bake;
  otherwise master goes straight to 1.0.0.
- ⏭ **1.0.0** — promote `schema_version=2`, the exit codes, and the CLI flag
  inventory to permanent contract (see "What 1.0 freezes" above).

## Open questions / deferred

- **Keep the `cargo search` fallback?** Today it's the safety net for environments
  where the sparse index is blocked. If 1.0 feedback shows nobody relies on it,
  drop it in 1.x (additive removal of a fallback, behavior-preserving for the
  common path).
- **Non-github release hosts.** The downloader's API-first path only understands
  `github.com`; GitLab / Gitea / self-hosted forges fall through to `cargo install`.
  Not a 1.0 blocker — `cargo install` is a correct (if slower) fallback.
