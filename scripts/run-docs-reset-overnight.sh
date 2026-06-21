#!/usr/bin/env bash
set -euo pipefail

delay_seconds="${DOCS_RESET_START_DELAY_SECONDS:-3600}"
phase_pause_seconds="${DOCS_RESET_PHASE_PAUSE_SECONDS:-300}"
checkout="${DOCS_RESET_CHECKOUT:-/tmp/rts-worktrees/docsreset-main}"
worktree_root="${RTS_WORKTREE_ROOT:-/tmp/rts-worktrees}"
plan="${DOCS_RESET_PLAN:-docsreset}"
repo_url="${DOCS_RESET_REPO_URL:-}"
model="${DOCS_RESET_MODEL:-}"
phase_list="${DOCS_RESET_PHASES:-1 2 3 4 5 6 7 8}"

if [ -z "$repo_url" ]; then
  repo_url="$(git config --get remote.origin.url)"
fi

if [ -z "$repo_url" ]; then
  echo "Could not determine origin URL. Set DOCS_RESET_REPO_URL." >&2
  exit 2
fi

if ! [[ "$delay_seconds" =~ ^[0-9]+$ ]]; then
  echo "DOCS_RESET_START_DELAY_SECONDS must be an integer, got: $delay_seconds" >&2
  exit 2
fi

if ! [[ "$phase_pause_seconds" =~ ^[0-9]+$ ]]; then
  echo "DOCS_RESET_PHASE_PAUSE_SECONDS must be an integer, got: $phase_pause_seconds" >&2
  exit 2
fi

read -r -a phases <<< "$phase_list"

mkdir -p "$worktree_root"

echo "docs-reset: sleeping ${delay_seconds}s before refreshing origin/main"
sleep "$delay_seconds"

if [ -e "$checkout" ] && [ ! -d "$checkout/.git" ]; then
  echo "docs-reset: checkout path exists but is not a git clone: $checkout" >&2
  exit 2
fi

if [ ! -d "$checkout/.git" ]; then
  echo "docs-reset: cloning clean runner checkout to $checkout"
  git clone "$repo_url" "$checkout"
fi

git -C "$checkout" fetch origin main
git -C "$checkout" checkout main

if [ -n "$(git -C "$checkout" status --porcelain=v1)" ]; then
  echo "docs-reset: runner checkout is dirty; refusing to continue: $checkout" >&2
  git -C "$checkout" status --short >&2
  exit 1
fi

git -C "$checkout" merge --ff-only origin/main

if [ ! -f "$checkout/plans/$plan/plan.md" ]; then
  echo "docs-reset: missing plans/$plan/plan.md after refresh from origin/main" >&2
  echo "docs-reset: make sure the docs reset plan PR has merged before running this wrapper." >&2
  exit 1
fi

for index in "${!phases[@]}"; do
  phase="${phases[$index]}"
  echo "docs-reset: refreshing origin/main before phase $phase"
  git -C "$checkout" fetch origin main
  git -C "$checkout" checkout main

  if [ -n "$(git -C "$checkout" status --porcelain=v1)" ]; then
    echo "docs-reset: runner checkout became dirty before phase $phase; stopping" >&2
    git -C "$checkout" status --short >&2
    exit 1
  fi

  git -C "$checkout" merge --ff-only origin/main

  args=(scripts/phase-runner.sh --plan "$plan" "$phase" --pr --wait)
  if [ -n "$model" ]; then
    args+=(--model "$model")
  fi

  echo "docs-reset: running phase $phase with phase-runner"
  (cd "$checkout" && "${args[@]}")

  if [ "$index" -lt "$((${#phases[@]} - 1))" ] && [ "$phase_pause_seconds" -gt 0 ]; then
    echo "docs-reset: sleeping ${phase_pause_seconds}s before next phase"
    sleep "$phase_pause_seconds"
  fi
done

echo "docs-reset: completed requested phases: $phase_list"
