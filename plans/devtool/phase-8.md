# Phase 8 - Workflow Rollout and Script Debt Boundary

Status: Not started.

## Goal

Finish the migration by making `./tool` the preferred workflow command and documenting which scripts
remain intentionally Unix-only. This phase should validate the full PR-first lifecycle after the
Rust-backed commands are in place.

## Scope

- Update docs to prefer `./tool run-all`, `./tool agent-pr`, and `./tool wait-pr` while preserving
  old path references where compatibility matters.
- Keep `tests/run-all.sh`, `scripts/agent-pr.sh`, and `scripts/wait-pr.sh` as stable shims unless
  the team explicitly chooses a later breaking cleanup.
- Audit internal callers such as phase-runner and docdrift for direct script dependencies.
- Run or document the PR-first canaries from `docs/pr-first-workflow.md`: docs-only PR,
  representative implementation PR, and phase-runner serial PR wait.
- Add a short "script debt boundary" note that names commands intentionally left Unix-only for now.
- Decide whether additional shell scripts should be migrated later based on workflow criticality,
  not line count alone.

## Expected Touch Points

- `README.md`
- `CLAUDE.md`
- `tests/README.md`
- `docs/context/testing.md`
- `docs/pr-first-workflow.md`
- `docs/design/testing.md` if the CI contract wording changes
- `scripts/phase-runner-agents.mjs` if command paths are updated
- `scripts/docdrift-sweep.mjs` if command paths are updated

## Implementation Notes

Do not remove stable script paths just because the Rust tool exists. Existing agents, GitHub
workflows, and local habits rely on those paths, and the required PR check is still named
`./tests/run-all.sh`. The rollout should change the preferred operator interface while keeping
compatibility cheap.

The script debt boundary should be explicit. For example, `deploy.sh`, `scripts/fly-logs.sh`,
macOS desktop helpers, sound preview helpers, and docdrift daily launchers can stay Unix-oriented
unless a future task needs them on Windows or finds reliability problems worth addressing.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool`
- `./tool run-all --only-rust` or the smallest representative migrated mode
- `./tool agent-pr --dry-run --verification "rollout dry run"`
- `./tool wait-pr <known-open-pr> --once` when a suitable PR exists
- `node scripts/check-docs-health.mjs`
- PR-first canary evidence recorded in the phase handoff

## Manual Testing Focus

Follow the updated docs as a developer would: discover the tool, run help, run a focused test mode,
dry-run PR creation, and inspect a wait-pr pending state. Confirm old script paths still work for
compatibility.

## Handoff Expectations

Summarize the final supported command surface, the scripts intentionally left alone, and any future
migration candidates. Include canary results or blockers so future agents know whether the workflow
is ready for normal use.
