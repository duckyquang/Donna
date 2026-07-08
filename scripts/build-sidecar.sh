#!/usr/bin/env bash
# Build donna-server and place it where Tauri's externalBin expects:
#   src-tauri/binaries/donna-server-<target-triple>[.exe]
# Usage: build-sidecar.sh [target-triple]   (defaults to the host triple)
set -euo pipefail
cd "$(dirname "$0")/.."

HOST="$(rustc -vV | sed -n 's/^host: //p')"
TARGET="${1:-$HOST}"

if [ "${1:-}" != "" ]; then
  cargo build -p donna-server --release --target "$TARGET"
  SRC="target/$TARGET/release/donna-server"
else
  cargo build -p donna-server --release
  SRC="target/release/donna-server"
fi

EXT=""
case "$TARGET" in *windows*) EXT=".exe" ;; esac

mkdir -p src-tauri/binaries
cp "$SRC$EXT" "src-tauri/binaries/donna-server-$TARGET$EXT"
echo "sidecar ready: src-tauri/binaries/donna-server-$TARGET$EXT"
