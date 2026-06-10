#!/usr/bin/env bash
# Shared "local CI" gate.
#
# Runs the full local gate (tests/run-all.sh) before every local commit and before
# every local merge commit.
#
# Why this and not pre-push: the project keeps direct pushes to main unblocked.
# pre-commit + pre-merge-commit abort before a bad local commit/merge is written,
# while pushes remain a normal Git operation.
#
# Hook coverage:
#   - any commit                     -> pre-commit fires here
#   - clean non-ff merge commit      -> pre-merge-commit fires here
#   - merge-with-conflicts           -> the resolving `git commit` re-fires pre-commit
#
# Gap to know about: a *fast-forward* merge creates no commit, so NO hook fires.
# Land feature branches on main with `git merge --no-ff <branch>` (or set
# `git config branch.main.mergeOptions --no-ff`) so the merge always creates a
# gated commit.
set -euo pipefail

hook="${1:-hook}"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

echo "$hook: running full local CI (tests/run-all.sh)…"
exec ./tests/run-all.sh
