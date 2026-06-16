#!/usr/bin/env bash
# Run executor-only phased plan passes in isolated git worktrees.
set -euo pipefail

WORKTREE_ROOT="${RTS_WORKTREE_ROOT:-/tmp/rts-worktrees}"
BASE_BRANCH="main"
DRY_RUN=0
PR_MODE=0
WAIT_FOR_PR=0
GH_BIN="${GH_BIN:-gh}"
MODEL=""
PLAN_NAME=""
FROM_PHASE=""
TO_PHASE=""
declare -a PHASES=()

usage() {
  cat <<'EOF'
Usage:
  scripts/phase-runner.sh --plan NAME PHASE [PHASE ...] [options]

Examples:
  scripts/phase-runner.sh --plan faction 4 --pr
  scripts/phase-runner.sh --plan faction 5.5 --pr
  scripts/phase-runner.sh --plan faction phase-4 phase-5 --pr --wait
  scripts/phase-runner.sh --plan faction --from 5 --to 6 --pr --wait
  scripts/phase-runner.sh --plan ai 2 --model gpt-5.4-mini --pr

Runs executor passes only. Each phase gets a separate worktree and branch under
/tmp/rts-worktrees. Each phase starts from the current local main, then the
runner pushes the completed phase branch, opens or updates an owned PR, and
arms auto-merge. With --wait, the runner waits for that PR to merge and verifies
the phase head is reachable from origin/main before reporting success or
starting the next phase.
Without --wait, the runner stops after opening and arming the first phase PR so
serial follow-up does not start from an assumed merge; treat that as a pending
handoff, not completion.
Phase ids may be numeric, decimal interstitials such as 5.5, or suffixed ids
such as 3a. Use --from/--to to discover all phase files in that interval; for
example --from 5 --to 6 runs phase-5.5 before phase-6 if both files exist.

The runner never creates plans or performs final review. It never merges or
pushes main; GitHub auto-merge and the required PR checks own that lifecycle.
Calling agents should treat the inner Codex executor as a long-running job:
wait for the command to finish, and if polling is unavoidable, poll no more than
once every 5 minutes. Do not tail the executor log during normal progress; the
runner prints the relevant tail on failure.

Options:
  --plan NAME       Plan directory name under plans/. Required.
  --base BRANCH     Must be main. Kept for compatibility with existing calls.
  --model MODEL     Optional Codex model override for executor passes.
  --from PHASE      Discover phases after PHASE, up to --to. Example: --from 5.
  --to PHASE        Discover phases through PHASE. Requires --from.
  --pr              Push the phase branch, open/update an owned PR, arm auto-merge, and stop pending merge.
  --wait            With --pr, wait for each phase PR to merge before reporting success or continuing.
  --dry-run         Print worktrees, branches, and prompts without running Codex.
  -h, --help        Show this help.

Environment:
  RTS_WORKTREE_ROOT=/tmp/rts-worktrees
  GH_BIN=gh
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --plan)
      PLAN_NAME="${2:-}"
      shift 2
      ;;
    --base)
      BASE_BRANCH="${2:-}"
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      shift 2
      ;;
    --from)
      FROM_PHASE="${2:-}"
      shift 2
      ;;
    --to)
      TO_PHASE="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --pr)
      PR_MODE=1
      shift
      ;;
    --wait)
      WAIT_FOR_PR=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --*)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      PHASES+=("$1")
      shift
      ;;
  esac
done

if [ -z "$PLAN_NAME" ]; then
  usage >&2
  exit 2
fi

if [ "$PR_MODE" != "1" ]; then
  echo "error: phase-runner is PR-first now; pass --pr, optionally with --wait" >&2
  usage >&2
  exit 2
fi

if [ "$WAIT_FOR_PR" = "1" ] && [ "$PR_MODE" != "1" ]; then
  echo "error: --wait requires --pr" >&2
  usage >&2
  exit 2
fi

if { [ -n "$FROM_PHASE" ] && [ -z "$TO_PHASE" ]; } || { [ -z "$FROM_PHASE" ] && [ -n "$TO_PHASE" ]; }; then
  echo "error: --from and --to must be used together" >&2
  usage >&2
  exit 2
fi

if [ -n "$FROM_PHASE" ] && [ "${#PHASES[@]}" -ne 0 ]; then
  echo "error: pass either explicit phases or --from/--to discovery, not both" >&2
  usage >&2
  exit 2
