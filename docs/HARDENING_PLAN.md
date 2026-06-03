# Hardening Plan

This plan tracks only the remaining correctness and maintainability upgrades for the RTS server and
client. Completed hardening tasks should be removed from this file instead of kept as status notes.

The goal is not to add new gameplay. The goal is to remove latent footguns and make the simulation
easier to reason about over time.

## Read First

Before implementing any item:

1. Read `DESIGN.md`.
2. Read `CLAUDE.md`.
3. Read this file.
4. Inspect the actual code paths before proposing or making changes.

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

## Remaining Work

### 1. Enforce client rules parity

The client manually mirrors gameplay-facing values from the server for UI, fog, command cards,
costs, supply, sight, footprint sizes, requirements, and build/train times. Manual mirroring will
drift.

Implement one of these, in preference order:

- Generate a client rules module or JSON blob from the server rule definitions.
- Or add a build-breaking parity test that serializes the server rules and compares them to
  `client/src/config.js`.

Acceptance criteria:

- The check runs from the normal test/CI path, not only as an optional script.
- Every gameplay-facing client value is either generated from the server truth or verified against
  it.
- Changing a server rule without updating/regenerating the client rule surface fails locally and in
  CI.
- `DESIGN.md` documents the chosen parity mechanism.

### 2. Extract internal simulation events

Protocol output DTOs still leak through simulation services. In particular, game services append
`protocol::Event` directly.

Implement internal domain events for simulation systems, then translate them to protocol events at
the `Game`/transport boundary.

Acceptance criteria:

- Simulation services do not need to import `crate::protocol::Event`.
- Event generation remains fog-safe: hidden enemy ids and positions must not leak through events.
- Replay comparison still uses the same core simulation output as live play.
- Existing compact snapshot/event wire shapes remain unchanged unless `DESIGN.md` is updated in the
  same change.

### 3. Add deterministic-iteration guardrails

Add a targeted guardrail, such as a unit test or lint-like check, that fails if a public raw mutable
entity iterator is introduced or if simulation code starts relying on unordered entity map
visitation in outcome-affecting paths.

Acceptance criteria:

- Outcome-affecting mutation uses stable entity-id visitation.
- Any intentionally order-independent raw map mutation is documented at the call site.
- The guardrail is narrow enough not to block internal order-independent maintenance helpers.

### 4. Tighten tick phase ownership

Introduce a small `TickContext` or phase-specific context type so each system receives only the
world access and derived state valid for its phase.

Acceptance criteria:

- Each system receives only the derived state valid for that phase.
- Rebuild points stay visible in `systems::run_tick`.
- The result remains a small orchestrator, not a full ECS rewrite.
- Targeted tests cover any newly introduced context/phase boundary.

### 5. Add targeted replay regression fixtures

Add replay fixtures only where they catch specific remaining risk.

Candidate additions:

- A stable fixture replay for protocol/event boundary refactors.
- A rules-parity regression fixture when generated client rules are introduced.
- A focused replay covering fog-gated events after internal event DTOs are added.

Acceptance criteria:

- Replays feed typed commands into a fresh game through the same simulation API as live play.
- Failures identify the first divergent tick/event/snapshot with actionable detail.
- Added fixtures are small enough to run in the normal Rust test suite.

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

Work should be considered complete only when it satisfies the following where relevant:

- Deterministic replay output remains stable across repeated runs with the same seed and command
  stream.
- Any new domain command or tick phase is covered by targeted tests.
- Translation layers keep wire shapes isolated from simulation logic.
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
