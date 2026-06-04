# Phase 3 - Entity Model Decomposition

Goal: split `server/src/game/entity.rs` into stable entity model modules while keeping `Entity` and
`EntityStore` easy for services to use.

## Target Components

- `entity/mod.rs`: public re-exports and high-level module documentation.
- `entity/kind.rs`: `EntityKind`, protocol string conversion, and kind classification helpers.
- `entity/order.rs`: `Order`, move/attack/gather/build intent structs, execution structs, and phase
  enums.
- `entity/state.rs`: `MovementState`, `CombatState`, `ProductionState`, `ConstructionState`,
  `WorkerState`, `ResourceNodeState`, `EntityStateGroups`, and `WeaponSetup`.
- `entity/entity.rs`: the `Entity` record and entity methods.
- `entity/store.rs`: `EntityStore`, id allocation, iteration, lookup, and retention behavior.
- `entity/tests.rs`: broad entity/store tests that do not fit a smaller inline module.

## Design Notes

This phase touches a foundational type used by every service. Keep external names stable by
re-exporting from `entity/mod.rs` so callers can continue using `crate::game::entity::EntityKind`,
`Entity`, `Order`, and `EntityStore`.

Do not use this phase to alter order semantics. The split should make the state machines easier to
see, not change how systems interpret them.

## Tests

- Run `cargo test` in `server/`.
- Pay attention to compile errors that indicate accidental visibility broadening. Prefer
  `pub(super)` inside the entity module tree when possible.

## Done

- Services import the same public entity names as before.
- Orders, state groups, entity helpers, and store logic are independently navigable.
- No simulation behavior changed.

