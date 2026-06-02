# Network Stutter Plan

Status: Phases 01-03 implemented; Phases 04-05 remain planned.

Date: 2026-06-02.

Read `DESIGN.md` first before changing networking contracts. This plan keeps the current WebSocket
transport and focuses on reducing repeated small client-visible stutters without a transport
rewrite.

## Short Answer

The best network-side work was to stop treating every outbound message as equal FIFO traffic. Before
Phases 01-03, the server sent snapshots at 30/s/player through the same per-player path as reliable
control messages. Under writer stalls, this could create stale snapshot backlog and catch-up
behavior even though the authoritative simulation continued.

Do the simpler WebSocket work in this order:

1. Prioritize reliable messages so `pong`, `error`, `start`, and `gameOver` do not sit behind
   pending snapshots.
2. Make snapshots latest-only so old unsent snapshots are replaced by newer state.
3. Slightly increase the interpolation buffer to absorb small receive jitter.
4. Optionally remove the obvious redundant entity interpolation/allocation path.
5. Compact or binary-encode WebSocket snapshots only if the simpler queue and buffer work still
   leaves obvious snapshot-size or parse/apply pressure.

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
- `client/src/config.js`: `SNAPSHOT_MS` and `INTERP_DELAY_MS`.
- `DESIGN.md`: current architecture contract: JSON over WebSocket, 30 Hz tick, 30 snapshots/s.

Current model:

- `TICK_HZ = 30`.
- `SNAPSHOT_EVERY_N_TICKS = 1`, so the server attempts 30 snapshots/s/player.
- Client commands are small JSON messages sent upstream through the same WebSocket.
- Server snapshots are full per-player state, fog-filtered, JSON text frames.
- The room task does not await socket writes directly. It sends reliable messages through a bounded
  per-player FIFO and snapshots through a replaceable latest-only slot.
- The per-connection writer task drains reliable messages first, sends at most one latest snapshot,
  then checks reliable messages again. It serializes `ServerMessage` with `serde_json::to_string`
  and awaits `sink.send(Message::Text(...))`.
- Pending snapshots are coalesced, so a backed-up player cannot accumulate seconds of stale 30 Hz
  snapshot frames in server memory. Resource remaining deltas are carried forward when pending
  snapshots are replaced.

## Existing Payload Samples

The first docs investigation sampled early-game frames only:

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

These are not worst-case numbers. Treat them only as context. Do not block Phases 01-03 on better
payload data.

## Phase Index

| Phase | File | Scope | Difficulty |
| --- | --- | --- | --- |
| 01 | [reliable message priority](phase-01-reliable-message-priority.md) | Keep control messages out from behind snapshot backlog. | Low-medium |
| 02 | [latest-only snapshots](phase-02-latest-only-snapshots.md) | Replace stale unsent snapshots with the newest state. | Low-medium |
| 03 | [interpolation buffer tuning](phase-03-interpolation-buffer.md) | Slightly increase render delay to absorb receive jitter. | Low |
| 04 | [entity interpolation cleanup](phase-04-entity-interpolation-cleanup.md) | Avoid redundant `entitiesInterpolated()` allocations. | Low |
| 05 | [compact/binary WebSocket snapshots](phase-05-websocket-compact-snapshots.md) | Reduce WebSocket frame size and parse/apply pressure. | Medium-high |

Take only one phase per branch unless the user explicitly asks for a broader implementation.

## Recommendation

Do Phases 01-03 first.

Reasoning:

- Reliable priority and latest-only snapshots directly target freeze/catch-up behavior.
- Interpolation buffer tuning can absorb small receive jitter without changing server behavior.
- Phase 04 is optional and intentionally narrow.
- Phase 05 should stay later because compact or binary protocols add test surface.

## References

- WebSocket RFC 6455: https://datatracker.ietf.org/doc/html/rfc6455
