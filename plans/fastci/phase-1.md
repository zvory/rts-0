# Phase 1 - Align CI Cargo Cache With Actual Target Dir

Status: Done.

## Goal

Make the GitHub Actions Cargo cache affect the Cargo artifacts used by `tests/run-all.sh`, without
changing local per-worktree target isolation.

## Current Evidence

`tests/run-all.sh` sets `CARGO_TARGET_DIR` to the output of `scripts/cargo-shared-target.sh` when
the caller does not provide one. In CI that means Cargo writes under `/tmp/rts-cargo-target/...`,
while `.github/workflows/main-tests.yml` currently caches `server/target`. Recent timing runs show
server build, Rust test, and Rust lint consume most of the full gate, so a working target cache is
the lowest-risk first optimization.

## Scope

- Update `Main test gate` so CI uses a stable Cargo target directory that is actually cached.
  The simplest acceptable approach is setting workflow-level or job-level
  `CARGO_TARGET_DIR: ${{ github.workspace }}/server/target`.
- Keep local `tests/run-all.sh` behavior unchanged unless a direct bug is found.
- Ensure the cache key can restore and save useful target artifacts across comparable runs.
  Prefer the existing `actions/cache` shape if sufficient; use a Rust cache action only if it is
  clearly safer and documented.
- Keep Cargo registry/git caching intact.
- Update test docs to clarify the CI override versus local per-worktree default.

## Out Of Scope

- Do not change which tests run.
- Do not split jobs yet.
- Do not introduce `sccache` for local worktrees.
- Do not change phase-runner behavior.

## Expected Touch Points

- `.github/workflows/main-tests.yml`
- `docs/context/testing.md`
- `tests/README.md`
- `plans/fastci/*` if implementation discoveries change the plan

## Implementation Checklist

- [ ] Capture a fresh pre-phase timing baseline from at least three recent `Main test gate` runs.
- [ ] Align CI `CARGO_TARGET_DIR` with the cached path, or align the cache path with CI's actual
      target dir.
- [ ] Confirm the cache step logs restore the intended target path.
- [ ] Keep local per-worktree target docs accurate.
- [ ] Run focused local syntax/validation checks.
- [ ] Open an owned PR with auto-merge armed and wait for merge.
- [ ] After merge, perform the post-merge speed acceptance gate before starting Phase 2.

## Focused Verification

- `git diff --check`
- `bash -n tests/run-all.sh scripts/cargo-shared-target.sh`
- YAML/workflow syntax review for `.github/workflows/main-tests.yml`
- If practical, `gh workflow view "Main test gate" --yaml` after push to confirm workflow shape

## Post-Merge Speed Acceptance Gate

Critical: `scripts/phase-runner.sh --pr --wait` success is not completion for this phase.

After the PR merges:

1. Fetch `origin/main` and identify the first `Main test gate` run whose head SHA includes the
   phase head.
2. Extract `Run full local CI suite` duration and `CI timing summary` rows for:
   `Server build (debug)`, `Rust fast scripted tests (cargo test)`, `Rust lint (cargo clippy)`,
   and `TOTAL tests/run-all.sh`.
3. Inspect cache logs for a target-cache hit or useful restore.
4. Compare against the recorded baseline. Accept only if Rust build/test/lint or total gate time
   improves materially, or if the first run is cold but subsequent comparable runs show the cache
   win.
5. If the cache path is still unused or timings do not improve, iterate with a follow-up PR.
6. Revert with a follow-up PR if the cache change adds complexity without producing measurable
   improvement after reasonable iteration.

## Manual Test Focus

Inspect the GitHub Actions log for one post-merge run. Confirm Cargo writes to the cached target
directory and that local documentation still tells agents how per-worktree target isolation works.

## Handoff Expectations

Report baseline run ids, post-merge run ids, cache-hit evidence, timing deltas, and the acceptance
decision. If accepted, tell Phase 2 whether the target cache is warm enough to evaluate Cargo test
shape cleanly.
