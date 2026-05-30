# PLAN

This is the working plan for building Bewegungskrieg in serial order with one or two agents.
It incorporates the previous `TODO.md` and `ARCHITECTURE_PLAN.md`.

Use this file as the operational source of truth. The main rule is simple: do not pull a large
RTS feature forward unless its listed gates are complete.

## Coordination Rules

- With one agent, proceed strictly by phase and task order.
- With two agents, Agent A should own the current server/simulation phase. Agent B may take only
  tasks explicitly marked `parallel-safe`.
- Do not edit the same module from two worktrees at the same time.
- Shared contracts require extra care: `DESIGN.md`, `server/src/protocol.rs`,
  `client/src/protocol.js`, `server/src/config.rs`, `client/src/config.js`, command handling,
  fog/snapshot code, and pathing/movement code.
- Branches must start with `zvorygin/`.
- Prefer `go test ./...` for Go repos; this repo is Rust/JS, so use the repo commands in
  `CLAUDE.md`.

## Foundation Already Done

- [x] Replace stringly entity kind checks in hot simulation paths with typed internal enums.
- [x] Split `systems.rs` into internal services.
- [x] Add spatial query layer.
- [x] Introduce `PathingService` boundary.
- [x] Extend map/passability around movement classes.
- [x] Rename the game to Bewegungskrieg.
- [x] Rename gas/minerals to oil/steel.
- [x] Enforce map-generation resource fairness.
- [x] Add tick-stamped command log and deterministic replay harness.
- [x] Command card grid hotkeys.
- [x] Feedback on move and attack commands.
- [x] Selected unit command card for hold/move/attack/stop.
- [x] Building production progress bars.
- [x] Defeat screen keeps latest map visible.
- [x] Larger command card.
- [x] Non-rifleman/non-worker units require oil.
- [x] Slower infantry, workers, and AT gun.
- [x] Darker silhouette for impassable terrain.
- [x] Larger tanks.
- [x] Fix workers getting stuck inside buildings after construction.

## Phase 1: Safety Harness Before More Simulation Complexity

Goal: make future simulation changes easy to verify and hard to silently break.

### 1.1 Tick Invariant Checks

- [x] Add `Game::assert_invariants()` for tests/debug builds.
- [x] Check entity id/store-key consistency.
- [x] Check no NaN or out-of-world coordinates.
- [x] Check supply equals living plus queued units.
- [x] Check buildings never overlap.
- [x] Check resource-node miner reservations are valid or ignored.
- [x] Check orders do not point at invalid required targets except in documented transition windows.
- [x] Check fog grids exist for all players and never for neutral owner.
- [x] Check snapshots never expose hidden enemy ids through entities, targets, or events.

Gates:

- Required before full unit collision.
- Required before forests/LoS blockers.
- Required before larger maps.
- Required before machine-gunner setup/teardown.
- Required before construction resumption.

### 1.2 Replay World Hashing

- [ ] Add deterministic `WorldHash` per tick over entity ids, kinds, owners, positions, hp,
  orders, resources, production/construction state, and fog-relevant state.
- [ ] Add first-divergent-tick diagnostics for replay mismatch.
- [ ] Add golden replay fixtures under `server/tests/replays/`.
- [ ] Add generated legal/illegal command-stream tests where practical.

Gates:

- Required before larger maps.
- Required before snapshot deltas.
- Required before advanced AI tech progression.

### 1.3 Executable Design Contracts

- [ ] Add or strengthen protocol mirror checks.
- [ ] Add or strengthen server/client balance/config mirror checks.
- [ ] Add system-order tests.
- [ ] Add command hardening tests.
- [ ] Add fog leak tests for position-bearing events.

Gates:

- Required before forests/LoS blockers.
- Required before factions.
- Required before TOML data files.
- Required before snapshot deltas.

Parallel-safe for Agent B:

- [ ] Restyle the main menu to be more WW2 themed and less sci-fi.
- [ ] Display "connection to server lost" when the connection drops.
- [ ] Display latency to server in milliseconds.
- [ ] Switch font to DIN 1451 Mittelschrift everywhere.

## Phase 2: Simulation Domain Boundaries

Goal: prevent `Entity`, commands, and order state from becoming a blob before adding richer RTS
mechanics.

### 2.1 Typed Component-Shaped Entity State

- [x] Split broad `Entity` fields into typed state groups while keeping `EntityStore` simple.
- [x] Introduce structures such as `MovementState`, `CombatState`, `ProductionState`,
  `ConstructionState`, `WorkerState`, and `ResourceNodeState`.
