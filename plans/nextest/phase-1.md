# Phase 1 - Local Nextest Runner

Status: Done.

## Goal

Make nextest the Rust test runner used by local repo commands. The normal local gate should no
longer run workspace `cargo test` for Rust unit/integration tests, and it should not keep the old
package-by-package timing wrapper as a recommended diagnostic path.

## Scope

- Add repo-owned nextest configuration, most likely `.config/nextest.toml`.
- Update `tests/run-all.sh` so the Rust test phase runs nextest.
- Make missing nextest a clear local failure with an install hint.
- Check whether Rust doctests exist. If they do, add an explicit `cargo test --doc` step beside
  nextest; if they do not, document that no doctest step is currently needed.
- Remove `RTS_CARGO_PACKAGE_TIMINGS` from the normal runner path.
- Remove or deprecate `tests/cargo-test-timed.sh` as part of this phase if no later phase needs it
  for migration cleanup.
- Update `tests/README.md` and `docs/context/testing.md` for local commands.

## Out Of Scope

- Do not change GitHub Actions in this phase unless needed for local command docs.
- Do not tune slow tests yet.
- Do not introduce optional canary modes.
- Do not change live Node or browser suite behavior.

## Expected Touch Points

- `.config/nextest.toml`
- `tests/run-all.sh`
- `tests/cargo-test-timed.sh`
- `tests/README.md`
- `docs/context/testing.md`
- `plans/nextest/phase-1.md`

## Implementation Checklist

- [x] Inspect the current Rust workspace for doctests.
- [x] Add nextest configuration with clear CI/local output defaults.
- [x] Replace the Rust `cargo test` call in `tests/run-all.sh` with nextest.
- [x] Keep `RTS_FULL_AI_TESTS=1` behavior working through nextest for full AI coverage.
- [x] Remove the package-by-package timing environment flag and stale help text.
- [x] Delete or clearly retire the package timing script if nothing still calls it.
- [x] Update docs to tell developers to install and use nextest.
- [x] Mark this phase done in the implementation commit.

## Focused Verification

- `bash -n tests/run-all.sh`
- `tests/run-all.sh --only-rust`
- `RTS_FULL_AI_TESTS=1 tests/run-all.sh --only-rust` if full AI routing changes
- Any doctest command added by this phase
- `git diff --check`

## Manual Test Focus

Run the local Rust-only gate from a clean worktree without nextest installed and confirm the error is
clear. Then install nextest and confirm the Rust-only gate prints useful nextest timing output.

## Handoff Expectations

The handoff must say whether doctests exist, which nextest profile is used locally, what happened to
`tests/cargo-test-timed.sh`, and what command Phase 2 should wire into CI.
