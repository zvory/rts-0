# Phase 01: Reliable Message Priority

Purpose: make reliable control messages take priority over snapshots on the existing WebSocket
transport.

This is a WebSocket-only improvement. It does not change the wire protocol.

## Problem

The current outbound path uses one per-player FIFO `mpsc::Sender<ServerMessage>`:

- room task sends `lobby`, `start`, `gameOver`, `error`, and `snapshot` through the same queue;
- `ping` replies enqueue `pong` through the same writer-side channel;
- the writer serializes and sends messages in channel order.

When snapshots are produced at 30/s, reliable messages can sit behind snapshots already queued for
that connection. The biggest user-facing risk is not command registration; it is control/health
messages being delayed by a snapshot backlog.

This phase is separate from latest-only snapshots. It should be possible to prioritize reliable
messages before latest-only snapshot delivery is complete.

## Target Behavior

Reliable messages stay FIFO relative to each other:

- `welcome`;
- `lobby`;
- `start`;
- `gameOver`;
- `pong`;
- `error`;
- any future reliable user-facing notice.

Snapshots are lower priority:

- if a reliable message and a snapshot are both pending, send the reliable message first;
- do not allow a backlog of snapshots to delay reliable messages that have not been sent yet;
- do not await socket writes inside `RoomTask`.

This cannot preempt a snapshot frame already being written to the socket. It only changes pending
message scheduling before the next write starts.

## Suggested Server Shape

Introduce an outbound classification near the connection boundary:

```rust
enum OutboundMessage {
    Reliable(ServerMessage),
    Snapshot(Snapshot),
}
```

Or use separate methods on a connection handle:

```rust
impl ConnectionSink {
    fn try_send_reliable(&self, msg: ServerMessage);
    fn try_send_snapshot(&self, snapshot: Snapshot);
}
```

Writer behavior:

1. Drain reliable messages first, preserving reliable FIFO order.
2. Send at most one snapshot before checking reliable messages again.
3. Keep the existing close/error behavior.
4. Log or count reliable queue full/closed separately from snapshot drops.

If this phase uses two channels:

- reliable channel can be small but must not silently drop critical messages;
- snapshot channel can remain bounded until Phase 02 makes it latest-only.

## Code Paths To Inspect

- `server/src/main.rs`
  - `handle_connection`
  - writer task around `msg_rx.recv().await`
  - direct `welcome` and `pong` sends
- `server/src/lobby.rs`
  - `RoomEvent::Join`
  - `RoomPlayer`
  - `broadcast`
  - `send_or_log`
  - `start_match`
  - `end_match`
  - `send_dev_start_to`
  - `send_dev_error`

Do not change `Game` or simulation internals.

## Tests

Existing tests should keep passing:

```bash
tests/run-all.sh
```

Add focused coverage if practical:

- slow snapshot queue plus a reliable `gameOver` confirms `gameOver` is sent before pending
  snapshots;
- many snapshots plus `pong` confirms `pong` is not stuck behind snapshot backlog;
- reliable messages preserve order relative to other reliable messages.

## Metrics

Add dev-gated counters/logs:

- reliable messages sent by type;
- reliable queue full/closed;
- snapshots sent;
- snapshots skipped/dropped;
- time from reliable enqueue to writer send.

## Done Criteria

- Reliable messages are no longer scheduled behind pending snapshots.
- Reliable FIFO order is preserved.
- Room tasks still never await socket writes.
- Existing tests pass.
