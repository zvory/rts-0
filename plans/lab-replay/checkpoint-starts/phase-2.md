# Phase 2 - Lab Start and Import Checkpoints

Status: Not started.

## Scope

Make blank labs, catalog labs, and imported lab setups establish a baseline `GameCheckpoint`. If the
old lab scenario JSON remains temporarily, treat it as an adapter into checkpoint state rather than
a separate long-term contract. A lab import or baseline reset should produce a new baseline
checkpoint and clear the current-branch action log.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs`
- `server/src/lobby/room_task/lab.rs`
- Lab scenario tests
- `docs/design/protocol.md` if lab artifact shape changes

## Verification

- Run focused lab import/export tests.
- Add tests for blank lab, catalog lab, and imported setup baseline checkpoint creation.

## Manual Testing Focus

Open blank and catalog labs, import a setup, and confirm entities and controls still appear
correctly.

## Handoff

The handoff must explain whether old lab scenario files remain accepted and how they map into
checkpoints.
