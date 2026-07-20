#!/usr/bin/env bash
# Wait cheaply until a PR is merged, failed, canceled, or closed unmerged.
set -euo pipefail

GH_BIN="${GH_BIN:-gh}"
INTERVAL_SECONDS="${RTS_WAIT_PR_INTERVAL_SECONDS:-300}"
TIMEOUT_SECONDS="${RTS_WAIT_PR_TIMEOUT_SECONDS:-0}"
MAIN_REF="${RTS_WAIT_PR_MAIN_REF:-origin/main}"
ONCE=0
PR=""

usage() {
  cat <<'EOF'
Usage: scripts/wait-pr.sh <pr> [options]

Waits for GitHub to report a PR merged, verifies the PR head SHA is reachable
from origin/main, fast-forwards the local main checkout, and runs merged-worktree
cleanup. Exits non-zero when checks fail, checks cancel, the PR closes unmerged,
the local main update cannot fast-forward, or the wait times out.

Options:
  --interval SECONDS         Sleep between polls, default: 300.
  --timeout SECONDS          Overall timeout; 0 means no timeout.
  --once                     Check once and exit non-zero if still pending.
  --main-ref REF             Ref that must contain the merged head, default: origin/main.
  -h, --help                 Show this help.

Test fixtures:
  RTS_WAIT_PR_VIEW_JSON      JSON returned instead of `gh pr view`.
  RTS_WAIT_PR_CHECKS_JSON    JSON returned instead of `gh pr checks`.
  RTS_WAIT_PR_SKIP_FETCH=1   Skip `git fetch origin main` before ancestry check.
EOF
}

main_worktree_path() {
  local worktree_path=""
  local line

  while IFS= read -r line; do
    case "$line" in
      "worktree "*) worktree_path="${line#worktree }" ;;
      "branch refs/heads/main")
        printf '%s\n' "$worktree_path"
        return 0
        ;;
    esac
  done < <(git worktree list --porcelain)

  return 1
}

refresh_main_checkout() {
  local main_worktree
  main_worktree="$(main_worktree_path || true)"
  if [ -z "$main_worktree" ] || [ ! -d "$main_worktree" ]; then
    echo "wait-pr: merged PR verified, but no local main worktree was found to refresh" >&2
    return 1
  fi

  echo "wait-pr: refreshing local main checkout at $main_worktree"
  git -C "$main_worktree" pull --ff-only origin main

  # A fast-forward pull normally invokes post-merge. Run cleanup explicitly too,
  # because an already-current checkout does not invoke that hook.
  if ! (cd "$main_worktree" && scripts/cleanup-worktrees.sh --auto); then
    echo "wait-pr: local main is current, but opportunistic worktree cleanup failed" >&2
  fi
}

deliver_patch_notes() {
  local head_sha="$1"
  local head_ref="$2"
  local view_json="$3"
  local -a delivery_args=(
    --repo "$repo_root"
    --head-branch "$head_ref"
    --delivery-ref "$head_sha"
    --deliver-discord
  )

  while IFS= read -r fragment_path; do
    delivery_args+=(--delivery-path "$fragment_path")
  done < <(jq -r '.files[]?.path | select(startswith("patch-notes/"))' <<<"$view_json")

  node scripts/patch-note-pass.mjs "${delivery_args[@]}"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --interval) INTERVAL_SECONDS="${2:?missing --interval value}"; shift ;;
    --timeout) TIMEOUT_SECONDS="${2:?missing --timeout value}"; shift ;;
    --once) ONCE=1 ;;
    --main-ref) MAIN_REF="${2:?missing --main-ref value}"; shift ;;
    -h|--help) usage; exit 0 ;;
    -*)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [ -n "$PR" ]; then
        echo "wait-pr: only one PR argument is supported" >&2
        exit 2
      fi
      PR="$1"
      ;;
  esac
  shift
done

if [ -z "$PR" ]; then
  echo "wait-pr: missing PR number or URL" >&2
  usage >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

started_at="$(date +%s)"

