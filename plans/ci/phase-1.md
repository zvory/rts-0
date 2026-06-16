# Phase 1 - PR CI contract

## Status

Done.

## Goal

Make the required PR CI signal explicit before branch protection starts enforcing it. This phase
does not need to enable protection yet; it prepares the workflows and documentation so later phases
can require stable, meaningful checks.

## Scope

- Update GitHub Actions so the canonical full gate runs for PRs targeting `main`.
- Decide whether the required gate is a single `Main test gate / ./tests/run-all.sh` check or a
  small set of required checks such as `Rust / test`, `Integration / integration`, and the full
  gate.
- Give required jobs stable names that branch protection can depend on.
- Add safe PR concurrency where appropriate so pushes to the same PR branch cancel superseded runs
  without canceling unrelated branches.
- Document the CI cost posture: standard public-repo runners are acceptable today, larger runners
  are out of scope, and `tests/run-all.sh` remains portable.
- Keep beta deploy behavior tied only to tested `main` commits, not unmerged PR heads.

## Expected touch points

- `.github/workflows/main-tests.yml`
- `.github/workflows/rust.yml`
- `.github/workflows/integration.yml`
- `.github/workflows/deploy-beta.yml`
- `README.md`
- `docs/context/testing.md`
- `docs/design/testing.md`
- `plans/ci/plan.md` if details shift during implementation

## Verification

- `gh workflow list` shows all intended workflows active.
- A dry-run or temporary branch PR confirms the required checks appear with the expected names.
- `gh pr checks <pr> --json name,workflow,state,bucket` shows the checks in a machine-readable
  shape usable by later helper scripts.
- Confirm beta deployment does not run for PR heads.

## Manual testing focus

Open a test PR or inspect a recent PR run and confirm a human can tell which check is the full
merge gate. Confirm canceled superseded PR runs do not hide the latest commit's CI result.

## Handoff expectations

Record the exact required check names chosen for Phase 2. Note any workflow timing or cache issues
that Phase 4 or Phase 6 should consider.
