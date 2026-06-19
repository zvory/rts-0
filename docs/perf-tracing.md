# Server performance tracing

## Browser frame phase profiling

During a live match or replay, the browser exposes a local-only frame profiler at
`window.__rtsPerf`. The profiler keeps bounded aggregate timings in memory so a local lag report can
separate slow frame gaps from concrete client work.

Useful console calls:

```js
window.__rtsPerf.summary()
window.__rtsPerf.text()
window.__rtsPerf.copy()
window.__rtsPerf.reset()
```

The summary includes total frames, slow-frame count, recent frames, approximate p50/p95 buckets, max
time, slow sample count, and worst-phase counts for match phases such as camera, input, prediction
visual advance, fog, renderer, HUD, minimap, observer analysis, and health publish. Renderer
sub-phases cover entity preparation, feedback view building, resources/buildings, units,
selection/HP, shot reveals, sweeps, fog draw, feedback/effects overlays, and placement.

Shape context is intentionally bounded: entity counts, selected count, remembered building count,
visible tile count, viewport/canvas size, device pixel ratio, prediction mode, and
hidden/focused state. Use `copy()` when filing a local browser performance note; it writes a
tab-separated phase table to the clipboard when the browser allows clipboard access, otherwise it
prints the same text to the console.

Optional server-side performance tracing is controlled by environment variables. It is off by
default and emits structured `tracing` logs under the `server::perf` target when enabled.

Every `ClientNetReport` upload includes a bounded report-window summary from the same profiler:
`frameWorkMaxMs`, `frameWorkP95Ms`, `slowFrameCount`, `worstFramePhase`, `worstFramePhaseMs`,
`rendererMaxMs`, `rendererP95Ms`, `entityCount`, `selectedCount`, `visibleTileCount`,
`viewportWidth`, `viewportHeight`, and `devicePixelRatioX100`. Report-window profiler counters reset
after each upload; the `window.__rtsPerf` debug summary remains cumulative until `reset()` is called.

The same upload also includes bounded snapshot diagnostics:

- Payload size: `snapshotBytesTotal`, `snapshotBytesMax`, `snapshotBytesAvg`, and
  `snapshotMessageCount`.
- Browser processing: `snapshotParseMaxMs`/`snapshotParseP95Ms`,
  `snapshotDecodeMaxMs`/`snapshotDecodeP95Ms`, `snapshotApplyMaxMs`/`snapshotApplyP95Ms`, and
  `predictionApplyMaxMs`/`predictionApplyP95Ms`.
- Cadence: `snapshotTickGapMax`, `staleSnapshotCount`, `duplicateSnapshotCount`,
  `skippedSnapshotCount`, `snapshotBurstCount`, and `snapshotBurstMax`.

Command-response diagnostics are also reported as bounded window aggregates keyed by the live
`matchRunId`: issued/send-accepted/server-received/sim-acknowledged/rejected counts,
issue-to-server-receipt latest/max/p95, server-receipt-to-sim-ack latest/max/p95,
issue-to-sim-ack latest/max/p95, ack-snapshot-received-to-applied latest/max/p95, oldest pending
command age, and max pending command count. The server receipt comes from a tiny reliable
`commandReceipt` message keyed only by `clientSeq`; it carries no command payload, unit ids, target
ids, positions, or player-entered text and does not reconcile prediction.

Raw snapshot JSON, raw timestamp arrays, raw phase arrays, recent frame records, stack traces, entity
ids, command payloads, command targets, and replay data are intentionally not uploaded.

## Modes

```bash
RTS_PERF=off      # default
RTS_PERF=spikes   # log ticks slower than RTS_PERF_SLOW_TICK_MS
RTS_PERF=sample   # log every RTS_PERF_SAMPLE_EVERY ticks and all slow ticks
RTS_PERF=full     # log every tick, phase, snapshot, and writer send
```

Useful thresholds:

```bash
RTS_PERF_SLOW_TICK_MS=33       # default is the configured tick interval
RTS_PERF_SLOW_PHASE_MS=8       # phase detail threshold on logged ticks
RTS_PERF_SLOW_SNAPSHOT_MS=8    # per-recipient snapshot detail threshold
RTS_PERF_SLOW_SEND_MS=10       # writer serialization/send threshold
RTS_PERF_SAMPLE_EVERY=300      # sample mode interval
RTS_PERF_LOG_SNAPSHOTS=1       # include every snapshot detail on logged ticks
```

Example local run:

```bash
cd server && RUST_LOG=info,server::perf=debug RTS_PERF=spikes cargo run --release
```

Four-AI local harness:

```bash
scripts/ai-perf-harness.sh --ticks 20000
```

The harness runs four AI players in one local match, enables sample perf tracing by default, and
exercises simulation, per-player snapshot fanout, snapshot compaction, and compact JSON
serialization without requiring browser clients. Override the usual `RTS_PERF*` and `RUST_LOG`
environment variables when you need a different trace shape, or pass `--perf full` for every tick.

Browser client performance harness:

```bash
node scripts/client-perf-harness.mjs --list
node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6
```

The browser harness starts a local server on an isolated port unless `RTS_URL` or `--base-url`
points at an already-healthy server. It drives headless Chrome with the existing
`tests/package.json` `puppeteer-core` dependency path, copies the preserved Matt/Alex replay into
`server/target/selfplay-artifacts/client_perf_matt_alex_match_54/replay.json` at runtime, and writes
one `summary.json` per workload under `target/client-perf/<workload>/<timestamp>/`. Pass `--trace`
to also write a Chrome `trace.json`; traces are opt-in because they are larger and machine-local.

