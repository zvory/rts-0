# Phase 0 - Contract and Architecture Inventory

Status: Pending.

## Goal

Confirm the exact implementation contract for Tank Traps before any implementation file changes.
Inventory the current server and client seams that will be affected by vehicle-only static blockers
and line construction.

## Scope

- Read the relevant capsules before editing this phase note:
  - `docs/context/planning.md`
  - `docs/context/server-sim.md`
  - `docs/context/client-ui.md`
  - `docs/context/protocol.md`
  - `docs/context/balance.md`
- Inventory server identity and rule data:
  - `server/crates/rules/src/kind.rs`
  - `server/crates/rules/src/defs.rs`
  - `server/crates/rules/src/economy.rs`
  - `server/crates/rules/src/combat.rs`
  - `server/crates/rules/src/faction.rs`
- Inventory protocol and compact kind-code mirrors:
  - `server/crates/protocol/src/lib.rs`
  - `server/src/protocol.rs`
  - `client/src/protocol.js`
  - `docs/design/protocol.md`
- Inventory construction and command distribution:
  - `server/crates/sim/src/game/command.rs`
  - `server/crates/sim/src/game/services/commands.rs`
  - `server/crates/sim/src/game/services/construction.rs`
  - `server/crates/sim/src/game/services/order_planner.rs`
  - `server/crates/sim/src/game/services/move_coordinator.rs`
- Inventory pathing and standability:
  - `server/crates/sim/src/game/services/occupancy.rs`
  - `server/crates/sim/src/game/services/standability.rs`
  - `server/crates/sim/src/game/services/pathing.rs`
  - `server/crates/sim/src/game/pathfinding.rs`
  - `server/crates/sim/src/game/services/movement/`
- Inventory elimination, fog, and remembered-building behavior:
  - `server/crates/sim/src/game/services/death.rs`
  - `server/crates/sim/src/game/snapshot.rs`
  - `server/crates/sim/src/game/fog.rs`
  - `server/crates/sim/src/game/building_memory.rs`
- Inventory client build UI and placement:
  - `client/src/config.js`
  - `client/src/hud_command_card.js`
  - `client/src/input/placement.js`
  - `client/src/input/index.js`
  - `client/src/input/commands.js`
  - `client/src/renderer/buildings.js`
- Identify focused tests to extend in later phases.

## Expected Deliverables

- A short implementation note appended to this phase file that answers:
  - which existing vehicle/body classification is canonical enough, or what new rules-level
    classification Phase 2 should introduce
  - which occupancy/pathing functions need movement-class-aware signatures
  - how under-construction Tank Traps enter vehicle blocker occupancy
  - which elimination helper must ignore Tank Traps
  - how zero-sight Tank Traps interact with own visibility and enemy remembered building projection
  - whether standard repeated `build` commands can support line placement without protocol changes
  - which client module should own Bresenham every-other-tile line generation
  - which targeted tests each later phase should extend
- No gameplay behavior changes.
- No UI behavior changes.

## Out of Scope

- Adding `TankTrap` to code.
- Changing pathing, construction, protocol, UI, or balance behavior.
- Running broad test bundles.

## Verification

- Use `rg` and focused file reads for the inventory.
- If this phase only updates the plan note, no automated suite is required.

## Manual Testing Focus

None. This phase is planning and inventory only.

## Handoff Expectations

The handoff must name the chosen blocker/movement abstraction, the files Phase 1 should edit for
dormant identity work, and any plan changes needed before implementation starts.
