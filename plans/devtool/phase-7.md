# Phase 7 - Wait-PR Rust Migration

Status: Not started.

## Goal

Move `wait-pr` behavior into the Rust dev tool while preserving its completion and failure
semantics. This phase should make the merge wait loop deterministic under fixture tests and usable
from Windows.

## Scope

- Implement `./tool wait-pr` and `tool.ps1 wait-pr`.
- Preserve options: `--interval`, `--timeout`, `--once`, `--main-ref`, and help output.
- Preserve fixture inputs or equivalent test-only inputs for PR view JSON, PR checks JSON, and
  fetch skipping.
- Parse GitHub PR view and check output with typed Rust structs.
- Fail on failed or canceled checks with a concise summary.
- Fail when a PR closes unmerged.
- Succeed only when the PR is merged and the head SHA is reachable from the configured main ref.
- Preserve timeout behavior and pending exit code behavior for `--once`.
- Keep `scripts/wait-pr.sh` as a compatibility shim for phase-runner, docdrift, and older docs.

## Expected Touch Points

- `server/crates/devtool/src/wait_pr.rs`
- `server/crates/devtool/src/github.rs`
- `server/crates/devtool/src/git.rs`
- `scripts/wait-pr.sh`
- `scripts/phase-runner-agents.mjs` only if it parses helper output too tightly
- `scripts/docdrift-sweep.mjs` only if it parses helper output too tightly
- `docs/pr-first-workflow.md`
- `CLAUDE.md` only if the preferred command path changes for agents

## Implementation Notes

The core state machine should be unit-tested without sleeping. Isolate clock and sleeper behavior
so tests can cover timeout and polling transitions instantly. Use an injectable command runner for
`gh pr view`, `gh pr checks`, `git fetch`, and `git merge-base --is-ancestor`.

The compatibility output should remain close enough that existing human instructions and automation
logs still make sense. If any script parses exact text, either preserve that text or update the
consumer in the same phase with tests.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool wait_pr`
- `./tool wait-pr <known-open-pr> --once` when a suitable PR exists, expecting pending exit behavior
- `scripts/wait-pr.sh <known-open-pr> --once` as a compatibility check when a suitable PR exists
- Fixture tests for merged, failed checks, canceled checks, closed-unmerged, pending, missing head
  SHA, timeout, and unreachable merged head
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

Run `--once` against an open PR and confirm the pending message is clear. For a merged PR, confirm
the command verifies reachability from `origin/main` before reporting success.

## Handoff Expectations

State whether `scripts/wait-pr.sh` is now only a shim and whether all fixture behavior from the old
script is covered. Call out any change to exit codes, polling messages, or failed-check summaries.
