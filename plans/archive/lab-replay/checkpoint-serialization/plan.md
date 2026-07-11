# Full Game Checkpoint Serialization Plan

> [!WARNING]
> **POTENTIALLY STALE SUBDIVISION - DO NOT IMPLEMENT YET.**
> This lab-replay subdivision depends on assumptions that may change when
> `plans/archive/game-state/plan.md` lands. Re-evaluate this subplan and its phase files before
> implementation.

## Purpose

Build the durable `GameCheckpoint` contract and prove that every authoritative game state can
round-trip through it. This stage is the foundation for checkpoint-backed starts, lab saves, and
future mid-game bug clips. It should not change replay behavior or lab UI yet.

## Phase Summaries

### [Phase 1 - State Inventory and Contract](phase-1.md)

Inventory authoritative state owned by `Game`, entities, services, queues, timers, RNG, projectiles,
fog, entity id allocation, and room-facing metadata. Draft the `GameCheckpoint` contract and
classify each field as serialized, derived on import, or explicitly transient. AI controller
decision memory should be documented as external/transient: restored AI slots can be driven by fresh
controllers, while replay correctness comes from recorded actions. This phase is documentation and
test-design heavy, with little or no runtime behavior change.

### [Phase 2 - Core Tick Zero Round Trip](phase-2.md)

Introduce the checkpoint DTOs and implement round-trip support for the minimal state needed to
restore a tick-zero match or empty lab. Cover map identity, players, teams, resources, tick count,
RNG seed/state, basic entity fields, and the entity allocator/high-water mark with exact entity-id
preservation. Add initial serde, validation, and canonical round-trip tests.

### [Phase 3 - Orders, Economy, and Production State](phase-3.md)

Expand checkpoint coverage to pending commands, active orders, build/production queues, rally data,
resource collection state, supply, tech or loadout state, and other non-combat long-lived state.
Resume tests should cover workers, building placement, production in progress, and command queues
across checkpoint restore. This phase should make normal economy gameplay safe to checkpoint in the
middle of a match.

### [Phase 4 - Combat, Effects, and Timed State](phase-4.md)

Expand checkpoint coverage to combat state, cooldowns, target references, projectiles, mortar shots,
smoke, death or impact timers, and other authoritative in-progress effects. Resume tests should
checkpoint while effects are active, continue both games, and compare resulting state and
fog-filtered projections. This phase is the main proof that bug clips can start from arbitrary
in-flight game state later.

### [Phase 5 - Deterministic Resume Harness](phase-5.md)

Build a reusable harness that checkpoints representative games, restores them, runs both original
and restored games for additional ticks, and compares semantic state. Include scripted scenarios
covering movement, combat, fog, production, buildings, smoke or mortar, and longer matches. Keep
generated artifacts under `target/` and make the harness opt-in for replay/checkpoint work.

### [Phase 6 - Public Checkpoint API and Docs](phase-6.md)

Finalize narrow public APIs for exporting, validating, and importing checkpoints without leaking
private internals across room or replay code. Update design docs and context capsules for the new
checkpoint contract. Leave clear follow-up notes for any state that is intentionally derived or
transient.

## Overall Constraints

- Do not use `Snapshot` as checkpoint input.
- Preserve entity ids exactly.
- Preserve the entity allocator/high-water mark exactly enough that post-restore spawns allocate the
  same future ids as the original game.
- Validate all ids, coordinates, counts, timers, queues, and references before import mutates a
  live game.
- Keep checkpoint serialization in the simulation boundary, not in lobby or client code.
- Prefer semantic comparisons over brittle JSON field ordering where the test is about game
  behavior.
- Each phase must land through the repo's normal owned-PR and wait-for-merge workflow.

## Handoff Requirements

Every phase handoff must name the checkpoint coverage added, the state still not covered, focused
tests that passed, and one manual game state worth checking by hand.
