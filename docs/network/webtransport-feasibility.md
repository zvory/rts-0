# WebTransport Feasibility

Status: docs-only investigation.

Date: 2026-06-02.

## Short Answer

Switching to WebTransport is plausible, but it is not a small transport swap. The useful version of
the change is not "WebSocket JSON on a WebTransport reliable stream"; that keeps most ordered-stream
behavior. The useful version is to keep commands and session control reliable, while sending
snapshots as latest-only state over WebTransport datagrams or independent QUIC streams.

The harder parts are:

- making snapshots safe to lose;
- keeping each snapshot payload below datagram size limits, or choosing a stream fallback;
- adding HTTP/3 and QUIC over UDP to a server that is currently simple axum HTTP plus `/ws`;
- preserving WebSocket fallback for browser/deploy environments where WebTransport is unavailable;
- proving the freeze is actually network head-of-line blocking and not browser main-thread work.

Before implementing WebTransport, do the measurement pass in this doc. There is also a lower-risk
WebSocket mitigation worth trying first: make outbound snapshots latest-only instead of queuing up
to 256 stale snapshot messages per player.

## Current Repo Facts

Read `DESIGN.md` first before changing network contracts.

Relevant files:

- `server/src/main.rs`: axum server, static file serving, `/ws` upgrade, per-connection reader and
  writer tasks.
- `server/src/lobby.rs`: one room task per game room, fixed-rate tick loop, snapshot fanout.
- `server/src/protocol.rs`: Rust JSON wire protocol.
- `client/src/net.js`: browser `WebSocket` wrapper and event emitter.
- `client/src/main.js`: derives `ws://.../ws`, owns heartbeat, creates `Match`, receives snapshots.
- `client/src/state.js`: parses current snapshots into the client model and buffers two snapshots
  for interpolation.
- `DESIGN.md`: current architecture contract: JSON over WebSocket, 30 Hz tick, 30 snapshots/s.

Current model:

- `TICK_HZ = 30`.
- `SNAPSHOT_EVERY_N_TICKS = 1`, so the server attempts 30 snapshots/s/player.
- Client commands are small JSON messages sent upstream through the same WebSocket.
- Server snapshots are full per-player state, fog-filtered, JSON text frames.
- The room task does not await socket writes directly. It uses `try_send` into a per-player
  `mpsc::Sender<ServerMessage>`.
- The per-connection writer task serializes `ServerMessage` with `serde_json::to_string` and awaits
  `sink.send(Message::Text(...))`.
- `PLAYER_CHANNEL_CAP = 256`. At 30 snapshots/s, a backed-up player can accumulate roughly 8.5 s of
  stale snapshot messages before the queue becomes full.

Important implication: a slow socket should not stall the room simulation globally, but an
individual connection can still receive old snapshots in order after a short network or socket
write stall. That can look like a client-side freeze followed by delayed or bursty state updates.

## Quick Measurement Already Run

I sampled the current WebSocket server on `127.0.0.1:8094` using Node's built-in `WebSocket`.
This was not a worst-case match. It is only a baseline for early-game payload size.

Normal two-player fog-filtered match, one recipient, first ~5 s of snapshots:

| metric | snapshot bytes | visible entities |
| --- | ---: | ---: |
| count | 152 | 152 |
| min | 1191 | 5 |
| p50 | 1192 | 5 |
| p90 | 1193 | 5 |
| p99 | 1193 | 5 |
| max | 1193 | 5 |
| avg | 1192 | 5 |

Dev self-play full-world watch, early run, one recipient, first ~15 s of snapshots:

| metric | snapshot bytes | visible entities |
| --- | ---: | ---: |
| count | 456 | 456 |
| min | 3626 | 10 |
| p50 | 3777 | 10 |
| p90 | 4028 | 12 |
| p99 | 4034 | 12 |
| max | 4034 | 12 |
| avg | 3803 | 10 |

Interpretation:

- Early normal snapshots are already near the common "keep this under roughly one UDP packet"
  target. Once WebTransport datagram headers are added, JSON snapshots of this shape may not fit
  comfortably in one datagram.
