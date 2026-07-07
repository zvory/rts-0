# Game State Ownership Plan

## Status

Active implementation plan for game-state ownership and internal checkpoint-readiness work. This
supersedes the deprecated `plans/lab-replay/` checkpoint program for now. It is not the public
checkpoint, replay-migration, or lab-migration plan; those products remain deferred until this
ownership sequence produces a checkpoint-readiness report. The executable phases currently defined
are
[Phase 0.5 - Derived-State Wipe Harness](phase-0.5.md),
[Phase 1 - State Ownership Inventory](phase-1.md),
[Phase 2 - Explicit DerivedState Shell](phase-2.md),
[Phase 3 - GameState Aggregate Shell](phase-3.md),
[Phase 4 - Cold Checkpoint V0](phase-4.md),
[Phase 5 - Movement And Economy Checkpoint Coverage](phase-5.md),
[Phase 6 - Visibility Combat And Effects Checkpoint Coverage](phase-6.md), and
[Phase 7 - Ownership Guardrails And Release Audit](phase-7.md).

## Purpose

Make authoritative simulation state easier to reason about, checkpoint, restore, and replay. The
goal is to move toward one explicit state ownership tree rooted at `Game`, so future gameplay
features cannot accidentally hide durable simulation state in service helpers, room code, caches, or
ad hoc side channels.

This plan should become a foundation for checkpoint-backed replay and future lab capture. Lab replay
is intentionally deferred until the game-state ownership model and checkpoint contract are
understood. When revisited, lab replay should consume this model rather than trying to discover every
piece of authoritative state during replay work.

## Phase Summaries

### [Phase 0.5 - Derived-State Wipe Harness](phase-0.5.md)

Build the first behavior-preserving proof that derived simulation state can be cleared and rebuilt at
a tick boundary without changing future authoritative results. The phase should add a test-only or
crate-private harness that runs paired games from the same setup and commands, clears/rebuilds the
derived-state path in one copy, and compares semantic state plus per-player fog-filtered snapshots
after additional ticks. This phase deliberately avoids durable checkpoint DTOs so derived-state
classification failures surface before broad serialization work begins.

### [Phase 1 - State Ownership Inventory](phase-1.md)

Document a complete ownership registry for every field currently stored on `Game`, classifying each
as authoritative/serialized, derived/rebuildable, transient, or compatibility metadata. The phase is
docs/registry work only and is intended to settle hard cases such as pending commands, command logs,
active construction projection state, pathing cache versus chosen paths, lab god mode, RNG, fog and
memory stores, effect stores, seed, loadout, and map metadata. It must leave behavior unchanged and
produce an executor-ready handoff that identifies any unresolved ownership blockers before
checkpoint DTO or code-movement phases begin.

### [Phase 2 - Explicit DerivedState Shell](phase-2.md)

Introduce a private `DerivedState` shell under `Game` for fields Phase 1 classified as
derived/rebuildable, initially wrapping the final snapshot `spatial` index and pathing cache/search
bookkeeping. The phase must preserve the existing tick pipeline by leaving phase-local derived state
in `systems.rs`, handing the final spatial index back to snapshot code, and clearing pathing cache
without losing the live default budget/cache configuration. It should extend the Phase 0.5
wipe/rebuild harness as the proof that the new boundary is behavior-preserving before any
`GameState` or checkpoint DTO work begins.

### [Phase 3 - GameState Aggregate Shell](phase-3.md)

Introduce a private `GameState` aggregate under `Game` for fields Phase 1 classified as
`authoritative/serialized` or `compatibility metadata`, after Phase 2 has moved rebuildable caches
into `DerivedState`. The phase is a behavior-preserving field move and borrow-shaping pass: public
`Game` methods stay stable, `systems::run_tick` may keep receiving split borrows, and services
should retain narrow mutation invariants instead of gaining broad mutable getters. It explicitly
stops before durable checkpoint DTOs, public schema/API changes, replay/lab behavior changes,
room/session state moves, or new architecture guardrails unless a touched code path requires a
targeted guardrail update.

### [Phase 4 - Cold Checkpoint V0](phase-4.md)

