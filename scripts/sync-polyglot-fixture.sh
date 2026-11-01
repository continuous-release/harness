#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

source_dir="${1:-../polyglot}"
dest="sandbox/fixtures/polyglot"

if [ ! -d "$source_dir" ]; then
  echo "usage: $0 /path/to/polyglot" >&2
  exit 1
fi

mkdir -p "$dest"
rsync -a --delete \
  --exclude='.git' \
  --exclude='.github' \
  --exclude='dist' \
  --exclude='dist-buck' \
  --exclude='node_modules' \
  --exclude='target' \
  --exclude='bazel-*' \
  --exclude='buck-out' \
  --exclude='.nx' \
  --exclude='result' \
  --exclude='__pycache__' \
  --exclude='*.pyc' \
  "$source_dir"/ "$dest"/

echo "synced $dest from $source_dir"