- Full-world snapshots do not fit a conservative one-datagram budget.
- These numbers do not explain a half-second freeze by themselves. Worst-case late-game snapshots,
  frame timings, JSON parse cost, and renderer cost still need measurement.

Re-run the baseline like this:

```bash
RTS_ADDR=127.0.0.1:8094 cargo run
```

Then connect throwaway WebSocket clients and record `MessageEvent.data.length` for every
`snapshot`. If you add code to the repo for this, keep it behind a dev-only flag or put it in a
test helper; do not leave ad-hoc logging in production paths.

## What WebTransport Actually Buys

WebSocket runs over one reliable ordered byte stream. If bytes for an older server-to-client frame
are delayed by TCP loss, later frames on that same connection cannot be delivered to browser
JavaScript first. This is the classic ordered-stream head-of-line problem.

WebTransport uses HTTP/3 over QUIC. It exposes:

- bidirectional streams: reliable, ordered per stream;
- unidirectional streams: reliable, ordered per stream;
- datagrams: unreliable, unordered, message-like, congestion-controlled.

QUIC removes connection-wide stream head-of-line blocking, but it does not make every byte
unblocked. A reliable QUIC stream still has ordered delivery within that stream. To reduce stutter
from obsolete snapshots, use one of these patterns:

1. Send each snapshot as one unreliable datagram.
2. Send each snapshot on a separate unidirectional stream and ignore or reset stale streams.
3. Keep WebSocket, but make snapshots latest-only so stale queued snapshots are discarded before
   the writer sends them.

Pattern 1 best matches the game state model if each snapshot is independently decodable and small
enough. Pattern 2 avoids TCP connection-wide HOL but still retransmits bytes for stale snapshots
unless the implementation cancels them. Pattern 3 does not solve TCP loss, but it can fix a very
real current risk: old snapshots accumulating in `PLAYER_CHANNEL_CAP = 256`.

## What WebTransport Will Not Fix

WebTransport does not help if the half-second freeze is caused by browser main-thread work.
Likely non-network causes in this repo:

- `JSON.parse` in `client/src/net.js` on every snapshot.
- `GameState.applySnapshot()` cloning entities, rebuilding maps, pruning selection, and appending
  visual events.
- `renderer.render()`, `hud.update()`, and `minimap.render()` on every animation frame.
- Browser garbage collection caused by repeated JSON object allocation.
- Server-side tick work or snapshot projection pauses, although the current room task avoids
  waiting on socket writes.

It also does not remove server serialization cost if snapshots remain JSON, and it does not remove
QUIC congestion control. Datagrams may drop under congestion; they are not a license to exceed the
network.

## First Measurement Pass

Do this before any transport rewrite.

### Client frame timing

Add temporary instrumentation around:

- `client/src/net.js` `_onMessage`:
  - raw payload byte length;
  - `JSON.parse` duration;
  - message tag;
  - snapshot tick.
- `client/src/state.js` `applySnapshot`:
  - duration;
  - entity count;
  - resource delta count;
  - event count.
- `client/src/main.js` `Match.frame`:
  - total frame duration;
  - `camera.update`;
  - `input.update`;
  - `fog.update`;
  - `renderer.render`;
  - `hud.update`;
  - `minimap.render`;
  - computed `alpha`;
  - gap between current and previous snapshot receive times.

Use `performance.mark()` / `performance.measure()` or compact `performance.now()` probes. Also add
a `PerformanceObserver` for `longtask` entries so a freeze can be distinguished from network lag.

### Server timing

Add temporary tracing around:

- `RoomTask::on_tick` total duration.
- `game.tick()`.
- `game.snapshot_for(player_id)`.
- `compact_snapshot_for_wire`.
- per-player outbound queue depth before enqueue if available.
- message kind and serialized byte length in the writer task.
- duration of `sink.send(...)`.

Do not estimate packet sizes in comments or PRs. Log the measured byte lengths.

### Network-loss reproduction

Create a repeatable reproduction with one of:

