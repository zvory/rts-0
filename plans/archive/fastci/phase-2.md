# Phase 2 - Restore Workspace Cargo Test Default

Status: Done.

## Goal

Recover the faster default Cargo test execution shape while preserving the timing wrapper as an
opt-in profiling tool.

## Current Evidence

The CI timing instrumentation added `tests/cargo-test-timed.sh`, which runs workspace default
members one package at a time. That gives useful package timing but changed the full gate from a
single workspace `cargo test --manifest-path server/Cargo.toml` into serial package test runs.
Post-instrumentation `Rust fast scripted tests` rows are roughly 8-10 minutes, and total
`tests/run-all.sh` rows are roughly 15-17 minutes.

## Scope

- Change normal `tests/run-all.sh` Rust test execution back to a single workspace `cargo test`
  invocation.
- Keep `tests/cargo-test-timed.sh` available for direct use and/or behind an explicit env flag
  such as `RTS_CARGO_PACKAGE_TIMINGS=1`.
- Preserve `--full-ai` behavior by passing `RTS_FULL_AI_TESTS=1` to the same selected Cargo test
  mode.
- Keep the final `CI timing summary` row for the Rust test phase.
- Update docs so the timing wrapper is described as profiling instrumentation, not the default
  full-gate path unless the env flag is set.

## Out Of Scope

- Do not remove any Rust tests.
- Do not reduce AI/self-play coverage.
- Do not split GitHub jobs yet.
- Do not change the Cargo profile.

## Expected Touch Points

- `tests/run-all.sh`
- `tests/cargo-test-timed.sh` only if an opt-in flag requires a small interface change
- `docs/context/testing.md`
- `tests/README.md`
- `plans/fastci/*` if implementation discoveries change the plan

## Implementation Checklist

- [ ] Capture a fresh pre-phase timing baseline after Phase 1 has been accepted.
- [ ] Add a small helper in `tests/run-all.sh` that chooses between normal workspace cargo test
      and package-timed profiling mode.
- [ ] Ensure both default and `--full-ai` modes preserve the same test coverage as before.
- [ ] Keep package-level timing available through direct `tests/cargo-test-timed.sh` and/or
      explicit env opt-in.
- [ ] Run focused local shell and targeted Cargo validation.
- [ ] Open an owned PR with auto-merge armed and wait for merge.
- [ ] After merge, perform the post-merge speed acceptance gate before starting Phase 3.

## Focused Verification

- `bash -n tests/run-all.sh tests/cargo-test-timed.sh`
- `tests/run-all.sh --no-client` if local time allows; otherwise run the smallest Rust command
  that proves the selected cargo-test path works
- `RTS_CARGO_PACKAGE_TIMINGS=1 tests/run-all.sh --no-client` if that env flag is added and local
  time allows
- `git diff --check`

## Post-Merge Speed Acceptance Gate

Critical: `scripts/phase-runner.sh --pr --wait` success is not completion for this phase.

After the PR merges:

1. Identify the first `Main test gate` run that includes the phase head and reaches
   `CI timing summary`.
2. Compare `Rust fast scripted tests (cargo test)` and `TOTAL tests/run-all.sh` against the
   Phase 1 accepted baseline.
3. Confirm the default log no longer shows the package-by-package `Cargo test package timing
   summary` unless the opt-in profiling flag is set.
4. Accept if the Rust test row and total gate improve without reducing the tested command surface.
5. If timings do not improve, inspect whether the target cache, Cargo lock contention, or
   workspace test invocation caused the miss; iterate with a follow-up PR if likely fixable.
6. Revert with a follow-up PR if normal workspace `cargo test` is not faster or loses useful
   diagnostics that cannot be preserved opt-in.

## Manual Test Focus

Read the final CI logs as a future investigator. Confirm the normal summary still shows phase-level
timings, and confirm the docs explain how to get package-level timings when needed.

## Handoff Expectations

Report baseline run ids, post-merge run ids, Rust test delta, total gate delta, whether package
timing remains opt-in, and the acceptance decision. If accepted, tell Phase 3 the latest main-run
wall-time baseline to use for queue/cancellation analysis.
