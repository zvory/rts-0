# Capsule: server simulation

Use when changing tick logic, services, rules, AI, or the `Game` core.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — tick and networking model
- [docs/design/server-sim.md](../design/server-sim.md) — Rust server and `Game` core API
- §3.1 `game::Game` public API; this is the seam and should stay stable
- §3.2 Concurrency model; the room task is the sole `Game` owner
- §3.3 Rules layer
- §3.5 Command planning and queued order semantics
- [docs/design/ai.md](../design/ai.md) — AI opponents
- [docs/design/testing.md](../design/testing.md) §9 and §10 — self-play and dev scenarios when
  touching scripted tests or scenario setup

## Code map
- `server/crates/sim/src/game/mod.rs` — public `Game` API.
- `server/crates/sim/src/game/lab.rs` — typed lab-only mutation and scenario setup API.
- `server/crates/sim/src/game/systems.rs` — thin tick orchestrator.
- `server/crates/sim/src/game/services/` — per-tick service helpers for commands, order planning,
  movement, combat, economy, production, construction, death, occupancy, supply, pathing,
  standability, spatial queries, and line of sight.
- `server/crates/sim/src/game/entity/`, `command.rs`, `commands.rs`, `snapshot.rs`, `fog.rs`,
  `building_memory.rs`, `pathfinding.rs`, and `map/` — core sim data and behavior.
- `server/crates/rules/src/` and `server/crates/sim/src/rules/projection.rs` — declarative rules
  and fog-gated projection policy.
- `server/crates/ai/src/` — AI opponents and self-play harnesses.
- `server/src/lobby/` — room-owned lifecycle, session policy, participants, tick control,
  projection/fanout, launch payloads, live ticks, replay/branch sessions, snapshots, and crash/dev
  replay loading.
- `server/src/main.rs` — room registry, HTTP/WebSocket wiring, and deployment drain coordination.
- Guardrails: `scripts/check-lobby-architecture.mjs`, `scripts/check-crate-boundaries.mjs`, and
  `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`.

## Invariants
- `Game::tick()` is panic-free: no `unwrap`/`expect`/unchecked indexing; stale ids are no-ops; use
  `checked_*` for anything derived from client input.
- The room task is the single owner of its `Game`. No locks.
- `lobby/` and `main.rs` only call the public `Game` API. Do not reach into internals.
- `rts-sim` must not depend on `rts-ai`, `rts-server`, Axum, or Tokio room machinery.

## When touching `rts-sim::game`
- Can the new logic be pure policy in `rts-rules` or a pure service helper instead of direct state
  mutation?
- Can mutation go through an existing entity/player helper rather than direct field writes?
- Did this add a new service-to-service import edge? Command/order edges need an exact allowlist
  entry and role-matrix justification.
- Did this increase a ratcheted file-size or public-export budget?

## Failed sim architecture checks
Prefer reducing coupling or moving logic behind an existing helper/API. If growth is intentional,
update the baseline with:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture --bless --reason "short reason"
```

Avoid broad allowlist additions unless the same change or a tracked follow-up explains the cleanup
path.

## Cross-capsule triggers
- Touching message construction → also read [protocol.md](protocol.md).
- Touching unit/building numbers → also read [balance.md](balance.md).
- Touching tests, CI, self-play, or dev scenarios → also read [testing.md](testing.md).
