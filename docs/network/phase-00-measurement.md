# Phase 00: Measure The Freeze

Purpose: prove where the half-second freeze happens before changing the transport. Do not claim
network head-of-line blocking, stale snapshot backlog, message parse/apply cost, or server tick
cost without a trace.

This phase is intentionally simple and can be done before WebTransport work starts.

## Questions To Answer

- Are snapshots arriving late, arriving in bursts, or arriving on time but taking too long to parse
  and apply?
- Is the server tick loop falling behind?
- Are socket writes blocking the per-connection writer?
- How large are worst-case snapshots in a real late-game or stress match?
- Does packet loss reproduce the freeze?

## Client Instrumentation

Add temporary or dev-gated probes. Keep them behind a query flag such as `?netDebug=1`, a local
constant, or a small debug module. Do not leave noisy production logging enabled by default.

Instrument `client/src/net.js` `_onMessage`:

- raw payload byte length;
- `JSON.parse` duration;
- parsed message tag;
- snapshot tick when `m.t === "snapshot"`;
- time since previous snapshot message.

Instrument `client/src/state.js` `applySnapshot`:

- duration;
- snapshot tick;
- entity count;
- resource delta count;
- event count;
- selection size before/after pruning;
- muzzle flash count after pruning.

Instrument `client/src/main.js` only for snapshot timing state:

- computed interpolation alpha;
- `prevRecvTime`, `currRecvTime`, and the gap between them;
- current snapshot tick and previous snapshot tick.

Optionally add a `PerformanceObserver` for `longtask` entries as a sanity check. Do not turn this
phase into unrelated UI optimization work:

```js
if (typeof PerformanceObserver !== "undefined") {
  const observer = new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) {
      console.log("[longtask]", Math.round(entry.duration), entry.startTime);
    }
  });
  observer.observe({ entryTypes: ["longtask"] });
}
```

Use compact structured logs. A weak follow-up agent should be able to paste log lines into a script
and compute p50/p90/p99.

## Server Instrumentation

Add temporary tracing around these paths:

- `RoomTask::on_tick` total duration in `server/src/lobby.rs`;
- `game.tick()`;
- `game.snapshot_for(player_id)`;
- `compact_snapshot_for_wire`;
- per-player outbound send result in `send_or_log`;
- message kind and serialized byte length in the writer task in `server/src/main.rs`;
- duration of `sink.send(Message::Text(...))`.

The current room task does not await socket writes directly, so look for per-player writer stalls
and queue growth separately from room tick stalls.

Do not estimate payload sizes in comments or PRs. Log measured byte lengths.

## Snapshot Size Baseline

The initial docs investigation sampled early-game frames only:

- normal fog-filtered player: about 1.2 KB per snapshot with 5 visible entities;
- early full-world self-play watch: about 3.8 KB per snapshot with 10-12 visible entities.

Those are not worst-case numbers. This phase needs a bigger sample:

- late-game human vs AI;
- self-play replay after armies and buildings exist;
- a local stress setup with many owned and visible entities;
- normal fog-filtered and dev full-world modes.

Record p50/p90/p99/max for:

- snapshot JSON byte length;
- entity count;
- resource delta count;
- event count;
- parse duration;
- apply duration;

## Network Loss Reproduction

Try at least one repeatable loss setup:

- Chrome DevTools throttling, if it reproduces the freeze;
- macOS Network Link Conditioner;
- a local proxy or OS-level packet-loss tool;
- deployed game with browser traces and measured packet loss.

Record:

- snapshot receive interval histogram;
- `net.latency` p50/p90/p99 from app-level pings;
- snapshot byte p50/p90/p99/max;
- dropped/stale snapshot counts, if Phase 01 coalescing has been added.

## Interpretation Guide

If snapshot receive intervals show 300-500 ms gaps under loss:

- network delivery or ordered-stream head-of-line is plausible;
- Phase 01 coalescing should still be tried first because it is simpler;
- WebTransport datagrams become more attractive if gaps remain after coalescing.

If the server tick loop falls behind:

- inspect `game.tick()`, snapshot projection, and per-player snapshot construction;
- do not start with transport changes.

If writer `sink.send(...)` stalls but the room tick loop does not:

- Phase 01 is directly relevant because stale snapshots can accumulate per player.

## Done Criteria

- There is one trace showing an actual freeze with client, server, and network timings.
- Worst-case snapshot payload p50/p90/p99/max are recorded.
- The trace identifies the dominant class of problem.
- The next phase recommendation is explicit: message parse/apply cleanup, Phase 01 coalescing, or
  deeper WebTransport work.
