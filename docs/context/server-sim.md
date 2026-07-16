# Capsule: server simulation

Use when changing tick logic, services, rules, AI, or the `Game` core.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — tick and networking model
- [docs/design/server-sim.md](../design/server-sim.md) — Rust server and `Game` API
- §3.1 `game::Game` public API; this is the seam and should stay stable
- §3.1.1 `Game` state ownership registry; update this when `GameState`/`DerivedState` fields change
- §3.1.2 Ownership guardrails and readiness audit; read before state-owner, checkpoint, replay/lab
  migration, or archcheck work
- §3.1.3 `GameCheckpointV1` embeddable payload contract; read before public checkpoint DTO,
  replay start-state, lab scenario, match-start artifact, or debug-document checkpoint work
- §3.2 Concurrency model; the room task is the sole `Game` owner
- §3.3 Rules layer
- §3.5 Command planning and queued order semantics
- [docs/projection-audit-checklist.md](../projection-audit-checklist.md) — projection checklist
- [docs/design/ai.md](../design/ai.md) — AI opponents
- [docs/design/testing.md](../design/testing.md) §9 and §10 — self-play and dev scenarios when
  touching scripted tests or scenario setup

## Code map
- `server/crates/sim/src/game/mod.rs`, `state.rs`, `derived_state.rs`, `lab.rs`, `systems.rs`,
  `entity/`, `command*.rs`, `snapshot.rs`, `fog.rs`, `building_memory.rs`, `pathfinding.rs`, and
  `map/` — `Game` API, durable/derived owners, lab ops, tick orchestration, and core sim behavior.
- `server/crates/sim/src/game/services/` — per-tick services; `order_planner.rs` and
  `order_execution.rs` own command/queue planning and issue-time mutation helpers.
- `server/crates/rules/src/` and `server/crates/sim/src/rules/projection.rs` — declarative rules
  and fog-gated projection policy.
- `server/crates/ai/src/` — AI opponents and self-play harnesses.
- `server/src/lobby/room_task.rs` plus `room_task/{types,lobby,live,lab,dev,replay,branch,lifecycle,helpers}.rs` —
  the room actor, room-owned state/types, mode handlers, lifecycle, summaries, and send helpers.
- `server/src/lab_scenarios.rs` — bundled lab scenario catalog and authoring validation.
- `server/src/lobby/session_policy.rs`, `participants.rs`, `tick_control.rs`, and
  `lab_timeline.rs` — lifecycle policy, issuer resolution, room-time, and lab rewind.
- `server/src/lobby/projection.rs`, `snapshot_fanout.rs`, and `snapshots.rs` — per-recipient
  visibility, fanout, compacting, and diagnostic snapshot options such as movement-path inclusion.
- `server/src/lobby/launch.rs`, `live_tick.rs`, `replay_session.rs`, `replay_branch.rs`,
  `connection.rs`, `dev_replay.rs`, and `crash_replay.rs` — launch, live/replay/branch execution,
  delivery, dev replay loading, and panic artifacts.
- `server/src/main.rs` — room registry, HTTP/WebSocket wiring, and deployment drain coordination.
- Guardrails: `scripts/check-lobby-architecture.mjs`, `scripts/check-crate-boundaries.mjs`, and
  `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` enforce
  lobby, crate, sim role, state-tree, and registry boundaries.

## Invariants
- `Game::tick()` is panic-free: no `unwrap`/`expect`/unchecked indexing; stale ids are no-ops; use
  `checked_*` for anything derived from client input.
- The room task is the single owner of its `Game`. No locks.
- `lobby/` and `main.rs` only call the public `Game` API. Do not reach into internals.
- `rts-sim` must not depend on `rts-ai`, `rts-server`, Axum, or Tokio room machinery.

## When touching `rts-sim::game`
- Can the new logic be pure policy in `rts-rules` or a pure service helper instead of direct state
  mutation?
- Does new durable state live under `GameState`, rebuildable cache/search state under
  `DerivedState`, and the registry classify it with a concrete checkpoint policy and evidence row?
- Can mutation go through an existing entity/player helper rather than direct field writes?
- Did this add a new service-to-service import edge? Command/order edges need an exact allowlist
  entry and role-matrix justification.
- Did this increase a ratcheted file-size or public-export budget?

## Failed sim architecture checks
Prefer reducing coupling or moving logic behind an existing helper/API. For intentional growth:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture --bless --reason "short reason"
```

Avoid broad allowlist additions unless this change or a tracked follow-up explains the cleanup path.

Registry failures are not ratchets: update §3.1.1 and move the owner under `GameState` or
`DerivedState`, or document it as room/session/test-only outside `Game`.

## Cross-capsule triggers
- Touching message construction → also read [protocol.md](protocol.md).
- Touching unit/building numbers → also read [balance.md](balance.md).
- Touching tests, CI, self-play, or dev scenarios → also read [testing.md](testing.md).
