# Phase 5 - Docs and Follow-Up Guidance

## Status

Done.

## Objective

Make repo docs match the Agents SDK runner direction.

## Scope

- Update `plans/README.md`.
- Update `docs/context/planning.md`.
- Update `CLAUDE.md`.
- Keep active `plans/phaserunner/*` docs focused on the Agents SDK-capable Node runner.
- Document follow-up extension points: prompt injection, local experimental iteration,
  repair/resume inspection, tracing, and sub-agent orchestration.

## Verification

- Search active docs and runner files for stale native-runner references.
- `scripts/phase-runner.sh --help`
- `node tests/phase_runner_agents.mjs`
- `git diff --check`

## Manual Testing Focus

Read the planning docs from an operator's point of view and confirm there is one obvious maintained
runner path.

## Handoff Expectations

Future phase-runner work should change `scripts/phase-runner-agents.mjs`, its tests, and the
Agents SDK executor path unless the user explicitly reopens the language/runtime decision.
