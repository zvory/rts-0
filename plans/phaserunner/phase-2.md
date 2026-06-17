# Phase 2 - CLI, Layout, Dry Run, and Prompt Rendering

## Status

Draft.

## Objective

Make the Rust runner capable of parsing real operator arguments and producing the same planned
actions and executor prompt without side effects.

## Scope

- Implement CLI parsing for the preserved option surface:
  `--plan`, explicit phases, `--from`, `--to`, `--base`, `--model`, `--pr`, `--wait`,
  `--dry-run`, and help.
- Preserve current validation rules: `--plan` required, `--pr` required in parity mode,
  `--wait` requires `--pr`, explicit phases cannot be combined with range discovery, `--base` must
  be `main`, and plan names must be simple directory names.
- Compute branch names, worktree paths, log paths, handoff paths, PR body paths, Codex log paths,
  timing paths, and active marker paths from the same conventions as the shell runner.
- Render the executor prompt from named sections while preserving the current prompt content and
  ordering.
- Implement dry-run output with no worktree creation, no Codex execution, no push, and no PR calls.
- Preserve no-wait dry-run behavior: stop after the first planned PR when `--wait` is not set.
- Add fixtures or snapshot-style tests for rendered prompts and dry-run plans.
- Keep the shell runner as the active `scripts/phase-runner.sh` implementation.

## Expected Touch Points

- `server/crates/phaserunner/src/*.rs`
- `server/crates/phaserunner/tests/` or module tests
- `server/Cargo.lock` if CLI dependencies are introduced
- `plans/phaserunner/phase-2.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-phaserunner`
- Compare Rust dry-run output against representative shell dry runs:
  `scripts/phase-runner.sh --plan <fixture> phase-1 --pr --dry-run` and
  `scripts/phase-runner.sh --plan <fixture> --from 1 --to 2 --pr --wait --dry-run`.
- `git diff --check`

## Manual Testing Focus

Run the Rust binary only in `--dry-run` mode against an existing small plan and confirm the branch,
worktree, log paths, PR actions, wait/no-wait stop behavior, and prompt are understandable to an
operator.

## Handoff Expectations

Record the dry-run comparison commands and whether any wording differences are intentional. Point
the next executor at the command-runner abstraction that Phase 3 should use for real side effects.
