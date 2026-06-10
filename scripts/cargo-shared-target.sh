#!/usr/bin/env bash
# Run cargo with the per-worktree server target dir used by local gate runs.
#
# Final Cargo artifacts must be isolated by worktree so unrelated branches do
# not overwrite each other's debug binaries, test harnesses, or self-play
# artifacts.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
repo_name="$(basename "$repo_root")"
repo_hash="$(
  if command -v shasum >/dev/null 2>&1; then
    printf '%s' "$repo_root" | shasum -a 256 | awk '{ print substr($1, 1, 12) }'
  else
    printf '%s' "$repo_root" | cksum | awk '{ print $1 }'
  fi
)"
TARGET_BASE_DIR="${RTS_CARGO_TARGET_BASE_DIR:-/tmp/rts-cargo-target}"
WORKTREE_TARGET_DIR="$TARGET_BASE_DIR/${repo_name}-${repo_hash}-server"

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="$WORKTREE_TARGET_DIR"
else
  export CARGO_TARGET_DIR
fi

if [ "${1:-}" = "--print-target-dir" ]; then
  printf '%s\n' "$CARGO_TARGET_DIR"
  exit 0
fi

exec cargo "$@"
