#!/usr/bin/env bash
# Summarize open agent-owned PRs and flag stalled PR lifecycle states.
set -euo pipefail

GH_BIN="${GH_BIN:-gh}"
STALE_HOURS="${RTS_PR_SWEEP_STALE_HOURS:-24}"
JSON_OUTPUT=0

usage() {
  cat <<'EOF'
Usage: scripts/pr-sweep.sh [options]

Lists open agent-owned PRs, including zvorygin/* PRs that are missing ownership
metadata. Flags missing auto-merge, stale PRs, failed CI, conflicts, missing
owner metadata, and needs-human state.

Options:
  --json                 Emit machine-readable JSON instead of a table.
  --stale-hours HOURS    Mark PRs older than this threshold stale, default: 24.
  -h, --help             Show this help.

Test fixtures:
  RTS_PR_SWEEP_JSON      JSON array returned instead of `gh pr list`.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --json) JSON_OUTPUT=1 ;;
    --stale-hours) STALE_HOURS="${2:?missing --stale-hours value}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [ -n "${RTS_PR_SWEEP_JSON:-}" ]; then
  pr_json="$RTS_PR_SWEEP_JSON"
else
  pr_json="$("$GH_BIN" pr list \
    --state open \
    --limit 100 \
    --json number,title,url,headRefName,headRefOid,author,createdAt,labels,autoMergeRequest,mergeStateStatus,isDraft,reviewDecision,statusCheckRollup,body)"
fi

now_epoch="$(date +%s)"

sweep_json="$(
  jq --argjson now "$now_epoch" --argjson stale_hours "$STALE_HOURS" '
    def label_names: [.labels[]?.name];
    def has_label($name): (label_names | index($name)) != null;
    def body_field($name):
      (.body // "" | capture("(?m)^" + $name + ":[[:space:]]*(?<value>[^\\n]+)")?.value // "");
    def check_state:
      if ([.statusCheckRollup[]? | select(
          ((.conclusion // "") | ascii_upcase) as $c
          | ($c == "FAILURE" or $c == "CANCELLED" or $c == "TIMED_OUT" or $c == "ACTION_REQUIRED")
        )] | length) > 0 then "failed"
      elif ([.statusCheckRollup[]? | select(
          ((.status // "") | ascii_upcase) as $s
          | ((.conclusion // "") == "" and $s != "COMPLETED")
        )] | length) > 0 then "pending"
      elif (.statusCheckRollup | length) == 0 then "none"
      else "passing"
      end;
    def age_hours:
      ((($now - ((.createdAt | fromdateiso8601) // $now)) / 3600) | floor);
    [
      .[]
      | select((.headRefName // "" | startswith("zvorygin/")) or has_label("agent-owned"))
      | . as $pr
      | (body_field("Agent-Owner")) as $owner
      | (body_field("Needs-Human") | ascii_downcase) as $needs_human
      | (check_state) as $checks
      | (age_hours) as $age
      | {
          number,
          title,
          url,
          owner: (if $owner == "" then "missing" else $owner end),
          age_hours: $age,
          head: .headRefName,
          head_sha: .headRefOid,
          auto_merge: (if .autoMergeRequest == null then "missing" else "armed" end),
          checks: $checks,
          merge_state: (.mergeStateStatus // "unknown"),
          flags: ([
            (if $owner == "" then "missing-owner" else empty end),
            (if .autoMergeRequest == null then "auto-merge-missing" else empty end),
            (if $checks == "failed" then "ci-failed" else empty end),
            (if ($needs_human == "true" or has_label("needs-human")) then "needs-human" else empty end),
            (if ((.mergeStateStatus // "") | ascii_upcase) == "DIRTY" then "conflicted" else empty end),
            (if $age >= $stale_hours then "stale" else empty end)
          ])
        }
    ]
  ' <<<"$pr_json"
)"

if [ "$JSON_OUTPUT" = "1" ]; then
  printf '%s\n' "$sweep_json"
  exit 0
fi

printf '%-6s %-14s %-6s %-24s %-10s %-18s %s\n' "PR" "OWNER" "AGE_H" "HEAD" "AUTO" "CHECKS" "FLAGS"
jq -r '
  .[]
  | [
      ("#" + (.number | tostring)),
      .owner,
      (.age_hours | tostring),
      .head,
      .auto_merge,
      .checks,
      (if (.flags | length) == 0 then "-" else (.flags | join(",")) end)
    ]
  | @tsv
' <<<"$sweep_json" | while IFS=$'\t' read -r number owner age head auto_merge checks flags; do
  printf '%-6s %-14s %-6s %-24s %-10s %-18s %s\n' \
    "$number" "$owner" "$age" "$head" "$auto_merge" "$checks" "$flags"
done
