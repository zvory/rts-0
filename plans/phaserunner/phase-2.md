# Phase 2 - Side-by-Side Node Runner

## Status

Done.

## Objective

Add a side-by-side Node runner that preserves the current operator behavior while the old entrypoint
remains available for comparison.

## Scope

- Add `scripts/phase-runner-agents.mjs`.
- Preserve CLI parsing for `--plan`, explicit phases, `--from`, `--to`, `--base`, `--model`,
  `--pr`, `--wait`, `--dry-run`, and help.
- Preserve branch, worktree, active-marker, log, handoff, PR body, Codex log, and timing paths.
- Preserve the executor prompt content and dry-run stop/continue behavior.
- Keep default execution on the `codex-cli` path for parity.

## Verification

- `node tests/phase_runner_agents.mjs`
- Side-by-side dry-run comparison with the old runner for explicit phases.
- Side-by-side dry-run comparison with the old runner for `--from/--to --wait`.
- `git diff --check`

## Manual Testing Focus

Use only `--dry-run` in this phase. Confirm branch names, worktree paths, PR actions, wait/no-wait
behavior, and prompt text are understandable to an operator.

## Handoff Expectations

Record any dry-run wording differences and keep the old entrypoint available until cutover.
