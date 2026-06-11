# Phase 5: Perspective-Aware Occupancy And A* Pathing

Status: planned

## Goal

Route player units using the map as that player understands it: terrain plus currently visible and
remembered building blockers, excluding never-seen hidden buildings.

## Scope

- Add an occupancy view variant that can be built from:
  - immutable terrain blockers,
  - live visible/owned buildings,
  - remembered enemy buildings from phase 1,
  - and optionally full live buildings for authoritative-only systems.
- Pass the requesting owner/player perspective into movement path requests.
- Keep full live occupancy for collision/damage/placement systems that must remain authoritative.
- Make path cache keys include the occupancy perspective/fingerprint so paths are never reused
  across incompatible blocker views.
- Keep debug path output owner-only and consistent with the path actually planned for that owner.

## Important Design Choices

- The movement planner may produce a path through an unseen live building. The movement executor
  must still obey live collision/occupancy and cannot phase through the structure.
- Terrain remains omniscient because the map terrain is known; this phase is about building
  blockers.
- Owned buildings should always block the owner because the owner knows them.
- Currently visible enemy buildings should refresh memory and block planning.
- Remembered enemy buildings should block planning if phase 4 accepts that player belief should
  affect route choice.

## Expected Touch Points

- `server/crates/sim/src/game/services/occupancy.rs`
- `server/crates/sim/src/game/services/pathing.rs`
- `server/crates/sim/src/game/services/move_coordinator.rs`
- `server/crates/sim/src/game/services/movement/`
- Phase 1 memory module
- `docs/design/server-sim.md`

## Verification

- Unit tests for occupancy fingerprints and passability differences by perspective.
- Unit tests for pathing through never-seen hidden buildings and around remembered buildings.
- `cd server && cargo test pathing`
- `cd server && cargo test movement`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Testing Focus

- Units should plan paths through never-seen hidden wall-offs.
- After scouting a wall-off, newly issued move orders should route around it if possible.
- Debug path overlays should match the owner's perspective, not omniscient blockers.
- Other players should not inherit the scouting player's path knowledge.

## Handoff

The handoff should name every path request still using full live occupancy and justify why. It
should tell the next agent where live blockage discovery is still missing or partial.
