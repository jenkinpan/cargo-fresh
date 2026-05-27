# cargo-fresh

[![Crates.io](https://img.shields.io/crates/v/cargo-fresh.svg)](https://crates.io/crates/cargo-fresh)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Wiki](https://img.shields.io/badge/wiki-Recipes_·_FAQ_·_Troubleshooting-blue)](https://github.com/jenkinpan/cargo-fresh/wiki)

<div align="center">

**Language / 语言**

[![English](https://img.shields.io/badge/English-Current-blue?style=for-the-badge)](README.md) [![中文](https://img.shields.io/badge/中文-中文版-green?style=for-the-badge)](README.zh.md)

</div>

---

> 📣 **1.0 is approaching.** I'm collecting real-world feedback to lock in the 1.0 contract (CLI shape, `--format=json` schema, exit codes, error hints). Window closes **2026-06-30**, then `1.0.0-rc.1`. Comment at [#3 Towards 1.0 — Feedback Wanted](https://github.com/jenkinpan/cargo-fresh/issues/3).

---

A Rust tool for checking and updating globally installed Cargo packages with interactive mode and smart prerelease detection. After installation, you can use it via the `cargo fresh` command. The tool automatically detects your system language and displays the interface in Chinese or English accordingly.

## Features

- 🔍 Automatically detect globally installed Cargo packages
- 📦 Check for the latest version of each package
- 🎨 Colored output with clear update status display
- ⚡ **Concurrent processing** for fast checking of multiple packages (3-5x faster)
- 🛠️ Command-line argument support for flexible usage
- 🔄 Default interactive update mode with one-click package updates
- 🧠 Smart prerelease version detection and prompting
- 🌍 Automatic language detection (Chinese/English)
- 🚀 Cargo subcommand support (`cargo fresh`)
- 🌐 Bilingual interface with smart language switching
- 🚀 **Batch operations** - automatically update all packages without confirmation
- 🔍 **Package filtering** - keep packages with `--filter` and drop them with `--exclude` (repeatable, glob patterns)
- 🧪 **Dry-run mode** - preview the exact cargo commands with `--dry-run` without changing anything
- 📦 **Source-aware updates** - handles crates.io, `git` (`--git URL [--rev]`), and local `path` installs, with `[git]` / `[path]` markers
- 🛡️ **Enhanced error handling** - intelligent retry mechanisms and user-friendly error messages
- 📊 **Fast version checks** - crates.io sparse index with connection pooling and a concurrency-limited request pool (`cargo search` fallback)
- ⚡ **Fast installation** - in-process binary downloader (since 0.11.0) streams GitHub Release tarballs directly, verifies sha256 when available, and atomically installs — no `cargo binstall` subprocess required. Falls back to `cargo install` for non-GitHub or unsupported packages

## Installation

### Install from crates.io (Recommended)

```bash
cargo install cargo-fresh
```
or
```bash
# Faster installation using pre-compiled binaries
cargo binstall cargo-fresh
```

**Note**: Since 0.11.0, cargo-fresh no longer depends on `cargo binstall` for the fast install path — it streams GitHub Release binaries in-process. The `--install-binstall` flag is deprecated (no-op + warning), and will be removed in 0.12.0. `--check-binstall` is still useful as a pre-flight probe to see which packages cargo-binstall would handle as prebuilt binaries.

### Install from source

```bash
# Clone the repository
git clone https://github.com/jenkinpan/cargo-fresh.git
cd cargo-fresh

# Build and install
cargo install --path .
```

### Install from GitHub

```bash
cargo install --git https://github.com/jenkinpan/cargo-fresh.git
```

## Language Support

The tool automatically detects your system language and displays the interface accordingly:

- **Chinese Environment**: Automatically displays Chinese interface
- **English Environment**: Automatically displays English interface
- **Language Detection**: Based on system environment variables (LANG, LC_ALL, LC_CTYPE)

You can also manually override the language by setting environment variables:

```bash
# Force English interface
LANG=en_US.UTF-8 cargo fresh

# Force Chinese interface  
LANG=zh_CN.UTF-8 cargo fresh
```

## Usage

### Basic Usage

After installation, you can use it in two ways:

```bash
# Method 1: As a cargo subcommand (recommended)
cargo fresh

# Method 2: Direct invocation
cargo-fresh
```

### Command Line Options

- `-v, --verbose`: Show detailed information
- `-u, --updates-only`: Show only packages with updates
- `--no-interactive`: Non-interactive mode (default is interactive mode)
- `--include-prerelease`: Include prerelease versions (alpha, beta, rc, etc.)
- `--batch`: Batch mode - automatically update all packages without confirmation
- `--filter <PATTERN>`: Keep only packages matching the glob pattern (`*`, `?`, `[abc]`)
- `--exclude <PATTERN>`: Drop packages matching the glob pattern; repeatable, applied after `--filter`
- `--dry-run`: Print the cargo commands that would run without executing them
- `--registry-url <URL>`: Override sparse index base URL (mirror support)
- `--format <FORMAT>`: `human` (default) or `json` for CI consumption
- `--check-binstall`: Probe each update candidate with `cargo binstall --dry-run` during the check phase and mark whether binstall would fetch a prebuilt binary (`[binstall: prebuilt]`) or compile from source (`[binstall: source build]`). Off by default — each probe spawns cargo and hits the network (~10s/package, run concurrently); requires `cargo-binstall` to be installed
- `-h, --help`: Show help information
- `-V, --version`: Show version information

### Exit Codes

`cargo fresh` returns the following exit codes — stable contract since 0.10.0:

| Code | Meaning                                                              |
|------|----------------------------------------------------------------------|
| 0    | No updates available, or all selected updates succeeded              |
| 1    | Updates available but not applied (e.g. `--format=json` without `--batch`, or `--no-interactive` with no selections) |
| 2    | At least one update failed                                           |
| 130  | User pressed Ctrl-C; remaining packages skipped                      |

Use `--format=json` for scripts: it disables colors, spinners, and prompts, and emits a single JSON object on stdout (schema version 1).

```bash
# CI gating: fail the job if any global package has an update
cargo fresh --format=json
# → exit 1 if updates are available, 0 otherwise

# Apply all updates in CI, fail on any update failure
cargo fresh --format=json --batch
# → exit 2 if any update failed, 0 otherwise
```

### Output streams

`cargo fresh` follows the standard CLI convention:

- **stdout** — machine-readable output only. With `--format=json`, exactly one JSON object per invocation. With `--format=human` (default), stdout is empty; pipe-safe.
- **stderr** — all status lines, spinners, prompts, and error messages.

This means `cargo fresh --format=json | jq '.'` works without filtering, and `cargo fresh > /dev/null` still shows progress.

### JSON schema

The full schema is at [`docs/json-schema.json`](docs/json-schema.json) (JSON Schema Draft 2020-12). The `schema_version=1` field shape is the 1.0 contract — within 1.x, fields are only added (never removed or renamed).

Three fields were added in 0.10.3 under `schema_version=1` (additive only — no `schema_version` bump):

- **`skipped[].reason_code`** — a stable enum (`path_source` / `git_source` / `unknown_source`). Branch on this in scripts rather than the prose `reason` string.
- **`version_check_errors[]`** — crates.io packages whose latest-version lookup failed, each with a `name`, `kind` (`not_found` / `unavailable`), and a human-readable `error` message. `fresh[]` excludes these packages, so an empty `updates_available` list can be trusted even when checks failed.
- **`summary.selected`**, **`summary.attempted`**, **`summary.check_errors`** — count of packages chosen for update, packages an install command was run for, and length of `version_check_errors[]` respectively.

```bash
# List names of packages that have updates available
cargo fresh --format=json | jq -r '.updates_available[].name'

# Get the count of failed updates after a batch run
cargo fresh --format=json --batch | jq '.summary.failed'

# Show every git-sourced update candidate
cargo fresh --format=json | jq '.updates_available[] | select(.source == "git")'

# Detect a Ctrl-C abort
cargo fresh --format=json --batch | jq '.aborted'

# Show packages whose version check failed (transient network issues etc.)
cargo fresh --format=json | jq '.version_check_errors[]'

# Branch on stable reason codes for skipped packages
cargo fresh --format=json | jq '.skipped[] | select(.reason_code == "git_source")'
```

### Examples

```bash
# Check all packages and show detailed information
cargo fresh --verbose

# Show only packages with updates
cargo fresh --updates-only

# Combine options
cargo fresh --verbose --updates-only

# Default interactive mode (recommended)
cargo fresh

# Show only packages with updates (interactive mode)
cargo fresh --updates-only

# Non-interactive mode
cargo fresh --no-interactive

# Include prerelease version checks (interactive mode)
cargo fresh --include-prerelease

# Non-interactive mode + prerelease versions
cargo fresh --no-interactive --include-prerelease

# Batch mode - automatically update all packages without confirmation
cargo fresh --batch

# Filter packages by name pattern (supports glob patterns)
cargo fresh --filter "cargo*"              # Only check packages starting with "cargo"
cargo fresh --filter "*mdbook*"            # Only check packages containing "mdbook"
cargo fresh --filter "nu*"                 # Only check packages starting with "nu"

# Exclude packages by glob pattern (repeatable, applied after --filter)
cargo fresh --exclude "cargo-fresh"                  # Check everything except cargo-fresh
cargo fresh --exclude "ripgrep" --exclude "tokei"    # Skip multiple packages
cargo fresh --filter "cargo*" --exclude "cargo-fresh"  # cargo* packages, minus cargo-fresh

# Dry-run: preview the exact cargo commands without changing anything
cargo fresh --dry-run                      # Preview updates for all packages
cargo fresh --dry-run --batch              # Preview a full batch update

# Combine new options with existing ones
cargo fresh --batch --filter "cargo*"      # Batch update only cargo packages
cargo fresh --verbose --filter "*mdbook*"  # Verbose check for mdbook packages
cargo fresh --batch --updates-only        # Batch update only packages with updates

# Generate shell completion scripts
cargo fresh completion zsh    # Generate zsh completion
cargo fresh completion bash   # Generate bash completion
cargo fresh completion fish   # Generate fish completion

# Generate cargo fresh subcommand completion
cargo fresh completion zsh --cargo-fresh    # Generate cargo fresh zsh completion
cargo fresh completion bash --cargo-fresh   # Generate cargo fresh bash completion

# Generate the man page (roff to stdout)
cargo fresh man | man -l -                                          # View directly
cargo fresh man > ~/.local/share/man/man1/cargo-fresh.1              # Install for `man cargo-fresh`
```

### Shell Completion Installation

#### Zsh
```bash
# Generate and install zsh completion
cargo-fresh completion zsh > ~/.zsh/completions/_cargo-fresh
# Or for cargo fresh subcommand
cargo-fresh completion zsh --cargo-fresh > ~/.zsh/completions/_cargo

# Add to your ~/.zshrc
echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc
```

#### Bash
```bash
# Generate and install bash completion
cargo-fresh completion bash > ~/.local/share/bash-completion/completions/cargo-fresh
# Or for cargo fresh subcommand
cargo-fresh completion bash --cargo-fresh > ~/.local/share/bash-completion/completions/cargo

# Source in your ~/.bashrc
echo 'source ~/.local/share/bash-completion/completions/cargo-fresh' >> ~/.bashrc
```

#### Fish
```bash
# Generate and install fish completion for the cargo-fresh binary
cargo-fresh completion fish > ~/.config/fish/completions/cargo-fresh.fish

# For the `cargo fresh` subcommand form, the file MUST be named cargo.fish
# (fish autoloads completions by command name; cargo-fresh.fish would only
# fire on `cargo-fresh<TAB>`, never on `cargo fresh<TAB>`).
cargo-fresh completion fish --cargo-fresh > ~/.config/fish/completions/cargo.fish

# Or, if you want it eager-loaded at shell start (avoids the naming trap):
cargo-fresh completion fish --cargo-fresh > ~/.config/fish/conf.d/cargo-fresh.fish
```

#### Nushell
```bash
# Generate and install nushell completion
cargo-fresh completion nushell > ~/.config/nushell/completions/cargo-fresh.nu
# Or for cargo fresh subcommand
cargo-fresh completion nushell --cargo-fresh > ~/.config/nushell/completions/cargo.nu
```

## Output Examples

cargo-fresh uses a cargo-style status format: a 12-char right-aligned bold verb
followed by the message. Colors carry the meaning — green (success), yellow
(warning), red (failure), dim (secondary). There are no emojis.

### Interactive Mode (Default)

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
Prerelease updates:
  Prerelease mdbook 0.4.52 -> 0.5.0-alpha.1

Update these packages? y
Include prerelease updates? n
Select packages (space to toggle, enter to confirm)
> [x] cargo-outdated
> [x] devtool

    Updating selected packages
   Running cargo binstall --force cargo-outdated --version 0.17.0
    Updated cargo-outdated 0.16.0 -> 0.17.0
   Running cargo binstall --force devtool --version 0.2.5
    Updated devtool 0.2.4 -> 0.2.5

Update Summary
    Updated cargo-outdated 0.16.0 -> 0.17.0
    Updated devtool 0.2.4 -> 0.2.5
    Finished 2 succeeded, in 4.2s
```

### Dry-run Mode

`--dry-run` prints the exact cargo commands (including the binstall→install
fallback) without modifying anything:

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
    Updating cargo-outdated 0.16.0 -> 0.17.0

    Dry run no packages will be modified
   Would run cargo-outdated: cargo binstall --force cargo-outdated --version 0.17.0
    Fallback cargo install --force cargo-outdated --version 0.17.0
```

### Non-Interactive Mode

In `--no-interactive` mode the available updates are listed but nothing is
updated (use `--batch` to update automatically):

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
       Fresh ripgrep 14.1.1
    Updating mdbook 0.4.52 -> 0.5.0-alpha.1
       Note no packages selected
```

Git and path installs are shown with a dimmed `[git]` / `[path]` marker, e.g.
`    Updating my-tool 0.1.0 -> 0.2.0 [git]`.

## Shell Completion Support

`cargo-fresh` supports automatic completion for multiple shells, making command-line usage more convenient.

### Supported Shells

- **Zsh** - Full completion support
- **Bash** - Basic completion support
- **Fish** - Native completion support
- **PowerShell** - Windows completion support
- **Elvish** - Modern shell completion support
- **Nushell** - Nushell completion support

### Installing Completions

#### Manual Installation

```bash
# 1. Generate completion script
cargo fresh completion zsh > ~/.zsh_completions/cargo-fresh.zsh

# 2. Add to zsh configuration
echo 'fpath=($HOME/.zsh_completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc

# 3. Reload configuration
source ~/.zshrc
```

#### Cargo Fresh Subcommand Completion

For `cargo fresh` subcommand completion:

```bash
# Generate cargo fresh subcommand completion
cargo fresh completion zsh --cargo-fresh > cargo-fresh-completion.zsh
cargo fresh completion bash --cargo-fresh > cargo-fresh-completion.bash

# Install cargo fresh completion
source cargo-fresh-completion.zsh  # For zsh
source cargo-fresh-completion.bash # For bash
```

#### Other Shell Installation

```bash
# Bash completion
cargo fresh completion bash > ~/.bash_completions/cargo-fresh.bash
echo 'source ~/.bash_completions/cargo-fresh.bash' >> ~/.bashrc

# Fish completion
cargo fresh completion fish > ~/.config/fish/completions/cargo-fresh.fish

# PowerShell completion
cargo fresh completion powershell > cargo-fresh.ps1
```

### Usage

After installation, you can use auto-completion in two ways:

#### Direct Command Completion
```bash
cargo fresh <TAB>
# Shows all available options:
# --batch  --dry-run  --exclude  --filter  --help  --include-prerelease
# --no-interactive  --updates-only  --verbose  --version
```

#### Cargo Subcommand Completion
```bash
cargo <TAB>        # Shows 'fresh' as a subcommand
cargo fresh <TAB>  # Shows all fresh options and parameters
```

## Technical Features

- **Sparse Index Checks**: Queries the crates.io sparse index directly over HTTP (single shared connection pool, concurrency-limited) instead of spawning `cargo search`; falls back to `cargo search` only when the index is unreachable
- **Concurrent Processing**: Uses the Tokio async runtime to check packages concurrently
- **Semver-based Comparison**: Uses real semver ordering so yanked-version rollbacks aren't flagged as updates and `1.0.0+build` re-publishes are
- **Source-aware Updates**: Detects crates.io / git / path install sources and picks the right `cargo install` strategy for each
- **Smart Version Detection**: Automatically distinguishes between stable and prerelease versions
- **Interactive Interface**: User-friendly command-line interaction experience
- **Colored Output**: Beautiful terminal output with clear status display
- **Enhanced Error Handling**: Intelligent retry mechanisms with exponential backoff and user-friendly error messages
- **Batch Operations**: Support for automated batch updates without user confirmation
- **Package Filtering**: Advanced filtering capabilities with glob pattern support
- **Type Safety**: Rust type system ensures code safety
- **Progress Bars**: Real-time update progress display for better user experience
- **Shell Completion**: Auto-completion support for multiple shells
- **Language Detection**: Automatic system language detection and interface adaptation
- **Cargo Integration**: Native cargo subcommand support for seamless workflow
- **Bilingual Support**: Complete Chinese and English interface with smart switching
- **Modular Architecture**: Clean, maintainable code structure with separate modules

## Shell Completion Troubleshooting

### Common Issues

#### Completion not working
If shell completion is not working, try the following:

1. **Verify completion installation**:
   ```bash
   # Check if completion files exist
   ls ~/.zsh/completions/_cargo-fresh  # For zsh
   ls ~/.local/share/bash-completion/completions/cargo-fresh  # For bash
   ```

2. **Reload shell configuration**:
   ```bash
   # For zsh
   source ~/.zshrc
   
   # For bash
   source ~/.bashrc
   
   # For fish
   # Restart fish shell
   ```

3. **Regenerate completion files**:
   ```bash
   # Generate fresh completion files
   cargo-fresh completion zsh > ~/.zsh/completions/_cargo-fresh
   cargo-fresh completion bash > ~/.local/share/bash-completion/completions/cargo-fresh
   ```

#### Missing options in completion
If you notice missing options in completion:

1. **Update cargo-fresh**:
   ```bash
   cargo install --force cargo-fresh
   ```

2. **Regenerate completion files**:
   ```bash
   cargo-fresh completion zsh > ~/.zsh/completions/_cargo-fresh
   ```

3. **Verify completion includes new options**:
   ```bash
   grep -E "(batch|filter)" ~/.zsh/completions/_cargo-fresh
   ```

#### Cargo fresh subcommand completion
For `cargo fresh` subcommand completion:

1. **Generate cargo fresh completion**:
   ```bash
   cargo-fresh completion zsh --cargo-fresh > ~/.zsh/completions/_cargo
   ```

2. **Verify cargo completion**:
   ```bash
   cargo <TAB>  # Should show 'fresh' as a subcommand
   cargo fresh <TAB>  # Should show all fresh options
   ```

## Stability Guarantees

Prior to 1.0.0 the project still ships breaking changes; once 1.0.0 lands the surface below is **promised** to follow semver:

| Surface | Stability |
|---|---|
| Exit codes (`0` / `1` / `2` / `130`) | Stable — never reused or removed within a major version |
| `--format=json` output, `schema_version=1` | Additive only — new fields may appear; existing fields will not be renamed or change types |
| CLI flags listed in `--help` | Stable — flags are not silently renamed; deprecations get one minor cycle of warning before removal |
| Source-aware install behavior (crates / git / path) | Stable |
| Human-readable status verbs (`Checking`, `Updating`, etc.) | **Not** stable — wording, color, alignment may change for UX improvements |
| Locale text (English / Chinese) | **Not** stable — phrasing tweaks expected; don't grep against `stdout` |
| Internal modules / library API (`cargo_fresh::*`) | **Not** stable — `src/lib.rs` exists for integration tests, not as a downstream API |

If you're scripting against cargo-fresh, anchor on exit codes and `--format=json`; never on colored status text.

## How cargo-fresh differs from cargo-update / cargo-install-update

[`cargo-update`](https://github.com/nabijaczleweli/cargo-update) is the long-standing tool in this space. cargo-fresh is a fresh take, not a fork — these are the differences that drove building it:

| | cargo-fresh | cargo-update |
|---|---|---|
| **Version source** | crates.io sparse index (HTTP, ~50–100ms/pkg, 16-way concurrent) | `cargo search` subprocess per package |
| **Source-aware updates** | Crates / `git+URL` / `path+DIR` each get the right install command | Registry + git; no `path` source |
| **Package selection** | `--filter "tokio*"` + `--exclude "*-test"` (globset glob) | Exact package names or `--all` (no glob/substring) |
| **Prerelease handling** | Explicit `--include-prerelease`; semver `.pre` check, not string `"rc"` | Per-package opt-in via `cargo-install-update-config` |
| **Output style** | Cargo-aesthetic 12-char verb prefixes; no emoji | Plain text |
| **JSON mode** | `--format=json` with versioned `schema_version=1` schema | None |
| **i18n** | English + Chinese auto-detected via `LANG` | English only |
| **Dry-run preview** | `--dry-run` prints the exact `cargo install` command per package | `-n`/`--dry-run` lists what would update |
| **binstall usage** | Opt-in via `--install-binstall`; otherwise hint only | Auto-used when available and config is default |
| **Install options preserved** | Yes — features (`--features` / `--no-default-features` / `--all-features`) restored from `.crates2.json` | Yes — `.crates2.json` features/profile, plus per-package `cargo-install-update-config` |
| **CI ergonomics** | Exit codes 0/1/2/130 + JSON + non-TTY auto-downgrade | Standard exit codes |

cargo-update is more mature. Both tools now preserve the features a package was installed with; cargo-update additionally preserves build profile and supports per-package config via `cargo-install-update-config`. Use whichever fits; both are healthy projects to depend on.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide. TL;DR:

1. Fork → branch → commit → PR
2. Before pushing: `cargo clippy --all-targets -- -D warnings` and `cargo test` must be green
3. User-visible changes need a `CHANGELOG.md` `[Unreleased]` entry + README sync

Security issues: see [SECURITY.md](SECURITY.md) — please don't file them as public issues.

## License

This project is licensed under the Apache 2.0 License. See the [LICENSE](LICENSE) file for complete license terms.

### License Summary

The Apache 2.0 License is a permissive open source license that allows you to:

- ✅ **Commercial use** - Use in commercial projects
- ✅ **Modification** - Modify the source code
- ✅ **Distribution** - Distribute original or modified code
- ✅ **Private use** - Use privately
- ✅ **Patent use** - Use related patents
- ✅ **Patent grant** - Automatic patent license grant

**Main requirements**:
- Include the original license and copyright notice when distributing
- Must state changes made to the source code
- Cannot use project name, trademarks, or product names for promotion

### Copyright Information

Copyright (c) 2025 Jenkin Pan

This project is open source under the Apache 2.0 License. See the [LICENSE](LICENSE) file for details.

## Related Links

- [Crates.io](https://crates.io/crates/cargo-fresh)
- [GitHub Repository](https://github.com/jenkinpan/cargo-fresh)
- [Issues](https://github.com/jenkinpan/cargo-fresh/issues)
