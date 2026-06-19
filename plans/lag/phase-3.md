# Phase 3 - Server Scheduler and History Buffer

## Phase Status

- [ ] Planned.

## Objective

Make the room task execute sequenced player commands on accepted effective ticks and keep enough
authoritative history to support bounded rollback in the next phase. This phase creates the
server-side scheduling and history contract that lets local two-tick prediction align with
authority instead of predicting earlier than the server can ever confirm.

## Scope

- Add these live-room primitives before changing command timing:
  - `ScheduledCommandEnvelope`: transport-owned command metadata, including connection id, seat id,
    `clientSeq`, command, requested/accepted execute tick, arrival order, source (`human` or `ai`),
    and diagnostics state.
  - `LiveCommandScheduler`: validates requested execute ticks, assigns accepted effective ticks,
    keeps deterministic per-tick ordering, and drains due commands into `Game::enqueue`.
  - `RollbackHistory`: stores tick-0 and rolling post-tick keyframes plus the command envelopes
    applied for each tick.
  - `CommandResultTracker`: stores owner-only result metadata and contiguous sim-consumption ACK
    state.
  - `CommandLeadController`: tracks per-connection lead recommendation and decay.
- Replace direct live-player `game.enqueue(...)` calls from `RoomTask::on_command` with scheduler
  insertion. `RoomTask` should still own connection/seat authority and `clientSeq` validation, but
  effective-tick policy belongs in `LiveCommandScheduler`.
- Add a rolling authoritative history buffer for at least `ROLLBACK_WINDOW_TICKS = 6` ticks plus
  tick 0. A restore for command tick `T` must start from the post-tick `T - 1` keyframe, so the
  history contract must say exactly whether a frame is pre-tick or post-tick.
- Store only the state needed to restore and replay deterministically:
  - authoritative `Game` keyframes
  - scheduled/applied command envelopes by effective tick and arrival order
  - AI commands already generated for each tick, not rerun AI thinking during replay
  - command result tracker state needed to rebuild owner-only metadata
  - lead controller state needed to keep recommendations deterministic
- For each sequenced player command:
  - validate `clientSeq` as today
  - accept the requested execute tick only inside `currentTick + 1` through
    `currentTick + MAX_FUTURE_EXECUTE_LEAD_TICKS`; initialize that constant to the rollback window
    unless this phase documents a smaller value
  - queue on the accepted effective tick if it has not passed
  - if the command arrives late, record exact rollback eligibility, possible clamped fallback
    diagnostics, and live-fallback metadata, but still apply late in this phase until Phase 4 enables
    restore/replay
- Keep command ordering deterministic:
  - stable by effective tick
  - stable by room arrival order within a tick
  - stable across branch-live seat aliases
- Keep AI ordering deterministic:
  - live AI thinking still runs once for the live tick
  - generated AI commands become scheduler envelopes for that tick
  - dry-run restore/replay uses recorded AI envelopes from history rather than invoking AI
    controllers again
- Instrument history and replay primitives before rollback is active:
  - clone/keyframe cost
  - restore cost
  - dry-run replay timing logs for 1, 2, 4, and 6 ticks where practical
  - dry-run command-count accounting up to the `MAX_REPLAY_COMMANDS = 1000` fuse
  - memory footprint for the history ring
- Track per-player command lead recommendations:
  - floor at two ticks
  - raise after repeated late arrivals or excessive correction signals
  - decay downward slowly after stable windows
- Include owner-only late/applied metadata in command result diagnostics.
- Preserve sim-consumption ACK semantics; do not drop pending client commands on socket receipt.
- Keep `Game` API changes narrow. If `Game` needs a helper for keyframe cloning or replay stepping,
  expose that helper deliberately; do not add transport metadata or scheduling policy to
  `SimCommand`.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/snapshot_fanout.rs`
- `server/src/lobby/participants.rs`
- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- Rust room-task tests
- tri-state command ACK scenarios

## Verification

- Rust tests for:
  - `RoomTask::on_command` no longer directly enqueues live human commands outside the scheduler
  - command queued for future effective tick
  - same-tick deterministic ordering
  - command arriving after requested execute tick applies late
  - future execute ticks beyond the acceptance window produce stable result metadata
  - history buffer stores and expires the six-tick rollback range
  - tick-0 and post-tick keyframe semantics are documented by tests
  - restoring a recent state and replaying without inserted commands reaches the same snapshot
  - replay uses recorded AI command envelopes rather than rerunning AI thinking
  - late command increments lead recommendation
  - stable windows decay lead recommendation toward two ticks
  - stale or wrapped `clientSeq` stays rejected
  - spectators and replay viewers cannot schedule gameplay commands
- Tri-state scenarios for:
  - healthy two-tick command executes on requested tick
  - late command within six ticks reports rollback eligibility but applies late before Phase 4
  - late command older than six ticks reports clamped-rollback eligibility or outside-window fallback
    metadata, but still applies late before Phase 4
  - repeated late commands raise future lead
  - prediction disabled remains authoritative-only
- Run:
  - focused `cargo test --manifest-path server/Cargo.toml -p rts-server ...`
  - `node tests/protocol_parity.mjs` if protocol fields changed in this phase
  - focused tri-state scheduling scenarios

## Manual Testing Focus

Use a local match with artificial latency profiles where practical. Confirm commands still execute,
late-command diagnostics appear under bad conditions, and normal healthy local play still feels like
a short fixed delay rather than remote echo.

## Handoff Expectations

The handoff must state the effective-tick acceptance window, history representation, measured
history/dry-run replay timing logs, keyframe tick semantics, scheduler/result/lead API names, AI
replay policy, late-command policy before rollback, lead adjustment thresholds, decay policy, the
replay command-count fuse, and what Phase 4 needs to enable actual catch-up rollback.
