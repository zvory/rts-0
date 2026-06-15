# Phase 0 - Inventory and Runtime Contract

Status: Not Started.

## Goal

Inventory the current ability, world-effect, projection, client preview, and test surfaces before
changing behavior. Define the shared naming and contracts that later phases will implement.

## Scope

- Inventory current ability metadata and execution:
  - `server/crates/rules/src/faction.rs`
  - `server/crates/sim/src/game/ability.rs`
  - `server/crates/sim/src/game/services/ability_orders.rs`
  - `server/crates/sim/src/game/hero_abilities.rs`
  - `server/crates/sim/src/game/services/order_queue.rs`
- Inventory existing non-entity world state patterns:
  - `server/crates/sim/src/game/smoke.rs`
  - `server/crates/sim/src/game/mortar.rs`
  - `server/crates/sim/src/game/artillery.rs`
  - `server/crates/sim/src/game/snapshot.rs`
  - `server/crates/sim/src/rules/projection.rs`
- Inventory wire and client mirrors:
  - `server/crates/contract/src/lib.rs`
  - `server/crates/protocol/src/lib.rs`
  - `server/src/protocol.rs`
  - `client/src/protocol.js`
  - `client/src/config.js`
- Inventory client ability UI:
  - `client/src/state.js`
  - `client/src/input/commands.js`
  - `client/src/minimap.js`
  - `client/src/hud_command_card.js`
  - `client/src/renderer/feedback.js`
  - `client/src/renderer/index.js`
- Inventory tests and checks that later phases should extend:
  - focused Rust sim tests around command validation, snapshots, fog privacy, and ability orders
  - `tests/client_contracts.mjs`
  - `tests/protocol_parity.mjs`
  - `scripts/check-client-architecture.mjs`
  - `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- Decide and record:
  - the server type names for active ability instances and projected ability world objects
  - the initial object kinds needed by this plan, such as return marker, anchor, and projectile
  - whether projectile visuals are projected as active objects, transient events, or both
  - the recast command shape to be implemented in Phase 4
  - whether the existing `ekatTeleport` and `ekatLineShot` ids should be repurposed for the new
    dash and projectile behavior or replaced with new ids; choose the convenient path, but do not
    preserve the old immediate teleport or immediate line-damage semantics as product behavior
  - the projectile return-target contract, including Ekat projectiles returning toward Ekat's current
    position rather than a fixed launch origin
  - how anchors are damaged or targeted in Phase 8 without becoming ordinary production entities

## Expected Deliverables

- This phase document updated with inventory notes and explicit decisions.
- A short implementation map naming the files each later phase should touch first.
- No gameplay behavior changes.
- No protocol or client behavior changes unless the phase only updates planning/design notes.

## Out of Scope

- Adding the runtime store.
- Adding new protocol fields.
- Implementing dash, return, projectile, or anchor behavior.
- Retuning Ekat stats or cooldowns.

## Verification

- Use `rg`/`fd` inventories and cite concrete files in the phase notes.
- No automated suite is required if this phase only updates planning notes.

## Manual Testing Focus

None. This is a contract and inventory phase with no intended player-facing change.

## Handoff Expectations

The handoff must name the chosen runtime/object names, the recast command shape, the old-id
repurpose/removal decision, the projectile return-target contract, the anchor targetability decision,
the projection strategy for projectile visuals, and the exact files Phase 1 should edit first.
