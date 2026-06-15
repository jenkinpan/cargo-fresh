# cargo-fresh

[![Crates.io](https://img.shields.io/crates/v/cargo-fresh.svg)](https://crates.io/crates/cargo-fresh)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Wiki](https://img.shields.io/badge/wiki-Recipes_·_FAQ_·_Troubleshooting-blue)](https://github.com/jenkinpan/cargo-fresh/wiki)

<div align="center">

**Language / 语言**

[![English](https://img.shields.io/badge/English-Current-blue?style=for-the-badge)](README.md) [![中文](https://img.shields.io/badge/中文-中文版-green?style=for-the-badge)](README.zh.md)

</div>

---

> **1.0 is approaching.** Feedback on the 1.0 contract (CLI shape, `--format=json` schema, exit codes, error hints) is open until **2026-06-30**, then `1.0.0-rc.1`. Comment at [#3 Towards 1.0 — Feedback Wanted](https://github.com/jenkinpan/cargo-fresh/issues/3).

---

`cargo-fresh` checks and updates your globally installed Cargo packages. It queries the crates.io sparse index in parallel, prefers prebuilt GitHub Release binaries over compiling from source, runs updates concurrently with `-j N`, and ships a stable `--format=json` contract for scripting. Installed as `cargo install cargo-fresh`; invoked as `cargo fresh`.

## Contents

- [Highlights](#highlights)
- [Installation](#installation)
- [Quick start](#quick-start)
- [CLI reference](#cli-reference)
- [Exit codes](#exit-codes)
- [JSON output](#json-output)
- [Shell completion](#shell-completion)
- [Output examples](#output-examples)
- [Language detection](#language-detection)
- [Stability guarantees](#stability-guarantees)
- [1.0 contract](#10-contract)
- [How cargo-fresh differs from cargo-update](#how-cargo-fresh-differs-from-cargo-update)
- [Contributing](#contributing)
- [License](#license)

## Highlights

- **Fast version checks** — crates.io sparse index over HTTP (~50–100 ms/pkg) with a shared connection pool and a 16-way concurrency cap. Falls back to `cargo search` only when the index is unreachable.
- **Source-aware updates** — crates.io, `git+URL [--rev]`, and `path+DIR` installs each get the correct `cargo install` strategy; `[git]` / `[path]` markers in the output.
- **In-process binary downloader** — fetches GitHub Release tarballs directly via the Releases API (with HEAD-probe fallback), verifies an `.sha256` sidecar when present, and atomically installs into `~/.cargo/bin`. No `cargo binstall` subprocess required.
- **Concurrent updates** — `-j N` / `--jobs N` (default 4) drives parallel package updates with rustup-style stacked progress rows. `-j 1` restores fully serial behavior.
- **Filtering** — `--filter PATTERN` keeps matches, `--exclude PATTERN` (repeatable) drops them; both support glob syntax (`*`, `?`, `[abc]`).
- **`--dry-run`** prints the exact `cargo install …` commands without touching anything.
- **`--format=json`** emits a single machine-readable object on stdout (Draft 2020-12 schema, `schema_version=2`) and disables all spinners/prompts. Stable contract; only additive changes within a major.
- **Install-option preservation** — features (`--features` / `--no-default-features` / `--all-features`) are read from `.crates2.json` and re-applied on update.
- **Bilingual UI** — English / Chinese auto-detected from `LANG` / `LC_ALL` / `LC_CTYPE`.

## Installation

### From crates.io (recommended)

```bash
cargo install cargo-fresh
# or, if you already have cargo-binstall:
cargo binstall cargo-fresh
```

`cargo-fresh` fetches GitHub Release binaries directly through the GitHub Releases API (with HEAD-probe fallback) — it does **not** invoke, depend on, or auto-install `cargo binstall`. The unauthenticated GitHub API quota is 60 requests/hour; set `GITHUB_TOKEN` (or `GH_TOKEN`, or have `gh auth login` configured) to raise it to 5 000/hour. This mainly matters for `--check-prebuilt` against many packages.

### From source

```bash
git clone https://github.com/jenkinpan/cargo-fresh.git
cd cargo-fresh
cargo install --path .
```

### From GitHub directly

```bash
cargo install --git https://github.com/jenkinpan/cargo-fresh.git
```

## Quick start

```bash
# Interactive: list updates, pick which to apply
cargo fresh

# Apply every available update without prompting
cargo fresh --batch

# Preview the cargo commands that would run, change nothing
cargo fresh --dry-run --batch

# Update only matching packages
cargo fresh --batch --filter "cargo-*"

# CI gate: exit 1 if any update is available
cargo fresh --format=json
```

## CLI reference

| Flag | Description |
|------|-------------|
| `-v, --verbose` | Per-package check details |
| `-u, --updates-only` | Only list packages with updates available |
| `--no-interactive` | Skip prompts; list updates but apply nothing (use `--batch` to apply) |
| `--batch` | Apply every selected update without prompting |
| `--include-prerelease` | Treat `α / β / rc` versions as candidates |
| `--filter <PATTERN>` | Keep only packages matching the glob (`*`, `?`, `[abc]`) |
| `--exclude <PATTERN>` | Drop matching packages; repeatable; applied after `--filter` |
| `--dry-run` | Print the exact `cargo install …` commands without running them |
| `--registry-url <URL>` | Override sparse-index base URL (mirror support) |
| `--no-cargo-search-fallback` | Don't fall back to `cargo search` when the sparse index fails (also `CARGO_FRESH_NO_FALLBACK=1`) |
| `--check-prebuilt` | Probe each candidate to mark `[prebuilt]` / `[source]` / `[unknown]`. Off by default — each probe issues a few HEAD requests |
| `--debug` | Emit downloader decision traces to stderr for issue reports. Not part of the 1.0 stability contract; don't parse it |
| `-j, --jobs <N>` | Concurrent package updates. Default `4`; `0` = unlimited; `1` = serial. `cargo install` fallback still serializes on cargo's `$CARGO_HOME` lock |
| `--format <FORMAT>` | `human` (default) or `json` |
| `-h, --help` / `-V, --version` | Help / version |

Subcommands: `cargo fresh completion <shell> [--install] [--yes]` (see [Shell completion](#shell-completion)) and `cargo fresh man` (renders via the system `man` when stdout is a TTY, raw roff otherwise).

## Exit codes

Stable contract since 0.10.0:

| Code | Meaning |
|------|---------|
| 0    | No updates available, or all selected updates succeeded |
| 1    | Updates available but not applied (`--format=json` without `--batch`, or `--no-interactive` with no selections) |
| 2    | At least one update failed |
| 130  | Ctrl-C; remaining packages skipped |

```bash
# Fail the CI job if any global package has an update
cargo fresh --format=json
# → exit 1 when updates exist, 0 otherwise

# Apply everything in CI, fail on any update failure
cargo fresh --format=json --batch
# → exit 2 if any update fails, 0 otherwise
```

## JSON output

`--format=json` emits one JSON object on **stdout** and routes all status / errors / prompts to **stderr**. That means `cargo fresh --format=json | jq '.'` works without filtering, and `cargo fresh > /dev/null` still shows progress in the terminal.

The full schema lives at [`docs/json-schema.json`](docs/json-schema.json) (JSON Schema Draft 2020-12). `schema_version=2` is the final pre-1.0 schema break (it renamed `updates_available[].binstall` → `prebuilt` and the enum `source_build` → `source`); within `schema_version=2` fields are only added, never renamed or removed.

Fields available beyond the bare `1` shape (additive history under `2`):

- **`skipped[].reason_code`** — stable enum (`path_source` / `git_source` / `unknown_source`). Branch on this in scripts rather than the prose `reason`.
- **`version_check_errors[]`** — packages whose latest-version lookup failed; each has `name`, `kind` (`not_found` / `unavailable`), and a human-readable `error`. `updates_available[]` excludes these.
- **`summary.selected` / `attempted` / `check_errors`** — counts for chosen / install-attempted / lookup-failed packages.
- **`version`** (top level) — the cargo-fresh release that produced the report (e.g. `"0.12.5"`), so archived JSON is self-describing. Branch on `schema_version` / `format`, not this.
- **`results[].install_method`** — which path actually ran: `prebuilt` (downloader fetched a prebuilt binary) / `source` (fell back to `cargo install`) / `null` (install didn't complete). Shares the `prebuilt` / `source` vocabulary with `updates_available[].prebuilt`, so you can compare the `--check-prebuilt` prediction against the real outcome.

```bash
# Names of packages with updates available
cargo fresh --format=json | jq -r '.updates_available[].name'

# Count of failed updates after a batch run
cargo fresh --format=json --batch | jq '.summary.failed'

# Git-sourced update candidates only
cargo fresh --format=json | jq '.updates_available[] | select(.source == "git")'

# Detect a Ctrl-C abort
cargo fresh --format=json --batch | jq '.aborted'

# Lookup errors (transient network issues etc.)
cargo fresh --format=json | jq '.version_check_errors[]'

# Branch on stable reason codes
cargo fresh --format=json | jq '.skipped[] | select(.reason_code == "git_source")'
```

## Shell completion

Supported shells: **bash**, **zsh**, **fish**, **powershell**, **elvish**, **nushell**.

### Recommended: interactive install

```bash
cargo fresh completion <shell> --install
```

`--install` opens a MultiSelect picker (space to toggle, enter to confirm):

```
Select which completions to install (space to toggle, enter to confirm)
> [x] cargo-fresh<TAB>  — top-level binary completion
  [x] cargo fresh<TAB>  — cargo subcommand completion
```

Both targets are checked by default. `cargo-fresh<TAB>` enables completion for the standalone binary; `cargo fresh<TAB>` enables it for the cargo subcommand form. Most users want both.

Add `--yes` to skip the prompt and install both targets (useful in scripts / CI):

```bash
cargo fresh completion fish --install --yes
```

Existing files are detected and overwrite is confirmed per-file. Where the destination dir isn't on the shell's auto-load path (zsh / powershell / elvish / nushell), cargo-fresh prints a one-line `Hint` with the exact `fpath=` / `. $PROFILE` / `use` / `source` line to add.

### Install locations

| Shell | Top-level (`cargo-fresh<TAB>`) | Cargo subcommand (`cargo fresh<TAB>`) |
|-------|--------------------------------|----------------------------------------|
| bash       | `~/.local/share/bash-completion/completions/cargo-fresh` | `~/.local/share/bash-completion/completions/cargo` |
| zsh        | `~/.zfunc/_cargo-fresh` (add `~/.zfunc` to `$fpath`) | `~/.zfunc/_cargo` |
| fish       | `~/.config/fish/completions/cargo-fresh.fish` | `~/.config/fish/completions/cargo.fish` |
| nushell    | `~/.config/nushell/completions/cargo-fresh.nu` | `~/.config/nushell/completions/cargo.nu` |
| elvish     | `~/.config/elvish/lib/cargo-fresh.elv` | `~/.config/elvish/lib/cargo.elv` |
| powershell | `~/.config/powershell/cargo-fresh.ps1` | `~/.config/powershell/cargo.ps1` |

Paths respect `XDG_CONFIG_HOME` / `XDG_DATA_HOME` when set.

### Manual install (redirect stdout)

If you'd rather pick the path yourself, omit `--install` and redirect:

```bash
# Top-level binary completion
cargo fresh completion zsh > ~/.zfunc/_cargo-fresh

# Cargo subcommand form
cargo fresh completion zsh --cargo-fresh > ~/.zfunc/_cargo
```

The `--cargo-fresh` flag switches between the two scripts. It's ignored when `--install` is set — the picker covers both.

## Output examples

cargo-fresh uses a cargo-style status format: a 12-char right-aligned bold verb followed by a message. Colors carry meaning — green (success), yellow (warning), red (failure), dim (secondary). No emojis.

### Interactive mode (default)

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
       Fresh ripgrep 14.1.1
    Updating cargo-outdated 0.16.0 -> 0.17.0
    Updating devtool 0.2.4 -> 0.2.5

Updates available:
Stable updates:
    Updating cargo-outdated 0.16.0 -> 0.17.0
    Updating devtool 0.2.4 -> 0.2.5

Update these packages? y
Select packages (space to toggle, enter to confirm)
> [x] cargo-outdated
> [x] devtool

  cargo-outdated  resolving
  cargo-outdated  [######################>      ] 1.2 MiB/2.1 MiB
  cargo-outdated  installed 2.10 MiB
         devtool  resolving
         devtool  installed 1.42 MiB

Update Summary
    Prebuilt cargo-outdated, devtool
    Finished 2 succeeded, in 4.2s
```

Each row is a live `MultiProgress` line: `pending` → `resolving` → `downloading X.X MiB` (or a byte-count bar when `Content-Length` is known) → `installed X.XX MiB`, then locks in as a static line so the screen accumulates the full history. With `-j N` (default 4) the rows update concurrently; the final summary lists each package in the selection order regardless of completion order.

### Dry-run mode

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
    Updating cargo-outdated 0.16.0 -> 0.17.0

    Dry run no packages will be modified
   Would run cargo-outdated: cargo install --force cargo-outdated --version 0.17.0
```

### Non-interactive mode

`--no-interactive` lists available updates but applies nothing (use `--batch` for that):

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
       Fresh ripgrep 14.1.1
    Updating mdbook 0.4.52 -> 0.5.0-alpha.1
       Note no packages selected
```

Git and path installs show a dimmed `[git]` / `[path]` marker: `Updating my-tool 0.1.0 -> 0.2.0 [git]`.

## Language detection

cargo-fresh auto-detects your system language from `LANG` / `LC_ALL` / `LC_CTYPE`:

- A `zh*` locale → Chinese UI
- Anything else → English UI

Override per-invocation:

```bash
LANG=en_US.UTF-8 cargo fresh   # force English
LANG=zh_CN.UTF-8 cargo fresh   # force Chinese
```

## Stability guarantees

Pre-1.0 still ships breaking changes; once 1.0.0 lands the surface below is **promised** to follow semver:

| Surface | Stability |
|---------|-----------|
| Exit codes (`0` / `1` / `2` / `130`) | Stable — never reused or removed within a major |
| `--format=json` output, `schema_version=2` | Additive only — fields may be added, never renamed or retyped |
| CLI flags listed in `--help` | Stable — deprecations get one minor cycle of warning before removal |
| Source-aware install behavior (crates / git / path) | Stable |
| Human-readable status verbs (`Checking`, `Updating`, …) | **Not** stable — wording, color, alignment may change |
| Locale text (English / Chinese) | **Not** stable — phrasing tweaks expected; don't grep `stdout` |
| Internal modules / library API (`cargo_fresh::*`) | **Not** stable — `src/lib.rs` exists for integration tests, not as a downstream API |

When scripting against cargo-fresh, anchor on exit codes and `--format=json`; never on colored status text.

## 1.0 contract

The pre-1.0 contract checklist lives in [`docs/1.0-contract.md`](docs/1.0-contract.md). It names the surfaces that will become semver-protected at 1.0, the surfaces that intentionally stay flexible, and the exact `schema_version=2` JSON rules.

Feedback is still open until **2026-06-30** on [#3 Towards 1.0 — Feedback Wanted](https://github.com/jenkinpan/cargo-fresh/issues/3). For downloader/prebuilt issues, include:

```bash
cargo fresh --debug --check-prebuilt 2>&1 | grep debug
```

`--debug` is for diagnostics only; its output format is not stable and should not be consumed by scripts.

## How cargo-fresh differs from cargo-update

[`cargo-update`](https://github.com/nabijaczleweli/cargo-update) is the long-standing tool in this space. cargo-fresh is a fresh take, not a fork — these are the differences that drove building it:

| | cargo-fresh | cargo-update |
|---|---|---|
| **Version source** | crates.io sparse index (HTTP, ~50–100 ms/pkg, 16-way concurrent) | `cargo search` subprocess per package |
| **Source-aware updates** | Crates / `git+URL` / `path+DIR` each get the right install command | Registry + git; no `path` source |
| **Package selection** | `--filter "tokio*"` + `--exclude "*-test"` (globset) | Exact package names or `--all` (no glob/substring) |
| **Prerelease handling** | Explicit `--include-prerelease`; semver `.pre` check | Per-package opt-in via `cargo-install-update-config` |
| **JSON mode** | `--format=json` with versioned `schema_version=2` | None |
| **i18n** | English + Chinese auto-detected via `LANG` | English only |
| **Dry-run preview** | Prints the exact `cargo install` command per package | Lists what would update |
| **Binary install** | In-process: GitHub Releases API + sha256 verify + atomic install | Spawns `cargo binstall` subprocess when available |
| **Concurrency** | `-j N` for updates (default 4) + 16-way concurrent index/HEAD probes | Sequential updates |
| **Install options preserved** | Yes — features from `.crates2.json` | Yes — features/profile + per-package `cargo-install-update-config` |
| **CI ergonomics** | Exit codes 0/1/2/130 + JSON + non-TTY auto-downgrade | Standard exit codes |

cargo-update is more mature. Both tools now preserve the features a package was installed with; cargo-update additionally preserves build profile and supports per-package config. Use whichever fits — both are healthy projects to depend on.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide. TL;DR:

1. Fork → branch → commit → PR.
2. Before pushing: `cargo clippy --all-targets -- -D warnings` and `cargo test` must be green.
3. User-visible changes need a `CHANGELOG.md` `[Unreleased]` entry + README sync.

Security issues: see [SECURITY.md](SECURITY.md) — please don't file them as public issues.

## License

Apache 2.0 — see [LICENSE](LICENSE). Copyright (c) 2025 Jenkin Pan.

## Related

- [Crates.io](https://crates.io/crates/cargo-fresh)
- [GitHub repository](https://github.com/jenkinpan/cargo-fresh)
- [Issues](https://github.com/jenkinpan/cargo-fresh/issues)
- [Wiki](https://github.com/jenkinpan/cargo-fresh/wiki) — recipes, FAQ, troubleshooting
