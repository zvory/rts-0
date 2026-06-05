# Phase 2 - Static Clearance Field

## Goal

Build a deterministic map/occupancy clearance field so routing and local movement can prefer open
space before the scout car contacts walls or buildings.

## Rationale

Scout cars currently ask whether a pose is legal, but they do not score how close that pose is to
static blockers. A route that is barely legal is treated nearly the same as one centered in open
space. Clearance fixes the "always shortest path, always near walls" failure mode.

## Scope

- Compute distance-to-static-blocker for tiles.
- Include terrain blockers and building occupancy.
- Rebuild or incrementally refresh when building occupancy changes.
- Keep the representation coarse and deterministic; tile-level distance is enough for the first
  pass.
- Expose helpers for:
  - clearance at tile;
  - clearance near world point;
  - minimum clearance sampled along a segment or candidate motion.

## Cost Model

Use two thresholds:

- **Hard clearance:** below this, a route/candidate is invalid for scout cars.
- **Preferred clearance:** below this, add cost but do not forbid the move.

The preferred cost must taper in narrow passages. Do not make a two-tile or otherwise intended
chokepoint impossible just because both sides are close.

## Code Areas

- `server/src/game/services/occupancy.rs`
- `server/src/game/services/pathing.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/map.rs` if a map-level terrain distance helper is cleaner

## Tests

- Clearance is zero or minimal on blocked tiles.
- Clearance increases away from buildings and stone.
- Building construction changes the clearance field used by new path requests.
- A route through a wide open area prefers the center over a wall-hugging route.
- A narrow but legal passage remains traversable.

## Done When

- Scout-car code can ask "how much static clearance does this route/candidate have?"
- The clearance field is cheap enough to use inside pathing and local movement tests.
