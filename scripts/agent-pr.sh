#!/usr/bin/env bash
# Run the final quality pass, then open or update an agent-owned PR and arm auto-merge.
set -euo pipefail

if [ "${RTS_ADVERSARIAL_QUALITY_PASS:-}" = "1" ]; then
  echo "agent-pr: refusing to run inside adversarial quality pass; the outer helper owns PR lifecycle" >&2
  exit 2
fi

if [ "${RTS_AGENT_PR_STABLE_COPY:-0}" != "1" ]; then
  stable_copy="$(mktemp -t rts-agent-pr-stable.XXXXXX)"
  cp "${BASH_SOURCE[0]}" "$stable_copy"
  chmod +x "$stable_copy"
  RTS_AGENT_PR_STABLE_COPY=1 RTS_AGENT_PR_STABLE_COPY_PATH="$stable_copy" exec bash "$stable_copy" "$@"
fi

STABLE_COPY_PATH="${RTS_AGENT_PR_STABLE_COPY_PATH:-}"
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
QUALITY_CONTEXT="adversarial-quality-pass"
CHANGED_FILES=()

usage() {
  cat <<'EOF'
Usage: scripts/agent-pr.sh [options]

Archives any plan newly completed by this branch, runs the adversarial quality
pass, opens or updates the PR for the current agent branch, writes predictable
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

CURRENT_BRANCH="$(git branch --show-current)"
if [ -z "$CURRENT_BRANCH" ]; then
  echo "agent-pr: could not determine current branch; detached HEAD is not supported" >&2
  exit 2
fi
if [ -z "$HEAD_BRANCH" ]; then
  HEAD_BRANCH="$CURRENT_BRANCH"
elif [ "$HEAD_BRANCH" != "$CURRENT_BRANCH" ]; then
  echo "agent-pr: head branch mismatch: current branch is '$CURRENT_BRANCH', but --head was '$HEAD_BRANCH'" >&2
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

quality_report_json="$(mktemp -t rts-adversarial-quality-pass.XXXXXX.json)"
quality_report_md="$(mktemp -t rts-adversarial-quality-pass.XXXXXX.md)"
tmp_body="$(mktemp -t rts-agent-pr.XXXXXX)"

cleanup() {
  rm -f "$quality_report_json" "$quality_report_md" "$tmp_body"
  if [ -n "$STABLE_COPY_PATH" ]; then
    rm -f "$STABLE_COPY_PATH"
  fi
}
trap cleanup EXIT

collect_changed_files() {
  CHANGED_FILES=()
  while IFS= read -r path; do
    [ -n "$path" ] && CHANGED_FILES+=("$path")
  done < <(git diff --name-only --no-renames "origin/$BASE_BRANCH...HEAD")
}

is_docs_only_change() {
  collect_changed_files
  if [ "${#CHANGED_FILES[@]}" -eq 0 ]; then
    return 1
  fi

  local policy_output
  local docs_only
  policy_output="$(node tests/select-suites.mjs --ci-policy "${CHANGED_FILES[@]}")"
  docs_only="false"
  while IFS='=' read -r key value; do
    if [ "$key" = "docs_only" ]; then
      docs_only="$value"
    fi
  done <<<"$policy_output"

  [ "$docs_only" = "true" ]
}

write_docs_only_quality_report() {
  cat >"$quality_report_md" <<'EOF'
## Adversarial quality pass

Verdict: skipped_docs_only

### Summary

Skipped Codex adversarial review because this branch changes only Markdown documentation files.

### Issues found

- None. Review was intentionally skipped for docs-only changes.

### Changes made

- None.

### Verification

- `tests/select-suites.mjs --ci-policy` classified this branch as `docs_only=true`.

### Remaining concerns

- None.

EOF
}

post_docs_only_status() {
  local final_head
  final_head="$(git rev-parse HEAD)"
  "$GH_BIN" api \
    -X POST \
    "repos/:owner/:repo/statuses/$final_head" \
    -f state=success \
    -f "context=$QUALITY_CONTEXT" \
    -f "description=skipped for docs-only changes"
}

archive_completed_plans() {
  if [ "$DRY_RUN" = "1" ]; then
    if git rev-parse --verify "origin/$BASE_BRANCH" >/dev/null 2>&1; then
      node scripts/archive-completed-plans.mjs --base "origin/$BASE_BRANCH" --dry-run
    else
      echo "agent-pr: would check for plans completed relative to origin/$BASE_BRANCH"
    fi
    return
  fi

  local status
  status="$(git status --porcelain=v1)"
  if [ -n "$status" ]; then
    echo "agent-pr: completed-plan archival requires a clean worktree before starting:" >&2
    printf '%s\n' "$status" >&2
    exit 1
  fi

  git fetch origin "+refs/heads/$BASE_BRANCH:refs/remotes/origin/$BASE_BRANCH"
  node scripts/archive-completed-plans.mjs --base "origin/$BASE_BRANCH" --commit
}

run_quality_pass() {
  if [ "$DRY_RUN" = "1" ]; then
    if git rev-parse --verify "origin/$BASE_BRANCH" >/dev/null 2>&1 && is_docs_only_change; then
      echo "agent-pr: would skip scripts/adversarial-quality-pass.mjs for docs-only branch $HEAD_BRANCH"
    else
      echo "agent-pr: would run scripts/adversarial-quality-pass.mjs for $HEAD_BRANCH"
    fi
    return
  fi

  local status
  status="$(git status --porcelain=v1)"
  if [ -n "$status" ]; then
    echo "agent-pr: quality pass requires a clean worktree before starting:" >&2
    printf '%s\n' "$status" >&2
    exit 1
  fi

  if is_docs_only_change; then
    echo "agent-pr: skipping adversarial quality pass for docs-only branch $HEAD_BRANCH"
    write_docs_only_quality_report
    git push -u origin "HEAD:refs/heads/$HEAD_BRANCH"
    post_docs_only_status
    return
  fi

  scripts/adversarial-quality-pass.mjs \
    --base "origin/$BASE_BRANCH" \
    --head-branch "$HEAD_BRANCH" \
    --report-file "$quality_report_json" \
    --markdown-report-file "$quality_report_md" \
    --gh-bin "$GH_BIN" \
    --push \
    --post-status

  local refreshed_branch
  refreshed_branch="$(git branch --show-current)"
  if [ "$refreshed_branch" != "$HEAD_BRANCH" ]; then
    echo "agent-pr: quality pass left checkout on unexpected branch '$refreshed_branch' (expected '$HEAD_BRANCH')" >&2
    exit 1
  fi
}

archive_completed_plans
run_quality_pass

needs_human="false"
auto_merge_text="requested"
if [ "$AUTO_MERGE" = "0" ]; then
  needs_human="true"
  auto_merge_text="disabled-needs-human"
fi

{
  cat <<EOF
<!-- rts-agent-pr:v1 -->
Agent-Owner: $OWNER
Lifecycle-Mode: $LIFECYCLE_MODE
Agent-Owned: true
Auto-Merge: $auto_merge_text
Focused-Verification: $FOCUSED_VERIFICATION
Needs-Human: $needs_human
<!-- /rts-agent-pr -->

EOF

  if [ -s "$quality_report_md" ]; then
    cat "$quality_report_md"
    printf '\n'
  fi

  if [ -n "$EXTRA_BODY" ]; then
    printf '%s\n' "$EXTRA_BODY"
  fi
} >"$tmp_body"

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
