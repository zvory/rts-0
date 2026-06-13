# DONE

Completed tasks moved from PLAN.md.

## Foundation

- Replace stringly entity kind checks in hot simulation paths with typed internal enums.
- Split `systems.rs` into internal services.
- Add spatial query layer.
- Introduce `PathingService` boundary.
- Extend map/passability around movement classes.
- Rename the game to Bewegungskrieg.
- Rename gas/minerals to oil/steel.
- Enforce map-generation resource fairness.
- Add tick-stamped command log and deterministic replay harness.
- Command card grid hotkeys.
- Feedback on move and attack commands.
- Selected unit command card for hold/move/attack/stop.
- Building production progress bars.
- Defeat screen keeps latest map visible.
- Larger command card.
- Non-rifleman/non-worker units require oil.
- Slower infantry, workers, and anti-tank gun.
- Darker silhouette for impassable terrain.
- Larger tanks.
- Fix workers getting stuck inside buildings after construction.

## Phase 1: Safety Harness

### 1.1 Tick Invariant Checks

- Add `Game::assert_invariants()` for tests/debug builds.
- Check entity id/store-key consistency.
- Check no NaN or out-of-world coordinates.
- Check supply equals living plus queued units.
- Check buildings never overlap.
- Check resource-node miner reservations are valid or ignored.
- Check orders do not point at invalid required targets except in documented transition windows.
- Check fog grids exist for all players and never for neutral owner.
- Check snapshots never expose hidden enemy ids through entities, targets, or events.

### 1.3 Executable Design Contracts (partial)

- Add shot overpenetration so attacks continue 25% of range past the primary target and
  deal 50% reduced damage to enemies behind it, discouraging clumping.

## Phase 2: Simulation Domain Boundaries

### 2.1 Typed Component-Shaped Entity State

- Split broad `Entity` fields into typed state groups while keeping `EntityStore` simple.
- Introduce structures such as `MovementState`, `CombatState`, `ProductionState`,
  `ConstructionState`, `WorkerState`, and `ResourceNodeState`.
- Add constructors per `EntityKind`.
- Add tests that each `EntityKind` has exactly the expected state groups.
- Avoid adding new top-level optional fields to `Entity` unless they apply to most kinds.

### 2.2 Central World Query and Mutation Helpers

- Add canonical helpers for owned units, completed buildings, town halls, visible entities,
  targetable enemies, resource reservation, building placement, spawn search, and path requests.
- Move repeated scans and predicates behind helpers where practical.
- Prefer helpers in new systems; hand-rolled scans need a clear local reason.

### 2.4 Explicit Order State Machines

- Separate order intent from execution state.
- Model gather, build, attack, and future setup/teardown as explicit state machines.
- Add transition tests for stale target, stop, death, retarget, no path, ownership loss,
  cancel, interrupt, resume, and completion.

### Parallel-safe (Agent B)

- Muzzle flare animations (client-side, no event semantics change).

## Phase 4: Movement, Collision, and Map Scale

### 4.1 Movement and Pathing Coordinator

- Add one coordinator for movement/path requests.
- Own path request budgeting per tick.
- Add shared paths or flow-field-style support for large selected groups where practical.
- Add goal spreading around target points.
- Add spawn-point search around buildings.
- Add `PathFailed` semantics.
- Add repath throttling and cache invalidation.
- Route commands through the coordinator instead of directly creating per-unit A* paths.

### 4.2 Unit Collision and Non-Stacking

- Implement unit collision and non-stacking.
- Preserve mining-worker exceptions through worker/gather state, not movement hacks.
- Add no-overlap invariants, path-failure behavior, blocked-goal behavior, and group-move tests.
- Re-enable `scripted_self_play_exercises_economy_tech_and_combat`: subsumed by the
  "reserve on arrival" build model.

## Phase 6: Advanced Unit Mechanics

### 6.3 Advanced AI Tech Progression (partial)

- Start the shared AI knowledge extraction by centralizing deterministic near-base
  build-site selection for both live AI and self-play.
- Extract shared worker-target, local spend-budget, and attack-wave selection helpers for
  both live AI and self-play.

## Product / Client Backlog

- Hotkeys in the command card should be grid style.

## Uncategorized

- Buildings should not be able to be built on top of each other.
- Control left click a unit will select all units of that type that are visible in the viewport.
- Factory and advanced training center should require 100 and 50 oil respectively.
- Add four oil patches, but cut the rate of oil gathering by four.
- Implement a way to view the self play tests with zero fog of war.
- Building a building on top of a unit will lock that unit inside the building — made impossible.
- Halve the mining rate for steel.
- Make steel gray, make oil black, change the icons, make the oil patch spawn 90 degrees away from the steel patch.
- Halve the number of initial steel and oil patches and create scattered patches; five scattered patches, one near each starting position and one near the centre.
- Workers visually latch onto a steel patch with a line to the steel patch.
- Add a white outline to oil patches.
- Limit how many units can be selected at a time to 12.
- Make the tank deal 100 damage so it one-shots every unit.
