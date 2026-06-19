# Phase 3 - Server Scheduler and History Buffer

## Phase Status

- [ ] Planned.

## Objective

Make the room task execute sequenced player commands on accepted effective ticks and keep enough
authoritative history to support bounded rollback in the next phase. This phase creates the
server-side scheduling and history contract that lets local two-tick prediction align with
authority instead of predicting earlier than the server can ever confirm.

## Scope

- Add a per-room scheduled command queue for live gameplay commands.
- Add a rolling authoritative history buffer for at least `ROLLBACK_WINDOW_TICKS = 26` ticks.
- Store the minimum state needed to restore and replay deterministically:
  - authoritative `Game` state or keyframes
  - consumed command log entries by effective tick
  - AI command ordering if AI is active
  - room/player metadata needed for snapshot fanout after replay
- For each sequenced player command:
  - validate `clientSeq` as today
  - accept the requested execute tick only inside a bounded future window
  - queue on the accepted effective tick if it has not passed
  - if the command arrives late, record rollback eligibility but still apply late in this phase
    until Phase 4 enables restore/replay
- Keep command ordering deterministic:
  - stable by effective tick
  - stable by room arrival order within a tick
  - stable across branch-live seat aliases
- Instrument history and replay primitives before rollback is active:
  - clone/keyframe cost
  - restore cost
  - dry-run replay cost for 5, 10, 20, and 26 ticks where practical
  - memory footprint for the history ring
- Track per-player command lead recommendations:
  - floor at two ticks
  - raise after repeated late arrivals or excessive correction signals
  - decay downward slowly after stable windows
- Include owner-only late/applied metadata in command result diagnostics.
- Preserve sim-consumption ACK semantics; do not drop pending client commands on socket receipt.

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
  - command queued for future effective tick
  - same-tick deterministic ordering
  - command arriving after requested execute tick applies late
  - history buffer stores and expires the expected tick range
  - restoring a recent state and replaying without inserted commands reaches the same snapshot
  - late command increments lead recommendation
  - stable windows decay lead recommendation toward two ticks
  - stale or wrapped `clientSeq` stays rejected
  - spectators and replay viewers cannot schedule gameplay commands
- Tri-state scenarios for:
  - healthy two-tick command executes on requested tick
  - late command reports rollback eligibility but applies late before Phase 4
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
history/dry-run replay costs, late-command policy before rollback, lead adjustment thresholds,
decay policy, and what Phase 4 needs to enable actual rollback.
