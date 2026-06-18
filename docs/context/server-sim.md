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
- `server/crates/sim/src/game/lab.rs` — typed lab-only `Game` mutation and scenario setup API
- `server/crates/sim/src/game/systems.rs` — thin tick orchestrator
- `server/crates/sim/src/game/services/` — small pure helpers, called in order by `systems.rs`
- `server/crates/sim/src/game/services/order_planner.rs` — pure command/queue planning policy
- `server/crates/sim/src/game/services/order_execution.rs` — narrow shared mutation helpers for
  issue-time command application and queued promotion
- `server/crates/rules/src/` plus `server/crates/sim/src/rules/projection.rs` — declarative rules
- `server/crates/ai/src/` — AI opponents and self-play harnesses
- `server/src/lobby/room_task.rs` — room-owned lifecycle, membership, phase transitions, match
  history, drain bookkeeping, and `Game` ownership
- `server/src/lobby/session_policy.rs` — names the room mode/phase policy choices for state
  source, joining, clocking, authority, mutation, visibility, diagnostics, persistence/export, start
  payloads, and UI affordances
- `server/src/lobby/participants.rs` — host fallback, active seats, spectator visibility seats,
  branch-live seat aliases, and command issuer resolution
- `server/src/lobby/tick_control.rs` — maps session clock policy plus replay/dev pause state to
  tick interval, countdown, live, replay, dev-watch, or no-op actions
- `server/src/lobby/projection.rs` — maps live, spectator, replay, branch-live, and dev-watch
  recipients to the appropriate `Game` snapshot API and event visibility; lab recipients use an
  explicit full-world projection. It also owns diagnostic snapshot options such as movement-path
  inclusion.
- `server/src/lobby/launch.rs` — shared start-payload stamping and send loop for live, branch-live,
  dev-watch, and lab starts
- `server/src/lobby/live_tick.rs` — live-match tick driver for AI enqueue, `Game::tick`,
  projection-backed snapshot fanout, observer analysis, and outcome detection
- `server/src/lobby/replay_session.rs` and `server/src/lobby/replay_branch.rs` — replay playback
  runtime and replay-branch staging/launch state
- `server/src/lobby/snapshot_fanout.rs`, `snapshots.rs`, `connection.rs`, `dev_replay.rs`, and
  `crash_replay.rs` — lobby-local delivery, compacting, dev replay loading, and panic artifacts
- `scripts/check-lobby-architecture.mjs` — lightweight guardrail that keeps lobby snapshot fanout
  routed through `projection.rs` and keeps lab mutation routing centralized in `room_task.rs`
- `server/src/main.rs` — room registry, HTTP/WebSocket wiring, and deployment drain coordination
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
