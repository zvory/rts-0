# Phase 4 - Final Ratchets and Documentation

Status: Done.

## Goal

Make nextest the durable Rust test contract for the repo. After this phase, normal local guidance,
CI workflows, selector policy, and docs should all agree that nextest is required for Rust tests.

## Scope

- Audit the repo for normal-gate references to plain `cargo test`.
- Keep explicit `cargo test --doc` references only if doctests exist and Phase 1 added that step.
- Update `tests/select-suites.mjs` policy text or expected suite names if it names old Rust commands.
- Update design/testing docs that still describe `cargo test` as the default Rust gate.
- Add a lightweight guard if practical so future edits do not reintroduce `RTS_CARGO_PACKAGE_TIMINGS`
  or package-by-package timing as a supported normal path.
- Confirm the current `Main test gate` logs are legible after a successful post-merge run.

## Out Of Scope

- Do not change gameplay, protocol, balance, or simulation behavior.
- Do not add new CI providers or paid runners.
- Do not add new optional profiling workflows.

## Expected Touch Points

- `docs/context/testing.md`
- `docs/design/testing.md`
- `tests/README.md`
- `tests/select-suites.mjs`
- `.github/workflows/main-tests.yml`
- `scripts/` docs or helpers that name Rust test commands
- `plans/nextest/phase-4.md`

## Implementation Checklist

- [x] Search for `cargo test`, `cargo-test-timed`, and `RTS_CARGO_PACKAGE_TIMINGS`.
- [x] Rewrite normal-gate references to nextest.
- [x] Preserve only intentional doctest or one-off developer examples.
- [x] Update selector or architecture guardrails if they expose suite names.
- [x] Add a small guard against retired timing routes if practical.
- [ ] Inspect a post-merge `Main test gate` run for final CI log quality. Executor note: this
  requires the outer PR/merge pass; `gh run list --workflow "Main test gate" --branch main --limit
  1` could not reach GitHub from this sandbox.
- [x] Mark this phase done in the implementation commit.

## Focused Verification

- `rg -n "cargo-test-timed|RTS_CARGO_PACKAGE_TIMINGS|cargo test" tests scripts docs .github`
- `node tests/select-suites.mjs --verify`
- `tests/run-all.sh --only-rust`
- `git diff --check`
- Post-merge: inspect `Main test gate` and verify the phase head is reachable from `origin/main`

## Manual Test Focus

Read the testing docs as a new developer and confirm there is one obvious Rust test command path.
Review the GitHub Actions page and confirm there are no obsolete Rust or Integration checks.

## Handoff Expectations

The final handoff must confirm nextest is enforced locally and in CI, name any remaining intentional
plain `cargo test` references, include final Rust job timing, and state that `plans/nextest` is
complete only if no normal Rust gate still uses the retired path.
