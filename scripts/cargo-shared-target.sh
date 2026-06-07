#!/usr/bin/env bash
# Run cargo with the shared server target dir used by worktree test runs.
#
# Cargo's own config cannot derive the primary checkout path from a git worktree, so direct
# `cargo test ...` commands in fresh worktrees otherwise fall back to that worktree's local
# server/target and rebuild dependencies from scratch.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_DIR="$REPO_ROOT/server"

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  GIT_COMMON_DIR="$(git -C "$REPO_ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
  if [ -n "$GIT_COMMON_DIR" ] && [ -d "$GIT_COMMON_DIR" ]; then
    PRIMARY_REPO_ROOT="$(cd "$GIT_COMMON_DIR/.." && pwd)"
    if [ -f "$PRIMARY_REPO_ROOT/server/Cargo.toml" ]; then
      export CARGO_TARGET_DIR="$PRIMARY_REPO_ROOT/server/target"
    else
      export CARGO_TARGET_DIR="$SERVER_DIR/target"
    fi
  else
    export CARGO_TARGET_DIR="$SERVER_DIR/target"
  fi
else
  export CARGO_TARGET_DIR
fi

if [ "${1:-}" = "--print-target-dir" ]; then
  printf '%s\n' "$CARGO_TARGET_DIR"
  exit 0
fi

exec cargo "$@"
