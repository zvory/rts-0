# WebTransport Feasibility Overview

Status: docs-only investigation.

Date: 2026-06-02.

Read `DESIGN.md` first before changing networking contracts. This overview is the index and
decision summary; each implementation-sized chunk lives in a phase file under `docs/network/`.

## Short Answer

Switching to WebTransport is plausible, but it is not a small transport swap. The useful version is
not "WebSocket JSON on a WebTransport reliable stream"; that keeps most ordered-stream behavior.
The useful version keeps commands and session control reliable, while sending snapshots as
latest-only state over WebTransport datagrams or independent QUIC streams.

Before WebTransport, do the simpler work:

1. Measure the freeze so we know whether the dominant problem is network delivery, stale snapshot
   backlog, message parse/apply, or server tick work.
2. Make WebSocket snapshot delivery latest-only so old snapshots do not queue behind socket stalls.
3. Optionally remove the obvious redundant entity interpolation/allocation path.
4. Refactor the transport boundary so later WebTransport work does not touch `Game`.

The current likely low-risk win is Phase 01: WebSocket snapshot coalescing. It directly addresses a
real repo issue: `PLAYER_CHANNEL_CAP = 256` means a backed-up player can accumulate roughly 8.5 s
of stale 30 Hz snapshots.

Phase 02 is intentionally narrow. It does not claim client-side rendering is the stutter cause; it
only removes repeated `entitiesInterpolated()` work that is obviously wasteful.

## Current Repo Facts

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

Important implication: a slow socket should not stall the room simulation globally, but an
individual connection can still receive old snapshots in order after a short network or socket
write stall. That can look like a client-side freeze followed by delayed or bursty state updates.

## Quick Baseline

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

- Early normal snapshots are already near the common "roughly one UDP packet" target. Once
  WebTransport datagram headers are added, JSON snapshots of this shape may not fit comfortably in
  one datagram.
- Full-world snapshots do not fit a conservative one-datagram budget.
- These numbers do not explain a half-second freeze by themselves. Worst-case late-game snapshots,
  receive gaps, stale backlog behavior, and message parse/apply cost still need measurement.

## Phase Index

| Phase | File | Scope | Difficulty |
| --- | --- | --- | --- |
| 00 | [measurement](phase-00-measurement.md) | Instrument and prove where the freeze happens. | Low |
| 01 | [WebSocket coalescing](phase-01-websocket-coalescing.md) | Keep WebSocket, but make snapshots latest-only. | Low-medium |
| 02 | [entity interpolation cleanup](phase-02-entity-interpolation-cleanup.md) | Avoid redundant `entitiesInterpolated()` allocations. | Low |
| 03 | [transport abstraction](phase-03-transport-abstraction.md) | Refactor boundaries without changing behavior. | Medium |
| 04 | [WebTransport control plane](phase-04-webtransport-control-plane.md) | Dev-only reliable WebTransport setup. | Medium-high |
| 05 | [unreliable snapshots](phase-05-unreliable-snapshots.md) | Datagrams or per-snapshot streams with stale rejection. | High |
| 06 | [payload optimization](phase-06-payload-optimization.md) | Binary/compact snapshots if datagrams are too large. | High |
| 07 | [deployment rollout](phase-07-deployment-rollout.md) | UDP/HTTP/3/TLS proof, fallback, metrics. | Medium-high |

Take only one phase per branch unless the user explicitly asks for a broader implementation.
Phases 00-02 are deliberately useful before WebTransport exists.

## Decision Matrix

| Approach | Stutter upside | Main risk |
| --- | --- | --- |
| Measure only | None directly | no fix yet |
| WebSocket latest-only snapshots | Medium if stale backlog is the freeze | TCP head-of-line remains |
| WebSocket binary snapshots | Medium if parse/GC is the freeze | does not fix TCP head-of-line |
| WebTransport reliable stream only | Low-medium | most ordered delivery remains |
| WebTransport datagram snapshots | High if network head-of-line is the freeze | unreliable-state semantics and payload size |
| WebTransport plus binary snapshots | Highest | largest implementation and test surface |

## Recommendation

Do Phase 00 and Phase 01 before a full WebTransport implementation. Phase 02 is optional but cheap.

Reasoning:

- The current code can queue many stale snapshots per client.
- Dropping old snapshots is semantically valid because snapshots are already full visible state,
  except for the `resourceDeltas` caveat called out in Phase 05.
- Coalescing directly targets freeze/catch-up behavior and keeps the simple deployment.
- Phase 02 removes redundant interpolation allocation without treating client rendering as the root
  cause.
- If packet-loss traces still show ordered-stream head-of-line after stale snapshot coalescing, the
  repo will have the measurement and reliability semantics needed for WebTransport datagrams.

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
