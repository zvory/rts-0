# AI-0: Boundary and Invariants

This phase freezes the AI architecture contract before more implementation work happens.

The current change set creates the canonical plan and the detailed phase files. Future agents
should treat this file as the first checklist before touching AI code.

## Purpose

- Keep gameplay AI, self-play, replay, and human clients on one command path.
- Prevent the next AI work from becoming another monolithic `think()` body.
- Make future strategy work serial and reviewable.
- Preserve determinism and artifact quality while self-play is migrated.

## Current Boundary

The authoritative game flow is:

1. `Game::tick()` advances the tick.
2. Each `AiController` gets a chance to enqueue ordinary `Command`s.
3. Pending commands are recorded for replay.
4. Systems apply commands and run simulation.
5. Fog is recomputed and snapshots are built.

This boundary is good. Do not bypass it.

Live AI currently reads authoritative state from `Map`, `EntityStore`, `SpatialIndex`, and
`PlayerState`. Self-play scripts currently read `PlayerView` and `Snapshot`. The shared AI core
must support both views without duplicating game-mechanics knowledge.

## Hard Rules

- AI emits only `Command`.
- AI does not mutate `Game`, `EntityStore`, `PlayerState`, pathing, fog, or spatial indexes.
- AI code must stay deterministic for the same world state.
- Every scan that affects command order must use a stable order.
- Invalid AI commands must remain harmless because they pass through normal command validation.
- Live AI may read authoritative state, but strategy logic should be written against constrained
  observations and facts.
- Self-play must keep replay/artifact support while it migrates.

## Information Model

Use one shared AI core, but keep adapters explicit:

- live adapter: builds observations from authoritative state
- self-play adapter: builds observations from `PlayerView`/`Snapshot`

The adapters may differ in what they can observe. They should not differ in knowledge of costs,
requirements, saturation rules, worker selection, build placement, production queues, or attack
readiness.

## Naming

Use these names consistently:

- shared core module: `server/src/game/ai_core/`
- old helper module during migration: `server/src/game/ai_shared.rs`
- live adapter: `server/src/game/ai.rs`
- self-play adapter/script wrapper: owned by `server/src/game/selfplay.rs`
- required profiles:
  - `rifle_flood_fast`
  - `rifle_flood_full_saturation`
  - `tech_to_tanks`

Older notes may use `tech_tree`. Treat that as the same strategic intent, but prefer
`tech_to_tanks` in new code.

## Recommended Worktree Scope

A future implementation agent should take exactly one of these scopes:

- one numbered phase file
- one clearly bounded subtask from a phase file
- one follow-up cleanup explicitly left by a completed phase

Do not combine live AI migration and self-play migration in the same branch unless the change is
mechanical and very small.

## Files Usually Touched

Expected AI work may touch:

- `AI-PLAN.md`
- `docs/ai/*.md`
- `DESIGN.md`, only if the public contract changes
- `server/src/game/mod.rs`
- `server/src/game/ai.rs`
- `server/src/game/ai_shared.rs`
- new `server/src/game/ai_core/*`
- `server/src/game/selfplay.rs`
- targeted tests in nearby modules

Avoid protocol/client changes until a phase explicitly requires profile selection UI or wire
changes. The first architecture pass does not require that.

## Done Criteria

AI-0 is done when:

- `AI-PLAN.md` is the canonical index.
- Detailed phase files exist and are linked from the index.
- The first implementation phase is clear enough for a future agent to start without
  rediscovering the architecture from scratch.
