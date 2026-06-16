#!/usr/bin/env bash
# Run executor-only phased plan passes in isolated git worktrees.
set -euo pipefail

WORKTREE_ROOT="${RTS_WORKTREE_ROOT:-/tmp/rts-worktrees}"
BASE_BRANCH="main"
DRY_RUN=0
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
  scripts/phase-runner.sh --plan faction 4
  scripts/phase-runner.sh --plan faction 5.5
  scripts/phase-runner.sh --plan faction phase-4 phase-5 --base main
  scripts/phase-runner.sh --plan faction --from 5 --to 6
  scripts/phase-runner.sh --plan ai 2 --model gpt-5.4-mini

Runs executor passes only. Each phase gets a separate worktree and branch under
/tmp/rts-worktrees. Each phase starts from the current local main, then the
runner merges the completed phase branch back to local main and pushes main
before starting the next phase.
Phase ids may be numeric, decimal interstitials such as 5.5, or suffixed ids
such as 3a. Use --from/--to to discover all phase files in that interval; for
example --from 5 --to 6 runs phase-5.5 before phase-6 if both files exist.

The runner never creates plans or performs final review. It does merge each
completed phase branch to main and push main, then verifies the phase commit is
reachable from local main before considering that phase done.
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
  echo "error: phase-runner now integrates each phase into local main; --base must be main" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && [ "$(git branch --show-current)" != "$BASE_BRANCH" ]; then
  echo "error: start phase-runner from the local $BASE_BRANCH checkout so it can merge and push phases" >&2
  exit 2
fi

if [ "$DRY_RUN" = "0" ] && ! git remote get-url origin >/dev/null 2>&1; then
  echo "error: origin remote is required because phase-runner must push $BASE_BRANCH after each phase" >&2
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
- Do not create or revise the overall plan.
- Do not run a final review pass.
- Do not merge, push, or open a PR; the outer phase runner handles merge and push after you commit.
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
  echo "phase-runner: creating $worktree_path from $phase_base_ref ($phase_base_commit) on $branch"
  if [ "$DRY_RUN" = "0" ]; then
    git worktree add "$worktree_path" -b "$branch" "$phase_base_ref"
    mkdir -p "$handoff_dir"
    mkdir -p "$log_dir"
  fi

  if [ "$DRY_RUN" = "1" ]; then
    echo "phase-runner: would run Codex in $worktree_path"
    printf '%s\n' "$prompt"
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
  echo "phase-runner: merging $branch into local $BASE_BRANCH"
  sync_main
  if ! git merge --no-ff --no-edit "$branch"; then
    echo "phase-runner: merge failed for $phase_id; resolve local $BASE_BRANCH and $worktree_path manually" >&2
    exit 1
  fi

  if ! git merge-base --is-ancestor "$phase_head" "$BASE_BRANCH"; then
    echo "phase-runner: merge completed but $phase_head is not reachable from local $BASE_BRANCH" >&2
    exit 1
  fi

  echo "phase-runner: pushing local $BASE_BRANCH to origin/$BASE_BRANCH"
  git push origin "$BASE_BRANCH"

  if ! git merge-base --is-ancestor "$phase_head" "$BASE_BRANCH"; then
    echo "phase-runner: $phase_id is not done because $phase_head is not on local $BASE_BRANCH after push" >&2
    exit 1
  fi

  echo "phase-runner: $phase_id merged to local $BASE_BRANCH and pushed"
  total_seconds=$((SECONDS - phase_start))
  PLAN_NAME="$PLAN_NAME" \
  PHASE_ID="$phase_id" \
  BRANCH="$branch" \
  BASE_REF="$phase_base_commit" \
  PHASE_HEAD="$phase_head" \
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

done

if [ "$DRY_RUN" = "1" ]; then
  echo "phase-runner: dry run finished. No worktrees were created and no phases were merged."
else
  echo "phase-runner: finished executor passes. Local $BASE_BRANCH contains all completed phase commits."
fi