load_view_json() {
  if [ -n "${RTS_WAIT_PR_VIEW_JSON:-}" ]; then
    printf '%s\n' "$RTS_WAIT_PR_VIEW_JSON"
  else
    "$GH_BIN" pr view "$PR" \
      --json number,url,state,mergedAt,headRefOid,headRefName,baseRefName,autoMergeRequest,mergeStateStatus,isDraft,files
  fi
}

load_checks_json() {
  if [ -n "${RTS_WAIT_PR_CHECKS_JSON:-}" ]; then
    printf '%s\n' "$RTS_WAIT_PR_CHECKS_JSON"
  else
    "$GH_BIN" pr checks "$PR" --json name,workflow,state,bucket,link 2>/dev/null || printf '[]\n'
  fi
}

summarize_failed_checks() {
  jq -r '
    .[]
    | select((.bucket // "" | ascii_downcase) as $bucket
        | ($bucket == "fail" or $bucket == "cancel"))
    | "- \(.workflow // "workflow") / \(.name): \(.state // .bucket // "unknown") \(.link // "")"
  '
}

while true; do
  view_json="$(load_view_json)"
  checks_json="$(load_checks_json)"

  number="$(jq -r '.number // empty' <<<"$view_json")"
  url="$(jq -r '.url // empty' <<<"$view_json")"
  state="$(jq -r '.state // empty' <<<"$view_json")"
  merged_at="$(jq -r '.mergedAt // empty' <<<"$view_json")"
  head_sha="$(jq -r '.headRefOid // empty' <<<"$view_json")"
  head_ref="$(jq -r '.headRefName // empty' <<<"$view_json")"
  merge_state="$(jq -r '.mergeStateStatus // empty' <<<"$view_json")"

  failed_count="$(jq '[.[] | select((.bucket // "" | ascii_downcase) as $bucket | ($bucket == "fail" or $bucket == "cancel"))] | length' <<<"$checks_json")"
  pending_count="$(jq '[.[] | select((.bucket // "" | ascii_downcase) as $bucket | ($bucket == "pending" or $bucket == "skipping"))] | length' <<<"$checks_json")"

  if [ "$failed_count" -gt 0 ]; then
    echo "wait-pr: PR #$number has failed or canceled checks: $url" >&2
    summarize_failed_checks <<<"$checks_json" >&2
    exit 1
  fi

  if [ "$state" = "CLOSED" ] && [ -z "$merged_at" ]; then
    echo "wait-pr: PR #$number closed unmerged: $url" >&2
    exit 1
  fi

  if [ -n "$merged_at" ] || [ "$state" = "MERGED" ]; then
    if [ -z "$head_sha" ]; then
      echo "wait-pr: PR #$number is merged but head SHA is unavailable: $url" >&2
      exit 1
    fi
    if [ "${RTS_WAIT_PR_SKIP_FETCH:-0}" != "1" ]; then
      git fetch --quiet origin main
    fi
    if git merge-base --is-ancestor "$head_sha" "$MAIN_REF"; then
      deliver_patch_notes "$head_sha" "$head_ref" "$view_json"
      refresh_main_checkout
      echo "wait-pr: PR #$number merged, $head_sha is reachable from $MAIN_REF, and local main is current"
      exit 0
    fi
    echo "wait-pr: PR #$number is merged, but $head_sha is not reachable from $MAIN_REF" >&2
    exit 1
  fi

  if [ "$ONCE" = "1" ]; then
    echo "wait-pr: PR #$number still pending (head=$head_ref checks_pending=$pending_count merge_state=$merge_state): $url" >&2
    exit 2
  fi

  if [ "$TIMEOUT_SECONDS" -gt 0 ]; then
    now="$(date +%s)"
    elapsed=$((now - started_at))
    if [ "$elapsed" -ge "$TIMEOUT_SECONDS" ]; then
      echo "wait-pr: timed out after ${elapsed}s waiting for PR #$number: $url" >&2
      exit 1
    fi
  fi

  echo "wait-pr: PR #$number pending (checks_pending=$pending_count merge_state=$merge_state); sleeping ${INTERVAL_SECONDS}s"
  sleep "$INTERVAL_SECONDS"
done
