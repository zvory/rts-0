# Phase 0 - Investigation and Fixtures

Goal: capture the current sliding/jiggle behavior well enough that later phases can prove they
improved it without guessing.

## Scope

This phase should add tests and, if useful, a local diagnostic harness. It should not change
gameplay behavior.

## Required Investigation

Inspect these code paths and write down how they interact in the PR summary or commit body:

- A* tile route creation in `server/src/game/pathfinding.rs`.
- Waypoint conversion in `server/src/game/services/pathing.rs`.
- Path request and final-goal snapping in `server/src/game/services/move_coordinator.rs`.
- Per-tick movement and tank body turning in `server/src/game/services/movement.rs`.
- Snapshot facing interpolation in `client/src/state.js`.
- Tank drawing in `client/src/renderer.js`.

## Suggested Fixtures

Add small Rust tests under the most local module possible. Prefer deterministic flat maps or
hand-authored obstacle maps inside tests.

Cover at least these cases:

- Long unobstructed diagonal movement produces a route with many tile waypoints today.
- A route around a rectangular obstacle includes a required corner.
- A tank's hull does not instantly snap to the destination angle when misaligned.
- Intermediate waypoint arrival does not require exact tile-center contact.

If an existing test already covers one of these, reference it and avoid duplication.

## Measurements to Capture

Do not invent subjective metrics. Record simple deterministic values:

- Original tile path length.
- World waypoint count.
- Number of heading changes above a threshold, for example 10 degrees.
- Whether a route has a legal straight segment from start to a later waypoint.

These measurements can be produced inside tests or as comments in test assertions. They should be
cheap and deterministic.

## Acceptance Criteria

- No gameplay behavior changes.
- At least one focused test demonstrates the current tile-center route shape that causes jitter.
- Tests use deterministic map setup and stable entity ids.
- The next phase has enough fixture coverage to validate line-segment legality.

## Do Not Do

- Do not change tank speed, turn rate, armor rules, or renderer behavior.
- Do not introduce client-only smoothing.
- Do not rewrite A*.

