#!/usr/bin/env bash
# Compatibility entrypoint for the Rust phase runner.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

if [ -n "${RTS_PHASERUNNER_BIN:-}" ]; then
  exec "$RTS_PHASERUNNER_BIN" "$@"
fi

exec cargo run --quiet --manifest-path "$repo_root/server/Cargo.toml" -p rts-phaserunner -- "$@"
