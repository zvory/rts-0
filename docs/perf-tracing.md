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
visible tile count, viewport/canvas size, device pixel ratio, match mode, local harness workload id,
prediction mode, and hidden/focused state. Use `copy()` when filing a local browser performance note; it writes a
tab-separated phase table to the clipboard when the browser allows clipboard access, otherwise it
prints the same text to the console.

`window.__rtsPerf.summary().renderDiagnostics` adds local-only bounded counters next to the timing
phases. These counters do not contain raw frames, raw entity arrays, entity ids, replay data, stack
traces, or command payloads; they are aggregate labels and counts that explain why a measured frame
path is busy:

- `renderer.pixi.displayObject.*` counts Pixi object churn: objects created, reused, hidden, or
  destroyed by the renderer pools.
- `renderer.rig.*` counts live rig instance reuse and SVG rig part redraws attempted, skipped, or
  completed. High completed redraws point at rig geometry churn; high skipped redraws mean the rig
  cache is doing useful work.
- `renderer.graphics.clear.*` counts `Graphics.clear()` calls for pooled entities, HP bars,
  selection rings, fog, placement, feedback, smoke, ability-object, and rig-part graphics.
- `renderer.redraw.*` counts draw-path attempts, completions, and failures for entities and named
  overlays. Pair this with `renderer.*` timing rows to identify which overlay needs deeper timing.
- `minimap.cache.*` and `minimap.invalidate.*` count static, resource, and fog-layer cache hits,
  misses, and stable invalidation reasons such as presentation, map data, resource layout, or fog
  revision.
- `entityViews.*` counts shared frame-view hits and intentional uncached fallback call sites.
- `hud.dirty.*` and `observer.dirty.*` count dirty-guard hits and misses for DOM panels that should
  not rebuild every RAF.

Recent slow frames also carry a bounded context block naming the slowest top-level `match.*` phase,
the slowest nested `renderer.*` phase, the slowest nested `minimap.*` phase, and the largest
diagnostic counters seen in that frame. Use timings to decide where milliseconds went, and use
counters to explain whether that cost came from object churn, redraw frequency, cache invalidation,
or a missing measurement category.

Optional server-side performance tracing is controlled by environment variables. It is off by
default and emits structured `tracing` logs under the `server::perf` target when enabled.

Every `ClientNetReport` upload includes a bounded report-window summary from the same profiler:
`frameWorkMaxMs`, `frameWorkP95Ms`, `slowFrameCount`, `worstFramePhase`, `worstFramePhaseMs`,
`rendererMaxMs`, `rendererP95Ms`, `entityCount`, `selectedCount`, `visibleTileCount`,
`viewportWidth`, `viewportHeight`, and `devicePixelRatioX100`. Report-window profiler counters reset
after each upload; the `window.__rtsPerf` debug summary remains cumulative until `reset()` is called.

The same upload also includes bounded snapshot diagnostics:

- Payload size: `snapshotBytesTotal`, `snapshotBytesMax`, `snapshotBytesAvg`,
  `snapshotMessageCount`, `snapshotBytesP95`, `snapshotSegmentBudgetBytes`,
  `snapshotOverSegmentBudgetCount`, `snapshotOverSegmentBudgetPctX100`, `snapshotByteSource`,
  `snapshotCodec`, `snapshotCodecVersion`, and `snapshotFrameKind`.
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

The canonical single-segment payload budget is 1280 bytes. Client measurements count only snapshot
WebSocket application payload bytes, currently `messagepack-application-payload` from binary
`messagepack-compact` frames, so they exclude WebSocket framing plus TLS, TCP, and IP overhead; a
1460-byte application payload is not a safe single-segment target. Raw snapshot payloads, raw
timestamp arrays, raw phase arrays, recent frame records, stack traces, entity ids, command
payloads, command targets, and replay data are intentionally not uploaded.

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
exercises simulation, per-player snapshot fanout, snapshot compaction, and the active
`messagepack-compact` snapshot serialization path without requiring browser clients. Override the
usual `RTS_PERF*` and `RUST_LOG` environment variables when you need a different trace shape, or pass
`--perf full` for every tick.

Browser client performance harness:

