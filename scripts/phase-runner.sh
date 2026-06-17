#!/usr/bin/env bash
# Stable compatibility entrypoint for the Agents SDK phase runner.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

if [ -n "${RTS_PHASERUNNER_BIN:-}" ]; then
  exec "$RTS_PHASERUNNER_BIN" "$@"
fi

exec node "$repo_root/scripts/phase-runner-agents.mjs" "$@"
