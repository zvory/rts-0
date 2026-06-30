# Phase 3 - Regression Harness Integration

Status: Not started.

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
