#!/usr/bin/env bash
# Shared "local CI" gate.
#
# Runs the full local gate (tests/run-all.sh) before ordinary local commits.
#
# Why this and not pre-push: the project keeps direct pushes to main unblocked.
# pre-commit aborts before a bad local commit is written, while pushes remain a
# normal Git operation.
#
# Hook coverage:
#   - ordinary commit                -> pre-commit fires here
#   - clean non-ff merge commit      -> pre-merge-commit fires here, skipped
#   - merge-with-conflicts           -> the resolving `git commit` re-fires
#                                      pre-commit with MERGE_HEAD, skipped
set -euo pipefail

hook="${1:-hook}"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [[ "$hook" == "pre-merge-commit" ]] || git rev-parse -q --verify MERGE_HEAD >/dev/null; then
  echo "$hook: merge commit detected; skipping full local CI (tests/run-all.sh)"
  exit 0
fi

echo "$hook: running full local CI (tests/run-all.sh)…"
exec ./tests/run-all.sh
