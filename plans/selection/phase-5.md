# Phase 5 - Cleanup, Docs, and Regression Coverage

Status: Not started.

## Goal

Finish the rollout by removing stale 12-unit cap assumptions, documenting the command-budget rule,
and adding focused regression coverage for the server/client contract. This phase should leave the
feature ready for playtesting and later tuning.

## Scope

- Remove obsolete 12-unit selection language, constants, comments, and tests.
- Update relevant docs:
  - `docs/design/hardening.md` for server rejection and defensive bounds
  - `docs/design/client-ui.md` for the selection grid and control-group behavior
  - `docs/design/balance.md` if command budget becomes a balance-tuning surface
  - `docs/context/` capsules if section lists or contract pointers shift
- Add or consolidate focused tests for:
  - server command-budget rejection
  - client selection budget helper
  - drag/double-click/shift selection admission
  - control-group save/add/recall
  - outgoing command send guard
  - HUD budget grid rendering
  - mirrored constants or generated config parity
- Collect factual patch-note bullets for the final merge:
  - selection/command bandwidth is supply-based
  - base command budget is 24 supply
  - each Command Car increases command budget by 12
  - Tanks consume 6 command supply using current authoritative balance

## Expected Deliverables

- No active code path still enforces the old 12 selected-unit cap.
- Docs describe the new command-budget rule and server rejection behavior.
- Focused regression coverage protects the budget rule, Command Car stacking, and UI display.
- Tuning constants are named and easy to change later.

## Verification

- Run the focused tests added or changed across this rollout.
- Run protocol/balance parity checks if Phase 1 introduced mirrored constants or generated config.
- Let the normal commit hook run the broad gate when committing the merge-ready phase unless the
  change is docs-only or the hook failure is confirmed unrelated.

## Manual Testing Focus

Play a short local match or scenario that exercises infantry selection, Tank-heavy selection,
Command Car-expanded selection, control groups, and normal combat orders. Watch specifically for
commands that appear selected but are rejected, over-noisy overflow feedback, and confusing grid
packing.

## Handoff Expectations

The final handoff must summarize the gameplay impact in plain language, list the patch-note bullets,
name the tests run, and identify any tuning concerns for playtests.
