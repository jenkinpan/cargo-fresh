#!/usr/bin/env bash
# Regenerate fixture archives. Run from repo root.
set -euo pipefail
cd "$(dirname "$0")"

# ripgrep-like: binary inside a versioned subdir
rm -rf ripgrep-14.1.2-x86_64-apple-darwin ripgrep-like.tar.gz
mkdir -p ripgrep-14.1.2-x86_64-apple-darwin
printf '#!/bin/sh\necho rg fake\n' > ripgrep-14.1.2-x86_64-apple-darwin/rg
chmod +x ripgrep-14.1.2-x86_64-apple-darwin/rg
tar czf ripgrep-like.tar.gz ripgrep-14.1.2-x86_64-apple-darwin
rm -rf ripgrep-14.1.2-x86_64-apple-darwin

# mdbook-like: binary at archive root
rm -rf mdbook mdbook-like.tar.gz
printf '#!/bin/sh\necho mdbook fake\n' > mdbook
chmod +x mdbook
tar czf mdbook-like.tar.gz mdbook
rm -f mdbook

# cargo-deny-like: zip with binary in subdir
rm -rf cargo-deny-0.19.7-x86_64-apple-darwin cargo-deny-like.zip
mkdir -p cargo-deny-0.19.7-x86_64-apple-darwin
printf '#!/bin/sh\necho cargo-deny fake\n' > cargo-deny-0.19.7-x86_64-apple-darwin/cargo-deny
chmod +x cargo-deny-0.19.7-x86_64-apple-darwin/cargo-deny
zip -q -r cargo-deny-like.zip cargo-deny-0.19.7-x86_64-apple-darwin
rm -rf cargo-deny-0.19.7-x86_64-apple-darwin
