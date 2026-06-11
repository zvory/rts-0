# Phase 1: Server-Side Building Memory

Status: implemented

## Goal

Create authoritative per-player memory of enemy buildings that have been seen. The memory should
record the latest state that was visible to that player and be usable by server systems without
consulting the client.

## Scope

- Add a memory store owned by `Game`, likely near `fog.rs` or as a new `building_memory.rs` module.
- Record one entry per `(player_id, building_entity_id)` for non-neutral buildings visible to that
  player.
- Store only fields that are safe and useful as stale intel: id, owner, kind, position, footprint,
  hp/build progress as last seen, construction/completion state as last seen, and the tick observed.
- Refresh records after current fog/smoke visibility is recomputed and before systems that need
  memory run.
- Decide and encode lifecycle rules for dead buildings:
  - If a building dies while visible to the player, update memory to dead/destroyed or remove it,
    depending on what later artillery/UI logic needs.
  - If a building dies while hidden, leave the stale record until the player scouts the location
    again or a later phase defines inferred removal.
- Keep records server-only in this phase. Do not add snapshot fields yet.

## Important Design Choices

- Memory should use the recipient player's actionable/current visibility, not client-side explored
  state.
- Smoke must suppress refreshes when it suppresses visibility.
- Lingering death vision needs an explicit choice. Recommended default: allow memory refresh from
  whatever `snapshot_for` would allow the player to see, but do not let death vision validate new
  commands that current actionable fog would reject.
- Construction sites count as buildings once visible because they can block movement and can be
  artillery targets.
- Neutral resource nodes are out of scope.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/fog.rs`
- `server/crates/sim/src/game/systems.rs`
- New `server/crates/sim/src/game/building_memory.rs` or equivalent
- Focused unit tests under `server/crates/sim/src/game/`
- `docs/design/server-sim.md` if the fog/memory contract is documented there

## Verification

- `cd server && cargo test building_memory`
- `cd server && cargo test fog`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- Broader `cd server && cargo test` if the module touches tick ordering or `Game` construction.

## Manual Testing Focus

- Scout an enemy building, move vision away, and confirm server tests show the record remains.
- Confirm never-scouted enemy buildings do not create records.
- Confirm smoke-covered buildings do not refresh memory while hidden.
- Confirm visible destruction updates or removes memory according to the implemented lifecycle.

## Handoff

The handoff should state the memory data model, tick ordering, lifecycle behavior for hidden
destruction, and whether death vision refreshes memory. It should tell the next agent to wire
artillery target selection to this store without exposing memory over the protocol yet.
