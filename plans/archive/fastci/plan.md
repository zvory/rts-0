# Fast CI Plan

## Status

Draft.

## Purpose

Reduce `Main test gate / ./tests/run-all.sh` wall time without weakening the required PR gate.
This plan covers only the previously identified high-value items 1, 2, 3, and 5: fix CI Cargo
cache alignment, restore normal workspace `cargo test` while keeping timing instrumentation
available on demand, cancel stale `main` push runs, and split the full gate into parallel jobs
only after cache behavior is proven. It intentionally does not include the fail-fast background
suite change or any test coverage reductions.

## Critical Speed-Acceptance Rule

`scripts/phase-runner.sh --pr --wait` reports that a phase implementation PR merged; that is not
enough for this plan. After every phase PR merges, the outer agent must inspect the next relevant
GitHub Actions run, compare timing against the recorded baseline for that phase, and either:

- record clear speed improvement and proceed to the next phase;
- iterate with a follow-up PR until the phase improves speed; or
- revert the phase with a follow-up PR when the evidence shows the change does not improve speed
  and further iteration is unlikely to help.

Do not change `scripts/phase-runner.sh` default behavior for everyone. Treat this as a plan-level
operator rule: the runner handles isolated implementation, owned PR creation, auto-merge, and
merge waiting; the calling agent handles post-merge timing acceptance before starting the next
explicit phase. Use explicit phase ids, not `--from 1 --to 5`, because phase 4 is intentionally
absent:

```bash
scripts/phase-runner.sh --plan fastci 1 2 3 5 --pr --wait
```

If running phases one at a time, use:

```bash
scripts/phase-runner.sh --plan fastci <phase> --pr --wait
```

Then run that phase's post-merge measurement gate before invoking the next phase.

## Baseline And Measurement Rules

- Before implementing each phase, collect a fresh baseline from the latest three completed
  `Main test gate` runs that match the phase target. Prefer successful runs; include failed runs
  only when the timing section reached `CI timing summary` and the failure is unrelated to the
  phase target.
- Record run id, event type, branch, queue delay, `Run full local CI suite` step duration,
  `tests/run-all.sh` total, and the phase-specific timing rows.
- Use `gh run list --workflow "Main test gate"` and `gh run view <run> --json ...` for run and
  step timings. Use `gh run view <run> --log` to extract the `CI timing summary`.
- A single noisy run is not enough to prove regression or success. Compare at least one fresh
  post-merge run to the baseline, then prefer the median of the next three comparable runs when
  enough data is available.
- If a phase changes workflow behavior and the next run is still queued behind old pre-phase
  work, wait for or inspect the first run that actually includes the phase head.
- The handoff after each accepted phase must include timing evidence and the decision:
  `accepted`, `iterating`, or `reverted`.

## Cross-Phase Constraints

- Keep `tests/run-all.sh` as the portable full gate unless a phase explicitly preserves an
  equivalent required signal through workflow composition.
- Do not remove required coverage in this plan.
- Keep local worktree isolation intact. Per-worktree target dirs remain the local default unless
  CI explicitly overrides them.
- Do not depend on paid or larger GitHub runners.
- Keep beta deploy behavior tied only to successful `Main test gate` runs from pushes to `main`.
- Preserve focused local verification guidance; GitHub Actions remains the authoritative full
  gate.
- Each phase must update `docs/context/testing.md` and `tests/README.md` when the test or workflow
  contract changes.
- Each implementation PR must be owned, auto-merge armed, and waited through the normal
  PR-first lifecycle before post-merge timing acceptance starts.

## Phase Summaries

### [Phase 1 - Align CI Cargo Cache With Actual Target Dir](phase-1.md)

Make CI cache the Cargo target directory that `tests/run-all.sh` actually uses, or explicitly make
the workflow provide a stable `CARGO_TARGET_DIR` that matches the existing cache path. This should
preserve local per-worktree isolation while letting GitHub Actions reuse Rust build artifacts
across comparable runs. After merge, measure server build, Rust test, Rust lint, and total gate
time; iterate or revert if the cache does not produce a real win.

### [Phase 2 - Restore Workspace Cargo Test Default](phase-2.md)

Stop running package-by-package Cargo tests by default in the full gate, because that profiling
wrapper changed execution shape and added minutes. Keep `tests/cargo-test-timed.sh` available
behind an explicit profiling flag or direct command so future investigations can still get
package-level timings. After merge, verify Rust test and total gate time improve without losing
coverage.

### [Phase 3 - Cancel Stale Main Push Runs](phase-3.md)

Update workflow concurrency so superseded `main` push runs do not queue behind older post-merge
pushes when a newer `main` commit exists. Keep PR-run cancellation behavior and beta deploy safety
intact. After merge, verify the latest `main` run is the one that remains active under overlapping
main pushes, or run a bounded canary if natural overlap is not available.

### [Phase 5 - Split Full Gate Into Parallel Jobs](phase-5.md)

Only after Phase 1 and Phase 2 are accepted, split the full gate into parallel GitHub Actions jobs
for Rust build/test/lint, live Node suites, and browser/tri-state coverage while preserving a
stable required full-gate signal. Measure whether the slowest required check and total time to
merge improve; iterate if duplicate compile/cache behavior erases the expected gain. Revert if
the split makes the required gate slower or harder to audit.

## Handoff Rules

Each phase handoff must include:

- implementation PR number, head SHA, and merge confirmation;
- focused local verification results;
- GitHub Actions run ids used for post-merge timing;
- baseline timing, post-merge timing, and acceptance decision;
- whether iteration or revert was considered;
- the exact next phase command or a stop reason;
- manual checks needed before continuing.

Do not report the overall `plans/fastci` rollout complete until every included phase has a
post-merge speed acceptance decision.
