# Capsule: server simulation

Use when changing tick logic, services, rules, AI, or the `Game` core.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — tick & networking model
- [docs/design/server-sim.md](../design/server-sim.md) — Rust server, `Game` core API
- §3.1 `game::Game` public API (the seam — keep stable)
- §3.2 Concurrency model (room task is sole owner; no locks)
- §3.3 Rules layer (`rules/`)
- [docs/design/ai.md](../design/ai.md) — AI opponents (`server/crates/ai`)
- [docs/design/testing.md](../design/testing.md) — self-play harness (only if touching scripted tests)

## Code map
- `server/crates/sim/src/game/mod.rs` — public `Game` API
- `server/crates/sim/src/game/systems.rs` — thin tick orchestrator
- `server/crates/sim/src/game/services/` — small pure helpers, called in order by `systems.rs`
- `server/crates/rules/src/` plus `server/crates/sim/src/rules/projection.rs` — declarative rules
- `server/crates/ai/src/` — AI opponents and self-play harnesses
- `server/src/lobby/`, `server/src/main.rs` — only touch sim via `game::Game`

## Invariants
- `Game::tick()` is **panic-free**: no `unwrap`/`expect`/unchecked indexing; stale ids = no-op;
  `checked_*` for anything derived from client input. A panic kills the room task.
- The room task is the single owner of its `Game`. No locks.
- `lobby/`/`main.rs` only call the public `Game` API. Don't reach into internals.

## Don't break
- The `Game` API signatures (§3.1). If you must change one, update
  [docs/design/server-sim.md](../design/server-sim.md) and all callers in the same commit.

## Cross-capsule triggers
- Touching message construction → also read [protocol.md](protocol.md).
- Touching unit/building numbers → also read [balance.md](balance.md).