```bash
node scripts/client-perf-harness.mjs --list
node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10
node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 10
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10
node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 10
node scripts/client-perf-harness.mjs --workload fog-combat-replay-stress --seconds 10
```

The browser harness starts a local server on an isolated port unless `RTS_URL` or `--base-url`
points at an already-healthy server. It drives headless Chrome with the existing
`tests/package.json` `puppeteer-core` dependency path, copies the preserved Matt/Alex replay into
`server/target/selfplay-artifacts/client_perf_matt_alex_match_54/replay.json` at runtime, and writes
one `summary.json` per workload under `target/client-perf/<workload>/<timestamp>/`. The
`--render-lag-suite` path runs the Matt/Alex replay, the vehicle-wall stress scenario, a
selected-unit HUD stress scenario, and a fog/combat-heavy Matt/Alex replay seek, then writes a rollup at
`target/client-perf/render-lag-comparison/<timestamp>/summary.json`. Each workload summary includes
`renderBudget` advisory output for 60, 120, 240, and 480 FPS frame-work budgets, including
per-budget margins and the next missed p95 budget. It also includes a local-only
`renderDiagnostics` block with the counter groups above, recent long-frame context, and the largest
nonzero counters for the sample. It also includes a `snapshotPacketBudget` block
with payload p95 bytes, the selected packet budget, over-budget count, and over-budget percentage
when the generated `ClientNetReport` includes them. Pass `--trace` to also write a Chrome
`trace.json`; traces are opt-in because they are larger and machine-local.

Render stress matrix:

```bash
node scripts/client-perf-harness.mjs --stress-matrix --render-lag-suite --seconds 4 --matrix-cpu 1,2 --matrix-viewport default --matrix-dpr 1 --matrix-repeat 1
node scripts/client-perf-harness.mjs --stress-matrix --render-lag-suite --seconds 10 --matrix-cpu 1,2,4 --matrix-viewport small,default,large --matrix-dpr 1,2 --matrix-repeat 3
```

The short command is the local low-end substitute smoke run. The longer command is the serious
before/after comparison: it runs repeated samples across workloads, Chrome CPU throttle factors,
small/default/large viewports, and explicit device scale factors. Matrix artifacts are written under
`target/client-perf/render-stress-matrix/<timestamp>/summary.json` and `summary.md`; each workload
sample still writes its own `target/client-perf/<workload>/<timestamp>/summary.json`. The matrix
rollup keeps Chrome traces opt-in with `--trace`, includes CPU throttle, viewport, DPR, repeat, and
artifact paths for every cell, and ranks advisory budget failures by first missed frame-work budget
and top measured phase.

Interpret CPU/DPR stress as pressure testing, not hardware emulation. Chrome CPU throttling changes
main-thread scheduling on the local machine, DPR changes canvas backing resolution, and viewport
changes visible/rendered area. A failing `cpu4-vplarge-dpr2` cell points to the subsystem to inspect
next on this machine; it does not claim to reproduce Matt's laptop exactly.

The default harness result fails for runtime errors, page/console/request errors, and missing
`window.__rtsPerf` or generated `ClientNetReport` summaries. It deliberately does not fail on an
absolute FPS, frame time, or trace-timing budget. Treat the numbers as local evidence for comparing
optimization branches on the same machine, not as a portable guarantee for other laptops.

For render-lag comparisons, read `renderBudget.frameWork` first: `frame.work` is total browser work
inside the RAF and should be compared to the 16.67 ms, 8.33 ms, 4.17 ms, and 2.08 ms frame-work
budgets for 60, 120, 240, and 480 FPS. A positive margin means the measured frame-work metric was
under that budget; a negative margin shows how far it missed. The 120 FPS result remains useful,
but a workload with p95 near 8 ms is only barely clearing 120 locally and should still be treated as
risky for weaker hardware if it misses the 240 FPS headroom target.

Use `frame.work` average, p95, and max instead of literal local `requestAnimationFrame` FPS for
branch comparisons. Local RAF FPS is constrained by display refresh rate, browser scheduling,
visibility, throttling, and frame gaps outside measured JS work; `frame.work` isolates the measured
client work inside frames. Average shows steady cost, p95 shows recurring expensive frames, and max
shows the worst single frame in that sample. Frame-gap and FPS fields still help diagnose pacing,
but they are not portable headroom guarantees.

