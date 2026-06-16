# Capsule: server simulation

Use when changing tick logic, services, rules, AI, or the `Game` core.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — tick & networking model
- [docs/design/server-sim.md](../design/server-sim.md) — Rust server, `Game` core API
- §3.1 `game::Game` public API (the seam — keep stable)
- §3.2 Concurrency model (room task is sole owner; no locks)
- §3.3 Rules layer (`rules/`)
- §3.5 Command planning and queued order semantics
- [docs/design/ai.md](../design/ai.md) — AI opponents (`server/crates/ai`)
- [docs/design/testing.md](../design/testing.md) — self-play harness (only if touching scripted tests)

## Code map
- `server/crates/sim/src/game/mod.rs` — public `Game` API
- `server/crates/sim/src/game/systems.rs` — thin tick orchestrator
- `server/crates/sim/src/game/services/` — small pure helpers, called in order by `systems.rs`
- `server/crates/sim/src/game/services/order_planner.rs` — pure command/queue planning policy
- `server/crates/sim/src/game/services/order_execution.rs` — narrow shared mutation helpers for
  issue-time command application and queued promotion
- `server/crates/rules/src/` plus `server/crates/sim/src/rules/projection.rs` — declarative rules
- `server/crates/ai/src/` — AI opponents and self-play harnesses
- `server/src/lobby/`, `server/src/main.rs` — only touch sim via `game::Game`
- `scripts/check-crate-boundaries.mjs` — enforces Cargo package dependency direction
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` —
  ratchets `rts-sim::game` internals against
  `server/crates/archcheck/baselines/sim-architecture.json`.

## Invariants
- `Game::tick()` is **panic-free**: no `unwrap`/`expect`/unchecked indexing; stale ids = no-op;
  `checked_*` for anything derived from client input. A panic kills the room task.
- The room task is the single owner of its `Game`. No locks.
- `lobby/`/`main.rs` only call the public `Game` API. Don't reach into internals.
- `rts-sim` must not depend on `rts-ai`, `rts-server`, Axum, or Tokio room machinery.

## When touching `rts-sim::game`
- Can the new logic be pure policy in `rts-rules` or a pure service helper instead of direct state
  mutation?
- Can mutation go through an existing entity/player helper rather than direct field writes?
- Did this add a new service-to-service import edge? Command/order edges need both an exact import
  allowlist entry and a role-matrix justification; residual `commands`/`order_queue` tick-system
  imports are explicit exceptions, not a blanket adapter bypass.
- Did this increase a ratcheted file-size or public-export budget?

## Failed sim architecture checks
- Prefer reducing coupling or moving logic behind an existing helper/API.
- If growth is intentional, update the baseline with
  `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture --bless --reason "short reason"`.
- Avoid broad allowlist additions unless the same change or a tracked follow-up explains the cleanup
  path.

## Don't break
- The `Game` API signatures (§3.1). If you must change one, update
  [docs/design/server-sim.md](../design/server-sim.md) and all callers in the same commit.

## Cross-capsule triggers
- Touching message construction → also read [protocol.md](protocol.md).
- Touching unit/building numbers → also read [balance.md](balance.md).