Add the first internal cold export/import proof by exporting a crate-private or test-friendly
`GameCheckpoint` from `GameState`, importing it into a fresh `GameState`, rebuilding
`DerivedState`, and ticking the restored game forward against the baseline. This phase should reuse
the Phase 0.5 semantic comparator/harness, comparing authoritative state plus per-player
fog-filtered snapshots after additional ticks while explicitly proving stable ids and allocator
state survive the checkpoint boundary. It remains behavior-preserving and internal: no public
checkpoint JSON/schema, wire protocol/client changes, replay keyframe replacement, lab scenario
migration, broad subsystem coverage promise, or AI decision determinism promise belongs in this
phase.

### [Phase 5 - Movement And Economy Checkpoint Coverage](phase-5.md)

Extend Phase 4's internal cold checkpoint path and semantic comparator over durable movement,
order, and economy state, including entity id allocation, active/queued orders, selected paths,
pending commands, command logs, player resources/upgrades/supply/scores, gather/build/train/research
progress, worker/resource reservations, and tick/RNG continuity where relevant. The phase should
mostly add focused tests plus internal DTO/comparator coverage, then prove restored games remain
semantically equivalent after additional ticks and through per-player snapshots where
movement/economy projections could diverge. It stays behavior-preserving and internal: no public
checkpoint schema/API, replay or lab migration, balance/gameplay change, or full
combat/projectile/smoke/ability/fog-memory coverage belongs in this phase except incidental state
needed by the movement/economy scenarios.

### [Phase 6 - Visibility Combat And Effects Checkpoint Coverage](phase-6.md)

Extend Phase 4/5's internal cold checkpoint path and semantic comparator over fog/projection-sensitive
and combat/effects durable state, including live fog output, team visibility, building memory,
trench memory/discovery/occupation, lingering sight, firing reveals, smoke, ability runtime, shell
stores, combat target/cooldown/facing/setup state, lab god mode, observer analysis output where
restore-sensitive, and event privacy surfaces. The phase must compare semantic authoritative state
after additional ticks plus normal per-player fog-filtered snapshots, selected-player/spectator
snapshots, full-world diagnostic snapshots, and any produced events without leaking fog-hidden
entity, position, target, ability payload, remembered occupant, or private event data. It remains
behavior-preserving and internal: no public checkpoint schema/API, replay or lab migration,
balance/gameplay change, final release audit, architecture guardrail phase, or broad UI/client work
belongs here.

### [Phase 7 - Ownership Guardrails And Release Audit](phase-7.md)

Add or tighten architecture and docs checks so future stateful simulation owners must be classified
in the ownership registry and stored under `GameState`/`DerivedState`, or explicitly documented as
room/session/test-only state. The phase should update the server-sim design/context docs, run a
final behavior-preserving audit that public `Game` APIs, wire protocol, replay behavior, lab
behavior, and private checkpoint behavior did not drift, and produce a checkpoint-readiness report
listing blockers before public checkpoint schema, replay migration, or lab migration. It remains a
guardrail and audit phase: no public checkpoint schema/API, replay/lab migration, gameplay/balance
change, direct main bypass, blanket service-stateless rewrite, or new checkpoint DTO coverage
scenarios belongs here except a small missing guardrail test discovered by the audit.

## Current Shape

The code is already relatively centralized: most authoritative simulation data is stored inside
`Game`, including entities, players, fog, building memory, pending commands, command logs, tick
state, smoke, trenches, ability runtime, shell stores, lab god mode, RNG, map metadata, and loadout
metadata.

The risk is that `Game` currently mixes several categories of state without a formal boundary:
authoritative gameplay state, derived performance caches, replay or compatibility metadata, and
runtime helpers. Some room-owned structures, such as lab timeline history and replay playback
cursor state, are appropriate session state, but the boundary between session state and
authoritative game state must be explicit.

The current replay and lab seek paths can clone an in-process `Game`. That is useful runtime
machinery, but it is not proof that checkpoint serialization is complete. The checkpoint contract
must prove cold export/import, not clone-based restore.

## Desired Architecture

