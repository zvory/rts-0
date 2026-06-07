# Phase 3 - Simulation Crate Without Server Dependencies

Status: Planned.

Goal: make `Game` and the simulation systems a reusable crate that does not import server shell,
transport, AI, or server perf implementation.

## Scope

- Move `game/`, sim-owned map loading, systems, services, entities, fog, replay core, and invariant
  checks into `rts-sim`.
- Keep the public `Game` seam documented in `docs/design/server-sim.md`, or update that doc in the
  same change if the API improves.
- Replace `crate::perf` usage with one of:
  - a tiny instrumentation trait passed into `tick`;
  - a no-op/default phase recorder type owned by sim;
  - a separate low-level instrumentation crate with no server dependency.
- Remove tokio/axum/tracing-subscriber dependencies from the sim crate.
- Ensure sim depends only on rules/domain, contract, random/serde/map-loading dependencies, and
  minimal logging/tracing if truly needed.
- Keep deterministic replay in sim if it is strictly command-log simulation replay; move artifact
  writing and CLI concerns out.

## Design Notes

This phase should still allow the server shell to own room tasks exactly as before. The server
creates a `Game`, enqueues commands, ticks it, reads snapshots, and checks outcomes.

Do not split every service into separate crates. The architecture invariant is package-level
direction, not maximum crate count.

## Tests

- `cd server && cargo test -p rts-sim` once the package exists.
- Full `cd server && cargo test`.
- Deterministic replay tests.
- Fog/snapshot tests.
- Movement/combat/economy service tests.

## Done

- `rts-sim` compiles without importing AI, lobby, main, axum, tokio room machinery, server perf, or
  protocol transport.
- `Game::tick()` behavior is unchanged and remains panic-free.
- The server shell can run a normal match by using only public sim APIs.
- Replay determinism tests still pass.

