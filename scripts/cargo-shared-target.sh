#!/usr/bin/env bash
# Run cargo with the shared server target dir used by worktree test runs.
#
# Repo-level `.cargo/config.toml` makes normal `cargo test ...` commands use the same shared
# cache automatically. This wrapper remains for scripts that want to print the configured target
# dir and for callers that prefer an explicit cargo entry point.
set -euo pipefail

SHARED_TARGET_DIR="/tmp/rts-cargo-target/rts-0-server"

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="$SHARED_TARGET_DIR"
else
  export CARGO_TARGET_DIR
fi

if [ "${1:-}" = "--print-target-dir" ]; then
  printf '%s\n' "$CARGO_TARGET_DIR"
  exit 0
fi

exec cargo "$@"
