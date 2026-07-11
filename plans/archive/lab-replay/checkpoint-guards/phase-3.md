# Phase 3 - Regression Harness Integration

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Make checkpoint resume tests easy to select when sim, replay, lab, or checkpoint code changes. Add
test selector rules, docs, or helper commands that run the right focused coverage without forcing
the broad historical suite by default. Preserve generated artifacts under `target/`.

## Expected Touch Points

- `tests/select-suites.mjs`
- `docs/context/testing.md`
- Checkpoint harness command docs

## Verification

- Run selector tests or dry-runs showing checkpoint-related changes select the harness.
- Run the harness once on the focused set.

## Manual Testing Focus

No manual gameplay testing is expected unless the harness produces an artifact worth opening.

## Handoff

The handoff must document when future agents should run checkpoint resume coverage.