The default harness result fails for runtime errors, page/console/request errors, and missing
`window.__rtsPerf` or generated `ClientNetReport` summaries. It deliberately does not fail on an
absolute FPS, frame time, or trace-timing budget. Treat the numbers as local evidence for comparing
optimization branches on the same machine, not as a portable guarantee for other laptops.

The Fly production deploy enables the low-noise spike mode in `fly.toml`:

```toml
RTS_PERF = "spikes"
RTS_PERF_SLOW_TICK_MS = "40"
```

That emits one `performance tick summary` log line when a server tick takes at least 40 ms. Phase,
snapshot, and WebSocket writer detail remains at `debug`, so the production default keeps basic
slow-tick visibility without logging every tick.

Perf tracing intentionally exits at startup if `RTS_PERF` is enabled in a debug build. Debug builds
include overflow checks and different optimization behavior, so their timings are not useful for
production lag diagnosis.

Tick summaries separate simulation, snapshot fanout, scheduler lag, room shape, entity counts,
snapshot coalescing, and the slowest concrete simulation phase. Snapshot and writer lines identify
whether lag is in per-player projection/compaction, JSON serialization, or socket writes.

## Incident examples

- [2026-06-19 beta Matt/Alex lag incident](network-incident-examples/2026-06-19-beta-matt-alex/)
  preserves raw Fly logs, replay artifacts, player-report quotes, parsed net reports, and an
  analysis-only write-up for a case where the server stayed healthy but one player had both
  network/snapshot jitter and poor local frame pacing.

## Structured server logging

Server logs in `server/src` must go through `server/src/structured_log.rs`. Use the helper macros
for ordinary logs:

```rust
crate::log_info!(room = %room, "room created");
crate::log_warn!(room = %room, error = %err, "room task failed");
```

Use a named helper function in `structured_log.rs` when a log needs stable fields, correlation, or
issue classification. Current high-signal helpers cover:

- `client_net_report` with `build_id`, `match_run_id`, and `primary_issue`.
- `performance tick summary` rows include `match_run_id` when emitted by a live room.
- `match_started` with `match_run_id`, map, seed, participants, build, and player counts.
- `match_ended` with `match_run_id`, duration, tick count, slow-tick count, head-of-line max, and
  replay/history context.

`scripts/check-structured-logging.sh` fails if new direct `tracing::{info,warn,error,debug,trace}`
calls are added under `server` outside the helper. The only exception is
`server/crates/sim/src/perf.rs`, which is the centralized simulation performance logging surface and
cannot depend back on the server crate. `tests/run-all.sh` runs that check as part of the
architecture policy gate.

`client_net_report` issue classification uses stable buckets:

- `client_renderer`: `rendererMaxMs >= 33` or `rendererP95Ms >= 16`; inspect `worstFramePhase`,
  entity count, visible tiles, viewport size, and DPR to separate paint-heavy scenes from network
  lag.
- `payload_pressure`: `snapshotBytesMax >= 262144` or `snapshotBytesAvg >= 131072`; inspect server
  snapshot/perf rows for projection or compaction pressure.
- `client_snapshot_parse`: `snapshotParseMaxMs >= 16` or `snapshotParseP95Ms >= 8`; points at
  browser JSON parsing cost for received snapshot frames.
- `client_snapshot_decode`: `snapshotDecodeMaxMs >= 16` or `snapshotDecodeP95Ms >= 8`; points at
  compact-protocol expansion cost after JSON parse.
- `client_snapshot_apply`: `snapshotApplyMaxMs >= 16`, `snapshotApplyP95Ms >= 8`,
  `predictionApplyMaxMs >= 16`, or `predictionApplyP95Ms >= 8`; points at applying the decoded
  snapshot into `GameState` or reconciling prediction overlays.
- `client_frame_work`: `frameWorkMaxMs >= 33` or `frameWorkP95Ms >= 24`; points at local browser
  work outside pure renderer cost.
- `client_frame_stall`: `frameGapMaxMs >= 100`, or `slowFrameCount > 0` when frame-work thresholds
  were not crossed; points at requestAnimationFrame gaps even when measured frame work was not the
  dominant issue.
- Existing buckets continue to separate `network_rtt`, `snapshot_gap`, `snapshot_jitter`,
  `snapshot_cadence`, `server_tick`, `server_scheduler_lag`, `websocket_backlog`, `pending_commands`,
  `prediction_correction`, `prediction_disabled`, and `wasm_budget`.
- `command_upload_delay`, `command_server_queue`, `command_response_delay`, `command_ack_apply`, and
  `command_rejected` classify command milestone issues before they fall through to generic RTT or
  prediction fallback buckets. Upload delay is high issue-to-receipt timing; server queue delay is
  high receipt-to-sim-ack timing; response delay is high issue-to-sim-ack or oldest pending age; ack
  apply points at browser processing after the ack snapshot arrives.

`snapshot_cadence` covers `snapshotTickGapMax >= 3`, stale/duplicate/skipped snapshot counters, or
`snapshotBurstMax >= 3`. Use it to distinguish receive burst/head-of-line symptoms from high RTT:
large payload fields plus parse/decode cost point at client payload pressure, while clean payload
fields plus high RTT/jitter point at the network path.

For Fly or local logs, start with:

```bash
scripts/fly-logs.sh beta recent | rg 'client_net_report|primary_issue'
```
