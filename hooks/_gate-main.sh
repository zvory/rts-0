#!/usr/bin/env bash
# Shared cheap local commit gate.
#
# Runs only low-risk checks before local commits. The authoritative full gate is
# GitHub Actions (`Main test gate / ./tests/run-all.sh`) on PRs and main pushes.
#
# Why this and not tests/run-all.sh: agents should use focused local
# verification while GitHub Actions owns the expensive full-suite merge signal.
#
# Hook coverage:
#   - ordinary commit                -> pre-commit fires here
#   - clean non-ff merge commit      -> pre-merge-commit fires here
#   - merge-with-conflicts           -> the resolving `git commit` re-fires
#                                      pre-commit with MERGE_HEAD
set -euo pipefail

hook="${1:-hook}"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

echo "$hook: running cheap staged diff checks (git diff --cached --check)"
git diff --cached --check

echo "$hook: running docs health check"
node scripts/check-docs-health.mjs
