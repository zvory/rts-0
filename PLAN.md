# PLAN

This is the working plan for building Bewegungskrieg in serial order with one or two agents.
It incorporates the previous `TODO.md` and `ARCHITECTURE_PLAN.md`.

Use this file as the operational source of truth. The main rule is simple: do not pull a large
RTS feature forward unless its listed gates are complete.

AI planning detail lives in [plans/ai/plan.md](plans/ai/plan.md). Keep the dependency chain summary
here in `PLAN.md`, and keep the concrete AI architecture, rollout phases, and matchup-test details
in the sub-plan so this file stays readable.

## Coordination Rules

- With one agent, proceed strictly by phase and task order.
- With two agents, Agent A should own the current server/simulation phase. Agent B may take only
  tasks explicitly marked `parallel-safe`.
- Do not edit the same module from two worktrees at the same time.
- Shared contracts require extra care: `docs/design/*.md`, `server/src/protocol.rs`,
  `client/src/protocol.js`, `server/src/config.rs`, `client/src/config.js`, command handling,
  fog/snapshot code, and pathing/movement code.
- Branches must start with `zvorygin/`.
- Prefer `go test ./...` for Go repos; this repo is Rust/JS, so use the repo commands in
  `CLAUDE.md`.

## Phase 1: Safety Harness Before More Simulation Complexity

Goal: make future simulation changes easy to verify and hard to silently break.

### 1.1 Tick Invariant Checks — DONE (see DONE.md)

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
- [ ] Add golden replay fixtures alongside the Rust replay tests.
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

### 2.1 Typed Component-Shaped Entity State — DONE (see DONE.md)

Gates:

- Required before full unit collision.
- Required before construction resumption.

### 2.2 Central World Query and Mutation Helpers — DONE (see DONE.md)

Gates:

- Required before forests/LoS blockers.
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

### 2.4 Explicit Order State Machines — DONE (see DONE.md)

Gates:

- Required before full unit collision.
- Required before machine-gunner setup/teardown.
- Required before construction resumption.


Parallel-safe for Agent B:

- [ ] Lobby system to see active lobbies.
- [ ] Basic settings menu, including surrender UI. Surrender should call a small
  `Game::eliminate(player)`-style API.
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

This is already implemented, but probably using a hack.
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

## Phase 4: Movement, Collision, and Map Scale

Goal: handle the hard movement work before larger maps and real collision amplify pathing debt.

### 4.1 Movement and Pathing Coordinator — DONE (see DONE.md)

Gates:

- Required before full unit collision.
- Required before larger maps.
- Required before forests/vehicle blockers.

### 4.2 Unit Collision and Non-Stacking — DONE (see DONE.md)

Depends on:

- Phase 1.1 invariants.
- Phase 2.1 typed entity state.
- Phase 2.4 order state machines.
- Phase 4.1 movement coordinator.

### 4.3 Maps Twice as Large

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

- [ ] Follow [plans/ai/plan.md](plans/ai/plan.md) for the detailed dependency chain and rollout order.
- [ ] Use the AI phase table in [plans/ai/plan.md](plans/ai/plan.md) as the serial implementation handoff plan.
- [ ] Keep one shared AI knowledge and action layer used by both live AI and self-play.
- [ ] Maintain the supported AI 2.1 and AI Turtle profiles as data-driven policy variations.
- [ ] Keep strategy differences mostly in priorities and thresholds, not duplicated mechanics.
- [ ] Make AI reason from shared definitions, tech availability, economy, army composition, and
  command results rather than duplicated constants.
- [ ] Replace brittle self-play scripts over time with personality-vs-personality matchup tests.
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

## Dependency Index

Use this index when deciding whether a TODO can start.

- Full unit collision: requires 1.1, 2.1, 2.4, 4.1.
- Larger maps: requires 1.1, 1.2, 4.1.
- Forests: requires 1.3, 2.2, 3.1, 4.1, 5.1.
- AI GG/leave: requires 2.2 and 1.3.
- Machine-gunner setup: requires 1.1, 2.1, 2.2, 2.4, 3.1, and maybe 5.1.
- Factions: requires 1.3, 2.2, 2.3, 3.1.
- TOML data: requires 1.3 and 3.1.
- Snapshot deltas: requires 1.2, 1.3, 5.1.
- Construction resumption: requires 1.1, 2.1, 2.2, 2.4.
- Advanced AI: requires 1.2, 2.2, 2.3, 3.1, and later relevant unit mechanics.
- Workers do not auto attack: prefer 2.2 and 3.1.



# Uncategorized Tasks

- [x] With a group of units selected, shift clicking a unit will deselect it.
- Should be possible to select multiple buildings at once.
- Cheat menu for development (only available while running locally): buttons to grant money, oil, or clear the fog of war completely.
- Control group system for units and buildings. ctrl+number creates a group. shift+number adds to a group. Tapping a group selects it. Double tapping moves the camera to the centroid.
- Shrink buildings so they don't take up their whole tile size, but leave some margin pixels on the sides — marginal, not enough for units to squeeze through, but enough for pathing to get easier.
- Worker mining animation.
- Create a sound system and a basic system for minimap alerts, for example when being attacked.
- Add an urban area in the center of the map that is hard to path through, with cover that tanks struggle with and that anti-tank gunners and machine gunners can hide in.