- Chrome DevTools throttling, if it can reproduce the freeze;
- macOS Network Link Conditioner;
- a local proxy or OS-level packet-loss tool;
- remote deployment with packet loss observed from browser traces.

Record:

- snapshot receive interval histogram;
- `net.latency` p50/p90/p99 from app-level pings;
- browser long-task p50/p90/p99;
- snapshot byte p50/p90/p99/max;
- dropped/stale snapshot counts, if instrumented.

If the browser shows long tasks during the freeze, fix parse/render pressure first. If long tasks
are absent but snapshot receive intervals show 300-500 ms gaps under loss, WebTransport or
latest-only snapshot queuing is more likely to help.

## Lower-Risk Fix To Try Before WebTransport

Current code treats snapshots and reliable control messages the same inside one per-player
`mpsc` queue. Since snapshots are full-state and superseded by newer snapshots, they should not
wait behind older snapshots.

Possible WebSocket-only change:

- Split each player's outbound path into:
  - reliable ordered control queue: `welcome`, `lobby`, `start`, `gameOver`, `pong`, `error`, and
    important notices;
  - latest snapshot slot: one replaceable `Snapshot` per player.
- Writer loop prioritizes reliable control messages, then sends the newest available snapshot.
- If five snapshots arrive while the socket is blocked, the writer sends only the latest one after
  the socket becomes writable.
- Keep snapshots full-state so dropping old snapshots is safe.

This keeps the simple WebSocket deployment and may directly address the "freeze then catch up"
pattern. It does not solve TCP loss blocking delivery of the newest snapshot while an older frame is
already in flight, but it avoids sending a backlog of stale frames after the blockage clears.

Tests for this mitigation:

- Existing `tests/server_integration.mjs` should keep passing.
- Add or adapt a test that simulates a slow writer and confirms the room does not enqueue more than
  one stale snapshot per player.
- Add logging to prove stale snapshots are coalesced rather than sent in order.

## WebTransport Protocol Shape

Keep the game authoritative exactly as it is. Only the transport fanout changes.

Suggested channels:

| Channel | WebTransport primitive | Reliability | Messages |
| --- | --- | --- | --- |
| Control stream | bidirectional stream | reliable, ordered | `welcome`, `join`, `ready`, `start`, `addAi`, `removeAi`, `setQuickstart`, `gameOver`, `error`, connection close |
| Command stream | same control stream or second bidirectional stream | reliable, ordered | `command`, `ping`, `pong`, `setReplaySpeed` |
| Snapshot channel | datagrams, if payload fits | unreliable, unordered | `snapshot` state, visual `attack`/`death`/`build` events |
| Snapshot fallback | unidirectional stream per snapshot | reliable per snapshot, independent across streams | oversize snapshots or environments without datagrams |

The first implementation can keep JSON payloads for control and commands. For snapshots, JSON over
datagrams is probably too large long-term, but it is useful for a dev spike if current payloads fit
the browser/server datagram limit.

### Datagram snapshot header

Use a tiny binary prefix even if the payload remains JSON:

```text
byte 0      protocol version, start at 1
byte 1      message kind, 1 = snapshot
bytes 2-5   tick, u32 little-endian
bytes 6-9   match/session id, u32 little-endian
bytes 10..  payload bytes
```

Client behavior:

- Drop datagrams with an unknown version.
- Drop datagrams for a previous match/session id.
- Drop snapshots with `tick <= current.tick`.
- Keep at most the two newest accepted snapshots for interpolation.
- If a gap occurs, interpolate between the newest two snapshots when possible; otherwise freeze on
  the newest snapshot. Do not extrapolate hidden state.

Server behavior:

- Send snapshots as best effort.
- Never retransmit old datagram snapshots.
- If a snapshot is too large for datagrams, either send it over the snapshot stream fallback or skip
  that tick and log the oversize payload. Do not chunk snapshots into multiple datagrams for the
  first version.

### Why not chunk datagrams first?

