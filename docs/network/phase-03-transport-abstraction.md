# Phase 03: Transport Abstraction

Purpose: prepare for WebTransport without changing behavior. This phase should leave WebSocket as
the only active transport and should not change gameplay semantics.

Do this only after Phases 01-02 or when the user explicitly wants transport groundwork.

## Non-Goals

- Do not add WebTransport yet.
- Do not change `server/src/protocol.rs` wire shapes.
- Do not change `Game`.
- Do not change snapshot rate or encoding.
- Do not remove WebSocket tests.

## Client Boundary

Keep the existing `Net` public API from `DESIGN.md` stable:

```js
join(name, room)
ready(isReady)
start()
addAi()
removeAi(id)
setQuickstart(enabled)
command(cmd)
ping()
setReplaySpeed(speed)
on(type, handler)
off(type, handler)
```

Refactor underneath `client/src/net.js` or split files:

- `WebSocketTransport`: owns current browser `WebSocket` behavior.
- `Net`: owns event dispatch, player id, ping latency, and typed helpers.

After this phase, `Net` should not know many WebSocket-specific details beyond choosing the
transport. The rest of the client should not change.

Suggested internal interface:

```js
class Transport {
  connect() {}
  send(obj) {}
  close() {}
  onOpen(cb) {}
  onClose(cb) {}
  onMessage(cb) {}
}
```

Messages can remain parsed JSON objects at this boundary for now. Binary snapshot parsing belongs
in later phases.

## Server Boundary

Preserve this flow:

```text
transport reader -> ClientMessage -> RoomEvent -> RoomTask -> Game
RoomTask -> per-player outbound sink -> transport writer
```

The room task must remain the only owner of `Game`. WebTransport code must not call `Game`
directly.

Server refactor targets:

- create a connection sink/handle abstraction for outbound messages;
- keep `RoomEvent::Join` conceptually the same;
- keep room event handling unchanged for commands, ready, start, AI, and replay speed;
- keep the WebSocket implementation wired through the new abstraction.

If Phase 01 has already introduced `Reliable` vs `Snapshot`, keep that split. If not, this phase can
introduce it as a no-op behavior split, but Phase 01 should still implement coalescing.

## Suggested Types

Use whatever names fit the codebase, but this shape is useful:

```rust
enum OutboundMessage {
    Reliable(ServerMessage),
    Snapshot(Snapshot),
}

#[derive(Clone)]
struct ConnectionSink {
    // hides the transport-specific queues
}
```

Methods:

```rust
impl ConnectionSink {
    fn try_send_reliable(&self, msg: ServerMessage);
    fn send_latest_snapshot(&self, snapshot: Snapshot);
}
```

The exact method return types should expose enough information for logs and tests:

- queued;
- replaced;
- full;
- closed.

## Tests

Run existing tests:

```bash
tests/run-all.sh
```

Add small tests only if the refactor creates pure helpers or reusable queue types. Avoid large test
harness rewrites in this phase.

## Done Criteria

- WebSocket behavior is unchanged from a user perspective.
- `Net` public API is unchanged.
- The server room task still talks through room events and connection sinks.
- No WebTransport dependency is required to build.
- Existing tests pass.
