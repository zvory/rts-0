# Phase 4 - PR Lifecycle and Serial Wait Parity

## Status

Draft.

## Objective

Complete current production behavior by adding branch push, owned PR creation, auto-merge checks,
and serial wait semantics to the Rust runner.

## Scope

- Push the phase branch to `origin` after local executor validation succeeds.
- Generate the phase runner PR body from the handoff JSON using the current sections:
  status, summary, files changed, focused verification, gameplay impact, next executor notes, and
  manual test notes.
- Invoke `scripts/agent-pr.sh --base main --head <branch> --verification <summary> --body-file
  <file>` from the phase worktree.
- Query `gh pr list` for the phase branch and validate that the PR is open, auto-merge is armed,
  and merge state is not dirty.
- Enrich the handoff JSON with PR number, URL, head SHA, auto-merge state, and merge wait state.
- Implement no-wait behavior by stopping after the first opened and armed PR with a pending
  handoff.
- Implement `--wait` behavior by invoking `scripts/wait-pr.sh`, fetching `origin/main`, verifying
  the phase head is an ancestor of `origin/main`, syncing local `main`, and then continuing to the
  next phase.
- Preserve failure inspectability for PR helper failure, missing PR, missing auto-merge, dirty merge
  state, wait failure, and unreachable merged head.
- Update timing JSON to include PR metadata and total phase duration.

## Expected Touch Points

- `server/crates/phaserunner/src/*.rs`
- `server/crates/phaserunner/tests/`
- `plans/phaserunner/phase-4.md`

## Verification

- Unit tests with fake command execution for push, PR helper invocation, PR readiness outcomes,
  no-wait stop, wait success, wait failure, unreachable head, and handoff enrichment.
- `cargo test --manifest-path server/Cargo.toml -p rts-phaserunner`
- Dry-run comparison against the shell runner for one explicit phase and one range-discovered
  `--wait` run.
- `git diff --check`

## Manual Testing Focus

Run one controlled docs-only canary through the Rust runner with `--pr --wait`. Confirm the PR body
contains the expected handoff sections, auto-merge is armed, `scripts/wait-pr.sh` reports merged,
and the phase head is reachable from `origin/main`.

## Handoff Expectations

Provide the canary PR link, focused verification, and any differences from the shell runner's
operator output. State whether Phase 5 can make the Rust runner the default entrypoint.
