# Phase 4 - Run-All Browser, Dependency Cache, and CI Parity

Status: Not started.

## Goal

Complete Rust-backed `run-all` parity for the current Unix and CI workflow. By the end of this
phase, `tests/run-all.sh` should be a compatibility shim around the Rust implementation for the
normal modes, while the GitHub required check name and coverage contract remain unchanged.

## Scope

- Implement browser smoke execution, tri-state browser scenario groups, and `RTS_RUN_WASM_TRI_STATE`
  handling.
- Preserve Chrome detection behavior for macOS and Linux, while keeping the detection seam ready for
  Windows in Phase 5.
- Implement client dependency hydration keyed by `tests/package-lock.json`, including lock
  behavior, `npm ci`, cache readiness markers, and safe replacement of `tests/node_modules`.
- Preserve timing detail support controlled by `RTS_SUITE_TIMING_DETAILS`.
- Preserve nextest JUnit summary support controlled by `RTS_NEXTEST_JUNIT_SUMMARY`.
- Route `tests/run-all.sh` through the Rust tool for all supported Unix modes.
- Keep the `Main test gate` aggregate check named `./tests/run-all.sh`.
- Update CI only where necessary to keep sub-modes and artifacts equivalent.

## Expected Touch Points

- `server/crates/devtool/src/run_all.rs`
- `server/crates/devtool/src/browser.rs`
- `server/crates/devtool/src/deps.rs`
- `server/crates/devtool/src/timing.rs`
- `tests/run-all.sh`
- `.github/workflows/main-tests.yml` only if wrapper or artifact behavior requires it
- `tests/README.md`
- `docs/context/testing.md` if the command contract changes
- `docs/design/testing.md` if the PR CI contract changes

## Implementation Notes

Treat this as a parity phase, not a redesign phase. The browser suites are intentionally serialized
after the parallel live Node batch because they are latency-sensitive; preserve that behavior unless
the phase produces evidence that a different order is equally stable. The dependency cache should
fail on lockfile/package mismatch and should not silently reuse stale dependencies.

If symlink behavior becomes a portability blocker, keep Unix parity in this phase and defer the
Windows-specific link/copy/junction decision to Phase 5. Do not rename the required GitHub check or
drop coverage to make the migration easier.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool run_all`
- `./tool run-all --only-rust`
- `./tool run-all --only-live-node` with an available server binary or local build
- `./tool run-all --only-browser` on a machine with Chrome
- `tests/run-all.sh --only-rust`
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

On a Unix-like development machine with Chrome, run a browser mode and confirm dependency hydration,
Chrome discovery, skipped WASM groups, and timing summaries match expectations. In CI, confirm the
split jobs still feed the aggregate `./tests/run-all.sh` check and that docs-only and client-only
classification still produce clear skip output.

## Handoff Expectations

State whether all Unix modes are now Rust-backed by default. Record any intentional output changes,
runtime regressions, or remaining shell fallback paths before the Windows phase starts.
