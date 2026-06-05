# Phase 0 - Baseline and Debug Harness

## Goal

Make scout-car failures measurable before changing behavior. The plan should not rely on subjective
"feels better" checks alone; we need fixtures that prove tight-space movement improved and that no
new clipping was introduced.

## Scope

- Add focused scout-car movement fixtures around buildings, stone walls, narrow lanes, and base
  traffic.
- Add debug-only movement metrics that can be asserted in tests or printed by self-play tools.
- Preserve current gameplay behavior except for test harness support.

## Code Areas

- `server/src/game/services/movement/tests.rs`
- `server/src/game/services/pathing.rs`
- `server/src/game/services/standability.rs`
- optional debug helpers under existing test-only modules

## Fixtures

- **Factory corner graze:** scout car routes around a factory corner from each cardinal direction.
- **Two-building alley:** scout car traverses a two-tile lane without hitting corner deadlocks.
- **Diagonal pinch:** scout car avoids diagonal one-tile pinches but uses valid wider corridors.
- **Wall-parallel lane:** scout car ordered along a wall should not repeatedly aim into the wall.
- **Blocked nose recovery:** scout car starts legal but with its front close to a building.
- **Traffic compression:** infantry/tank traffic near a factory should not wedge the scout car
  forever.

## Metrics

Track these in tests where useful:

- ticks to reach destination;
- minimum static clearance during route;
- count of blocked static step attempts;
- count of reverse recovery activations;
- count of repaths;
- maximum consecutive no-progress ticks;
- whether any accepted pose is statically illegal.

## Done When

- There are failing or fragile fixtures that reproduce the current wall-riding/stuck behavior.
- The fixtures can distinguish "arrived cleanly" from "arrived after repeated wall ramming."
- The tests are deterministic across repeated runs.
