#!/usr/bin/env bash
# Run executor-only phased plan passes in isolated git worktrees.
set -euo pipefail

WORKTREE_ROOT="${RTS_WORKTREE_ROOT:-/tmp/rts-worktrees}"
BASE_BRANCH="main"
DRY_RUN=0
MODEL=""
PLAN_NAME=""
declare -a PHASES=()

usage() {
  cat <<'EOF'
Usage:
  scripts/phase-runner.sh --plan NAME PHASE [PHASE ...] [options]

Examples:
  scripts/phase-runner.sh --plan faction 4
  scripts/phase-runner.sh --plan faction phase-4 phase-5 --base main
  scripts/phase-runner.sh --plan ai 2 --model gpt-5.4-mini

Runs executor passes only. Each phase gets a separate worktree and branch under
/tmp/rts-worktrees. When multiple phases are provided, each later phase starts
from the previous phase branch so the final branch contains the accumulated work.

The runner never merges, pushes, creates plans, or performs final review.
Calling agents should treat the inner Codex executor as a long-running job:
wait for the command to finish, and if polling is unavoidable, poll no more than
once every 5 minutes. Do not tail the executor log during normal progress; the
runner prints the relevant tail on failure.

Options:
  --plan NAME       Plan directory name under plans/. Required.
  --base BRANCH     Starting branch for the first phase. Defaults to main.
  --model MODEL     Optional Codex model override for executor passes.
  --dry-run         Print worktrees, branches, and prompts without running Codex.
  -h, --help        Show this help.

Environment:
  RTS_WORKTREE_ROOT=/tmp/rts-worktrees
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
    --dry-run)
      DRY_RUN=1
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

if [ -z "$PLAN_NAME" ] || [ "${#PHASES[@]}" -eq 0 ]; then
  usage >&2
  exit 2
fi

case "$PLAN_NAME" in
  *[!a-z0-9_-]*|"")
    echo "error: plan name must be a simple plans/ directory name: $PLAN_NAME" >&2
    exit 2
    ;;
esac

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if ! command -v codex >/dev/null 2>&1; then
  echo "error: codex CLI is not available on PATH" >&2
  exit 2
fi

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required to parse Codex's structured handoff" >&2
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
  case "$raw" in
    phase-*) printf '%s\n' "$raw" ;;
    [0-9]*) printf 'phase-%s\n' "$raw" ;;
    *)
      echo "error: invalid phase '$raw'; use N or phase-N" >&2
      exit 2
      ;;
  esac
}

json_get_status() {
  node -e '
    const fs = require("fs");
    const data = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(data.status || "");
  ' "$1"
}

phase_marked_done() {
  local phase_file="$1"
  node -e '
    const fs = require("fs");
    const text = fs.readFileSync(process.argv[1], "utf8");
    process.exit(/^Status:\s*Done\.?\s*$/im.test(text) ? 0 : 1);
  ' "$phase_file"
}

previous_ref="$BASE_BRANCH"

for raw_phase in "${PHASES[@]}"; do
  phase_id="$(normalize_phase "$raw_phase")"
  phase_file="$plan_dir/$phase_id.md"

  if [ ! -f "$phase_file" ]; then
    echo "error: missing phase file: $phase_file" >&2
    exit 2
  fi

  branch="zvorygin/$PLAN_NAME-$phase_id"
  worktree_path="$WORKTREE_ROOT/$PLAN_NAME-$phase_id"
  handoff_dir="$worktree_path/plans/$PLAN_NAME/handoffs"
  handoff_file="$handoff_dir/$phase_id.json"
  log_dir="$WORKTREE_ROOT/phase-runner-logs/$PLAN_NAME"
  codex_log="$log_dir/$phase_id.codex.log"
  timing_file="$log_dir/$phase_id.timing.json"

  if git show-ref --verify --quiet "refs/heads/$branch"; then
    echo "error: branch already exists: $branch" >&2
    exit 2
  fi

  if [ -e "$worktree_path" ]; then
    echo "error: worktree path already exists: $worktree_path" >&2
    exit 2
  fi

  prompt="$(
    cat <<EOF
\$phase-runner

Execute exactly one planned phase in this RTS repository.

Plan: plans/$PLAN_NAME/plan.md
Phase: plans/$PLAN_NAME/$phase_id.md
Current branch: $branch

This is an executor pass only:
- Do not create or revise the overall plan.
- Do not run a final review pass.
- Do not merge, push, or open a PR.
- Implement only this phase.
- Stage and commit only files belonging to this phase.
- The phase is not completed until your task changes are committed successfully on $branch.
- Mark plans/$PLAN_NAME/$phase_id.md done if and only if the phase is committed successfully.
- Run the smallest targeted verification appropriate for the changed files.
- Commit with the normal git commit hook and let it run the full local test gate. Do not duplicate
  broad full-suite verification inside the executor pass before committing.
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

Return a compact JSON handoff matching the requested schema.
EOF
  )"

  phase_start=$SECONDS
  echo "phase-runner: creating $worktree_path from $previous_ref on $branch"
  if [ "$DRY_RUN" = "0" ]; then
    git worktree add "$worktree_path" -b "$branch" "$previous_ref"
    mkdir -p "$handoff_dir"
    mkdir -p "$log_dir"
  fi

  if [ "$DRY_RUN" = "1" ]; then
    echo "phase-runner: would run Codex in $worktree_path"
    printf '%s\n' "$prompt"
    previous_ref="$branch"
    continue
  fi

  codex_args=(
    exec
    --cd "$worktree_path"
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

  status="$(json_get_status "$handoff_file")"
  if [ "$status" != "completed" ]; then
    echo "phase-runner: $phase_id reported status '$status'; leaving worktree for inspection: $worktree_path" >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi

  if [ -n "$(git -C "$worktree_path" status --porcelain=v1)" ]; then
    echo "phase-runner: $phase_id reported completed but left uncommitted changes; leaving worktree for inspection: $worktree_path" >&2
    git -C "$worktree_path" status --short >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi

  if [ "$(git -C "$worktree_path" rev-list --count "$previous_ref..HEAD")" -eq 0 ]; then
    echo "phase-runner: $phase_id reported completed but created no commit over $previous_ref; leaving worktree for inspection: $worktree_path" >&2
    echo "phase-runner: last 80 log lines from $codex_log" >&2
    tail -80 "$codex_log" >&2 || true
    exit 1
  fi

  if ! phase_marked_done "$worktree_path/plans/$PLAN_NAME/$phase_id.md"; then
    echo "phase-runner: $phase_id reported completed but did not mark the phase document done" >&2
    exit 1
  fi

  echo "phase-runner: executor committed $branch"
  total_seconds=$((SECONDS - phase_start))
  PLAN_NAME="$PLAN_NAME" \
  PHASE_ID="$phase_id" \
  BRANCH="$branch" \
  BASE_REF="$previous_ref" \
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

  previous_ref="$branch"
done

echo "phase-runner: finished executor passes. Final branch: $previous_ref"
echo "phase-runner: review manually, then merge and push according to repo policy."
