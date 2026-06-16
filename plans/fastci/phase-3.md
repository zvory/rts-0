# Phase 3 - Cancel Stale Main Push Runs

Status: Pending.

## Goal

Prevent old post-merge `main` push runs from keeping newer `main` commits pending when several PRs
merge close together.

## Current Evidence

The `Main test gate` workflow currently cancels superseded runs for pull requests, but not pushes
to `main`. Recent post-merge runs showed a newer `main` push waiting behind an older `main` run,
which increases time-to-latest-main-green even when individual job duration is unchanged.

## Scope

- Update `Main test gate` concurrency so stale `main` push runs are canceled when a newer push to
  `main` starts.
- Preserve PR branch cancellation for superseded PR commits.
- Do not cancel unrelated branches.
- Preserve beta deploy safety: deployment must only follow a successful `Main test gate` run for
  the actual `main` commit that remains current.
- Document the queue-time acceptance rule.

## Out Of Scope

- Do not change test commands.
- Do not split jobs.
- Do not change branch protection or required check names unless unavoidable.
- Do not change phase-runner behavior.

## Expected Touch Points

- `.github/workflows/main-tests.yml`
- `.github/workflows/deploy-beta.yml` only if the deploy trigger needs a guardrail check
- `docs/context/testing.md`
- `tests/README.md` only if user-facing command docs need clarification
- `plans/fastci/*` if implementation discoveries change the plan

## Implementation Checklist

- [ ] Capture a fresh pre-phase queue baseline from recent `main` push runs:
      run created time, job started time, completion time, and whether newer `main` runs were
      pending.
- [ ] Adjust workflow concurrency expression carefully.
- [ ] Confirm PR runs for the same PR still cancel superseded commits.
- [ ] Confirm `main` pushes cancel only older `main` push runs, not the latest run.
- [ ] Confirm beta deploy cannot run from a canceled or stale `main` run.
- [ ] Open an owned PR with auto-merge armed and wait for merge.
- [ ] After merge, perform the post-merge speed acceptance gate before starting Phase 5.

## Focused Verification

- YAML/workflow syntax review for `.github/workflows/main-tests.yml`
- `git diff --check`
- `gh workflow view "Main test gate" --yaml` after push, if practical
- Inspect `deploy-beta.yml` event filters if the concurrency expression changes push behavior

## Post-Merge Speed Acceptance Gate

Critical: `scripts/phase-runner.sh --pr --wait` success is not completion for this phase.

After the PR merges:

1. Inspect the next `main` push run that includes the phase head.
2. If another `main` push happens while that run is active, verify GitHub cancels the stale older
   run and keeps the newest `main` run.
3. If natural overlap is unavailable, use a bounded docs-only canary pair only if it is safe and
   approved by the operator running the phase. Otherwise record the measurement as temporarily
   inconclusive and do not start Phase 5 until either natural overlap proves the behavior or the
   operator explicitly accepts the config-only evidence.
4. Compare time-to-latest-main-green before and after the change when overlapping runs exist.
5. Iterate if cancellation does not behave as intended.
6. Revert if the workflow cancels the wrong runs, blocks deploys, or makes required-check state
   ambiguous.

## Manual Test Focus

Inspect the Actions UI around overlapping post-merge `main` runs. Confirm a human can see which
run is current and that canceled runs are not mistaken for required failures on the latest commit.

## Handoff Expectations

Report queue baseline, post-merge run ids, any canceled run ids, whether beta deploy behavior was
affected, and the acceptance decision. If acceptance is config-only because no overlapping runs
occurred, explicitly say Phase 5 must not depend on claimed queue-time improvement yet.
