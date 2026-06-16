# Phase 1 - Dormant Kind and Mirrored Metadata

Status: Pending.

## Goal

Add Tank Trap as a known entity kind with mirrored stats and protocol vocabulary, but keep it hidden
from normal worker construction UI until the blocker and line-placement systems are ready.

## Scope

- Add `TankTrap` / `tank_trap` identity to the rules kind enum and round-trip parsing.
- Add a `BuildingDef` for Tank Trap:
  - 200 HP
  - armored
  - 1x1 footprint
  - 15 steel, 0 oil
  - 0 sight
  - 10-second build time
  - no trains, no weapon, no supply
  - Training Centre build requirement
- Add the Tank Trap kind to protocol string vocabularies and compact kind codes.
- Mirror client metadata in `client/src/protocol.js` and `client/src/config.js`, including a
  readable label, icon, cost, footprint, sight `0`, build ticks, and requirement text.
- Add Tank Trap to faction building/catalog data only as needed for snapshots and future build
  eligibility. Do not add it to `WORKER_BUILDABLE` or the default worker build-card sequence in
  this phase unless Phase 0 explicitly revised the exposure plan.
- Update `docs/design/protocol.md` and `docs/design/balance.md`.
- Add focused tests for kind parsing/protocol parity/client metadata if existing tests make that
  cheap.

## Expected Deliverables

- Tank Trap kind round-trips between Rust rules, protocol DTOs, compact snapshots, and client
  constants.
- Balance docs list Tank Trap stats and explain that it is a vehicle obstacle, not an elimination
  building.
- Worker build UI still does not expose Tank Trap.
- No movement, construction, pathing, or placement behavior depends on Tank Trap yet.

## Out of Scope

- Static blocker behavior.
- Construction acceptance.
- Elimination rules.
- Renderer art.
- Worker build-card exposure.
- Line placement.

## Verification

- Run focused Rust tests for kind parsing or rule definitions touched by this phase.
- Run the smallest existing protocol/client parity check that covers kind codes if the compact
  protocol changes.
- Run `cargo fmt` for touched Rust crates.

## Manual Testing Focus

None required. The kind should remain hidden from normal gameplay.

## Handoff Expectations

The handoff must identify the assigned compact kind code, all mirrored stat surfaces updated, tests
run, and whether Tank Trap remains unbuildable from the worker UI.
