# Phase 1 - Behavior Model and Tests

## Status

Done.

## Objective

Capture the current phase-runner contract in testable JavaScript units without running Codex or
touching GitHub.

## Scope

- Model phase id normalization and ordering for numeric, decimal, and suffixed ids.
- Model `--from/--to` discovery against a supplied plan directory.
- Model completion marker detection for the accepted done-marker forms.
- Model handoff helpers, verification summary handling, PR body generation, PR readiness checks,
  and handoff enrichment.
- Add focused tests in `tests/phase_runner_agents.mjs`.

## Verification

- `node tests/phase_runner_agents.mjs`
- `git diff --check`

## Manual Testing Focus

No live phase execution. Confirm tested examples match the old runner's accepted phase ids and
done-marker forms.

## Handoff Expectations

Point later work at `scripts/phase-runner-agents.mjs` as the maintained behavior model.
