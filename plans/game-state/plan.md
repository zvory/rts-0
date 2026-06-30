# Game State Ownership Plan

## Status

Concept draft for review. This is not yet a full multi-phase implementation plan and does not
define phase files, branch sequencing, or exact API wiring.

## Purpose

Make authoritative simulation state easier to reason about, checkpoint, restore, and replay. The
goal is to move toward one explicit state ownership tree rooted at `Game`, so future gameplay
features cannot accidentally hide durable simulation state in service helpers, room code, caches, or
ad hoc side channels.

This plan should become a foundation for checkpoint-backed replay and lab capture. Lab replay should
consume a clearly owned and checkpointable game state model rather than trying to discover every
piece of authoritative state during replay work.

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
resources, scores, RNG state, entity allocators, fog-relevant memory, building memory, trench
discovery, smokes, shell stores, ability runtime, firing reveals, lingering sight, lab god mode, and
any other state that changes future authoritative behavior or projected state.

`DerivedState` owns only cache and performance data. Derived state must be clearable and rebuildable
at any time without changing gameplay, replay output, scoring, command validity, or fog/projection
results. If clearing a field changes authoritative behavior, that field is misclassified and belongs
in `GameState`.

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

AI controller memory is outside the checkpoint contract. Restored AI player slots may be resumed by
fresh controllers, while deterministic replay remains authoritative through recorded actions.

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

## Verification Strategy

The original-vs-restored comparator should come early, not after most checkpoint work is complete.
The first useful version should prove that a simple exported game can be restored, have derived
state cleared or rebuilt, tick forward, and match the original game's semantic state and
fog-filtered projections.

Later work should extend that comparator instead of inventing separate proof mechanisms. It should
cover movement, orders, economy, production, combat, projectiles, smoke, ability runtime, fog memory,
trench discovery, building memory, scoring, lab god mode, and replay-relevant command timing.

The comparator should intentionally clear or rebuild `DerivedState` in at least one path. This makes
derived-state misclassification visible as a test failure.

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

## Relationship To Lab Replay

The lab replay plan should depend on this ownership work. Checkpoint-backed replay and lab capture
need confidence that `Game` contains all authoritative state and that derived state can be rebuilt
without changing behavior.

Once this plan is refined into executable phases, `plans/lab-replay/` should reference it as the
foundation for checkpoint serialization. Lab replay should then focus on checkpoint artifacts,
action timing, lab operation capture, schema break handling, and product UI instead of also
discovering the simulation ownership model.

## Non-Goals For This Draft

- Do not define exact Rust APIs yet.
- Do not split implementation phases yet.
- Do not choose the final checkpoint JSON/schema shape here.
- Do not require all services to become stateless in one pass.
- Do not move room/session state into `Game` unless it is authoritative simulation state.

## Open Questions

- Which existing `Game` fields are authoritative, derived, runtime metadata, or compatibility
  metadata?
- Should command history and score counters be checkpointed as part of `GameState`, or should some
  replay products deliberately rebase them at checkpoint boundaries?
- Which pathing data is pure cache, and what tests prove a cold restored path cache behaves the
  same as a warm original cache?
- How strict should service-private mutation be in Rust privacy terms versus architecture-check
  enforcement?
- Where should the eventual state ownership registry live so it stays close enough to code and docs
  to remain accurate?
