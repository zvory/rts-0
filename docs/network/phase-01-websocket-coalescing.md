# Phase 01: WebSocket Snapshot Coalescing

Purpose: reduce freeze/catch-up behavior without WebTransport by preventing stale snapshots from
queueing behind a stalled WebSocket writer.

This is the first implementation phase because it is simpler than WebTransport, preserves the
current deployment, and targets a concrete repo issue.

## Problem

The current room code sends every outbound message through the same per-player queue:

- `RoomTask::on_tick` builds one `Snapshot` per connected player.
- `send_or_log` uses `try_send` into a per-player `mpsc::Sender<ServerMessage>`.
- `PLAYER_CHANNEL_CAP = 256`.
- The per-connection writer in `server/src/main.rs` serializes and awaits each send in order.

At 30 snapshots/s, a backed-up player can accumulate about 8.5 s of stale snapshots. If the socket
unblocks, the writer may deliver old snapshots before the newest one. The room simulation keeps
running, but the affected player can see a freeze or delayed catch-up.

Snapshots are full visible state. Older snapshots are superseded by newer snapshots, so they should
not sit in a FIFO backlog.

## Target Behavior

Reliable messages stay ordered and reliable:

- `welcome`;
- `lobby`;
- `start`;
- `gameOver`;
- `pong`;
- `error`;
- any future user-facing reliable notice.

Snapshots become latest-only:

- while the writer is busy, newer snapshots replace older unsent snapshots;
- when the writer can send again, it sends only the newest pending snapshot;
- if five snapshots arrive during a socket stall, at most one snapshot is sent afterward;
- the client sees a jump to the newest state rather than an ordered backlog of stale states.

This does not solve TCP head-of-line blocking for bytes already in flight. It does avoid sending
obsolete frames after the blockage clears.

## Suggested Server Shape

Introduce an internal outbound type near the connection boundary:

```rust
enum OutboundMessage {
    Reliable(ServerMessage),
    Snapshot(Snapshot),
}
```

Or use two channels/handles:

```rust
struct PlayerSink {
    reliable_tx: mpsc::Sender<ServerMessage>,
    snapshot_tx: watch::Sender<Option<Snapshot>>,
}
```

The exact type is less important than the invariant:

- reliable messages are FIFO;
- snapshots are replaceable.

Possible implementation patterns:

### Pattern A: reliable channel plus latest snapshot slot

- `RoomPlayer` stores a small connection handle instead of only `mpsc::Sender<ServerMessage>`.
- Reliable messages use `try_send`.
- Snapshot fanout writes into a latest snapshot slot.
- The writer task uses `tokio::select!` over reliable messages and snapshot updates.
- Reliable messages should be prioritized over snapshots when both are ready.

This is the clearest model, but requires a small connection-handle refactor.

### Pattern B: custom bounded queue with snapshot replacement

- Keep one channel but send `OutboundMessage`.
- Before enqueueing a snapshot, replace any pending unsent snapshot for that player.
- This usually requires a writer-owned buffer or a custom queue because plain `mpsc` does not let
  the producer remove older queued messages.

This may be harder than Pattern A.

### Pattern C: writer pulls snapshots from an atomic/watch slot

- Room writes latest snapshot into a `watch` channel or similar.
- Writer sends snapshots only when it observes a new version.
- Reliable messages remain an `mpsc` channel.

This is close to Pattern A and often easiest with tokio.

## Code Paths To Inspect

- `server/src/main.rs`
  - `handle_connection`
  - the writer task around `msg_rx.recv().await`
  - `handle_client_message`
- `server/src/lobby.rs`
  - `RoomEvent::Join`
  - `RoomPlayer`
  - `start_match`
  - `send_dev_start_to`
  - `send_dev_error`
  - `on_tick`
  - `on_tick_dev_selfplay`
  - `end_match`
  - `broadcast`
  - `send_or_log`
  - `player_channel_cap`

Do not change `Game` or simulation internals for this phase.

## Snapshot Safety

Dropping old snapshots is safe for normal entity state because snapshots are full visible state:

- `tick` is absolute;
- resources are absolute for the player;
- `entities` is the current visible set;
- events are transient flavor.

Caveat: `resourceDeltas` are incremental in the client model. WebSocket coalescing can drop a
resource delta if the skipped snapshot was the only one containing that update. Decide whether this
matters for Phase 01.

Pragmatic choices:

1. Accept the current risk for Phase 01 and document it as unchanged from possible packet loss in
   future WebTransport work.
2. Make resource remaining values repeat while visible before introducing coalescing.
3. Treat snapshots containing `resourceDeltas` as not droppable.

Option 2 is cleaner but touches protocol semantics. Option 3 keeps semantics but weakens coalescing
when mining is active. If a weak implementation agent is unsure, choose Option 3 for the first pass
and leave Option 2 for Phase 05.

## Client Changes

Ideally none.

The client already buffers two snapshots and computes interpolation from receive times. If old
snapshots are skipped, it should interpolate between the two latest accepted snapshots. Verify that:

- `GameState.applySnapshot` handles tick jumps;
- `computeAlpha` does not assume every tick arrives;
- selection pruning remains correct when an entity disappears between skipped snapshots.

If needed, add a guard to ignore older snapshots:

```js
if (this._cur && msg.tick <= this._cur.tick) return;
```

Only add this if tests or logs show stale delivery is still possible.

## Tests

Existing tests should keep passing:

```bash
tests/run-all.sh
```

Add focused coverage if practical:

- a unit-style server test for the connection handle proving latest snapshots replace older ones;
- a simulated slow writer that receives reliable messages plus many snapshots and confirms only the
  newest snapshot is serialized;
- a browser/client smoke path confirming normal match rendering still works.

Do not rely only on visual testing.

## Metrics To Add

Add counters or logs, preferably dev-gated:

- snapshots produced per player;
- snapshots sent per player;
- snapshots replaced/coalesced per player;
- reliable messages sent;
- writer send duration p50/p90/p99;
- latest pending snapshot age when sent.

These metrics make the Phase 01 outcome measurable:

- if coalesced count is high during a freeze and the freeze improves, this phase likely helped;
- if coalesced count is low and freezes remain, look at Phase 00 traces.

## Done Criteria

- Reliable messages stay ordered and are not dropped behind snapshots.
- Old unsent snapshots are replaced by newer snapshots.
- Existing tests pass.
- A slow-writer or loss reproduction shows fewer stale snapshots delivered after a stall.
- The code still preserves the room task invariant: no socket write await inside `RoomTask`.
