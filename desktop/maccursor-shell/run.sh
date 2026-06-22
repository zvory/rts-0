#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "maccursor-shell requires macOS." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "maccursor-shell requires cargo." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec cargo run --manifest-path "$SCRIPT_DIR/src-tauri/Cargo.toml" -- "$@"
