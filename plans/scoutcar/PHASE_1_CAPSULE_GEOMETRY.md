# Phase 1 - Capsule Geometry

## Goal

Replace the scout car's authoritative collision body with a capsule/pill shape so the corners are
shaved off. The visible unit can remain a truck; the server body should match the intended
navigability.

## Rationale

The current scout car body is an oriented rectangle. That preserves worst-case corners and makes
tight turns around building corners more brittle than the unit fantasy requires. A capsule still
has length and facing, but its rounded ends reduce snagging.

## Scope

- Add a `Capsule` or `OrientedCapsule` body variant to geometry.
- Use the capsule only for scout cars at first.
- Keep tanks on oriented boxes.
- Update standability, body AABB, unit-unit overlap, unit-building overlap, and segment queries.
- Keep client visuals truck-like; optional debug overlays may draw the authoritative capsule.

## Code Areas

- `server/src/game/services/geometry.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/services/movement/collision.rs`
- `server/src/game/invariants.rs`
- `client/src/config.js` if client advisory body metadata changes
- `client/src/renderer/shared.js` only for optional debug/advisory rendering

## Tests

- Capsule AABB covers both round ends and the straight body.
- Capsule vs building rectangle intersection rejects clipping.
- Capsule vs circle and capsule vs oriented box overlap resolve deterministically.
- Scout car can stand in positions where the former rectangular corner would clip but the capsule
  is clear.
- Scout car still cannot pass through one-tile diagonal pinches or building footprints.

## Non-Goals

- Do not change tank geometry.
- Do not tune speed, weapon behavior, or economy.
- Do not introduce physics forces.

## Done When

- Scout-car static legality uses the capsule body everywhere the server currently uses the oriented
  vehicle body.
- Tests prove the capsule is less corner-snaggy without permitting wall/building overlap.
