# Phase 1 - Cheap Final-Head Preflight

## Phase Status

- [ ] Ready for implementation.

## Objective

Catch the recent high-frequency deterministic failures before launching the adversarial Codex child,
then enforce the same cheap checks on the review-produced final head before push or status. This is
a fail-fast ordering change, not a local replacement for the GitHub full gate.

## Work

- Add one small reusable preflight helper, or an equivalently small shared function, that accepts the
  repository root and base ref and runs these existing checks in a fixed order:
  - `git diff --check origin/main...HEAD`
  - `node scripts/check-docs-health.mjs`
  - `node scripts/check-source-file-sizes.mjs`
  - `node tests/select-suites.mjs --verify`
  - `node scripts/check-faction-assumptions.mjs`
  - `node scripts/check-deploy-assets.mjs`
- Keep the list intentionally fast and deterministic. Do not add Cargo, nextest, server startup,
  browser, screenshot, Interact, or full client-contract execution.
- In `scripts/agent-pr.sh`, run the helper after fetching/plan archival has established the final
  base and before either docs-only handling or the Codex quality pass. A failure must stop before
  Codex, push, PR mutation, auto-merge, or success status.
- Run the same helper inside the quality-pass lifecycle after Codex changes and touched-Rust
  formatting are committed, but before `--push` or `--post-status` can act. A failure must leave the
  local branch intact for correction while guaranteeing the unverified head was not pushed or
  marked successful.
- Avoid two separate check lists in shell and JavaScript. The normal helper and quality-pass runner
  must invoke the same source of truth.
- Preserve dry-run behavior: describe that preflight would run without executing mutable GitHub or
  Codex operations.
- Add compact elapsed-time logging per command only if it falls out naturally from the helper; do
  not build historical telemetry.
- Update `docs/design/testing.md` and `docs/pr-first-workflow.md` with the new order and recovery
  behavior.

## Expected Touch Points

- `scripts/agent-pr.sh`
- `scripts/adversarial-quality-pass.mjs`
- one small helper under `scripts/` if sharing through the existing files is awkward
- `tests/adversarial_quality_pass.mjs`
- `docs/design/testing.md`
- `docs/pr-first-workflow.md`

## Required Tests

- A pre-review check failure never invokes the fake Codex binary and never invokes fake GitHub push,
  PR, merge, or status operations.
- A fake Codex review that leaves a source-size/policy-invalid final head is rejected before push and
  status.
- A clean valid branch runs the helper twice, reaches push/status once, and retains the quality-pass
  report.
- Markdown-only behavior still skips Codex and completes its existing status/report path after the
  cheap checks pass.
- Failure output names the exact failed command and preserves its useful stdout/stderr.
- Existing head-branch mismatch, nested-agent refusal, clean-worktree, bounded input, and dry-run
  tests remain green.

## Verification

- `node tests/adversarial_quality_pass.mjs`
- `bash -n scripts/agent-pr.sh tests/run-all.sh`
- `node --check scripts/adversarial-quality-pass.mjs`
- syntax check for any new helper
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

In a disposable worktree, introduce a harmless source-size violation and confirm `agent-pr.sh`
stops before printing the Codex-launch line. Then run a dry-run on a valid Markdown-only branch and
a valid non-docs branch, confirming the displayed ordering is preflight, review when applicable,
final-head preflight, push/status, and PR lifecycle.

## Handoff Expectations

Report the final fast-check list, where the two invocations occur, how failures prove no push/status
happened, and the focused test command. Tell the Phase 2 agent where existing PR metadata is loaded,
where the final reviewed-head status is posted, and how a different review base reaches the bounded
changed-path manifest.

## Deferred

- Cargo or nextest preflight.
- Running selected functional suites automatically.
- Incremental post-CI review.
