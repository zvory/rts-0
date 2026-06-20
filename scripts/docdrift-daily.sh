#!/usr/bin/env bash
# Launchd-friendly entrypoint for the documentation drift daily sweep.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

exec node scripts/docdrift-sweep.mjs --full --head origin/main "$@"
