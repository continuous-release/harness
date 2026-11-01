#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${RELEASE_VERSION:-$(cat version.txt)}"
export RELEASE_VERSION="$VERSION"

bash scripts/build-rust.sh
node scripts/build-ts.mjs --version "$VERSION" --out dist/polyglot.tgz
node scripts/build-python.mjs --version "$VERSION" --out dist/polyglot.whl
node scripts/manifest.mjs \
  --version "$VERSION" \
  --out dist \
  --artifact "polyglot=dist/polyglot" \
  --artifact "polyglot.tgz=dist/polyglot.tgz" \
  --artifact "polyglot.whl=dist/polyglot.whl"
node scripts/verify.mjs --dist dist
