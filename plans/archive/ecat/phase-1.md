# Phase 1 - Server Ability Runtime Skeleton

Status: Done.

## Goal

Add a server-side active ability runtime skeleton that can own persistent ability state independent
of entities, smoke clouds, mortar shells, or artillery shells. Do not expose new gameplay yet.

## Scope

- Add a new sim-local module for ability runtime state, for example
  `server/crates/sim/src/game/ability_runtime.rs` or a similarly named module chosen in Phase 0.
- Add typed data for:
  - stable runtime object ids that are never reused during a match
  - owning player id
  - source caster id
  - ability kind
  - object kind
  - world position
  - created tick
  - expiry tick
  - optional per-object payload needed by later phases
- Add a store to `Game`, clone it for replay keyframes, initialize it in match setup, and tick it
  from `systems.rs` at an explicit lifecycle point.
- Implement expiry and stale-caster cleanup rules without relying on panics or `unwrap()`.
- Keep the store private to `rts-sim` and free of server, lobby, Tokio, AI, and client concerns.
- Add focused tests for id allocation, expiry, deterministic clone behavior, and stale caster
  cleanup.

## Expected Deliverables

- A compiled server ability runtime store with no player-facing behavior.
- `Game::tick()` still runs through the documented derived-state boundaries.
- Replay keyframe cloning preserves active runtime state.
- The new module is accepted by the sim architecture check or has a narrow documented baseline
  update if the architecture checker requires it.

## Out of Scope

- Protocol projection.
- Client rendering.
- Recast command behavior.
- Dash, projectile, or anchor implementation.
- Generic scripting or data-driven effect execution.

## Verification

- Run targeted Rust tests for the new runtime module.
- Run `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  a new sim module or service edge is added.
- Run `cargo fmt` for touched Rust crates.

## Manual Testing Focus

None required beyond confirming a normal local match still starts if the phase touches setup or
tick wiring.

## Handoff Expectations

The handoff must identify the runtime module name, tick order placement, store fields that are
stable for Phase 2 projection, tests added, and any architecture-check baseline or follow-up.
