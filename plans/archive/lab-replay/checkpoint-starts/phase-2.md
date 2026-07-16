# Phase 2 - Lab Start and Import Checkpoints

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Make blank labs, catalog labs, and imported lab setups establish a baseline `GameCheckpoint`.
"Lab Scenario" should become a product label over checkpoint-backed lab setup payloads, not a
separate long-term legacy setup serialization contract. If old scenario JSON remains
temporarily, it must be an explicit adapter into checkpoint state. A lab import or baseline reset
should produce a new baseline checkpoint and clear the current-branch action log.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lab_scenarios.rs`
- `server/assets/lab-scenarios/**` and manifest data if persisted setup shape changes
- `client/src/lab_client.js`
- `client/src/lab_panel.js`
- `client/src/lab_scenario_authoring.js`
- `client/src/lab_scenario_authoring_flow.js`
- Lab scenario tests
- `docs/design/protocol.md` and `docs/design/client-ui.md` if lab artifact/import/export shape
  changes

## Verification

- Run focused lab import/export tests.
- Add tests for blank lab, catalog lab, and imported setup baseline checkpoint creation.
- Add or adjust catalog and authoring tests if the persisted lab setup shape changes.

## Manual Testing Focus

Open blank and catalog labs, import a setup, and confirm entities and controls still appear
correctly.

## Handoff

The handoff must explain whether old lab scenario files remain accepted, how they map into
checkpoints, and which user-visible "Lab Scenario" labels remain as product copy.