`Game` should be the single aggregate root for authoritative simulation state. Any data that can
change future tick results, command validity, damage, fog or projection, scoring, entity ids,
checkpoint restore, or replay output must be reachable from `Game`.

The preferred shape is one rooted tree, not one flat mega-struct:

```text
Game
  GameState      authoritative and checkpointed state
  DerivedState   rebuildable cache and performance state
```

`GameState` owns durable simulation data. Examples include entities, players, orders, queues,
selected movement paths and waypoints, resource reservations, resources, scores, RNG state, entity
allocators, fog-relevant memory, building memory, trench discovery, smokes, shell stores, ability
runtime, firing reveals, lingering sight, lab god mode, and any other state that changes future
authoritative behavior or projected state.

`DerivedState` owns only cache and performance data. Derived state must be clearable and rebuildable
at a tick boundary without changing gameplay, replay output, scoring, command validity, or
fog/projection results. If clearing a field changes authoritative behavior, that field is
misclassified and belongs in `GameState`.

Pathing is the main hard case. The chosen path that a unit is already following is authoritative and
belongs under `GameState` with that unit's movement/order state. The pathfinding service's reusable
cache and search bookkeeping are derived; clearing them after import must not change the already
chosen path, command validity, or future result except for allowed performance cost.

The derived-state contract is intentionally testable: a checkpoint or test clone should be able to
drop every `DerivedState` field, rebuild it from `GameState`, continue ticking under the same command
stream, and match the untouched game semantically. Fields that fail this test are either
authoritative state in disguise or need a stronger rebuild path before checkpoint serialization uses
them.

## Service Ownership

Services should own invariants and mutation rules for their part of the state tree, but they should
not own hidden long-lived authoritative state outside `Game`.

For example, a building-memory module may define a `BuildingMemoryState` type with private fields
and expose focused refresh/query functions. `GameState` stores the `BuildingMemoryState`, while the
building-memory service remains responsible for its invariants. Other systems may pass the state
around, but should not get broad mutable access to its internals.

This gives the repo two useful properties:

- Serialization can walk one authoritative state tree.
- Encapsulation can still prevent unrelated systems from mutating each other's private state.

## Tick Boundary

`Game::tick()` should advance only state owned by `Game`. Tick systems may receive references into
`GameState` and `DerivedState`, but they should not depend on mutable authoritative state owned by
room code, client code, AI controller internals, global singletons, or hidden service instances.

AI controller memory is outside the checkpoint contract. Checkpoints preserve AI player slots and
the authoritative world they occupy, but exact future AI decisions after restoring a live game from
fresh controllers are out of scope and are not required to be bit-for-bit identical. Deterministic
replay remains authoritative through recorded actions; implementation should still avoid needless
AI divergence where practical.

Perf telemetry and diagnostics may observe tick work, but must not feed back into simulation
results.

## Room and Runtime State

Not all state outside `Game` is wrong. Room and lobby code may own connection/session concerns such
as sockets, joined viewers, replay playback speed, selected replay vision, lab operator connection
ids, UI capabilities, and lifecycle bookkeeping.

The dividing line is authority. If state changes the authoritative world or the future projected
result of that world, it belongs under `GameState` or must become an explicit recorded action that
mutates `GameState`.

Lab timeline history and replay cursor/keyframes are runtime mechanisms. They may remain outside
`Game`, but they must rebuild or drive `Game` through public, authoritative operations rather than
becoming a second source of simulation truth.

## Checkpoint Policy

Every state owner under `Game` should have one explicit checkpoint policy:

- Serialized: stored directly in `GameState` checkpoints.
- Derived: rebuilt from serialized state during import or before use.
- Transient: intentionally dropped because it cannot affect future authoritative behavior.

Transient should be rare and justified. Silent omissions are not acceptable.

The checkpoint contract must preserve stable entity ids and allocator/high-water state. Any future
migration that remaps ids must make the remap explicit, and replay actions must never silently
target stale ids.

Existing lab scenario import/export is not the checkpoint contract. Lab scenarios may remain a
temporary product label or adapter, but the durable setup format should converge on checkpoints.
Scenario-style restore that respawns entities and returns an id remap is acceptable for today's lab
authoring flow, but it is not acceptable for checkpoint restore.

