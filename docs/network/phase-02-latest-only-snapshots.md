# Phase 02: Latest-Only Snapshots

Purpose: reduce freeze/catch-up behavior on the current WebSocket path by preventing stale
snapshots from queueing behind a stalled writer.

This phase is the main WebSocket-side fix for the reported symptom. It keeps the current deployment
and uses the existing full-state snapshot model.

## Problem

The current room code attempts one snapshot per connected player every tick:

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

Reliable messages remain handled by Phase 01. Snapshot behavior becomes latest-only:

- while the writer is busy, newer snapshots replace older unsent snapshots;
- when the writer can send again, it sends only the newest pending snapshot;
- if five snapshots arrive during a socket stall, at most one snapshot is sent afterward;
- the client sees a jump to the newest state rather than an ordered backlog of stale states.

This does not solve TCP head-of-line blocking for bytes already in flight. It does avoid sending
obsolete frames after the blockage clears.

## Suggested Server Shape

Prefer one reliable FIFO queue plus one replaceable snapshot slot:

```rust
struct ConnectionSink {
    reliable_tx: mpsc::Sender<ServerMessage>,
    snapshot_tx: watch::Sender<Option<Snapshot>>,
}
```

Other shapes are fine if they preserve the invariant:

- reliable messages are FIFO and prioritized;
- snapshots are replaceable;
- the room task never awaits socket writes.

Writer loop:

1. Prefer reliable messages.
2. Observe the latest snapshot version.
3. If the writer is ready to send a snapshot, take the latest one.
4. After sending a snapshot, check reliable messages again before sending another snapshot.

Do not use a plain `mpsc` snapshot queue unless old pending snapshots can be replaced.

## Snapshot Safety

Dropping old snapshots is safe for normal entity state because snapshots are full visible state:

- `tick` is absolute;
- resources are absolute for the player;
- `entities` is the current visible set;
- events are transient flavor.

Caveat: `resourceDeltas` are incremental in the client model. Latest-only delivery can drop a
resource delta if the skipped snapshot was the only one containing that update.

Pragmatic choices:

1. Make resource remaining values repeat while visible before introducing latest-only delivery.
2. Treat snapshots containing `resourceDeltas` as not droppable.
3. Accept the current risk temporarily and document it for Phase 05.

Option 1 is cleanest. Option 2 is safer for a narrow first pass. Option 3 is acceptable only if the
user explicitly accepts stale resource-display risk before compact snapshot work.

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

- simulated slow writer receives many snapshots and confirms only the newest snapshot is sent;
- reliable messages still beat pending snapshots from Phase 01;
- resource deltas are not lost under the chosen safety policy.

## Metrics

Add counters or logs, preferably dev-gated:

- snapshots produced per player;
- snapshots sent per player;
- snapshots replaced/coalesced per player;
- latest pending snapshot age when sent;
- writer send duration p50/p90/p99.

## Done Criteria

- Old unsent snapshots are replaced by newer snapshots.
- Reliable messages are not dropped behind snapshots.
- Existing tests pass.
- A slow-writer or loss reproduction shows fewer stale snapshots delivered after a stall.
