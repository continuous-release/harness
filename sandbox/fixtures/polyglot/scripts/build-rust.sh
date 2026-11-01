#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${RELEASE_VERSION:-$(cat version.txt)}"
export RELEASE_VERSION="$VERSION"

node scripts/codegen.mjs --check
mkdir -p dist

if [ -n "${POLYGLOT_RUST_TARGET:-}" ]; then
  cargo build --release --offline --manifest-path rust/Cargo.toml --target "$POLYGLOT_RUST_TARGET"
  cp "rust/target/$POLYGLOT_RUST_TARGET/release/polyglot" dist/polyglot
else
  cargo build --release --offline --manifest-path rust/Cargo.toml
  cp rust/target/release/polyglot dist/polyglot
fi

chmod +x dist/polyglot