Chunking creates a reliability protocol. If one chunk is lost, the whole snapshot cannot be decoded,
and the client needs reassembly buffers, expiry, duplicate handling, and memory limits. That is the
complexity WebTransport was supposed to avoid. Prefer smaller self-contained snapshots or a
per-snapshot stream fallback.

## Making Snapshots Safe To Lose

Unreliable snapshots require every accepted snapshot to be independently useful. Audit these fields:

- `tick`: already present and enough for ordering.
- `steel`, `oil`, `supplyUsed`, `supplyCap`: current absolute values, safe to lose older values.
- `entities`: current visible entity views, safe because every snapshot is full visible state.
- `events`: transient visual flavor, mostly safe to lose.
- `resourceDeltas`: not safe if treated as the only way the client updates static resource
  remaining counts.

`resourceDeltas` is the biggest current semantic trap. Today `start.map.resources` carries static
resource positions, and snapshots carry visible remaining updates. If an unreliable snapshot with
the latest remaining amount is lost, the client may render a stale resource amount indefinitely
until another visible delta arrives.

Possible fixes:

1. Include visible resource remaining values in every snapshot, even if unchanged.
2. Move resource remaining updates to the reliable control stream.
3. Add a periodic reliable resource keyframe.
4. Reintroduce visible resource entities for WebTransport snapshots and keep WebSocket compaction
   as a legacy-transport optimization.

Option 1 is the simplest protocol rule: current render-critical state is absolute, repeated, and
safe to lose once.

Also decide whether `notice` events are important user feedback. If "Not enough steel" must be
seen, move notices out of unreliable snapshot events and onto the reliable control stream.

## Payload Size Strategy

Datagrams need conservative sizing. Do not assume that a 4 KB JSON snapshot will fit. The browser
API exposes datagram limits; the server stack will have its own limit; the network path has an MTU.

Initial rule:

- Log the actual WebTransport datagram max size at connection start.
- Keep snapshot datagrams below the lower of client/server max size.
- Also keep a conservative target around a single UDP packet until measurement proves otherwise.
- If the snapshot is larger, use stream fallback or skip the datagram and wait for the next smaller
  snapshot. Do not silently truncate.

If snapshots do not fit, reduce size before building a complex chunking protocol:

- Replace JSON snapshots with a compact binary snapshot format.
- Intern strings (`kind`, `state`, optional enum fields) into small numeric ids.
- Quantize positions to fixed-point integers if visual quality allows it.
- Omit unchanged optional fields only if the snapshot remains independently decodable.
- Consider lowering snapshot rate from 30/s to 15/s while keeping 60 fps interpolation.
- Consider interest filtering beyond fog if late-game payloads are large.

Binary encoding is not required for reliable control messages. Keep lobby/control JSON for
simplicity unless measurement shows it matters.

## Server Implementation Options

Current server:

- axum 0.8 with `ws` feature;
- HTTP static file serving and `/ws` in one process;
- no HTTP/3, no QUIC, no UDP listener.

Likely Rust options:

### Option A: `wtransport`

Use the `wtransport` crate as a separate QUIC/WebTransport endpoint and keep axum for HTTP static
files plus WebSocket fallback.

Pros:

- purpose-built WebTransport API;
- likely fastest dev spike;
- can share the existing `Lobby` instance with both transports if server startup is refactored.

Cons:

- separate listener and TLS setup;
- not an axum route;
- deployment must expose UDP/QUIC to the process;
- connection handling must be adapted to the existing `RoomEvent` and outbound message model.

### Option B: `h3` plus `h3-webtransport`

Build an HTTP/3 server stack directly.

Pros:

- closer to the underlying RFC model;
- more control if we later want a single HTTP/3 server for multiple routes.

Cons:

- more low-level;
- more moving parts;
- higher risk of spending effort on server plumbing instead of transport semantics.

### Option C: proxy terminates WebTransport

Do not assume this is available. WebTransport support in reverse proxies and hosting platforms is
not as routine as HTTPS/WSS. If a proxy terminates HTTP/3, the upstream still needs a compatible
way to receive WebTransport sessions, streams, and datagrams. Treat this as a deployment research
task, not an implementation shortcut.