## Verification Strategy

The original-vs-restored comparator should come early, not after most checkpoint work is complete.
The first useful version must be a cold `Game -> GameCheckpoint -> Game` restore that does not use
`Game::clone_for_replay_keyframe` or any equivalent full-struct clone. It should prove that a simple
exported game can be restored, have derived state cleared or rebuilt, tick forward, and match the
original game's semantic state and fog-filtered projections.

Comparison should start from semantic equivalence rather than raw byte equality. The comparator may
compare canonical DTOs or carefully selected internal struct views, but it must cover every field
classified as authoritative and should ignore fields explicitly classified as derived or transient.
For fog-sensitive behavior, compare per-player fog-filtered snapshots in addition to the
authoritative state view.

Later work should extend that comparator instead of inventing separate proof mechanisms. It should
cover movement, orders, economy, production, combat, projectiles, smoke, ability runtime, fog memory,
trench discovery, building memory, scoring, lab god mode, and replay-relevant command timing.

The comparator should intentionally clear or rebuild `DerivedState` in at least one path. This makes
derived-state misclassification visible as a test failure.

Phase 0.5 should land this derived-state wipe/rebuild proof before the plan grows durable
`GameCheckpoint` DTOs. Later cold export/import tests should reuse the same comparator instead of
creating a separate confidence mechanism.

## Safety And Migration Principles

This refactor should be behavior-preserving before it is feature-enabling. Moving state into a
clearer tree should not change gameplay, command behavior, fog, replay playback, lab behavior, or
match-history output on its own.

Prefer small, reviewable ownership moves over a broad rewrite. Keep service APIs narrow, keep fields
private where practical, and avoid broad mutable getters that let unrelated systems bypass service
invariants.

Architecture checks should eventually enforce the rule that new stateful simulation owners must
appear under `GameState` or `DerivedState`, with an explicit checkpoint policy. They should also
flag hidden mutable simulation state in services unless it is clearly derived, test-only, or
runtime/session state outside the authoritative simulation.

## Execution Constraints

- The phase runner may execute only phase files that exist in this directory. At present that means
  Phase 0.5 and Phase 1 through Phase 7; public checkpoint schema, checkpoint-backed replay, and lab
  migration work require a separate follow-up plan after Phase 7's readiness report.
- Each phase must land through the repo's normal owned-PR workflow with auto-merge armed, then wait
  until GitHub reports the PR merged and the phase head SHA is reachable from `origin/main`.
- After implementing a phase, the implementing agent must provide a handoff naming what changed, what
  the next agent should do, focused verification that passed, and the core manual testing focus.

## Relationship To Lab Replay

The existing `plans/lab-replay/` checkpoint program is deprecated for now. Checkpoint-backed replay
and lab capture should be revisited only after this ownership work can clearly state what is
authoritative, what is derived, and what a cold checkpoint restore must prove.

Once this plan is refined into executable phases and the core checkpoint contract exists, a new lab
replay plan can reference it as the foundation. That later plan should focus on checkpoint
artifacts, action timing, lab operation capture, schema break handling, and product UI instead of
also discovering the simulation ownership model.

## Non-Goals For This Plan

- Do not define exact public Rust APIs yet.
- Do not split the full public checkpoint, replay-migration, or lab-migration sequence here beyond
  the explicit Phase 0.5 and Phase 1 through Phase 7 ownership/internal-readiness work.
- Do not choose the final checkpoint JSON/schema shape here.
- Do not require all services to become stateless in one pass.
- Do not move room/session state into `Game` unless it is authoritative simulation state.

## Open Questions

- Which existing `Game` fields are authoritative, derived, runtime metadata, or compatibility
  metadata?
- Should command history and score counters be checkpointed as part of `GameState`, or should some
  replay products deliberately rebase them at checkpoint boundaries?
- Which pathing details beyond selected unit paths are pure cache, and what tests prove a cold
  restored path cache behaves the same as a warm original cache?
- How strict should service-private mutation be in Rust privacy terms versus architecture-check
  enforcement?
- Where should the eventual state ownership registry live so it stays close enough to code and docs
  to remain accurate?
