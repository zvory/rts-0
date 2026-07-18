#!/usr/bin/env bash
# Launchd-friendly entrypoint for the documentation drift daily sweep.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

max_commits="${DOC_DRIFT_MAX_COMMITS:-300}"
codex_timeout_seconds="${DOC_DRIFT_CODEX_TIMEOUT_SECONDS:-300}"
observability_dir="${DOC_DRIFT_OBSERVABILITY_DIR:-.docdrift}"
runner_worktree="${DOC_DRIFT_RUNNER_WORKTREE:-.docdrift/worktrees/docdrift-runner}"
failure_file="$observability_dir/last-failure.md"
stdout_log="$(mktemp "${TMPDIR:-/tmp}/rts-docdrift-daily-stdout.XXXXXX")"
stderr_log="$(mktemp "${TMPDIR:-/tmp}/rts-docdrift-daily-stderr.XXXXXX")"
started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

cleanup() {
  rm -f "$stdout_log" "$stderr_log"
}
trap cleanup EXIT

case "$runner_worktree" in
  /*) ;;
  *) runner_worktree="$repo_root/$runner_worktree" ;;
esac

refresh_runner_worktree() {
  git fetch origin main
  mkdir -p "$(dirname "$runner_worktree")"

  if [ -e "$runner_worktree/.git" ]; then
    top_level="$(git -C "$runner_worktree" rev-parse --show-toplevel)"
    expected_top="$(cd "$runner_worktree" && pwd -P)"
    if [ "$top_level" != "$expected_top" ]; then
      echo "docdrift runner path is not its own checkout: $runner_worktree" >&2
      return 1
    fi
    dirt="$(git -C "$runner_worktree" status --short)"
    if [ -n "$dirt" ]; then
      echo "docdrift runner worktree has uncommitted changes; recover or remove $runner_worktree" >&2
      return 1
    fi
    git -C "$runner_worktree" checkout --detach origin/main
    return 0
  fi

  if [ -e "$runner_worktree" ]; then
    echo "docdrift runner path exists but is not a git worktree: $runner_worktree" >&2
    return 1
  fi

  git worktree add --detach "$runner_worktree" origin/main
}

command=(
  node "$runner_worktree/scripts/docdrift-sweep.mjs"
  --full
  --repo "$repo_root"
  --head origin/main
  --max-commits "$max_commits"
  --codex-timeout-seconds "$codex_timeout_seconds"
  "$@"
)
command_display="$(printf "%q " "${command[@]}")"
command_display="${command_display% }"

set +e
{
  refresh_runner_worktree &&
  "${command[@]}"
} >"$stdout_log" 2>"$stderr_log"
status=$?
set -e
cat "$stdout_log"
cat "$stderr_log" >&2

finished_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

if [ "$status" -eq 0 ]; then
  rm -f "$failure_file"
  exit 0
fi

mkdir -p "$observability_dir"
{
  echo "# Documentation Drift Daily Failure"
  echo
  echo "- Started: \`$started_at\`"
  echo "- Finished: \`$finished_at\`"
  echo "- Exit code: \`$status\`"
  echo "- Working directory: \`$repo_root\`"
  echo "- Command: \`$command_display\`"
  echo
  echo "## stderr tail"
  echo
  echo '```text'
  tail -n 80 "$stderr_log" || true
  echo '```'
  echo
  echo "## stdout tail"
  echo
  echo '```text'
  tail -n 80 "$stdout_log" || true
  echo '```'
} > "$failure_file"

echo "docdrift daily failed; wrote $failure_file" >&2
exit "$status"