## Client Implementation Shape

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

Add a transport abstraction under `client/src/net.js` or split files:

- `WebSocketTransport`: current behavior.
- `WebTransportTransport`: new behavior.
- `Net`: owns fallback choice and keeps the rest of the app unchanged.

Connection flow:

1. If `window.WebTransport` exists and the page is a secure context, try WebTransport first when
   enabled by a query flag or config.
2. Wait for `transport.ready`.
3. Open the reliable control stream.
4. Start reading incoming bidirectional/unidirectional streams and datagrams.
5. If setup fails, fall back to WebSocket unless a dev flag requires WebTransport.
6. Emit the same `"open"` and `"close"` synthetic events as today.

Feature flags:

- `?transport=ws`: force WebSocket.
- `?transport=webtransport`: force WebTransport and show errors instead of fallback.
- default during rollout: WebSocket.
- eventual default: try WebTransport, fall back to WebSocket.

## Server Refactor Shape

Do not let WebTransport code touch `Game` directly. Preserve the existing seam:

```text
transport reader -> ClientMessage -> RoomEvent -> RoomTask -> Game
RoomTask -> per-player outbound sink -> transport writer
```

Suggested refactor:

- Introduce an internal outbound enum that separates reliability class:
  - `Reliable(ServerMessage)`;
  - `Snapshot(Snapshot)`.
- Keep `RoomEvent::Join { msg_tx, ... }` conceptually the same, but replace the raw
  `mpsc::Sender<ServerMessage>` with a small connection handle if needed.
- The room remains the only owner of `Game`.
- The WebSocket writer serializes both reliable messages and snapshots to JSON text.
- The WebTransport writer sends reliable messages on the control stream and snapshots through the
  datagram/stream snapshot path.

For a first docs-to-code spike, avoid changing `protocol.rs` shapes. Use the same serde messages
until transport mechanics work, then optimize snapshot encoding separately.

## Deployment Questions

The existing `docs/fly.md` says Fly proxies HTTPS and WSS traffic to the container on port 8080.
That is enough for WebSocket. WebTransport needs HTTP/3 over QUIC, which means UDP reachability and
TLS/ALPN behavior must be proven.

Deployment questions for a follow-up agent:

- Can the deployed environment pass browser WebTransport traffic to the Rust process?
- Does it support UDP services on the desired public port?
- If the platform terminates TLS, can it proxy WebTransport sessions/datagrams upstream?
- If the Rust process terminates TLS itself, how will it get a valid certificate?
- Can the static site and WebTransport endpoint share an origin, or will CORS / certificate /
  port restrictions make that awkward?
- How does fallback to WebSocket behave when UDP is blocked by a corporate network?

Do not remove WebSocket until deployment support is proven in production-like conditions.

## Testing Plan

Keep the existing WebSocket tests. They are valuable fallback coverage and easy to run:

```bash
tests/run-all.sh
```

Additional WebTransport tests:

- Unit-test frame encode/decode for datagram headers.
- Unit-test stale snapshot rejection by tick/session id.
- Unit-test resource remaining semantics under lost snapshots.
- Browser smoke test for WebTransport connection establishment and a playable match.
- Loss simulation test:
  - drop every Nth snapshot datagram;
  - reorder snapshots;
  - duplicate snapshots;
  - assert the client keeps rendering and never applies older ticks.
- Oversize snapshot test:
  - force payload above datagram max;
  - assert stream fallback or explicit skip/log behavior.
- Fallback test:
  - force WebTransport unavailable;
  - assert WebSocket path still works.

Node's built-in WebSocket is enough for the current integration tests. Do not assume Node has a
browser-equivalent `WebTransport` API. Use a real browser for WebTransport smoke coverage.

## Rollout Plan

### Phase 0: measurement

Add temporary or dev-gated instrumentation. Capture traces for the actual freeze. Decide whether
the dominant issue is:

- network delivery gaps;
- stale snapshot backlog;
- JSON parse/allocation;
- renderer/HUD/minimap frame work;
- server tick/snapshot construction.

