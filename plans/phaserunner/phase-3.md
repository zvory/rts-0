# Phase 3 - Executor Lifecycle Core

## Status

Draft.

## Objective

Implement real single-phase execution in Rust from main synchronization through local executor
result validation, while leaving PR automation to the next phase.

## Scope

- Add a command execution adapter that can run real commands in production and fake commands in
  tests.
- Check required tools for non-dry execution: `codex`, `gh`, and `jq` while PR helper scripts still
  need them. The Rust runner should not require `node` for its own parsing or JSON handling.
- Require a clean local `main` checkout and an `origin` remote for non-dry parity mode.
- Fetch and fast-forward local `main` from `origin/main` before the phase.
- Reject existing local phase branches and existing worktree paths before creating a phase
  worktree.
- Create the phase worktree and active marker using the current branch/path conventions.
- Invoke Codex with the current schema path, handoff output path, worktree directory, git common
  directory, sandbox mode, optional model, and rendered prompt.
- On Codex failure or blocked handoff, print the relevant log tail and leave the worktree/logs for
  inspection.
- Validate completed handoffs by checking clean worktree state, at least one commit over the base
  commit, and a done marker in the phase file.
- Record timing JSON for the local executor portion.
- Keep branch push, PR creation, and wait behavior unavailable or explicitly blocked until Phase 4.

## Expected Touch Points

- `server/crates/phaserunner/src/*.rs`
- `server/crates/phaserunner/tests/`
- `scripts/phase-runner-result.schema.json` only if a typed-schema sync decision is made
- `plans/phaserunner/phase-3.md`

## Verification

- Unit tests with fake command execution for success, Codex failure, blocked handoff, dirty
  worktree, missing commit, missing done marker, existing branch, and existing worktree.
- `cargo test --manifest-path server/Cargo.toml -p rts-phaserunner`
- A local no-PR fixture or mocked integration test that proves the Rust runner builds the same
  Codex command arguments without running Codex.
- `git diff --check`

## Manual Testing Focus

Do not run the Rust runner on a real implementation phase unless the phase owner explicitly
authorizes a local-only canary. If a canary is authorized, use a throwaway docs-only plan and do not
push or open a PR in this phase.

## Handoff Expectations

Name every side-effecting command the Rust runner can execute after this phase. Include the failure
mode coverage added in tests and any remaining shell-only behavior that Phase 4 must close.
