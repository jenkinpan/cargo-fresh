# Contributing to cargo-fresh

Thanks for considering a contribution! This document captures the conventions you need to know. Anything beyond this scope is in `CLAUDE.md` and `ROADMAP.md`.

## Development environment

- **MSRV**: Rust 1.86. The latest stable works locally; CI runs both stable and 1.86.
- **Dependencies**: `Cargo.lock` is committed (this is a binary crate, reproducible builds are required).

## Pre-submission checklist

```bash
cargo clippy --all-targets -- -D warnings   # zero-warning policy
cargo test                                  # unit + integration tests must be green
cargo build --release                       # the release path occasionally surfaces warnings debug misses
```

If you changed user-visible behavior, also update:

- The `[Unreleased]` section of `CHANGELOG.md`
- `README.md` (keep English and Chinese READMEs in sync)
- `src/locale/texts.rs` (any new strings need both English and Chinese, plus an entry in `tests::test_text_consistency`)

## Code style

### CLI output

**All `println!` must go through the `display::status*` family.** The cargo-aesthetic 12-char right-aligned verb prefix is the project's visual signature — see the "Status verb dictionary" in `CLAUDE.md`. New verbs go in that table.

Color semantics:

| Color | Used for |
|-------|----------|
| Green bold | Success (Checking / Found / Updated / Finished) |
| Yellow bold | Warning (Fallback / Note / Unchanged / Skip) |
| Red bold | Failure (Failed / Finished when any package failed) |
| Dim | Secondary information (Fresh / Package N/M / Hint / Check / Latest) |

### Error handling

- Keep internal paths on `anyhow::Result`; the error chain is short enough that forcing thiserror everywhere is noise
- Failures that suggest an **actionable** next step ("install X", "set HTTPS_PROXY") belong in `src/errors.rs` as a `CargoFreshError` variant
- `main` runs failures through `errors::hint_for` to find a matching hint and prints it after the error chain

### Internationalization

- All user-facing strings live in `src/locale/texts.rs`
- Multi-placeholder templates **must** use named placeholders + `language.format_text("key", &[("name", val)])`. Chained `.replace("{}", x)` is only safe for single-placeholder strings.
- Adding a new language: add a `match` arm and update `texts::tests::test_text_consistency` with the full key list.

## Tests

- Unit tests live alongside code in `#[cfg(test)] mod tests`
- Integration tests live in `tests/`:
  - `tests/cli.rs` uses `assert_cmd` to drive the real binary against its external contract (no byte-for-byte snapshots — a typo fix shouldn't churn snapshots)
  - `tests/sparse_index_http.rs` uses `wiremock` to verify HTTP behavior without network access
- For locale detection: use the pure function `detect_from_locale(&str)`. **Never** call `env::set_var` from tests — that race is the reason `--test-threads=1` is no longer needed.

## Commits & PRs

- Commit subjects follow [Conventional Commits](https://www.conventionalcommits.org/): `feat(scope): ...` / `fix(scope): ...` / `refactor(scope): ...` / `test: ...` / `docs: ...`
- BREAKING changes use `!`, e.g. `feat(binstall)!: stop auto-installing cargo-binstall by default`
- Commit bodies in English or Chinese are both fine — both styles exist in the repo history

## Release process

Fully automated; see "Release process" in `CLAUDE.md`. Short version:

```bash
# 1. bump version
$EDITOR Cargo.toml CHANGELOG.md
git commit -am "chore: release vX.Y.Z"

# 2. tag and push
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin master vX.Y.Z
```

GitHub Actions handles crates.io publishing and multi-platform binaries from there.

## Stability before 1.0

See the "Stability Guarantees" section in README. In short: CLI exit codes and `--format=json schema_version=1` fields are committed; colors and locale wording may shift.

## Code of conduct

Be kind and constructive in issues, PRs, and Discussions. Hostile communication will be closed and locked.

## License

By contributing you agree your work is released under Apache-2.0.