- [x] Add constructors per `EntityKind`.
- [x] Add tests that each `EntityKind` has exactly the expected state groups.
- [x] Avoid adding new top-level optional fields to `Entity` unless they apply to most kinds.

Gates:

- Required before full unit collision.
- Required before machine-gunner setup/teardown.
- Required before construction resumption.

### 2.2 Central World Query and Mutation Helpers

- [x] Add canonical helpers for owned units, completed buildings, town halls, visible entities,
  targetable enemies, resource reservation, building placement, spawn search, and path requests.
- [x] Move repeated scans and predicates behind helpers where practical.
- [x] Prefer helpers in new systems; hand-rolled scans need a clear local reason.

Gates:

- Required before forests/LoS blockers.
- Required before rally points.
- Required before AI GG/leave.
- Required before machine-gunner setup/teardown.
- Required before factions.
- Required before advanced AI tech progression.
- Required before workers-not-auto-attacking unless handled by defs first.

### 2.3 Formal Command Processor

- [ ] Split command handling into normalize/cap, authorize, validate, reserve, domain action,
  and apply steps.
- [ ] Return typed command errors such as `NotEnoughSupply`, `RequirementNotMet`,
  `InvalidTarget`, `NoPath`, and `CannotBuildThere`.
- [ ] Convert typed errors into `notice` events only at the boundary.
- [ ] Keep AI, replay, and human clients on the same command path.

Gates:

- Required before factions.
- Required before advanced AI tech progression.

### 2.4 Explicit Order State Machines

- [x] Separate order intent from execution state.
- [x] Model gather, build, attack, rally, and future setup/teardown as explicit state machines.
- [x] Add transition tests for stale target, stop, death, retarget, no path, ownership loss,
  cancel, interrupt, resume, and completion.

Gates:

- Required before full unit collision.
- Required before rally points.
- Required before machine-gunner setup/teardown.
- Required before construction resumption.

### 2.5 Correct Construction Resumption

- [ ] Fix building construction so a worker pulled away does not leave the site permanently
  unbuildable.
- [ ] Support worker assignment, interruption, reassignment, cancellation, resumption, and
  completion as explicit transitions.
- [ ] Add invariants/tests for orphaned construction sites.

Depends on:

- Phase 2.1 typed entity state.
- Phase 2.2 world helpers.
- Phase 2.4 order state machines.
- Phase 1.1 invariants.

Parallel-safe for Agent B:

- [ ] Lobby system to see active lobbies.
- [ ] Basic settings menu, including surrender UI. Surrender should call a small
  `Game::eliminate(player)`-style API.
- [ ] Muzzle flare animations if they stay client-side and do not alter event semantics.
- [ ] Find a source of copyright-free assets for units, buildings, resources, and UI.

## Phase 3: Data Definitions and Balance Surface

Goal: make units, buildings, weapons, resources, tech requirements, and factions data-driven
before faction/unit-specific mechanics sprawl across systems.

### 3.1 Shared Definition Registry

- [ ] Introduce authoritative `UnitDef`, `BuildingDef`, `WeaponDef`, `ResourceDef`,
  `FactionDef`, and `TechRequirement` structures.
- [ ] Keep definitions as Rust constants initially if that is faster.
- [ ] Make systems query definitions instead of adding new special cases.
- [ ] Add server/client config generation or mirror validation.

Gates:

- Required before forests/cover weapon interactions.
- Required before machine-gunner setup/teardown.
- Required before factions.
- Required before TOML data files.
- Required before advanced AI tech progression.
- Strongly preferred before workers-not-auto-attacking.

### 3.2 Workers Should Not Auto Attack

- [ ] Represent worker combat behavior as data, e.g. `auto_acquire: false` or
  `attack_requires_order: true`.
- [ ] Route target acquisition through canonical world/combat predicates.
- [ ] Add tests that workers can attack if explicitly ordered, if desired, but do not auto-acquire.

Depends on:

- Phase 3.1 definitions, or a narrowly scoped interim combat flag with a migration path.
- Phase 2.2 world helpers.

### 3.3 TOML Data Files

- [ ] Move definitions to TOML only after the in-memory schema is stable.
- [ ] Add schema validation.
- [ ] Generate or validate the client-facing config subset.
- [ ] Keep protocol strings and data ids stable.

Depends on:

- Phase 3.1 definitions.
- Phase 1.3 executable contracts.

## Phase 4: Movement, Rally, Collision, and Map Scale

Goal: handle the hard movement work before larger maps and real collision amplify pathing debt.

### 4.1 Movement and Pathing Coordinator

- [x] Add one coordinator for movement/path requests.
- [x] Own path request budgeting per tick.
- [x] Add shared paths or flow-field-style support for large selected groups where practical.
- [x] Add goal spreading around target points.
- [x] Add spawn-point search around buildings.
- [x] Add `PathFailed` semantics.
- [x] Add repath throttling and cache invalidation.
- [x] Route commands through the coordinator instead of directly creating per-unit A* paths.

Gates:

- Required before full unit collision.
- Required before larger maps.
- Required before forests/vehicle blockers.
- Required before rally points if rally immediately path-orders produced units.

### 4.2 Building Rally Points

- [ ] Add rally target state to production buildings.
- [ ] Add command/protocol/client support for setting rally points.
- [ ] On unit production completion, spawn safely and issue the rally order through the movement
  coordinator.
- [ ] Add tests for rally to reachable, unreachable, and blocked points.

Depends on:

- Phase 2.2 world helpers.
- Phase 2.4 order state machines.
- Phase 4.1 movement coordinator.

### 4.3 Unit Collision and Non-Stacking

- [x] Implement unit collision and non-stacking.
- [x] Preserve mining-worker exceptions through worker/gather state, not movement hacks.
- [x] Add no-overlap invariants, path-failure behavior, blocked-goal behavior, and group-move tests.

Follow-ups:

- [x] Re-enable `scripted_self_play_exercises_economy_tech_and_combat`: subsumed by the
  "reserve on arrival" build model. Buildings no longer spawn at command apply time, so
  the worker's path goal stays on a walkable tile until it arrives.

Depends on:

- Phase 1.1 invariants.
- Phase 2.1 typed entity state.
- Phase 2.4 order state machines.
- Phase 4.1 movement coordinator.

### 4.4 Maps Twice as Large

- [ ] Increase map dimensions.
- [ ] Measure pathing, snapshot, fog, and AI cost.
- [ ] Keep replay/world-hash tests green.

Depends on:

- Phase 1.2 replay world hashing.
- Phase 4.1 movement coordinator.
- Phase 1.1 invariants.

## Phase 5: Visibility, Forests, and Snapshot Transport

Goal: add richer terrain and network efficiency without leaking hidden information.

### 5.1 Visibility Filter / Snapshot Builder

- [ ] Create a single outbound visibility boundary for entities, events, target ids, projectiles,
  attack alerts, minimap data, and future deltas.
- [ ] No system should directly decide what an enemy player may know.
- [ ] Add tests for each position-bearing event.

Gates:

- Required before forests/LoS blockers.
- Required before snapshot deltas.
- Required before any new event carrying hidden ids or coordinates.

### 5.2 Forests: LoS Blockers, Cover, Infantry, and Tanks

- [ ] Generate large forest blobs.
- [ ] Forests block LoS unless the viewer is inside them.
- [ ] Infantry inside forests can attack and become visible when attacking.
- [ ] Forests provide cover/miss chance.
- [ ] Tanks cannot enter forests but can shoot into them.
- [ ] Add visibility, combat, pathing, and replay tests.

Depends on:

- Phase 3.1 definitions.
- Phase 2.2 world helpers.
- Phase 4.1 movement coordinator.
- Phase 5.1 visibility boundary.
- Phase 1.3 executable contracts.

### 5.3 Snapshot Baselines, Deltas, or Dirty Flags

- [ ] Preserve current `snapshot_for(player)` as the semantic oracle.
- [ ] Add baseline/delta equivalence tests.
- [ ] Ensure deltas preserve fog filtering and hidden-id behavior exactly.

Depends on:

- Phase 1.2 replay world hashing.
- Phase 5.1 visibility boundary.
- Phase 1.3 executable contracts.

## Phase 6: Advanced Unit Mechanics and Factions

Goal: add distinctive RTS mechanics after the engine has data definitions, command handling,
orders, visibility, and movement boundaries.

### 6.1 Machine Gunner Setup / Teardown

- [ ] Add setup command.
- [ ] Setup takes five seconds with no moving or shooting.
- [ ] Deployed machine gunner cannot move or rotate without a three-second teardown.
- [ ] Deployed mode has elevated damage and a fixed 40-degree field of fire.
- [ ] Add setup, teardown, arc, target, stop, death, and replay tests.

