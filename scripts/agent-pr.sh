#!/usr/bin/env bash
# Open or update an agent-owned PR and arm auto-merge.
set -euo pipefail

GH_BIN="${GH_BIN:-gh}"
BASE_BRANCH="main"
HEAD_BRANCH=""
TITLE=""
OWNER=""
LIFECYCLE_MODE="normal"
FOCUSED_VERIFICATION=""
BODY_FILE=""
EXTRA_BODY=""
EXTRA_LABELS=()
AUTO_MERGE=1
DRY_RUN=0
DRAFT_FLAG=0

usage() {
  cat <<'EOF'
Usage: scripts/agent-pr.sh [options]

Opens or updates the PR for the current agent branch, writes predictable
ownership metadata into the body, applies agent labels, and arms auto-merge.

Options:
  --base BRANCH              Base branch, default: main.
  --head BRANCH              Head branch, default: current branch.
  --title TITLE              PR title, default: last commit subject.
  --owner OWNER              Agent/user owning the PR, default: gh user or git user.
  --lifecycle MODE           Lifecycle mode, default: normal.
  --verification TEXT        Focused local verification summary.
  --body-file FILE           Extra body text to append after ownership metadata.
  --label LABEL              Extra PR label to apply. Repeatable.
  --draft                    Create the PR as a draft when opening it.
  --no-auto-merge            Do not arm auto-merge; marks the PR needs-human.
  --dry-run                  Print actions without changing GitHub state.
  -h, --help                 Show this help.

Expected agent labels:
  agent-owned, automerge, ci-failed, needs-human, docdrift-sweep
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --base) BASE_BRANCH="${2:?missing --base value}"; shift ;;
    --head) HEAD_BRANCH="${2:?missing --head value}"; shift ;;
    --title) TITLE="${2:?missing --title value}"; shift ;;
    --owner) OWNER="${2:?missing --owner value}"; shift ;;
    --lifecycle) LIFECYCLE_MODE="${2:?missing --lifecycle value}"; shift ;;
    --verification) FOCUSED_VERIFICATION="${2:?missing --verification value}"; shift ;;
    --body-file) BODY_FILE="${2:?missing --body-file value}"; shift ;;
    --label) EXTRA_LABELS+=("${2:?missing --label value}"); shift ;;
    --draft) DRAFT_FLAG=1 ;;
    --no-auto-merge) AUTO_MERGE=0 ;;
    --dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [ -z "$HEAD_BRANCH" ]; then
  HEAD_BRANCH="$(git branch --show-current)"
fi
if [ -z "$HEAD_BRANCH" ]; then
  echo "agent-pr: could not determine head branch" >&2
  exit 2
fi
case "$HEAD_BRANCH" in
  zvorygin/*) ;;
  *)
    echo "agent-pr: head branch must start with zvorygin/: $HEAD_BRANCH" >&2
    exit 2
    ;;
esac

if [ -z "$TITLE" ]; then
  TITLE="$(git log -1 --format=%s)"
fi
if [ -z "$OWNER" ]; then
  OWNER="$("$GH_BIN" api user --jq .login 2>/dev/null || true)"
fi
if [ -z "$OWNER" ]; then
  OWNER="$(git config user.name || true)"
fi
if [ -z "$OWNER" ]; then
  OWNER="unknown"
fi
if [ -z "$FOCUSED_VERIFICATION" ]; then
  FOCUSED_VERIFICATION="Not recorded by helper caller."
fi
if [ -n "$BODY_FILE" ]; then
  EXTRA_BODY="$(cat "$BODY_FILE")"
fi

tmp_body="$(mktemp -t rts-agent-pr.XXXXXX)"
trap 'rm -f "$tmp_body"' EXIT

needs_human="false"
auto_merge_text="requested"
if [ "$AUTO_MERGE" = "0" ]; then
  needs_human="true"
  auto_merge_text="disabled-needs-human"
fi

cat >"$tmp_body" <<EOF
<!-- rts-agent-pr:v1 -->
Agent-Owner: $OWNER
Lifecycle-Mode: $LIFECYCLE_MODE
Agent-Owned: true
Auto-Merge: $auto_merge_text
Focused-Verification: $FOCUSED_VERIFICATION
Needs-Human: $needs_human
<!-- /rts-agent-pr -->

$EXTRA_BODY
EOF

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf 'agent-pr: would run:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

ensure_label() {
  local name="$1"
  local color="$2"
  local description="$3"
  if [ "$DRY_RUN" = "1" ]; then
    echo "agent-pr: would ensure label $name"
    return
  fi
  "$GH_BIN" label create "$name" --color "$color" --description "$description" >/dev/null 2>&1 || true
}

ensure_label "agent-owned" "0E8A16" "Owned by an automated agent with PR body metadata"
ensure_label "automerge" "5319E7" "Auto-merge should be armed when checks pass"
ensure_label "ci-failed" "D73A4A" "CI failed and needs an agent or human decision"
ensure_label "needs-human" "FBCA04" "Human review or decision is required before merge"
ensure_label "docdrift-sweep" "1D76DB" "Generated documentation drift sweep PR"

existing_pr_json=""
if [ "$DRY_RUN" != "1" ]; then
  existing_pr_json="$("$GH_BIN" pr list \
    --base "$BASE_BRANCH" \
    --head "$HEAD_BRANCH" \
    --state open \
    --json number,url \
    --jq '.[0] // empty')"
fi

label_args=(--add-label agent-owned)
if [ "$AUTO_MERGE" = "1" ]; then
  label_args+=(--add-label automerge)
else
  label_args+=(--add-label needs-human)
fi
if [ "${#EXTRA_LABELS[@]}" -gt 0 ]; then
  for extra_label in "${EXTRA_LABELS[@]}"; do
    label_args+=(--add-label "$extra_label")
  done
fi

pr_number=""
pr_url=""
if [ -n "$existing_pr_json" ]; then
  pr_number="$(jq -r '.number' <<<"$existing_pr_json")"
  pr_url="$(jq -r '.url' <<<"$existing_pr_json")"
  run "$GH_BIN" pr edit "$pr_number" --title "$TITLE" --body-file "$tmp_body" "${label_args[@]}"
else
  create_args=(pr create --base "$BASE_BRANCH" --head "$HEAD_BRANCH" --title "$TITLE" --body-file "$tmp_body" --label agent-owned)
  if [ "$AUTO_MERGE" = "1" ]; then
    create_args+=(--label automerge)
  else
    create_args+=(--label needs-human)
  fi
  if [ "${#EXTRA_LABELS[@]}" -gt 0 ]; then
    for extra_label in "${EXTRA_LABELS[@]}"; do
      create_args+=(--label "$extra_label")
    done
  fi
  if [ "$DRAFT_FLAG" = "1" ]; then
    create_args+=(--draft)
  fi
  if [ "$DRY_RUN" = "1" ]; then
    run "$GH_BIN" "${create_args[@]}"
  else
    pr_url="$("$GH_BIN" "${create_args[@]}")"
    pr_number="${pr_url##*/}"
  fi
fi

if [ "$AUTO_MERGE" = "1" ]; then
  if [ "$DRY_RUN" = "1" ]; then
    run "$GH_BIN" pr merge "<opened-or-existing-pr>" --auto --merge
  else
    run "$GH_BIN" pr merge "$pr_number" --auto --merge
  fi
fi

if [ "$DRY_RUN" = "1" ]; then
  echo "agent-pr: dry run complete for $HEAD_BRANCH -> $BASE_BRANCH"
else
  echo "agent-pr: PR $pr_number ready: $pr_url"
fi
