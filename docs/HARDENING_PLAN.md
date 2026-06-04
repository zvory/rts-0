# Hardening Plan

This plan captures the next set of correctness and maintainability upgrades for the RTS server and
client. The goal is not to add new gameplay. The goal is to remove latent footguns and make the
simulation easier to reason about over time.

The four priorities are:

1. Make determinism a first-class invariant.
2. Separate wire protocol types from internal simulation types.
3. Make tick phases and derived state boundaries explicit.
4. Keep client gameplay config in parity with the server.

## Read First

Before implementing any phase:

1. Read `DESIGN.md`.
2. Read `CLAUDE.md`.
3. Read this file.
4. Read the specific phase file for the work you are taking on.

## Hardening Principles

1. **Determinism is a contract.** Replay and live simulation must produce the same authoritative
   result from the same command stream. Any iteration order that affects outcomes must be stable
   and documented.
2. **Protocol types stay at the boundary.** Wire shapes are for transport. Simulation code should
   operate on typed domain objects, not JSON-oriented protocol structs.
3. **Derived state has a lifecycle.** Occupancy, spatial indexes, and other tick-scoped helpers
   must be explicitly rebuilt at named phases. Do not rely on ad hoc reuse of stale data.
4. **Mirrored config must not drift.** If the client presents gameplay-facing values, they must
   either be generated from the server or verified by tests against the server source of truth.
5. **Changes must preserve replayability.** Any change that affects simulation behavior should be
   tested both in live play and replay mode when applicable.

## Current Risks

### 1. Mutable entity access must use stable id iteration

`EntityStore::iter()` and `ids()` sort ids, which gives stable read ordering and stable mutable
visitation when systems iterate ids and then call `get_mut(id)`. `EntityStore::iter_mut()` has been
removed so simulation systems cannot accidentally depend on raw `HashMap::values_mut()` order.

Recommended policy:

- Do not add a raw mutable entity iterator without documenting why order cannot affect outcomes.
- Any mutation whose result could depend on visitation order must iterate over sorted ids first.
- Prefer shared `iter()` for read-only scans and `ids()` + `get_mut(id)` for mutation.

### 2. Protocol coupling inside simulation code

Status: command input has been extracted. `ClientMessage::Command` and replay command artifacts are
translated into `game::command::SimCommand` before they enter `Game`; AI and self-play also emit
`SimCommand` directly, and command services operate on `EntityKind` instead of protocol strings.
`Snapshot`, `StartPayload`, and `Event` remain protocol-facing output DTOs at the `Game` seam.

Recommended policy:

- Keep the translation layer at the boundary: `ClientMessage -> SimCommand`.
- Use typed domain commands internally, for example:
  - `SimCommand::Move`
  - `SimCommand::AttackMove`
  - `SimCommand::Attack`
  - `SimCommand::Gather`
  - `SimCommand::Build { kind: EntityKind, ... }`
  - `SimCommand::Train { unit: EntityKind, ... }`
  - `SimCommand::Cancel`
  - `SimCommand::Stop`
- Keep protocol parsing, string-to-kind mapping, and JSON shape concerns outside the command
  application core.

### 3. Tick pipeline is implicit

Status: `systems::run_tick` now rebuilds named phase state at explicit boundaries:
`PreCommandDerivedState`, `PostMovementDerivedState`, `PreCollisionDerivedState`, and
`FinalDerivedState`. The pipeline still remains a small orchestrator rather than a full ECS, but
occupancy and spatial indexes are no longer anonymous local variables whose validity has to be
inferred from nearby comments.

Prior risk: `run_tick` accepted many inputs and manually rebuilt occupancy/spatial indexes at
different points in the frame. That was workable, but it made it easy to pass a stale derived
structure into a later system.

Recommended policy:

- Introduce a `TickContext` or similar world-access pattern.
- Make the tick pipeline explicit, for example:
  - `PreCommandDerivedState`
  - `CommandPhase`
  - `MovementPhase`
  - `PostMovementDerivedState`
  - `Combat/Economy/Production/Construction`
  - `FinalDerivedState`
- The important invariant is not a full ECS. The important invariant is that every phase knows
  which derived state is valid and which derived state must be rebuilt before continuing.

### 4. Client rules table can drift

The client currently mirrors gameplay-facing values from the server for UI, fog, command cards,
costs, supply, sight, and build times. Manual mirroring will eventually diverge.

Recommended policy:

- Best option: generate a client rules module or JSON blob from the server rule definitions.
- Acceptable interim option: add a parity test that serializes the server rules and compares them
  to the client table.
- Treat parity failures as build-breaking, not advisory.

## Phases

- [Phase 0 - Determinism Audit](PHASE_0_DETERMINISM_AUDIT.md)
- [Phase 1 - Protocol Boundary Extraction](PHASE_1_PROTOCOL_BOUNDARY.md)
- [Phase 2 - Tick Context and Derived-State Boundaries](PHASE_2_TICK_CONTEXT.md)
- [Phase 3 - Client Rules Parity](PHASE_3_CLIENT_RULES_PARITY.md)
- [Phase 4 - Replay and Determinism Regression Harness](PHASE_4_REPLAY_HARNESS.md)

Do not combine phases unless the user explicitly asks for a larger change. Each phase should
leave the repo in a playable, debuggable state.

## Non-Negotiable Invariants

1. **Iteration order is deterministic wherever it can affect outcome.** This includes entity
   mutation, command application, production queues, combat resolution, and replay evaluation.
2. **Replay and live simulation use the same core logic.** Replays feed the same typed commands
   into a fresh game; the only difference is the source of command input.
3. **Protocol types do not leak inward.** Internal simulation code should not need to understand
   JSON transport concerns.
4. **Tick-scoped derived state is explicit.** No phase may silently consume stale occupancy or
   spatial data.
5. **Client gameplay rules match the server.** Any gameplay-facing number or table visible to the
   player must be sourced from, or verified against, the server truth.
6. **Hardening must not weaken fog.** Boundary refactors must continue to respect per-player
   visibility and ownership rules.

## Suggested Acceptance Criteria

Phase work should be considered complete only when it satisfies the following where relevant:

- Deterministic replay output remains stable across repeated runs with the same seed and command
  stream.
- Any new domain command or tick phase is covered by targeted tests.
- The translation layer keeps wire shapes isolated from simulation logic.
- The client rules parity check passes in CI or an equivalent local test.
- Documentation remains in sync with any contract changes.

## Testing Guidance

Use targeted Rust tests and replay-style tests first, then widen as needed:

```bash
cd server && cargo fmt && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

When a change alters protocol or rules parity, update the relevant docs and tests in the same
change. If a phase changes a contract described in `DESIGN.md`, update `DESIGN.md` at the same
time.