Exit criteria:

- one trace showing a freeze with correlated network, parse, frame, and server timings;
- snapshot payload p50/p90/p99/max for a worst-case match;
- a clear call on whether WebTransport is solving the top problem.

### Phase 1: WebSocket latest-only snapshots

Implement the lower-risk queue discipline change. This may reduce stutter without changing
deployment.

Exit criteria:

- stale snapshots are coalesced;
- reliable messages are not dropped behind snapshots;
- existing tests pass;
- freeze trace improves or does not.

### Phase 2: transport abstraction

Refactor client and server transport boundaries without changing behavior.

Exit criteria:

- WebSocket behavior stays identical;
- `Net` public API remains unchanged;
- `RoomTask` still only sees room events and outbound connection handles.

### Phase 3: dev-only WebTransport reliable control

Add WebTransport connection setup and reliable stream control messages. Keep snapshots on reliable
streams initially if needed.

Exit criteria:

- browser can join lobby, start game, send commands;
- WebSocket fallback remains available;
- deployment is not changed yet.

### Phase 4: unreliable/latest snapshots

Move snapshots to datagrams or per-snapshot streams with stale rejection.

Exit criteria:

- client tolerates lost/reordered/duplicated snapshots;
- resource remaining semantics are fixed;
- oversized snapshots have explicit behavior;
- stutter trace improves under packet loss.

### Phase 5: payload optimization

If datagrams are too large, design binary snapshots or lower snapshot rate before adding chunking.

Exit criteria:

- worst-case snapshots fit the chosen datagram budget, or stream fallback is deliberately accepted;
- parse/allocation cost is lower than JSON baseline.

### Phase 6: production deployment

Prove UDP/HTTP/3/TLS behavior in the deployment environment and keep WebSocket fallback.

Exit criteria:

- real browser connects over WebTransport in deployed environment;
- WebSocket fallback works when WebTransport fails;
- metrics distinguish WebSocket vs WebTransport sessions.

## Decision Matrix

| Approach | Difficulty | Stutter upside | Main risk |
| --- | --- | --- | --- |
| Measure only | Low | None directly | no fix yet |
| WebSocket latest-only snapshots | Low-medium | Medium if stale backlog is the freeze | TCP HOL remains |
| WebSocket binary snapshots | Medium | Medium if parse/GC is the freeze | does not fix TCP HOL |
| WebTransport reliable stream only | Medium-high | Low-medium | most ordered delivery remains |
| WebTransport datagram snapshots | High | High if network HOL is the freeze | unreliable-state semantics and payload size |
| WebTransport plus binary snapshots | High | Highest | largest implementation and test surface |

## Recommended Next Step

Do Phase 0 and Phase 1 before a full WebTransport implementation.

Reasoning:

- The current code can queue many stale snapshots per client.
- Dropping old snapshots is semantically valid because snapshots are already full visible state,
  except for the `resourceDeltas` caveat.
- This change directly targets freeze/catch-up behavior and keeps the simple deployment.
- If packet-loss traces still show ordered-stream HOL after stale snapshot coalescing, the repo will
  have the measurement and reliability semantics needed for a WebTransport datagram implementation.

## References

- W3C WebTransport Working Draft: https://www.w3.org/TR/webtransport/
- WebTransport over HTTP/3 IETF draft: https://datatracker.ietf.org/doc/html/draft-ietf-webtrans-http3
- HTTP Datagrams and Capsules RFC 9297: https://datatracker.ietf.org/doc/html/rfc9297
- QUIC RFC 9000: https://datatracker.ietf.org/doc/html/rfc9000
- WebSocket RFC 6455: https://datatracker.ietf.org/doc/html/rfc6455
- Chrome WebTransport docs: https://developer.chrome.com/docs/capabilities/web-apis/webtransport
- MDN WebTransport API reference: https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API
- `wtransport` crate docs: https://docs.rs/wtransport/latest/wtransport/
- `h3-webtransport` crate docs: https://docs.rs/h3-webtransport/latest/h3_webtransport/
