#!/usr/bin/env bash
# Shared "local CI" gate.
#
# Runs the full suite (tests/run-all.sh) — but ONLY when the commit or merge is
# landing on `main`. Feature-branch commits stay fast (no tests); anything that
# becomes part of `main` is verified *before* the commit/merge is finalized.
#
# Why this and not pre-push: pre-push fires after the merge commit already
# exists on local `main`, so the breakage is "already done" by the time it runs.
# pre-commit + pre-merge-commit both abort BEFORE `main` is mutated, which is the
# local-CI-before-main behavior we want.
#
# Hook coverage for code reaching `main`:
#   - direct commit on main          -> pre-commit fires here
#   - clean (non-ff) merge into main -> pre-merge-commit fires here
#   - merge-with-conflicts into main -> the resolving `git commit` re-fires pre-commit
#   - feature-branch commit          -> skipped (branch != main)
#
# Gap to know about: a *fast-forward* merge creates no commit, so NO hook fires.
# Land feature branches on main with `git merge --no-ff <branch>` (or set
# `git config branch.main.mergeOptions --no-ff`) so the merge always creates a
# gated commit.
set -euo pipefail

hook="${1:-hook}"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

branch="$(git symbolic-ref --short HEAD 2>/dev/null || true)"
if [ "$branch" != "main" ]; then
  echo "$hook: on '${branch:-detached HEAD}' (not main) — skipping local CI."
  exit 0
fi

echo "$hook: landing on main — running local CI (tests/run-all.sh)…"
exec ./tests/run-all.sh
