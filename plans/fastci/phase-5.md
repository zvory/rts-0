# Phase 5 - Split Full Gate Into Parallel Jobs

Status: Pending.

## Goal

Reduce wall-clock time for the required full gate by running independent suites in parallel
GitHub Actions jobs, while preserving an auditable required signal and the same coverage.

## Preconditions

- Phase 1 must be accepted with evidence that CI Cargo caching works.
- Phase 2 must be accepted with evidence that the normal Rust test path is faster than the
  package-by-package profiling wrapper.
- Phase 3 should be accepted or explicitly marked as queue-only pending evidence. If Phase 3 is
  inconclusive, this phase may still proceed for per-run duration improvement, but the handoff must
  keep queue-time claims separate.

## Scope

- Split the full gate into parallel jobs only where coverage remains equivalent. Candidate jobs:
  Rust build/test/lint/architecture checks, live Node API suites, and browser/tri-state suites.
- Preserve or replace the stable required check in a way branch protection can enforce.
  If a synthetic aggregate job is used, it must depend on all required jobs and fail when any
  required job fails.
- Avoid duplicate cold Rust compiles where possible. Use Phase 1 cache behavior and job structure
  to prevent the split from making compile time worse overall.
- Keep `tests/run-all.sh` runnable locally as the portable full gate.
- Keep GitHub logs easy to audit: each job should have a clear name and timing output.
- Update docs and selector policy only as needed to explain the new required check structure.

## Out Of Scope

- Do not remove tests.
- Do not skip browser tri-state coverage in this plan.
- Do not move to paid/larger runners.
- Do not change branch protection manually unless the phase explicitly documents the required
  check-name migration and verifies it.
- Do not change phase-runner behavior.

## Expected Touch Points

- `.github/workflows/main-tests.yml`
- `tests/run-all.sh` only if flags are needed to run clean sub-suites without changing local full
  behavior
- `docs/context/testing.md`
- `tests/README.md`
- `docs/pr-first-workflow.md` if required check names or canary guidance changes
- `plans/fastci/*` if implementation discoveries change the plan

## Implementation Checklist

- [ ] Capture a fresh pre-phase baseline from the accepted Phase 2 or Phase 3 state.
- [ ] Decide job boundaries based on actual suite dependencies, not wishful parallelism.
- [ ] Ensure live Node and browser jobs each start or receive a server correctly.
- [ ] Ensure Rust jobs do not fight over the same mutable target directory in the same job
      workspace.
- [ ] Preserve a stable required aggregate signal or document and verify the new required checks.
- [ ] Run focused local checks for any shell changes.
- [ ] Open an owned PR with auto-merge armed and wait for merge.
- [ ] After merge, perform the post-merge speed acceptance gate before declaring the rollout done.

## Focused Verification

- `bash -n tests/run-all.sh` if changed
- YAML/workflow syntax review for `.github/workflows/main-tests.yml`
- `git diff --check`
- If flags are added, run the smallest local command for each new `tests/run-all.sh` mode
- Inspect `gh pr checks` on the phase PR to confirm every split job and aggregate check reports
  clearly

## Post-Merge Speed Acceptance Gate

Critical: `scripts/phase-runner.sh --pr --wait` success is not completion for this phase.

After the PR merges:

1. Identify the first `Main test gate` run that includes the phase head and all split jobs.
2. Record each job's start, completion, conclusion, and timing summary.
3. Compare the slowest required path and total time-to-merge against the accepted pre-phase
   baseline. Keep queue delay separate from job duration.
4. Confirm branch protection or required checks still require every intended coverage area.
5. Accept only if wall time improves materially and logs remain auditable.
6. Iterate if duplicate compile work, cache misses, server startup duplication, or required-check
   naming undermines the win.
7. Revert if the split is slower, flaky, or makes coverage/merge status harder to reason about.

## Manual Test Focus

Inspect a PR checks page and a post-merge Actions run. Confirm a human can tell which jobs cover
Rust, live Node, browser/tri-state, and aggregate pass/fail state without reading workflow YAML.

## Handoff Expectations

Report baseline run ids, post-merge run ids, job timing table, slowest required path delta,
required-check names, cache behavior, and the acceptance decision. If accepted, summarize the
final expected `Main test gate` wall time and any remaining optimization opportunities that would
require coverage tradeoffs.
