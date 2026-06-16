# Phase 0 - Contract and Architecture Inventory

Status: Done.

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
  - whether the current oriented-vehicle-body helper is intentionally equivalent to vehicle-body
    blocker class or only a geometry helper that Phase 2 must separate
  - which occupancy/pathing functions need movement-class-aware signatures
  - which server placement policy API Phase 3 should use for both issue-time and arrival-time
    validation
  - how under-construction Tank Traps enter vehicle blocker occupancy
  - which `is_building()` behaviors Tank Trap intentionally inherits and which ones it opts out of
  - which elimination helper must ignore Tank Traps
  - how zero-sight Tank Traps interact with own visibility and enemy remembered building projection
  - whether standard repeated `build` commands can support line placement without protocol changes
  - which client module should own Bresenham every-other-tile line generation
  - which client placement policy helper Phase 4 should use to mirror server policy advisory-side
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

## Implementation Note

Phase 0 inventory found no plan-blocking ambiguity. Later phases can proceed without a protocol
shape change if Tank Traps are represented as a normal building kind plus kind-specific movement
and placement policy.

### Movement and Static Blockers

- `server/crates/rules/src/kind.rs` has `uses_oriented_vehicle_body`, which currently matches the
  exact Tank Trap blocker target set: Anti-Tank Gun, Mortar Team, Artillery, Scout Car, Tank, and
  Command Car. Treat this as evidence of the current vehicle-body membership, not as the final
  Tank Trap abstraction.
- Phase 2 should introduce one explicit rules-level movement/body classification, for example
  `MovementBodyClass::{Infantry, VehicleBody}`, and map those six kinds to `VehicleBody`. The
  oriented-body helper is a geometry and movement-control helper, because it also drives facing,
  route shaping, body rotation, and pivot/car movement behavior. Reusing it directly as the
  static-blocker class would couple Tank Trap semantics to hull-render/path geometry.
- `server/crates/sim/src/game/services/occupancy.rs` is the central static blocker layer today:
  every `is_building()` footprint sets one `blocked` grid used by pathing, clearance, and
  `Passability`. Phase 2 should split this into at least all-ground static blockers and
  vehicle-only static blockers while keeping terrain in the static clearance fingerprint.
- Movement-class-aware signatures are needed in `Occupancy::passable`, `building_blocked_at_tile`,
  the clearance/fingerprint pathing call path, `standability::unit_static_standable*`,
  `unit_static_segment_standable`, `PathingService::request/request_tile_path`, and the movement
  helpers under `server/crates/sim/src/game/services/movement/` that call static standability.
  Existing all-building behavior should remain the default for ordinary buildings.
- Under-construction Tank Traps should enter vehicle-blocker occupancy as soon as
  `EntityStore::spawn_building(..., completed=false)` creates the scaffold in
  `construction_system`. Occupancy already includes under-construction buildings because it checks
  `is_building()` only; Phase 2 should preserve that behavior for vehicle blockers.

### Construction and Placement

- Server issue-time build validation is in `server/crates/sim/src/game/services/commands.rs`
  `order_build`; arrival-time authoritative validation is in
  `server/crates/sim/src/game/services/construction.rs` before resources are charged and the
  scaffold is spawned. Both currently call
  `standability::building_site_clear_for_build_intent`.
- Phase 3 should replace that direct helper call with one shared server placement policy selected
  by building kind, used at both issue time and arrival time. The policy should keep ordinary
  buildings on the current all-entity footprint rejection path and let Tank Trap reject terrain,
  resources, ordinary buildings, vehicle-body units, out-of-bounds coordinates, tech failure, and
  affordability failure, while allowing infantry overlap.
- `server/crates/sim/src/game/services/order_queue.rs` has queued-build preflight that also calls
  `building_site_clear_for_build_intent`; update it to the same policy so queued Tank Trap commands
  fail for the same reasons as immediate commands.
- Current repeated standard `build` commands can support line placement without a wire change.
  `client/src/protocol.js` already sends `{ c:"build", units, building, tileX, tileY, queued }`,
  and `order_planner.rs` distributes queued builds across selected workers by queue length. Phase 5
  can send one immediate build command for the first selected-worker sites, then send additional
  queued build commands for overflow sites against the selected worker set even when Shift is not
  held.

