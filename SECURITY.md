# Security Policy

## Reporting a vulnerability

If you discover a security issue in cargo-fresh, please **do not** open a public issue.

Contact the maintainer privately via one of:

- GitHub Security Advisories (preferred): <https://github.com/jenkinpan/cargo-fresh/security/advisories/new>
- Email: see the maintainer's GitHub profile linked from `Cargo.toml`

Please include:

1. The class of vulnerability and its scope of impact
2. Reproduction steps / PoC (commands plus any required environment configuration)
3. Affected version range
4. Suggested mitigation (optional)

I will acknowledge receipt within 72 hours. Fixes will be disclosed via GitHub Security Advisories, with a CVE assigned where applicable.

## Supported versions

| Version | Status |
|---------|--------|
| 0.10.x | Current |
| < 0.10.0 | No longer supported; please upgrade |
| 1.x (unreleased) | Planned |

## Trust boundary

cargo-fresh is a **local CLI**. It:

- Reads `~/.cargo/.crates.toml` / `cargo install --list`
- Makes HTTPS requests to `index.crates.io` (or a user-configured mirror)
- Spawns `cargo install` / `cargo binstall` subprocesses, **which download and compile/run arbitrary crate code**

Specifically:

- cargo-fresh **does not vet the trustworthiness of crates**. Use `--dry-run` to inspect what would be installed before applying.
- `--registry-url` is used directly as an HTTP base URL. Pointing it at a hostile server leaks which crates you have installed.
- `cargo binstall` pulls prebuilt binaries from GitHub Releases; the trust model for that step belongs to cargo-binstall, not cargo-fresh.

## Out of scope

The following are not security issues — please file them as regular issues instead:

- Upgrading to a version that gets yanked (check with `--dry-run` first)
- A subprocess invoked by cargo-fresh crashes while cargo-fresh itself exits cleanly
- Locale string typos / UI color-contrast complaints (these are UX bugs)