Estimate player impact from ratios, not flat FPS deltas. For example, reducing local `frame.work`
p95 from 12 ms to 6 ms roughly doubles measured p95 work headroom on that machine; do not describe
it as adding a fixed number of FPS for players. Relative improvement on the same machine and
workload is more useful evidence than projecting the same millisecond or FPS delta onto weaker
laptops.

Recurring top-level `match.*` phases above 1-2 ms p95 are advisory follow-up candidates;
`match.renderer` and `match.minimap` are top-level phases, while `renderer.*` rows are nested
renderer subphases and must not be added back into `frame.work`. If local-only minimap probes such
as `minimap.*` rows are enabled during an investigation, treat them as nested minimap detail under
`match.minimap`, not as a separate top-level cost. Always inspect p95 bucket, max, worst-phase
count, and shape context (`entityCount`, `selectedCount`, visible tiles, viewport, and device pixel
ratio) together.

Keep evidence streams separate. Matt and Alex beta FPS/network reports are per-player browser
observations from deployed matches; `matt-alex-replay` is a local replay of preserved match 54 data;
`vehicle-wall-stress` and `selected-unit-hud-stress` are local no-fog dev scenarios. Do not average
or merge those rows when deciding whether a branch improved render cost.

Snapshot codec bake-off:

```bash
node scripts/snapshot-codec-bakeoff.mjs --fixture
node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6 --snapshot-codec-bakeoff
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6 --snapshot-codec-bakeoff
```

The standalone bake-off reads local compact snapshot frames from JSON/JSONL or deterministic
fixtures and compares compact JSON, offline deflate, a protobuf-style schema TLV, MessagePack, CBOR,
and a custom positional binary. It reports p50/p95/p99/max encoded bytes, over-budget rate, and
local encode/decode timings. `--snapshot-codec-bakeoff` makes the browser harness capture bounded
snapshot frames in memory, normalize MessagePack frames back to compact snapshot JSONL, write
`snapshot-frames.jsonl`, and attach `snapshot-codec-bakeoff.*` artifacts beside the normal summary.

Codec bake-off artifacts are local developer evidence only. The live protocol now defaults to
`messagepack-compact` binary snapshot frames; compact JSON remains a historical baseline in the
bake-off report, and deflate numbers are compressed payload bytes from Node zlib rather than
verified WebSocket extension wire bytes.

Snapshot transport diagnostics:

```bash
node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6 --snapshot-codec-bakeoff
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6 --snapshot-codec-bakeoff
scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots
scripts/fly-logs.sh beta recent | rg 'client_net_report|websocket_compression|snapshot_byte_source|snapshot_codec|writer_send'
```

Use these commands to compare current MessagePack payloads with local compact JSON baselines and to
confirm the browser/server transport labels. The browser harness writes a top-level `websocket`
block and the generated `ClientNetReport` includes `websocketExtensions`, `websocketCompression`,
`snapshotByteSource`, `snapshotCodec`, `snapshotCodecVersion`, and `snapshotFrameKind`.
`scripts/parse-net-report-logs.mjs` surfaces the same fields under Transport diagnostics for local
or Fly logs.

Interpretation:

- `websocketCompression=permessage-deflate` means Chrome reports the WebSocket extension as
  negotiated. `snapshotBytes*` still measure browser-delivered application payload bytes, not
  compressed wire bytes.
- `websocketCompression=none` means the WebSocket completed without a negotiated compression
  extension. That is the expected result for the current Axum 0.8 / Tungstenite 0.29 server stack;
  Tungstenite 0.29 does not implement `permessage-deflate`.
- Compression is no longer the next packet-plan step. Treat WebSocket compression fields as
  diagnostics only unless the user explicitly reopens that route; compare `snapshotBytes*` against
  MessagePack full-snapshot baselines and later fog-safe delta measurements, not compressed-wire
  claims, unless the measurement source is explicitly labeled.

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

## Network incident parser

