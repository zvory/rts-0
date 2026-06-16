#!/usr/bin/env bash
# Validate predictable ownership metadata for zvorygin/* PRs.
set -euo pipefail

HEAD_REF="${GITHUB_HEAD_REF:-}"
BODY_FILE=""
BODY_TEXT="${RTS_PR_BODY:-}"

usage() {
  cat <<'EOF'
Usage: scripts/check-pr-ownership.sh [options]

For zvorygin/* PRs, verifies the PR body contains the rts-agent-pr:v1 metadata
block used by agent helpers. Non-agent branches pass without checks.

Options:
  --head-ref BRANCH      PR head branch. Defaults to GITHUB_HEAD_REF.
  --body-file FILE       File containing the PR body.
  -h, --help             Show this help.

When --body-file is omitted in GitHub Actions, the script reads
.pull_request.body from GITHUB_EVENT_PATH.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --head-ref) HEAD_REF="${2:?missing --head-ref value}"; shift ;;
    --body-file) BODY_FILE="${2:?missing --body-file value}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

if [ -z "$BODY_TEXT" ] && [ -n "$BODY_FILE" ]; then
  BODY_TEXT="$(cat "$BODY_FILE")"
elif [ -z "$BODY_TEXT" ] && [ -n "${GITHUB_EVENT_PATH:-}" ] && [ -f "$GITHUB_EVENT_PATH" ]; then
  BODY_TEXT="$(jq -r '.pull_request.body // ""' "$GITHUB_EVENT_PATH")"
fi

if [ -z "$HEAD_REF" ]; then
  echo "check-pr-ownership: missing PR head ref" >&2
  exit 2
fi

case "$HEAD_REF" in
  zvorygin/*) ;;
  *)
    echo "check-pr-ownership: non-agent branch $HEAD_REF; skipping"
    exit 0
    ;;
esac

missing=0
require_pattern() {
  local description="$1"
  local pattern="$2"
  if ! grep -Eq "$pattern" <<<"$BODY_TEXT"; then
    echo "check-pr-ownership: missing $description" >&2
    missing=1
  fi
}

require_pattern "metadata marker" '<!-- rts-agent-pr:v1 -->'
require_pattern "Agent-Owner field" '^Agent-Owner:[[:space:]]*[^[:space:]]'
require_pattern "Lifecycle-Mode field" '^Lifecycle-Mode:[[:space:]]*[^[:space:]]'
require_pattern "Agent-Owned true field" '^Agent-Owned:[[:space:]]*true[[:space:]]*$'
require_pattern "Auto-Merge field" '^Auto-Merge:[[:space:]]*(requested|armed|disabled-needs-human)[[:space:]]*$'
require_pattern "Focused-Verification field" '^Focused-Verification:[[:space:]]*[^[:space:]]'
require_pattern "Needs-Human field" '^Needs-Human:[[:space:]]*(true|false)[[:space:]]*$'
require_pattern "metadata end marker" '<!-- /rts-agent-pr -->'

if [ "$missing" = "1" ]; then
  echo "check-pr-ownership: zvorygin/* PRs must use scripts/agent-pr.sh or the PR template metadata block" >&2
  exit 1
fi

echo "check-pr-ownership: ownership metadata present for $HEAD_REF"