### Building Behavior Boundaries

- Tank Trap should inherit generic building identity for targetability, HP/combat damage,
  construction scaffolds/progress, death cleanup, snapshots, remembered-building projection, and
  command parsing that accepts `EntityKind::is_building()`.
- Tank Trap must opt out through explicit policy/catalog data from production anchors, rally,
  training, research, supply, sight/fog reveal, and elimination survival. Do not make it a
  production anchor in `server/crates/rules/src/faction.rs`.
- `Game::alive_players` currently survives on any `world_query::owned_buildings(...).next()`.
  Phase 3 should introduce a helper such as `world_query::owned_elimination_buildings` or
  `rules::building_counts_for_elimination(kind)` and use it there so Tank Traps do not keep a
  player alive. `Game::eliminate` can still remove Tank Traps as owned buildings.
- Zero sight is straightforward: `Fog::stamp_sight_at` returns immediately for radius 0, so owned
  Tank Traps will not reveal fog if their `BuildingStats.sight_tiles` is 0. Enemy Tank Traps will
  still be remembered through `building_memory.rs` once visible because memory keys off
  `entity.is_building()` and projected visibility.

### Phase 1 Identity Files

Phase 1 dormant identity should edit `server/crates/rules/src/kind.rs`,
`server/crates/rules/src/defs.rs`, `server/crates/rules/src/faction.rs`,
`server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`,
`server/crates/sim/src/protocol.rs` if its adapter mirror needs the new kind,
`client/src/protocol.js`, `client/src/config.js`, `docs/design/protocol.md`, and
`docs/design/balance.md`. Keep it out of `DEFAULT_WORKER_BUILDABLES` and the client
`WORKER_BUILDABLE` list until Phase 5.

### Client Ownership

- `client/src/input/placement.js` should own the advisory per-site placement policy helper in
  Phase 4, with names matching the server placement policy. It currently rejects any intersecting
  entity for all buildings through `footprintValidAgainstEntities`; Tank Trap should add a
  kind-aware path that allows infantry overlap and rejects vehicle-body overlap.
- `client/src/input/placement.js` is also the best owner for the Phase 5 Bresenham every-other-tile
  and diagonal-bridge line generation because it already owns placement hover, confirmation, and
  command-card hotkey helpers. `client/src/input/index.js` should continue to compose placement
  behavior rather than owning line math directly.
- `client/src/renderer/buildings.js` can render the placeholder hedgehog as a special building
  branch while preserving construction progress and remembered-building rendering. It already
  draws buildings generically from `STATS` footprint metadata.

### Targeted Tests for Later Phases

- Phase 1: `cargo test --manifest-path server/Cargo.toml -p rts-rules stable_kind_ids_round_trip
  every_entity_kind_has_exactly_one_def`, `cargo test --manifest-path server/Cargo.toml -p
  rts-protocol`, `node tests/protocol_parity.mjs`, and
  `node scripts/check-faction-catalog-parity.mjs`.
- Phase 2: focused sim tests in `occupancy.rs`, `standability.rs`, `pathing.rs`, and
  `movement/tests.rs` for infantry pass-through, vehicle-body rejection, two-tile vehicle gaps,
  diagonal pinch behavior, and under-construction blockers.
- Phase 3: focused command/construction/elimination/fog-memory tests in `commands.rs`,
  `construction.rs`, `game/mod.rs` or `game/tests.rs`, `fog.rs`, `building_memory.rs`, and
  `snapshot_memory_tests.rs`.
- Phase 4: client placement/render checks around `placement.js`, `renderer/buildings.js`, command
  card descriptors as needed, plus `node scripts/check-client-architecture.mjs`.
- Phase 5: client line-generation unit coverage for horizontal, vertical, diagonal, shallow, steep,
  invalid-site skip, selected-worker immediate dispatch, and overflow queued build dispatch.
- Phase 6: integrate the above into dev scenarios/self-play smoke coverage proving vehicles cannot
  pass Tank Trap lines while infantry can.
