# Phase 04: WebTransport Reliable Control Plane

Purpose: add dev-only WebTransport connection setup for reliable control messages while keeping
snapshots simple. This phase proves browser/server mechanics before unreliable snapshots add
protocol complexity.

## Preconditions

- Phase 03 transport abstraction is complete, or the user explicitly accepts a larger combined
  change.
- WebSocket fallback remains working.
- The deployment target is not assumed to support WebTransport yet.

## What To Build

Add a WebTransport transport that can:

- establish a browser `WebTransport` session;
- open a reliable bidirectional stream for JSON control messages;
- send client messages currently sent by WebSocket;
- receive server messages currently received by WebSocket;
- join lobby;
- start a match;
- send commands;
- receive enough server state to prove the match is playable.

For this phase, snapshots can use a reliable stream. That does not solve head-of-line stutter, but
it proves the control plane without also solving unreliable state semantics.

## Client Shape

Feature flags:

- `?transport=ws`: force WebSocket.
- `?transport=webtransport`: force WebTransport and show errors instead of fallback.
- default during this phase: WebSocket.

Connection flow:

1. Check `window.WebTransport`.
2. Check that the page is a secure context.
3. Create `new WebTransport(url)`.
4. Wait for `transport.ready`.
5. Open the reliable control stream.
6. Start read loops for incoming streams.
7. Emit the same `"open"` and `"close"` events as WebSocket.
8. On setup failure, fall back to WebSocket unless forced by query flag.

Keep JSON for control messages:

```text
u32 little-endian byte length
UTF-8 JSON payload
```

Do not rely on stream read chunks matching message boundaries. Reliable streams are byte streams,
so frame JSON messages explicitly.

## Server Options

Current server:

- axum 0.8 with `ws` feature;
- HTTP static file serving and `/ws` in one process;
- no HTTP/3;
- no QUIC;
- no UDP listener.

Likely Rust options:

### `wtransport`

Use the `wtransport` crate as a separate QUIC/WebTransport endpoint and keep axum for HTTP static
files plus WebSocket fallback.

Pros:

- purpose-built WebTransport API;
- likely fastest dev spike;
- can share the existing `Lobby` instance if server startup is refactored.

Cons:

- separate listener and TLS setup;
- not an axum route;
- deployment must expose UDP/QUIC to the process;
- connection handling must adapt to the existing `RoomEvent` and outbound message model.

### `h3` plus `h3-webtransport`

Build an HTTP/3 server stack directly.

Pros:

- closer to the underlying RFC model;
- more control if a single HTTP/3 server becomes important.

Cons:

- more low-level;
- more moving parts;
- higher risk of spending effort on server plumbing instead of game transport semantics.

Recommendation for a spike: try `wtransport` first unless there is a deployment reason to use
lower-level `h3` APIs.

## Server Integration

Do not duplicate room logic. The WebTransport accept loop should mirror `handle_connection`:

- allocate `player_id`;
- create outbound connection sink;
- send `welcome`;
- parse `ClientMessage`;
- call the same `handle_client_message` logic or an extracted equivalent;
- send `RoomEvent::Leave` on close.

If `handle_client_message` is currently too WebSocket-specific, extract a shared helper rather than
copying behavior.

## Local Development Issues

Browser WebTransport usually requires a secure context and certificate handling. A weak follow-up
agent should verify:

- whether `https://localhost` or a local certificate is required;
- whether the browser accepts the test certificate;
- whether the WebTransport URL can share the same host/port as the static app;
- whether the dev server needs a second UDP port.

Document the exact local run commands in this phase if implementation proceeds.

## Tests

Keep WebSocket tests. Add a browser smoke test if possible:

- load the client with `?transport=webtransport`;
- connect;
- join;
- ready/start;
- receive `start`;
- send a command;
- receive snapshots.

Node's built-in WebSocket is not enough for WebTransport coverage. Use a real browser.

## Done Criteria

- WebSocket remains the default.
- Forced WebTransport can connect locally in a real browser.
- Lobby and command flow work over WebTransport reliable streams.
- WebSocket fallback still passes existing tests.
- No unreliable snapshot semantics are introduced yet.
