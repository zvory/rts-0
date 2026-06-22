# Phase 4 - Repeatable Hotspot Workflow

Status: done.

## Goal

Make future hotspot passes repeatable enough to compare before and after cleanup work. The output
should let another agent rerun the analysis without reconstructing the method from chat history.

## Scope

- Read the Phase 1 baseline, Phase 2 responsibility map, and Phase 3 candidate ranking.
- Create or update `plans/hotspots/runbook.md`.
- Add a small repo-local script only if it stays focused on metrics collection and is clearly better
  than a documented command sequence. If added, prefer `scripts/hotspot-analysis.mjs` and have it
  emit JSON or CSV without requiring external dependencies.
- If a script is not added, the runbook must include exact shell commands and filtering rules that
  reproduce the useful parts of the Phase 1 baseline.
- Add or document an architectural-group mapping so future runs can compare groups after files split
  or move. This can live in `plans/hotspots/group-map.md`, in the runbook, or in a script-local
  mapping table.
- Explain how to rerun the analysis before and after a cleanup PR, including how to interpret
  history disruption from file splits.
- Update `plans/hotspots/extraction-candidates.md` only if the repeatable workflow changes the
  recommended ranking.

## Expected Touch Points

- `plans/hotspots/runbook.md`
- optional `plans/hotspots/group-map.md`
- optional `scripts/hotspot-analysis.mjs`
- optional `plans/hotspots/evidence/*.json` or `.csv`
- `plans/hotspots/phase-4.md`

Do not modify runtime source, tests, protocol files, client modules, CSS, or design docs in this
phase unless a tiny tooling script requires a documented test/check update.

## Workflow Requirements

- The workflow must distinguish current-file hotspots from raw stale path churn.
- The workflow must explain whole-file rename tracking and split/copy limitations.
- The workflow must support recent-window analysis, not just all-time churn.
- The workflow must support architectural-group summaries so splitting a file does not erase the
  apparent risk.
- The workflow must keep generated artifacts, archived plans, fixtures, build outputs, and bulky
  replay captures out of the default ranking.
- The workflow must be cheap enough to run during planning, but it can be slower than ordinary
  development commands because it is not a commit hook.

## Verification

- `git status --short`
- `git diff --check`
- If a script is added:
  - run the script and confirm it exits successfully;
  - confirm the output includes the expected top hotspot families;
  - run the smallest relevant formatting or lint check for the script.
- If only a runbook is added:
  - execute at least the primary command sequence once and confirm it reproduces the documented
    output shape.

## Manual Review Focus

Review whether the workflow is practical for a future cleanup audit. A good result should answer:
what command do I run, what files or groups do I inspect, what false positives do I ignore, and how
do I tell whether a cleanup improved the situation?

## Handoff

After implementation, mark this phase done and summarize the final repeatable workflow, any script
or group map added, the recommended rerun cadence, and the top cleanup plan that should be created
next. Include enough command detail that a future agent can rerun the hotspot pass from a clean
worktree without reading earlier phase logs.