Use `scripts/parse-net-report-logs.mjs` when a player-reported lag incident has preserved Fly JSONL
logs. It reads Fly JSONL from `scripts/fly-logs.sh search` or raw tracing text, strips ANSI tracing
decoration, extracts `client_net_report`, `match_started`, `match_ended`, `performance tick summary`,
`performance snapshot timing`, and `performance writer timing` rows, then emits a compact markdown
summary plus JSON/TSV machine-readable output.

Example:

```bash
scripts/fly-logs.sh beta search \
  --from 2026-06-19T00:50:00Z \
  --to 2026-06-19T01:15:00Z \
  --filter 'client network report|match started|match ended|performance tick summary|performance snapshot timing|performance writer timing' \
  > /tmp/rts-lag-window.jsonl

node scripts/parse-net-report-logs.mjs --out-dir /tmp/rts-lag-summary /tmp/rts-lag-window.jsonl
open /tmp/rts-lag-summary/incident-summary.md
```

For a quick terminal view:

```bash
node scripts/parse-net-report-logs.mjs /tmp/rts-lag-window.jsonl
node scripts/parse-net-report-logs.mjs --format tsv /tmp/rts-lag-window.jsonl
node scripts/parse-net-report-logs.mjs --format json /tmp/rts-lag-window.jsonl
```

The markdown table is the operator-facing summary. The JSON output preserves per-match and per-player
metrics, row counts, classifications, and missing-data warnings for follow-up scripts. The TSV output
is intentionally flat so it can be pasted into a spreadsheet or attached to a bug report.

Classification is evidence-bounded:

- Server tick/scheduler pressure requires server tick, scheduler lag, slow-tick, or performance tick
  rows. Clean `server_tick_ms`, `server_lag_ms`, and `slow_tick_count` values are evidence against
  server-lag blame, not proof that every host resource was perfect.
- Server snapshot projection/compact/serialization pressure requires `performance tick summary` or
  `performance snapshot timing` rows. Older incidents without those rows are reported as unavailable.
- WebSocket writer/send pressure requires writer timing, high buffered bytes, or head-of-line/backlog
  evidence.
- Client network/snapshot delivery pressure uses RTT, bad RTT samples, snapshot jitter, snapshot gaps,
  stale/duplicate/skipped snapshot counters, and burst counters.
- Browser processing pressure uses payload size, packet-budget p95/rate, frame parse, compact decode,
  snapshot apply, prediction apply, frame work, renderer timing, frame gaps, and FPS estimates.
- Command path pressure uses legacy acknowledged-command latency when that is all an old log has, and
  uses the newer upload/server-receipt/sim-ack/downstream-apply milestones when present.

The parser always prints that packet loss, retransmit behavior, and per-packet browser transport data
are unavailable. Packet-budget p95 and over-budget-rate fields are payload-byte aggregates only, not
proof of packet fragmentation on the user's path. Treat WebSocket/TCP head-of-line or WebTransport
theories as unsupported unless the available rows show concrete writer backlog, snapshot
burst/coalescing, or downstream delivery gaps.

The preserved Matt/Alex incident is the canonical fixture:

```bash
node scripts/parse-net-report-logs.mjs \
  docs/network-incident-examples/2026-06-19-beta-matt-alex/fly-match-54-all.jsonl \
  docs/network-incident-examples/2026-06-19-beta-matt-alex/fly-match-55-all.jsonl
```

The expected shape is: server tick/scheduler pressure not indicated; Matt's player rows show high
RTT/snapshot timing and low frame pacing; older payload, packet-budget, parse/decode/apply, command
milestone, and server snapshot timing fields are reported as unavailable instead of zero.

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
- `packet_budget_pressure`: at least 120 snapshot-frame samples, `snapshotBytesP95` above the
  reported `snapshotSegmentBudgetBytes`, and `snapshotOverSegmentBudgetPctX100 >= 5000`. This is
  separate from `payload_pressure`; it is meant to highlight persistent single-segment budget
  pressure without replacing the older pathological-frame thresholds.
- `client_snapshot_parse`: `snapshotParseMaxMs >= 16` or `snapshotParseP95Ms >= 8`; points at
  browser frame parsing cost for received snapshot frames.
- `client_snapshot_decode`: `snapshotDecodeMaxMs >= 16` or `snapshotDecodeP95Ms >= 8`; points at
  compact-protocol expansion cost after frame parse.
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
