#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "maccursor-spike requires macOS." >&2
  exit 1
fi

if ! command -v swiftc >/dev/null 2>&1; then
  echo "maccursor-spike requires swiftc from Xcode command line tools." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/.build"
BINARY="$BUILD_DIR/maccursor-spike"
MODULE_CACHE="$BUILD_DIR/module-cache"

mkdir -p "$BUILD_DIR" "$MODULE_CACHE"
swiftc -module-cache-path "$MODULE_CACHE" "$SCRIPT_DIR/src/MacCursorSpike.swift" -o "$BINARY"
exec "$BINARY" "$@"
