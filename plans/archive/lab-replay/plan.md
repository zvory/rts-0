# Checkpoint Backed Replay and Lab Capture Plan

> [!WARNING]
> **POTENTIALLY STALE - DO NOT IMPLEMENT.**
> This plan predates the completed `plans/archive/game-state/` ownership work and is superseded by the
> archived `plans/archive/checkpoint/plan.md` roadmap for checkpoint serialization, replay migration, and lab
> scenario migration. Treat this directory as historical reference only.

## Status

**POTENTIALLY STALE / deprecated draft.** Do not execute this plan or any `plans/lab-replay/`
phase files as the current roadmap; use `plans/archive/checkpoint/plan.md` instead.

This plan captured an earlier checkpoint-first lab replay program, but it is no longer the active
source of truth. The game-state ownership work has now landed, and the fresh executable roadmap is
archived at `plans/archive/checkpoint/plan.md`. Lab replay, lab save, and checkpoint-backed replay
work should be planned or executed from a new active roadmap.

## Purpose

Make authoritative game state round-trip serializable before replay and lab capture are rebuilt on
top of it. A replay should become a starting `GameCheckpoint` plus an ordered stream of
authoritative actions. Lab scenarios, normal match starts, mid-game saves, and replay starts should
all use the same checkpoint contract instead of separate initializer families.

Backwards compatibility with old replay artifacts is explicitly out of scope. This is pre-alpha, so
old `ReplayArtifactV1` files and old match-history replay launches may be rejected with a clear
incompatibility reason. Short beta replay dead zones during the schema transition are acceptable as
long as failures are clear and the final stage restores newly captured replay launch.

## Core Model

- `Map` is static world data: dimensions, terrain, resource layout, pathing inputs, and map
  identity/hash.
- `GameCheckpoint` is complete authoritative simulation state at one exact tick. It must include
  enough state to rebuild a `Game`, continue ticking, and produce the same future authoritative
  results as the original game.
- `Scenario` is a product/UI word, not a separate serialization contract. Code, catalog assets,
  import/export and validation should move to checkpoint-backed lab setup payloads;
  visible UI copy may still say "Lab Scenario" where that is the player-facing concept.
- `ReplayArtifact` is `start: GameCheckpoint` plus `actions: ReplayAction[]`.
- `ReplayAction` is a typed authoritative action applied at a recorded tick through public game or
  lab APIs.
- AI controller memory is outside the checkpoint contract. Checkpoints preserve AI player slots as
  players, but a restored live AI starts from its normal cold controller/profile state. Replays stay
  authoritative through recorded actions.

## Program Structure

This plan is intentionally split into subplans because each stage is large enough to need its own
multi-phase execution sequence.

### [Stage 1 - Full Game Checkpoint Serialization](checkpoint-serialization/plan.md)

Define `GameCheckpoint` and make `Game -> GameCheckpoint -> Game` work for all authoritative state.
Start with inventory and minimal tick-zero state, then expand through orders, production, combat,
projectiles, smoke, timers, RNG, fog-relevant state, and other in-progress effects. Finish with
deterministic resume tests that prove a restored game continues like the original game.

### [Stage 2 - Checkpoint Architecture Guards](checkpoint-guards/plan.md)

Add guardrails so future simulation features cannot silently bypass checkpoint serialization. The
guards should combine documented ownership rules, targeted architecture checks, and regression
harness coverage rather than relying on memory or reviewer vigilance. This stage should make hidden
authoritative state a test failure or an explicit reviewed exception.

### [Stage 3 - Checkpoint Backed Starts and Replays](checkpoint-starts/plan.md)

Make normal match starts, lab starts, imported setups, and replay starts all use serialized
checkpoints as their starting state. Break the old replay schema and replace old initializer logic
with `ReplayArtifact { start: GameCheckpoint, actions }`. Match-history and dev replay loaders
should accept only the new schema and reject older artifacts cleanly.

### [Stage 4 - General Replay Actions and Lab Save](replay-actions/plan.md)

Extend the replay action stream beyond normal player commands. Add explicit tick semantics and typed
actions for player commands, lab operator mutations, and `issueCommandAs`. Use that stream to build
"save replay so far" from the active lab baseline checkpoint plus the retained current-branch action
timeline.

## Overall Constraints

- Do not make client snapshots a restore format. Snapshots are fog-filtered views, not authoritative
  state.
- Do not keep separate persisted contracts for `GameScenario` and `GameCheckpoint`. A scenario is a
  checkpoint used as a start state.
- `GameCheckpoint` must be versioned, validated, bounded, and round-trip serializable.
- Checkpoints must preserve stable entity ids exactly. If a migration ever remaps ids, that remap
  must be explicit and no replay action may target stale ids. The entity allocator/high-water mark
  is part of that contract, so future spawns cannot collide with restored ids.
- Checkpoint import must validate map identity/hash, dimensions, player ids, teams, ownership,
  entity ids, command/order references, pathing bounds, resource counts, cooldown ranges, and action
  queue limits.
- Round-trip tests must include both `Game -> checkpoint -> Game` and
  `checkpoint -> Game -> checkpoint` paths where canonical serialization is expected.
- Resume tests must compare original and restored games after additional ticks, including
  fog-filtered per-player projections where visibility could regress.
- All future authoritative state must choose one checkpoint policy: serialized, derived during
  import, or explicitly transient and safe to drop. Silent omissions are not acceptable.
- Replays should use one runtime and one viewer. Source-specific logic belongs in checkpoint
  generation, replay capture, and typed action application.
- Replay action timing must use one explicit convention. Do not allow match commands and lab
  commands to drift by one tick.
- Generated characterization artifacts and goldens belong under `target/` and stay out of git.
- Each executable phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait until the PR is definitely merged and the head SHA is reachable from
  `origin/main`.
- After each phase, the implementing agent must provide a handoff describing what changed, what the
  next agent should do, and the core manual testing focus.

## Out Of Scope

- Supporting old replay artifacts after the schema break.
- Uploading lab captures to a production sharing service.
- Treating match history as the storage product for lab bugs.
- Serializing AI controller decision memory. Restored AI players may be resumed by fresh external AI
  controllers.
- Serializing purely client-side animation queues that have no authoritative gameplay effect.

## Open Review Questions

- Should a checkpoint embed full map data, reference map data by stable id/hash, or support both?
- Should lab timeline cap resets become a new baseline checkpoint automatically, or should "save
  replay so far" fail if the original action stream was truncated?
- What canonical serialization format should tests compare: JSON value equality, normalized stable
  DTOs, or semantic game-state equality?
- Which old match-history rows should hide their replay button after the schema break, and what
  message should players see?
