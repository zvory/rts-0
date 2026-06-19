# Phase 4 - Bounded Server Rollback

## Phase Status

- [ ] Planned.

## Objective

Use the Phase 3 history buffer to honor late commands that arrive within the 26-tick rollback
window. The server should restore recent authority, insert the late command at its intended
effective tick, replay to present, and emit corrected snapshots without making rollback an
unbounded CPU liability.

## Scope

- Define `ROLLBACK_WINDOW_TICKS = 26` as the initial maximum rollback distance.
- Add a `RollbackEngine` or equivalent helper that is owned by the live scheduler/history layer.
  `RoomTask` and `LiveTickDriver` may request rollback, but restore/insert/replay/fallback details
  should not be hand-coded in the room event handler.
- When a command arrives after its requested `executeTick`:
  - if `currentTick - executeTick <= 26` and history is available, roll back and insert it
  - if the command is outside the window, execute late at the next legal tick and raise future lead
  - if rollback replay exceeds budget or fails, execute late and report fallback metadata
- Replay deterministically from the restored tick to the current tick:
  - restore the Phase 3 post-tick keyframe immediately before the inserted command's effective tick
  - original commands remain in stable effective-tick/order order
  - inserted late command joins the correct tick with stable ordering
  - recorded AI envelopes are replayed exactly; if the history lacks deterministic AI envelopes,
    rollback is unsupported for that room and the command falls back late
  - sim events are regenerated from the corrected authority for the replayed ticks, but old visual
    effects already delivered to clients are not individually undone
- After rollback:
  - update per-player ACK/result metadata
  - send corrected latest snapshots through the normal fog-filtered fanout path
  - record rollback replay ticks, elapsed time, and fallback reasons
- Preserve ACK semantics:
  - socket receipt stays diagnostic-only
  - `lastSimConsumedClientSeq` advances only for contiguous client sequences whose commands have
    been applied in the corrected authoritative stream
  - rollback must not double-consume commands, duplicate ACKs, or regress the last consumed seq
- Keep anti-cheat out of scope. The server accepts the client's intended execute tick inside the
  bounded window because play feel is the priority.
- Start with a conservative enablement rule: rollback is active only for room modes and command
  histories that Phase 3 proves replayable. Other rooms/commands produce `rollbackUnsupported` and
  execute late rather than partially rolling back.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/replay_session.rs` if reusable replay helpers exist
- `server/src/lobby/snapshot_fanout.rs`
- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim/src/perf.rs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- Rust rollback and room-task tests
- tri-state rollback scenarios

## Verification

- Rust tests for:
  - one late command inside 26 ticks rolls back and applies at intended tick
  - command exactly at the 26-tick boundary is handled according to the documented rule
  - command outside the window executes late and records fallback metadata
  - command inside the window but missing a keyframe or recorded AI stream falls back with
    `rollbackUnsupported`
  - command inside the window but over budget falls back with `rollbackBudgetExceeded`
  - rollback replay without inserted commands is snapshot-identical to uninterrupted authority
  - inserted command ordering is deterministic with same-tick existing commands
  - rollback does not double-consume commands or duplicate ACKs
  - rollback never emits full-world snapshots or hidden target ids to a normal active player
  - rollback after deaths/combat either works or is explicitly excluded with a fallback reason
  - rollback cost metrics are recorded
- Tri-state scenarios for:
  - healthy two-tick command needs no rollback
  - late move inside 26 ticks rolls back and converges
  - late move outside 26 ticks falls back to late execution
  - burst of two late commands replays once or in a documented deterministic sequence
  - prediction disabled still uses authoritative scheduling/rollback without local prediction
- Run:
  - focused `cargo test --manifest-path server/Cargo.toml -p rts-server ...`
  - focused `cargo test --manifest-path server/Cargo.toml -p rts-sim ...`
  - focused rollback tri-state scenarios
  - `node tests/protocol_parity.mjs` if protocol metadata changes

## Manual Testing Focus

Use artificial latency or a test profile that delays command delivery by less than and greater than
26 ticks. Inside the window, the command should be honored as if it landed on its intended tick;
outside the window, the game should fall back to late execution and future lead adjustment.

## Handoff Expectations

The handoff must state whether rollback is enabled for all live rooms or a narrower subset, the
measured replay costs, the fallback budget, unsupported rollback cases, ACK/result behavior after a
rollback, whether AI-backed rooms are supported, and whether server-side optimization is needed
before broader prediction work continues.
