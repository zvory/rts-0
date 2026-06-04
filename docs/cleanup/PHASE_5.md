# Phase 5 - Combat Service Decomposition

Goal: split `server/src/game/services/combat.rs` into combat components while preserving fog-safe
events, target semantics, and tick safety.

## Target Components

- `services/combat/mod.rs`: `combat_system`, combat mode selection, and orchestration.
- `services/combat/acquisition.rs`: target resolution, auto-acquisition, line-of-sight checks, and
  stale target handling.
- `services/combat/chase.rs`: chase goal calculation, tank standoff goals, path refresh decisions,
  and attack-move resume behavior.
- `services/combat/weapons.rs`: tank turret rotation, machine-gunner setup/teardown readiness, and
  weapon/body facing helpers.
- `services/combat/damage.rs`: damage application, death handling, score/stat updates, and victim
  resolution.
- `services/combat/projection.rs`: shot blockers, segment intersection helpers, and terrain/entity
  obstruction handling.
- `services/combat/events.rs`: attack events and under-attack notices gated by fog/visibility.
- `services/combat/tests.rs`: behavior tests grouped around acquisition, damage, blockers, and
  fog-safe event emission.

## Design Notes

`combat_system` should remain the only combat service entry point from `systems.rs`. Avoid letting
target acquisition own pathing or movement state directly; chase/standoff should continue to go
through `MoveCoordinator`.

Fog-safe event emission is a security boundary. Keep visibility checks explicit and near the event
helpers so future combat feedback cannot leak hidden positions or target ids.

## Tests

- Run `cargo test` in `server/`.
- Add targeted tests if visibility helper extraction broadens APIs.
- Run regression tests if event payload shape or fog behavior changes.

## Done

- Combat orchestration is readable at the system level.
- Acquisition, chase, weapon readiness, damage, projection, and event emission have separate
  ownership.
- Existing combat behavior and fog filtering are unchanged.