Depends on:

- Phase 2.1 typed entity state.
- Phase 2.4 order state machines.
- Phase 3.1 definitions.
- Phase 2.2 world helpers.
- Phase 5.1 visibility boundary if setup emits visible events.
- Phase 1.1 invariants.

### 6.2 Two Factions: Soviets and Germans

- [ ] Design faction differences before implementation.
- [ ] Add faction ids to player/match setup and definitions.
- [ ] Add faction-specific units, buildings, stats, and tech requirements.
- [ ] Update client UI/config generation.
- [ ] Add protocol/design tests and replay coverage.

Depends on:

- Phase 3.1 definitions.
- Phase 2.3 command processor.
- Phase 2.2 world helpers.
- Phase 1.3 executable contracts.

### 6.3 Advanced AI Tech Progression

- [ ] AI attacks once with riflemen.
- [ ] AI then ecos/techs to attack with machine gunner plus riflemen.
- [ ] AI then ecos/techs to attack with tank plus riflemen and machine gunners.
- [ ] AI reasons from definitions, tech availability, economy, army composition, and command
  results rather than duplicated constants.
- [ ] Keep AI replay deterministic.

Depends on:

- Phase 2.3 command processor.
- Phase 3.1 definitions.
- Phase 1.2 replay world hashing.
- Phase 2.2 world helpers.
- Phase 6.1 machine-gunner mechanics if machine-gunner support depends on setup behavior.

### 6.4 AI GG and Leave After Losing All Town Halls

- [ ] Centralize "has town hall" and defeated predicates.
- [ ] AI emits GG/leave behavior after losing all town halls.
- [ ] Add game-over and AI elimination tests.

Depends on:

- Phase 2.2 world helpers.
- Phase 1.3 executable contracts.

This can move earlier if the town-hall predicate is centralized cleanly.

## Product / Client Backlog

These are mostly independent from simulation architecture. Prefer Agent B for these while Agent A
handles server architecture, as long as shared protocol/config files are not touched without
coordination.

- [ ] Restyle the main menu to be more WW2 themed and less sci-fi.
- [ ] Lobby system to see active lobbies.
- [ ] Basic settings menu or surrender UI.
- [ ] Display latency to server in milliseconds.
- [ ] Display "connection to server lost" when connection is lost.
- [ ] Muzzle flare animations.
- [ ] Find copyright-free assets for units, buildings, resources, and UI.
- [ ] Switch font to DIN 1451 Mittelschrift everywhere.

Already done:

- [x] Hotkeys in the command card should be grid style.

## Dependency Index

Use this index when deciding whether a TODO can start.

- Full unit collision: requires 1.1, 2.1, 2.4, 4.1.
- Larger maps: requires 1.1, 1.2, 4.1.
- Forests: requires 1.3, 2.2, 3.1, 4.1, 5.1.
- Rally points: requires 2.2, 2.4, 4.1.
- AI GG/leave: requires 2.2 and 1.3.
- Machine-gunner setup: requires 1.1, 2.1, 2.2, 2.4, 3.1, and maybe 5.1.
- Factions: requires 1.3, 2.2, 2.3, 3.1.
- TOML data: requires 1.3 and 3.1.
- Snapshot deltas: requires 1.2, 1.3, 5.1.
- Construction resumption: requires 1.1, 2.1, 2.2, 2.4.
- Advanced AI: requires 1.2, 2.2, 2.3, 3.1, and later relevant unit mechanics.
- Workers do not auto attack: prefer 2.2 and 3.1.



# Uncategorized Tasks:
 - DONE: buildings should not be able to be be built on top of each other
 - with a group of units selected, shift clicking a unit will deselect it
 - control left click a unit will select all units of that type that are visible in the viewport
 - DONE: tank factory and advanced training center should   require 100 and 50 oil respecitvely
 - DONE: Add four oil patches, but cut the rate of oil gathering by four
 - should be possible to select multiple buildings at once
 - cheat menu for development (only available while running locally), buttons to grant money, oil, or clear the fog of war completely
 - implement a way to view the self play tests with zero fog of war, because we spend an enormous amount of tokens breaking and debugging them, and letting me just watch will be super helpful and token efficient
 - DONE: building a building on top of a unit will lock that unit inside the building. it should be impossible to build a building on top of a unit.
 - halve the mining rate for steel
