# Phase 4 - Bounded Server Rollback

## Phase Status

- [ ] Planned.

## Objective

Use the Phase 3 history buffer to honor late commands through exact rollback when possible and
clamped rollback when exact rollback is too old but a safe replay tick still exists. The server should
restore recent authority, enter a non-reentrant catch-up replay, insert late commands at their
intended or clamped ticks when the replay cursor has not passed them, replay greedily to present, and
emit corrected snapshots.

## Scope

- Define `ROLLBACK_WINDOW_TICKS = 6` as the initial maximum rollback distance. At 30 Hz this is
  exactly 200 ms.
- Define `MAX_REPLAY_COMMANDS = 1000` as the initial catch-up command-count fuse. This is a safety
  cap against pathological command bursts, not a normal tuning lever.
- Add a `RollbackEngine` or equivalent helper that is owned by the live scheduler/history layer.
  `RoomTask` and `LiveTickDriver` may request rollback, but restore/insert/replay/fallback details
  should not be hand-coded in the room event handler.
- When a command arrives after its requested `executeTick`:
  - if `currentTick - executeTick <= 6` and history is available, roll back and insert it
  - if the command is outside the exact rollback window but an older history keyframe is still
    available inside the rollback ring, try clamped rollback at the oldest safe replayable tick and
    mark `rollbackClamped` or equivalent owner-only metadata
  - validate clamped commands against the restored clamped-tick state, not the originally requested
    tick; if a target, fog, cost, placement, cooldown, ownership, or command-family guard fails at
    the clamped state, reject/no-op or fall back according to the normal authoritative command result
    contract
  - if clamped rollback is unsupported, the required history is missing, deterministic replay is
    unsupported, or the replay command-count fuse is hit, execute late at the earliest legal tick and
    raise future lead; while catch-up is active, earliest legal means the earliest replay tick whose
    command list has not yet been drained
- Rollback is non-reentrant:
  - once catch-up replay starts, no nested rollback may begin until the replay exits
  - commands that arrive while catch-up replay is active are drained between replay ticks
  - if a newly arrived command's accepted tick is still ahead of or equal to the replay cursor, insert
    it into that replay tick's deterministic command list
  - if the accepted tick is already behind the replay cursor, apply the command at the earliest
    replay tick whose command list has not yet been drained, mark it `lateDuringReplay` or
    equivalent, and raise future lead when appropriate
  - if catch-up has already replayed through present and there is no remaining replay tick, new
    commands leave the catch-up path and follow ordinary live scheduling: future accepted ticks stay
    queued for that future tick, while already-late commands apply on the next live tick
- Replay deterministically from the restored tick to the current live tick:
  - restore the Phase 3 post-tick keyframe immediately before the inserted command's effective tick
  - original commands remain in stable effective-tick/order order
  - inserted late commands join the correct tick with stable ordering
  - newly arrived commands absorbed during replay join the earliest legal replay tick under the
    cursor rules above
  - recorded AI envelopes are replayed exactly; if the history lacks deterministic AI envelopes,
    rollback is unsupported for that room and the command falls back late
  - sim events are regenerated from the corrected authority for the replayed ticks, but old visual
    effects already delivered to clients are not individually undone
  - the replay target is the live tick captured when catch-up begins; wall-clock ticks elapsed during
    catch-up are room tick delay to record, not extra live ticks to chase inside the same catch-up pass
- After rollback:
  - update per-player ACK/result metadata
  - send corrected latest snapshots through the normal fog-filtered fanout path, not intermediate
    replay frames
  - record rollback replay ticks, elapsed time, replay command count, absorbed-during-replay command
    count, clamped rollback count, metronome delay, and fallback reasons
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
- Command families may opt out of clamped rollback even when they support exact rollback. The phase
  must explicitly document which commands are clamp-safe at rollout; movement can be the first
  supported family, while combat, build, gather, and ability commands may remain live-fallback-only
  until their clamped-tick validation semantics are covered.

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
  - one late command inside 6 ticks rolls back and applies at intended tick
  - command exactly at the 6-tick boundary is handled according to the documented rule
  - clamp-safe command outside the exact rollback window applies at the oldest safe replayable tick
    and records `rollbackClamped`
  - command outside the window with unsupported clamped rollback executes late and records fallback
    metadata
  - command inside the window but missing a keyframe or recorded AI stream falls back with
    `rollbackUnsupported`
  - command inside the window but past the active replay cursor applies at the earliest replay tick
    whose command list has not yet been drained with `lateDuringReplay`
  - command bursts beyond `MAX_REPLAY_COMMANDS` fall back with `rollbackCommandCapExceeded`
  - rollback replay without inserted commands is snapshot-identical to uninterrupted authority
  - inserted command ordering is deterministic with same-tick existing commands
  - commands from both players that arrive during catch-up are absorbed into a single non-reentrant
    replay when their accepted ticks have not passed
  - alternating late commands from both players do not trigger nested rollback or repeated restore
    loops
  - rollback does not double-consume commands or duplicate ACKs
  - rollback never emits full-world snapshots or hidden target ids to a normal active player
  - rollback emits corrected latest snapshots only, with no intermediate replay-frame fanout
  - rollback after deaths/combat either works or is explicitly excluded with a fallback reason
  - rollback catch-up timing, metronome delay, clamped count, and command-count diagnostics are
    recorded
- Tri-state scenarios for:
  - healthy two-tick command needs no rollback
  - late move inside 6 ticks rolls back and converges
  - late move outside the exact rollback window clamps to the oldest safe replayable tick when enabled
  - outside-window command with clamped rollback unsupported falls back to late execution
  - burst of two late commands from one player replays once or in a documented deterministic sequence
  - burst or alternating late commands from two players complete one catch-up pass without nested
    rollback
  - command arriving behind the active replay cursor executes at the earliest undrained replay tick
  - command arriving after catch-up has no remaining replay ticks follows ordinary live scheduling
  - prediction disabled still uses authoritative scheduling/rollback without local prediction
- Run:
  - focused `cargo test --manifest-path server/Cargo.toml -p rts-server ...`
  - focused `cargo test --manifest-path server/Cargo.toml -p rts-sim ...`
  - focused rollback tri-state scenarios
  - `node tests/protocol_parity.mjs` if protocol metadata changes

## Manual Testing Focus

Use artificial latency or a test profile that delays command delivery by less than and greater than
6 ticks. Inside the window, the command should be honored as if it landed on its intended tick unless
it arrived behind an active replay cursor; outside the exact window, clamp-safe commands should use
the oldest safe replayable tick, while unsupported or behind-cursor commands should execute at the
earliest undrained replay tick during catch-up and adjust future lead. Only commands that arrive
after catch-up has no remaining replay tick should wait for ordinary live scheduling.

## Handoff Expectations

The handoff must state whether rollback is enabled for all live rooms or a narrower subset, the
measured replay timing logs, replay command-count fuse, absorbed-during-replay behavior, active
replay cursor fallback behavior, clamped rollback support matrix, metronome-delay behavior,
unsupported rollback cases, ACK/result behavior after rollback, and whether AI-backed rooms are
supported.
