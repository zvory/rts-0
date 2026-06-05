# Phase 6 - Traffic and Group Movement

## Goal

Make scout cars reliable in mixed-unit traffic after static routing and local planning are fixed.

## Rationale

Static geometry is the main failure source, but scout cars also get pushed or throttled into bad
poses by nearby units. Traffic should bias candidate selection without allowing dynamic units to
make the car clip static blockers.

## Scope

- Feed nearby-unit penalties into the scout-car candidate scorer.
- Prefer candidates with lower predicted unit overlap.
- Keep heavy/braced units as strong traffic blockers.
- Let soft infantry influence the score without making scout cars freeze unnecessarily.
- Keep hard unit-unit collision cleanup deterministic and separate from static legality.

## Group Movement

- Formation goals for scout cars should use capsule standability and clearance preference.
- Mixed groups should avoid assigning scout cars to goal positions that force immediate wall turns.
- If multiple scout cars share a destination, add deterministic offsets or route corridor variation
  so they do not all chase the same narrow point.

## Code Areas

- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/movement/scout_car.rs`
- `server/src/game/services/movement/collision.rs`
- `server/src/game/services/spatial.rs`

## Tests

- Scout car behind infantry near a building waits or takes an open arc instead of pushing into the
  building.
- Multiple scout cars ordered through a lane do not permanently wedge each other.
- Mixed tank/scout/infantry group movement keeps all vehicle bodies statically legal.
- Formation goal assignment avoids wall-adjacent scout-car goals when safer nearby goals exist.
- Traffic behavior is deterministic across repeated runs.

## Done When

- Static obstacle fixes hold under realistic unit traffic.
- Scout cars can move with armies without becoming the first unit wedged against base structures.
