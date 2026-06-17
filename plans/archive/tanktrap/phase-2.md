# Phase 2 - Movement-Class Static Blockers

Status: Done.

## Goal

Introduce a shared static blocker model so some static objects can block vehicle-body units without
blocking infantry. This phase should build the system that makes Tank Traps work naturally through
pathing and standability instead of scattered Tank Trap checks.

## Scope

- Add or confirm a rules-level movement/body classification that includes:
  - infantry/small ground units that ignore Tank Trap blockers
  - vehicle-body units that are blocked by Tank Trap blockers
- Prefer explicit paired classifications:
  - `StaticBlockerClass`: `AllGround`, `VehicleBodyOnly`, and possibly `None`
  - `MovementBodyClass`: `InfantryLike` and `VehicleBody`
  If Phase 0 proves an existing helper is canonical enough, record that choice instead of adding a
  parallel concept.
- Ensure Tank, Scout Car, Command Car, Anti-Tank Gun, Mortar Team, and Artillery use the
  vehicle-body blocker class.
- Refactor static occupancy so it can answer at least:
  - terrain passability
  - all-ground static blockers from normal buildings
  - vehicle-only static blockers from Tank Trap footprints
  - combined static clearance/fingerprint for the movement class being routed
- Update standability and movement path requests to ask passability for the moving unit's class.
- Make Tank Trap footprints block vehicle-body units while under construction and when complete.
- Preserve current behavior for ordinary buildings: they still block infantry and vehicles.
- Keep unit-unit soft overlap behavior unchanged.
- Add focused simulation tests for:
  - infantry can stand on and path through a Tank Trap tile
  - each vehicle-body kind rejects standing on a Tank Trap tile
  - vehicles naturally pass through a gap only when the body/clearance rules fit
  - vehicles cannot path through the diagonal corner gaps created by a line whose consecutive Tank
    Trap sites are diagonal-touching rather than a knight's move apart
  - if diagonal-touching Tank Trap sites still leave a vehicle-pathable gap, inflate Tank Trap
    static blocker clearance by 0.5 tiles for vehicle A* requests rather than adding line-placement
    special cases to pathfinding
  - under-construction Tank Traps already block vehicle-body units
  - ordinary buildings still block all ground movement

## Expected Deliverables

- A movement-class-aware occupancy/passability API with narrow call-site updates.
- Vehicle-only blocking works through shared pathing/standability semantics.
- Existing vehicle path smoothing and static-fingerprint caching remain deterministic and scoped to
  the blocker class that produced a path.
- Path cache fingerprints are scoped per movement/body class. A Worker and Tank path request against
  the same start/goal must not reuse an incompatible Tank Trap blocker result.
- `Game::tick()` stays panic-free for stale ids and malformed coordinates.

## Out of Scope

- Worker build-card exposure.
- Client line placement.
- AI strategic use of Tank Traps.
- New repair, cancel, or salvage mechanics.

## Verification

- Run focused Rust tests for occupancy, standability, pathing, and movement changes.
- Run `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  new service/module edges are introduced.
- Run `cargo fmt` for touched Rust crates.

## Manual Testing Focus

Use a debug/dev scenario or small local harness to inspect one infantry unit and one vehicle moving
through the same Tank Trap line. Confirm the infantry crosses and the vehicle routes around or stops
unless the gap is wide enough; specifically include a shallow or steep line that would have produced
knight-move trap spacing under a naive every-other-tile cadence.

## Handoff Expectations

The handoff must describe the new blocker/movement API, which unit kinds are vehicle-body blockers,
how path-cache fingerprints are scoped, tests run, and any vehicle movement edge cases left for
Phase 6 regression coverage.