fi

case "$PLAN_NAME" in
  *[!a-z0-9_.-]*|*/*|.|..|"")
    echo "error: plan name must be a simple plans/ directory name: $PLAN_NAME" >&2
    exit 2
    ;;
esac

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"
git_common_dir="$(git rev-parse --path-format=absolute --git-common-dir)"

if [ "$DRY_RUN" = "0" ] && ! command -v codex >/dev/null 2>&1; then
  echo "error: codex CLI is not available on PATH" >&2
  exit 2
fi

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required to parse Codex's structured handoff" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && ! command -v "$GH_BIN" >/dev/null 2>&1; then
  echo "error: $GH_BIN is required to open and inspect PRs" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required by PR helper scripts" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && [ -n "$(git status --porcelain=v1)" ]; then
  echo "error: current checkout is dirty; start from a clean checkout before running phases" >&2
  exit 2
fi

plan_dir="$repo_root/plans/$PLAN_NAME"
plan_file="$plan_dir/plan.md"
schema_file="$repo_root/scripts/phase-runner-result.schema.json"

if [ ! -f "$plan_file" ]; then
  echo "error: missing plan entry point: $plan_file" >&2
  exit 2
fi

if [ ! -f "$schema_file" ]; then
  echo "error: missing result schema: $schema_file" >&2
  exit 2
fi

mkdir -p "$WORKTREE_ROOT"

normalize_phase() {
  local raw="$1"
  local label
  case "$raw" in
    phase-*) label="${raw#phase-}" ;;
    *) label="$raw" ;;
  esac
  if [[ ! "$label" =~ ^[0-9]+(\.[0-9]+)?[a-z]?$ ]]; then
    echo "error: invalid phase '$raw'; use N, N.M, Na, phase-N, phase-N.M, or phase-Na" >&2
    exit 2
  fi
  printf 'phase-%s\n' "$label"
}

discover_phases() {
  node -e '
    const fs = require("fs");
    const path = require("path");
    const planDir = process.argv[1];
    const from = process.argv[2];
    const to = process.argv[3];

    function parse(raw) {
      const label = String(raw || "").replace(/^phase-/, "");
      const match = /^([0-9]+)(?:\.([0-9]+))?([a-z])?$/.exec(label);
      if (!match) throw new Error(`invalid phase id: ${raw}`);
      return {
        id: `phase-${label}`,
        major: Number(match[1]),
        decimal: match[2] == null ? null : Number(`0.${match[2]}`),
        suffix: match[3] || "",
      };
    }

    function cmp(a, b) {
      return (
        a.major - b.major ||
        ((a.decimal ?? 0) - (b.decimal ?? 0)) ||
        Number(Boolean(a.suffix)) - Number(Boolean(b.suffix)) ||
        a.suffix.localeCompare(b.suffix)
      );
    }

    const fromKey = parse(from);
    const toKey = parse(to);
    if (cmp(fromKey, toKey) >= 0) {
      throw new Error(`--from must be before --to: ${from} .. ${to}`);
    }

    const phases = fs.readdirSync(planDir)
      .filter((name) => /^phase-[0-9]+(?:\.[0-9]+)?[a-z]?\.md$/.test(name))
      .map((name) => parse(path.basename(name, ".md")))
      .filter((phase) => cmp(phase, fromKey) > 0 && cmp(phase, toKey) <= 0)
      .sort(cmp)
      .map((phase) => phase.id);

    if (phases.length === 0) {
      throw new Error(`no phase files discovered after ${from} through ${to}`);
    }
    process.stdout.write(`${phases.join("\n")}\n`);
  ' "$plan_dir" "$FROM_PHASE" "$TO_PHASE"
}

json_get_status() {
  node -e '
    const fs = require("fs");
    const data = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(data.status || "");
  ' "$1"
}

json_get_verification() {
  node -e '
    const fs = require("fs");
    const data = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    const verification = Array.isArray(data.verification) ? data.verification : [];
    const text = verification.filter(Boolean).join("; ");
    process.stdout.write(text || "Focused verification not recorded by executor.");
  ' "$1"
}

write_pr_body() {
  local handoff_file="$1"
  local body_file="$2"
  node -e '
    const fs = require("fs");
    const handoff = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    const list = (items) => Array.isArray(items) && items.length
      ? items.map((item) => `- ${item}`).join("\n")
      : "- Not recorded.";
    const text = [
      "## Phase runner handoff",
      "",
      `Status: ${handoff.status || "unknown"}`,
      "",
      "### Summary",
      "",
      handoff.summary || "Not recorded.",
      "",
      "### Files changed",
      "",
      list(handoff.files_changed),
      "",
      "### Focused verification",
      "",
      list(handoff.verification),
      "",
      "### Gameplay impact",
      "",
      handoff.gameplay_impact || "Not recorded.",
      "",
      "### Next executor notes",
      "",
      handoff.next_executor_notes || "Not recorded.",
      "",
      "### Manual test notes",
      "",
      handoff.manual_test_notes || "Not recorded.",
      "",
    ].join("\n");
    fs.writeFileSync(process.argv[2], text);
  ' "$handoff_file" "$body_file"
}

get_pr_json() {
  local branch="$1"
  "$GH_BIN" pr list \
    --base "$BASE_BRANCH" \
    --head "$branch" \
    --state open \
    --limit 1 \
    --json number,url,state,headRefOid,headRefName,autoMergeRequest,mergeStateStatus,isDraft
}

json_get_pr_field() {
  local pr_json="$1"
  local field="$2"
  PR_JSON="$pr_json" FIELD="$field" node -e '
    const data = JSON.parse(process.env.PR_JSON || "[]");
    const pr = Array.isArray(data) ? data[0] : data;
    if (!pr) process.exit(0);
    const value = pr[process.env.FIELD];
    if (value == null) process.exit(0);
    if (typeof value === "object") process.stdout.write(JSON.stringify(value));
    else process.stdout.write(String(value));
  '
}

ensure_pr_ready() {
  local pr_json="$1"
  local branch="$2"
  local number
  local url
  local state
  local auto_merge
  local merge_state
  number="$(json_get_pr_field "$pr_json" number)"
  url="$(json_get_pr_field "$pr_json" url)"
  state="$(json_get_pr_field "$pr_json" state)"
  auto_merge="$(json_get_pr_field "$pr_json" autoMergeRequest)"
  merge_state="$(json_get_pr_field "$pr_json" mergeStateStatus)"

  if [ -z "$number" ]; then
    echo "phase-runner: agent-pr did not leave an open PR for $branch" >&2
    return 1
  fi
  if [ "$state" != "OPEN" ]; then
    echo "phase-runner: PR #$number is not open ($state): $url" >&2
    return 1
  fi
  if [ -z "$auto_merge" ]; then
    echo "phase-runner: PR #$number is missing auto-merge: $url" >&2
    return 1
  fi
  if [ "$merge_state" = "DIRTY" ]; then
    echo "phase-runner: PR #$number has merge conflicts: $url" >&2
    return 1
  fi
}

enrich_handoff_with_pr() {
  local handoff_file="$1"
  local pr_json="$2"
  local phase_head="$3"
  local wait_state="$4"
  PR_JSON="$pr_json" PHASE_HEAD="$phase_head" WAIT_STATE="$wait_state" node -e '
    const fs = require("fs");
    const handoffPath = process.argv[1];
    const handoff = JSON.parse(fs.readFileSync(handoffPath, "utf8"));
    const data = JSON.parse(process.env.PR_JSON || "[]");
    const pr = Array.isArray(data) ? data[0] : data;
    if (pr) {
      handoff.pr_number = pr.number ?? null;
      handoff.pr_url = pr.url ?? "";
      handoff.head_sha = process.env.PHASE_HEAD || pr.headRefOid || "";
      handoff.auto_merge_state = pr.autoMergeRequest ? "armed" : "missing";
      handoff.merge_wait_state = process.env.WAIT_STATE || "not_waited";
    }
    fs.writeFileSync(handoffPath, `${JSON.stringify(handoff, null, 2)}\n`);
  ' "$handoff_file"
}

phase_marked_done() {
  local phase_file="$1"
  node -e '
    const fs = require("fs");
    const text = fs.readFileSync(process.argv[1], "utf8");
    const singleLineStatus = /^Status:\s*Done\.?\s*$/im.test(text);
    const headingStatus = /^##\s+Status\s*\n+\s*Done\.?\s*$/im.test(text);
    const checklistStatus = /^##\s+Phase Status\s*\n+(?:[ \t]*\n)*\s*-\s*\[x\]\s*Done\.?\s*$/im.test(text);
    process.exit(singleLineStatus || headingStatus || checklistStatus ? 0 : 1);
  ' "$phase_file"
}

if [ -n "$FROM_PHASE" ]; then
  while IFS= read -r discovered_phase; do
    [ -n "$discovered_phase" ] && PHASES+=("$discovered_phase")
  done < <(discover_phases)
  echo "phase-runner: discovered phases: ${PHASES[*]}"
fi

if [ "${#PHASES[@]}" -eq 0 ]; then
  usage >&2
  exit 2
fi

if [ "$BASE_BRANCH" != "main" ]; then
  echo "error: phase-runner opens PRs against main; --base must be main" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && [ "$(git branch --show-current)" != "$BASE_BRANCH" ]; then
  echo "error: start phase-runner from the local $BASE_BRANCH checkout so each phase starts from main" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && ! git remote get-url origin >/dev/null 2>&1; then
  echo "error: origin remote is required because phase-runner pushes phase branches" >&2
  exit 2
fi

sync_main() {
  git fetch origin "$BASE_BRANCH"
  git merge --ff-only "origin/$BASE_BRANCH"
}

for raw_phase in "${PHASES[@]}"; do
  phase_id="$(normalize_phase "$raw_phase")"
  phase_file="$plan_dir/$phase_id.md"

  if [ ! -f "$phase_file" ]; then
    echo "error: missing phase file: $phase_file" >&2
    exit 2
  fi

  branch="zvorygin/$PLAN_NAME-$phase_id"
  worktree_path="$WORKTREE_ROOT/$PLAN_NAME-$phase_id"
  log_dir="$WORKTREE_ROOT/phase-runner-logs/$PLAN_NAME"
  handoff_dir="$log_dir/handoffs"
  handoff_file="$handoff_dir/$phase_id.json"
  pr_body_file="$log_dir/$phase_id.pr-body.md"
  codex_log="$log_dir/$phase_id.codex.log"
  timing_file="$log_dir/$phase_id.timing.json"
  active_marker_dir="$WORKTREE_ROOT/phase-runner-active"
  active_marker="$active_marker_dir/${branch//\//__}"

  if [ "$DRY_RUN" = "0" ] && git show-ref --verify --quiet "refs/heads/$branch"; then
    echo "error: branch already exists: $branch" >&2
    exit 2
  fi

  if [ "$DRY_RUN" = "0" ] && [ -e "$worktree_path" ]; then
    echo "error: worktree path already exists: $worktree_path" >&2
    exit 2
  fi

  if [ "$DRY_RUN" = "0" ]; then
    echo "phase-runner: syncing local $BASE_BRANCH from origin/$BASE_BRANCH before $phase_id"
    sync_main
  fi
  phase_base_ref="$BASE_BRANCH"
  phase_base_commit="$(git rev-parse "$phase_base_ref")"

  prompt="$(
    cat <<EOF
\$phase-runner

Execute exactly one planned phase in this RTS repository.

Plan: plans/$PLAN_NAME/plan.md
Phase: plans/$PLAN_NAME/$phase_id.md
Current branch: $branch

This is an executor pass only:
- You are already running inside the assigned clean worktree for this phase. This satisfies the
  repository worktree requirement; do not create another worktree or switch to another checkout.
- Do not create or revise the overall plan.
- Do not run a final review pass.
- Do not merge, push, or open a PR; the outer phase runner handles branch push and PR automation after you commit.
- Implement only this phase.
- Stage and commit only files belonging to this phase.
- The phase is not completed until your task changes are committed successfully on $branch.
- Mark plans/$PLAN_NAME/$phase_id.md done if and only if the phase is committed successfully.
- Run the smallest targeted verification appropriate for the changed files.
- Commit with the normal git commit hook. Do not run the broad full local gate unless the phase
  explicitly requires it; GitHub Actions is the authoritative full gate after the PR opens.
- If the commit hook fails, do not return completed. Inspect the failure, keep working, run focused
  checks, and retry the commit until it succeeds.
- You may commit with --no-verify only for pure documentation changes or when you have conclusively
  confirmed the only failing hook check is unrelated to this phase. Document that evidence in the
  JSON handoff verification or notes.
- Avoid broad formatting commands such as workspace-wide cargo fmt unless they are required for the
  phase diff. If formatting is needed, keep any formatter drift outside the phase scope out of the
  final diff.
- Prefer plain filesystem renames/moves over git mv inside this sandboxed executor session.
- If the phase is ambiguous, too broad, blocked by failing verification or commit-hook failure you
  cannot repair, or needs human design/product input, stop and report status "blocked".
- Include focused verification, next-step notes, and manual-test notes detailed enough for the
  outer phase runner to write an owned PR body.

Return a compact JSON handoff matching the requested schema.
EOF
  )"

  phase_start=$SECONDS
  echo "phase-runner: creating $worktree_path from $phase_base_ref ($phase_base_commit) on $branch"
  if [ "$DRY_RUN" = "0" ]; then
    git worktree add "$worktree_path" -b "$branch" "$phase_base_ref"
    mkdir -p "$active_marker_dir"
    printf 'plan=%s\nphase=%s\nbranch=%s\nworktree=%s\n' "$PLAN_NAME" "$phase_id" "$branch" "$worktree_path" >"$active_marker"
    mkdir -p "$handoff_dir"
    mkdir -p "$log_dir"
  fi

  if [ "$DRY_RUN" = "1" ]; then
    echo "phase-runner: would run Codex in $worktree_path"
    echo "phase-runner: would push $branch to origin"
    echo "phase-runner: would run scripts/agent-pr.sh --base $BASE_BRANCH --head $branch --verification <executor verification>"
    if [ "$WAIT_FOR_PR" = "1" ]; then
      echo "phase-runner: would run scripts/wait-pr.sh <opened-pr> before reporting success or continuing"
      echo "phase-runner: would fetch origin/$BASE_BRANCH and verify the phase head is reachable from origin/$BASE_BRANCH"
    else
      echo "phase-runner: would stop with a pending handoff after arming auto-merge for $branch"
    fi
    printf '%s\n' "$prompt"
    if [ "$WAIT_FOR_PR" != "1" ]; then
      break
    fi
    continue
  fi

  codex_args=(
    exec
    --cd "$worktree_path"
    --add-dir "$git_common_dir"
    --sandbox workspace-write
    --output-schema "$schema_file"
    --output-last-message "$handoff_file"
  )
  if [ -n "$MODEL" ]; then
    codex_args+=(--model "$MODEL")
  fi
  codex_args+=("$prompt")

  echo "phase-runner: running Codex executor for $phase_id (log: $codex_log)"
  echo "phase-runner: inner executor may run for 10-20 minutes; calling agents should wait and poll no more than once every 5 minutes"
  executor_start=$SECONDS
  if ! codex "${codex_args[@]}" >"$codex_log" 2>&1; then
    echo "phase-runner: Codex failed for $phase_id; leaving worktree at $worktree_path" >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi
  executor_seconds=$((SECONDS - executor_start))

  status="$(json_get_status "$handoff_file")"
  if [ "$status" != "completed" ]; then
    echo "phase-runner: $phase_id reported status '$status'; leaving worktree for inspection: $worktree_path" >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi
  rm -f "$active_marker"

  if [ -n "$(git -C "$worktree_path" status --porcelain=v1)" ]; then
    echo "phase-runner: $phase_id reported completed but left uncommitted changes; leaving worktree for inspection: $worktree_path" >&2
    git -C "$worktree_path" status --short >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi

  if [ "$(git -C "$worktree_path" rev-list --count "$phase_base_commit..HEAD")" -eq 0 ]; then
    echo "phase-runner: $phase_id reported completed but created no commit over $phase_base_commit; leaving worktree for inspection: $worktree_path" >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi

  if ! phase_marked_done "$worktree_path/plans/$PLAN_NAME/$phase_id.md"; then
    echo "phase-runner: $phase_id reported completed but did not mark the phase document done" >&2
    exit 1
  fi

  phase_head="$(git -C "$worktree_path" rev-parse HEAD)"
  echo "phase-runner: executor committed $branch at $phase_head"
  echo "phase-runner: pushing $branch to origin"
  git -C "$worktree_path" push -u origin "$branch"

  verification_summary="$(json_get_verification "$handoff_file")"
  write_pr_body "$handoff_file" "$pr_body_file"
  echo "phase-runner: opening/updating owned PR for $branch"
  (
    cd "$worktree_path"
    GH_BIN="$GH_BIN" scripts/agent-pr.sh \
      --base "$BASE_BRANCH" \
      --head "$branch" \
      --verification "$verification_summary" \
      --body-file "$pr_body_file"
  )

  pr_json="$(get_pr_json "$branch")"
  pr_number="$(json_get_pr_field "$pr_json" number)"
  pr_url="$(json_get_pr_field "$pr_json" url)"
  if ! ensure_pr_ready "$pr_json" "$branch"; then
    enrich_handoff_with_pr "$handoff_file" "$pr_json" "$phase_head" "blocked"
    echo "phase-runner: PR lifecycle blocked; leaving worktree for repair: $worktree_path" >&2
    exit 1
  fi
  echo "phase-runner: PR #$pr_number armed for auto-merge: $pr_url"

  merge_wait_state="not_waited"
  if [ "$WAIT_FOR_PR" = "1" ]; then
    echo "phase-runner: waiting for PR #$pr_number to merge before continuing"
    if ! (cd "$worktree_path" && GH_BIN="$GH_BIN" scripts/wait-pr.sh "$pr_url"); then
      echo "phase-runner: PR #$pr_number did not reach a merged state; leaving worktree for repair: $worktree_path" >&2
      enrich_handoff_with_pr "$handoff_file" "$pr_json" "$phase_head" "blocked"
      exit 1
    fi
    git fetch origin "$BASE_BRANCH"
    if ! git merge-base --is-ancestor "$phase_head" "origin/$BASE_BRANCH"; then
      echo "phase-runner: PR #$pr_number merged, but $phase_head is not reachable from origin/$BASE_BRANCH" >&2
      enrich_handoff_with_pr "$handoff_file" "$pr_json" "$phase_head" "blocked"
      exit 1
    fi
    sync_main
    merge_wait_state="merged"
    echo "phase-runner: PR #$pr_number merged and $phase_head is reachable from origin/$BASE_BRANCH"
  fi

  enrich_handoff_with_pr "$handoff_file" "$pr_json" "$phase_head" "$merge_wait_state"
  echo "phase-runner: $phase_id PR lifecycle recorded in $handoff_file"
  total_seconds=$((SECONDS - phase_start))
  PLAN_NAME="$PLAN_NAME" \
  PHASE_ID="$phase_id" \
  BRANCH="$branch" \
  BASE_REF="$phase_base_commit" \
  PHASE_HEAD="$phase_head" \
  PR_NUMBER="$pr_number" \
  PR_URL="$pr_url" \
  AUTO_MERGE_STATE="armed" \
  MERGE_WAIT_STATE="$merge_wait_state" \
  WORKTREE="$worktree_path" \
  CODEX_LOG="$codex_log" \
  EXECUTOR_SECONDS="$executor_seconds" \
  TOTAL_SECONDS="$total_seconds" \
  TIMING_FILE="$timing_file" \
  node -e '
    const fs = require("fs");
    const payload = {
      plan: process.env.PLAN_NAME,
      phase: process.env.PHASE_ID,
      branch: process.env.BRANCH,
      baseRef: process.env.BASE_REF,
      phaseHead: process.env.PHASE_HEAD,
      pr: {
        number: Number(process.env.PR_NUMBER),
        url: process.env.PR_URL,
        autoMergeState: process.env.AUTO_MERGE_STATE,
        mergeWaitState: process.env.MERGE_WAIT_STATE,
      },
      worktree: process.env.WORKTREE,
      codexLog: process.env.CODEX_LOG,
      timingsSeconds: {
        executor: Number(process.env.EXECUTOR_SECONDS),
        total: Number(process.env.TOTAL_SECONDS),
      },
    };
    fs.writeFileSync(process.env.TIMING_FILE, `${JSON.stringify(payload, null, 2)}\n`);
  '
  echo "phase-runner: timing saved to $timing_file (${total_seconds}s total)"

  if [ "$WAIT_FOR_PR" != "1" ]; then
    echo "phase-runner: stopped with a pending handoff after opening and arming PR #$pr_number because --wait was not set"
    break
  fi
done

if [ "$DRY_RUN" = "1" ]; then
  echo "phase-runner: dry run finished. No worktrees were created and no PRs were opened."
else
  if [ "$WAIT_FOR_PR" = "1" ]; then
    echo "phase-runner: finished executor passes. Each completed phase PR merged before the next phase started."
  else
    echo "phase-runner: finished with a pending handoff after arming the first phase PR. Run scripts/wait-pr.sh before claiming completion or starting follow-up work."
  fi
fi
