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
- Mark plans/$PLAN_NAME/$phase_id.md done if and only if the phase is completed.
- Run the smallest targeted verification appropriate for the changed files.
- If the phase is ambiguous, too broad, blocked by failing verification, or needs human design/product input, stop and report status "blocked".

Return a compact JSON handoff matching the requested schema.
EOF
  )"

  echo "phase-runner: creating $worktree_path from $previous_ref on $branch"
  if [ "$DRY_RUN" = "0" ]; then
    git worktree add "$worktree_path" -b "$branch" "$previous_ref"
    mkdir -p "$handoff_dir"
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

  echo "phase-runner: running Codex executor for $phase_id"
  if ! codex "${codex_args[@]}"; then
    echo "phase-runner: Codex failed for $phase_id; leaving worktree at $worktree_path" >&2
    exit 1
  fi

  status="$(json_get_status "$handoff_file")"
  if [ "$status" != "completed" ]; then
    echo "phase-runner: $phase_id reported status '$status'; leaving worktree for inspection: $worktree_path" >&2
    exit 1
  fi

  if [ -z "$(git -C "$worktree_path" status --porcelain=v1)" ]; then
    echo "phase-runner: $phase_id completed but produced no file changes; leaving branch uncommitted" >&2
  else
    git -C "$worktree_path" add -A
    git -C "$worktree_path" commit -m "Execute $PLAN_NAME $phase_id" \
      -m "Executor pass for plans/$PLAN_NAME/$phase_id.md." \
      -m "Handoff saved to plans/$PLAN_NAME/handoffs/$phase_id.json."
    echo "phase-runner: committed $branch"
  fi

  previous_ref="$branch"
done

echo "phase-runner: finished executor passes. Final branch: $previous_ref"
echo "phase-runner: review manually, then merge and push according to repo policy."
