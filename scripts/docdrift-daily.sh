#!/usr/bin/env bash
# Launchd-friendly entrypoint for the documentation drift daily sweep.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

max_commits="${DOC_DRIFT_MAX_COMMITS:-300}"
observability_dir="${DOC_DRIFT_OBSERVABILITY_DIR:-.docdrift}"
failure_file="$observability_dir/last-failure.md"
stdout_log="$(mktemp "${TMPDIR:-/tmp}/rts-docdrift-daily-stdout.XXXXXX")"
stderr_log="$(mktemp "${TMPDIR:-/tmp}/rts-docdrift-daily-stderr.XXXXXX")"
started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

cleanup() {
  rm -f "$stdout_log" "$stderr_log"
}
trap cleanup EXIT

command=(node scripts/docdrift-sweep.mjs --full --head origin/main --max-commits "$max_commits" "$@")
command_display="$(printf "%q " "${command[@]}")"
command_display="${command_display% }"

set +e
"${command[@]}" > >(tee "$stdout_log") 2> >(tee "$stderr_log" >&2)
status=$?
set -e

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
