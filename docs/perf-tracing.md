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
selection/HP, shot reveals, sweeps, fog draw, feedback/effects overlays, and placement. The profiler
also records `renderer.update` for backend scene translation and `renderer.present` for the one
synchronous Pixi/Babylon presentation. Both are nested inside `match.renderer` and must not be
added back into `frame.work`. The renderer frame counter advances only after a successful present.
The profiler counts complete frame work and actual present work strictly above `1000 / 60` ms;
these 60 FPS counters do not change the existing 33 ms slow-frame definition. Its histogram has a
17 ms boundary so near-budget p95 values remain visible. The profiler also records
`frame.rafDispatch`, the browser callback dispatch delay between the RAF timestamp and
actual JavaScript entry, and `frame.unattributed`, which is `frame.work` minus top-level `match.*`
phase time for the same frame. High `frame.rafDispatch` p95 points at scheduling pressure before the
frame callback starts. High `frame.unattributed` p95 means the benchmark has found frame work that
the current named phases do not explain yet; use recent long-frame context and, when needed,
`--trace` to decide where to add finer timing.

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
diagnostic counters seen in that frame. They also include RAF dispatch delay, the top-level named
phase total, and the unattributed frame work for that frame. Use timings to decide where milliseconds went, and use
counters to explain whether that cost came from object churn, redraw frequency, cache invalidation,
or a missing measurement category.

Optional server-side performance tracing is controlled by environment variables. It is off by
default and emits structured `tracing` logs under the `server::perf` target when enabled.

Every `ClientNetReport` upload includes a bounded report-window summary from the same profiler:
`frameWorkMaxMs`, `frameWorkP95Ms`, `slowFrameCount`, `worstFramePhase`, `worstFramePhaseMs`,
`frameRafDispatchMaxMs`, `frameRafDispatchP95Ms`, `frameUnattributedMaxMs`,
`frameUnattributedP95Ms`, `frameWorkBudgetMissCount`, `presentBudgetMissCount`, `rendererMaxMs`,
`rendererP95Ms`, `rendererUpdateMaxMs`, `rendererUpdateP95Ms`, `rendererPresentMaxMs`,
`rendererPresentP95Ms`, `topRendererPhase`,
`topRendererPhaseMs`, `topRenderDiagnosticGroup`, `topRenderDiagnosticGroupCount`,
`clientFramePhases`, `rendererFramePhases`, `renderDiagnosticCounters`, `entityCount`,
`selectedCount`, `visibleTileCount`, `viewportWidth`, `viewportHeight`, and
`devicePixelRatioX100`. The uploaded phase arrays are capped at five allowlisted labels and include
only count, max, and p95 buckets. The uploaded diagnostic counters are capped at five stable groups
such as `renderer.pixi.displayObject`, `renderer.rig.redraw`, `renderer.graphics.clear`,
`renderer.redraw`, `renderer.groundDecals`, `minimap.cache`, `minimap.invalidate`, `hud.dirty`,
`observer.dirty`, and `entityViews.*`; raw per-frame rows and unrecognized local labels stay
local-only. Report-window profiler counters reset after each upload; the `window.__rtsPerf` debug
summary remains cumulative until `reset()` is called. Uploaded durations are rounded and clamped to
integer milliseconds; update/present max and p95 remain stable scalars even when their phase rows
are displaced from the capped top-five arrays.
HUD `jit` in the live health readout is snapshot arrival jitter; it is not JavaScript compiler/JIT
time.

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
- Late-frame prediction coverage: `snapshotLateFrameCount` and
  `predictedSnapshotLateFrameCount`, which count frames where the last snapshot was late and whether
  an owned-unit predicted snapshot overlay was present on those frames. The derived
  `predictedSnapshotLateFramePctX100` reports that coverage as a percentage multiplied by 100, and
  `predictionActiveLateFrameCount` counts late-snapshot frames where the prediction controller was
  locally predicting or resyncing. Interpret these with `predictionReplayMaxMs`,
  `predictionReplayMaxTicks`, `predictionReplayBudgetExceededCount`, and correction fields: late
  frames with no predicted overlay mean prediction was absent for local owned world coverage, while
  high replay or correction fields in the same window point at prediction work/correction pressure.

Command-response diagnostics are also reported as bounded window aggregates keyed by the live
`matchRunId`: issued/send-accepted/server-received/sim-acknowledged/rejected counts,
issue-to-WebSocket-send-accepted latest/max/p95, issue-to-server-receipt latest/max/p95,
server-receipt-to-sim-ack latest/max/p95, issue-to-sim-ack latest/max/p95,
ack-snapshot-received-to-applied latest/max/p95, oldest pending command age, max pending command
count, stable family counts for `move`, `attackMove`, `build`, `train`, and `other`, up to five
bounded lifecycle exemplars, `commandBurstBucketMs`, `commandBurstMax`,
`commandBurstFrameGapMaxMs`, `commandBurstWorstFramePhase`, and `commandBurstWorstFramePhaseMs`.
The burst bucket is a fixed 250 ms sliding client window and only counts commands that passed local
command-budget checks and reached the browser WebSocket send path. `commandsIssued` is the
report-window total and catches sustained rapid input that may never reach the short-bucket burst
threshold.
The server receipt comes from a tiny reliable
`commandReceipt` message keyed only by `clientSeq`; it carries no command payload, unit ids, target
ids, positions, or player-entered text and does not reconcile prediction.

Prediction health fields include stable disable-reason buckets
`predictionDisableUserCount`, `predictionDisableReplayCount`, `predictionDisableSpectatorCount`,
`predictionDisableCompatibilityCount`, `predictionDisableWasmCount`, and
`predictionDisableOtherCount`, plus `predictionReplayMaxMs`, `predictionReplayMaxTicks`, and
`predictionReplayBudgetExceededCount` for report-window WASM pending-command replay work. Detailed
WASM loader errors stay in local debug output and are not uploaded as labels.

The server augments the `client_net_report` log row with per-connection outbound counters consumed
on the same report cadence: `server_command_receipts_accepted`, `server_command_receipts_rejected`,
server command lifecycle counts/timings for frame deserialize, deserialize-to-room enqueue, room
queue, room handling, receipt send age, and accepted-to-sim-ack, bounded server command lifecycle
exemplars, `server_reliable_drained_before_snapshot`, `server_reliable_drained_before_snapshot_max`,
`server_snapshot_waited_behind_reliable`, `server_snapshot_sent`,
`server_snapshot_send_age_latest_ms`, `server_snapshot_send_age_max_ms`,
`server_snapshot_send_age_avg_ms`, `server_snapshot_slot_stored`,
`server_snapshot_slot_replaced`, and `server_snapshot_slot_closed`. It also emits bounded snapshot
lifecycle windows: `server_snapshot_project_*_ms`, `server_snapshot_compact_*_ms`,
`server_snapshot_queue_age_*_ms`, `server_snapshot_serialize_*_ms`,
`server_snapshot_writer_send_*_ms`, `server_snapshot_writer_taken`, and
`server_snapshot_payload_bytes_*` for latest/max/p95/avg/count/total application payload bytes.
Payload composition is summarized in JSON string fields `server_snapshot_payload_sections` and
`server_snapshot_entity_kinds`; the stable section labels are `entities`, `visibility`,
`resourceDeltas`, `events`, `smokes`, `abilityObjects`, `trenches`, `playerStatus`, `netStatus`,
and `other`. Entity-kind bytes are proportional approximations from the entity section rather than
per-entity serialization traces. These are server-only structured-log fields, not client protocol
fields. One reliable message before a snapshot with no send age or slot replacement is normal
ordering, not outbound pressure.

The canonical single-segment payload budget is 1280 bytes. Client measurements count only snapshot
WebSocket application payload bytes, currently `messagepack-application-payload` from binary
`messagepack-compact` frames, so they exclude WebSocket framing plus TLS, TCP, and IP overhead; a
1460-byte application payload is not a safe single-segment target. Raw snapshot payloads, raw
timestamp arrays, raw phase arrays, recent frame records, stack traces, entity ids, command
payloads, command targets, and replay data are intentionally not uploaded.

Logged slow or sampled server ticks also include bounded pathing diagnostics for the movement
coordinator passes `awaiting_paths`, `promote_queued_orders`, and `promoted_awaiting_paths`.
The tick summary carries aggregate fields:

- `pathing_awaiting_start`, `pathing_promoted_awaiting_start`, and
  `pathing_promote_queued_for_path`: unit counts at the start of the two awaiting-path passes and
  units staged by queued-order promotion.
- `pathing_requests`, `pathing_processed`, `pathing_deferred`, `pathing_still_awaiting`,
  `pathing_success`, and `pathing_failed`: per-logged-tick request volume and outcome counts.
- `pathing_cache_hits` and `pathing_cache_misses`: LRU path cache reuse at the pathing-service
  boundary.
- `pathing_budget_exhausted`: count of exhausted pathfinding budgets plus exhausted coordinator
  pass budgets on that logged tick.
- `pathing_worst_request_ms`, `pathing_explored_nodes_max`, and `pathing_path_len_max`: worst
  bounded request timing, A* expanded-node count, and tile-path length observed on the logged tick.
- `pathing_top_source` and `pathing_top_source_count`: the largest stable source family among
  processed path requests, falling back to queued path sources when no request was processed. Stable
  families are `move`, `attackMove`, `attack`, `gather`, `build`, `deconstruct`, `ability`, and
  `other`.

Each logged tick also emits one `event="pathing"` row per instrumented pass. Those rows include the
same counts at pass granularity plus `source_counts`, `queued_source_counts`,
`group_size_buckets`, `path_len_buckets`, `explored_node_buckets`, `worst_request_bucket`,
`cache_available`, `complexity_available`, and `fuse_triggered`. `source_counts` describes
processed path requests; `queued_source_counts` describes grouped orders staged for later path
requests. Bucket values are aggregate labels only; the logs do not include raw paths, raw
positions, full unit id lists, entity ids, targets, command payloads, or player-entered text. The
reset window is one coordinator pass inside one logged tick, so missing pathing rows mean perf
tracing did not log that tick, not that pathing was free. `fuse_triggered` is currently `false`
because the coordinator exposes budget exhaustion but no separate pathing fuse.

Use these fields with the normal phase timings. `slowest_phase=awaiting_paths` plus high
`pathing_requests`, deferred counts, or coordinator budget exhaustion points at request volume.
High `pathing_worst_request_ms` or `pathing_explored_nodes_max` with modest request counts points at
path complexity. A slow `promote_queued_orders` phase with high `queued_for_path` but few processed
requests points at queued-order promotion staging work. Cache and complexity are directly measured
when the row says they are available; do not infer missing cache or complexity fields from duration
alone.

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

Hellhole isolation pair:

```bash
# Server only: Game API in/out, no listener, socket, browser, or real-time pacing.
scripts/hellhole-perf-harness.sh --ticks 900

# Client only: checked-in snapshots, no WebSocket or live simulation.
node scripts/client-perf-harness.mjs --workload supply-300-hellhole-stream --seconds 30
```

The server command restores the canonical four-player scenario and measures each direct API round
trip as `Game::tick()` plus one full-world snapshot, production compaction, and MessagePack
encoding. It runs as fast as the server can complete work, reports aggregate average/p95/p99/max
timings and payload size, and accepts `--json` for machine-readable output. It deliberately omits
the room scheduler, WebSocket send, and browser so client speed cannot throttle it.

For an explicitly combined visual check, run:

```bash
scripts/hellhole-perf-harness.sh --integrated --seconds 60
```

Integrated mode builds a release server, runs the canonical live Lab workload at the ordinary 30
Hz cadence, and opens the controlled Chrome window visibly. It is useful for seeing both halves in
tandem, but its result is end-to-end evidence rather than an isolated server or client measurement.
The integrated workload is opt-in and is excluded from the default workload set and
`--render-lag-suite`.

Browser client performance harness:

```bash
node scripts/client-perf-harness.mjs --list
node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10
node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 10
node scripts/client-perf-harness.mjs --active-supply-pair --seconds 10
node scripts/client-perf-harness.mjs --workload supply-300-hellhole-stream --seconds 10
```

Canonical client CPU flame graph:

```bash
git fetch origin main
node scripts/client-flamegraph.mjs --preview
```

The flame-graph command runs the deterministic `supply-300-hellhole-stream` workload for 15 seconds
at the default viewport, DPR 1, CPU throttle 1, and a 500 microsecond V8 sampling interval. The
harness completes workload assertions, resets its local performance window, and observes at least
30 rendered frames before CPU sampling begins, so module loading and setup do not dominate the
profile. It writes the raw `.cpuprofile`, the ordinary harness `summary.json`, a ranked function
summary, and SVG/PNG flame graphs under ignored `target/client-perf/flamegraphs/`; `--preview`
publishes the PNG through the normal 24-hour Tailnet Preview service.

Flame width is inclusive sampled CPU time. The heading and ranked JSON use self time aggregated by
function, source URL, and line so the same function reached through multiple call stacks is not
understated. Read both views: a wide parent identifies an expensive subsystem, while high self time
identifies the function doing the work. The colors distinguish game-client JavaScript, Pixi,
browser/native work, and idle/garbage collection; they are navigation aids rather than performance
budgets.

Useful variants retain the same one-command workflow:

```bash
node scripts/client-flamegraph.mjs --workload supply-300-active --seconds 20 --preview
node scripts/client-flamegraph.mjs --cpu-throttle 4 --viewport 1440x900 --dpr 1 --preview
```

Before writing client optimization phases, capture from a clean worktree on current `origin/main`,
inspect the ranked self/inclusive functions and their source, and pair the result with
`frame.work`/renderer/fog phase evidence from the same harness summary. Use the snapshot stream as
the repeatable renderer-isolation lane; use `supply-300-active` when the conclusion depends on
prediction or production-shaped active-player behavior. A page cannot grant itself V8 Profiler
access, so remote playtester function profiles require a later DevTools, extension, or launcher
workflow rather than a silent in-page upload.

The browser harness starts a local server on an isolated port unless `RTS_URL` or `--base-url`
points at an already-healthy server. It drives headless Chrome with the repository-root
`package.json` `puppeteer-core` dependency and writes one `summary.json` per workload
under `target/client-perf/<workload>/<timestamp>/`. The checked-in workload set includes the
`vehicle-wall-stress` and `selected-unit-hud-stress` live dev scenarios, the active-player
`supply-200-active`/`supply-300-active` pair, and the client-only
`supply-300-hellhole-stream`. The opt-in `supply-300-hellhole-integrated` workload is not included
in default or render-lag-suite runs. The active pair uses the same fixed local seed (`0x5a000300`),
viewport, DPR, CPU throttle, duration, and repeat settings. Both measured browsers join as player 1
with compatible WASM prediction enabled; sampling fails for spectator/disabled prediction,
client-mutated setup, wrong supply/cap/composition, or wrong projected regular-entity count. The
authoritative per-player 200-supply mix is worker/rifleman/machine gunner/panzerfaust/anti-tank
gun/mortar/artillery/scout car/tank/command car = `7/7/7/7/7/7/6/7/6/6`, with 135 regular entities
in player 1's projection. The 300-supply mix is `12/10/10/10/10/10/10/10/9/9`, with 201 projected
regular entities. Both preserve the normal production supply cap of 50; the fixture bypasses no
production rule because simulation setup creates the units directly. Assertions complete before
the profiler/report windows reset, then two successful explicit presents must occur before sampling.

The snapshot-stream workload is the client-only isolation lane. It fetches the generated
`client/assets/snapshot-streams/supply-300-hellhole.rtsstream` artifact and feeds its exact compact
MessagePack snapshots into the normal decoder and renderer at 30 Hz. Its setup assertion fails unless
the page reports no WebSocket and no live simulation. Regenerate the thirty-second, 900-frame artifact
with `cargo run --release --manifest-path server/Cargo.toml --bin generate_hellhole_snapshot_stream`.
Preserved schema 2
incident replays are analysis evidence only and are not replay-harness workloads. The
`--render-lag-suite` path runs the current workload set, then writes a rollup at
`target/client-perf/render-lag-comparison/<timestamp>/summary.json`. Each workload summary includes
`renderBudget` advisory output for 60, 120, 240, and 480 FPS frame-work budgets, including
per-budget margins and the next missed p95 budget. The same block includes `frameAttribution`,
which reports top-level named work, `frame.unattributed` average/p95/max, `frame.rafDispatch`, and
the average percentage of `frame.work` covered by named top-level phases. It also includes a local-only
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

CPU-throttle results are synthetic scheduling-pressure evidence on the machine running Chrome, not
real-device certification. In particular, neither active 300 supply nor the Lab full-world baseline
claims that 300 supply is safe on player hardware.

Recurring top-level `match.*` phases above 1-2 ms p95 are advisory follow-up candidates;
`match.renderer` and `match.minimap` are top-level phases, while `renderer.*` rows are nested
renderer subphases and must not be added back into `frame.work`. If local-only minimap probes such
as `minimap.*` rows are enabled during an investigation, treat them as nested minimap detail under
`match.minimap`, not as a separate top-level cost. Always inspect p95 bucket, max, worst-phase
count, and shape context (`entityCount`, `selectedCount`, visible tiles, viewport, and device pixel
ratio) together. If `frame.unattributed` or `frame.rafDispatch` outranks the named `match.*` rows,
do not start by optimizing the largest named subsystem alone; first inspect recent long frames or a
trace to explain the missing time, then add a narrower phase label or fix the scheduling,
off-RAF, or uncaptured work it reveals.

Keep evidence streams separate. Matt and Alex beta FPS/network reports are per-player browser
observations from deployed matches, and their preserved replay JSON files are historical schema 2
artifacts rejected by the current replay loader. `vehicle-wall-stress` and
`selected-unit-hud-stress` are local no-fog dev scenarios. Capture a fresh schema 3 replay before
using replay playback as a browser harness workload again; do not average or merge these evidence
streams when deciding whether a branch improved render cost.

Snapshot codec bake-off:

```bash
node scripts/snapshot-codec-bakeoff.mjs --fixture
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6 --snapshot-codec-bakeoff
node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 6 --snapshot-codec-bakeoff
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
node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6 --snapshot-codec-bakeoff
node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 6 --snapshot-codec-bakeoff
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

The Fly game-server deploys enable the low-noise spike mode in `fly.mainline.toml` and
`fly.beta.toml`:

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

Use `scripts/capture-net-incident.mjs` when a report needs a complete incident directory rather
than a one-off parser summary. It can package preserved local logs or run a bounded beta Fly query,
then writes raw logs, parser outputs, standalone agent digest files, key metrics, replay/DB
availability, player-report notes, a neutral `analysis.md`, and a beta evidence checklist:

```bash
node scripts/capture-net-incident.mjs \
  --fixture soupman-alex \
  --out-dir /tmp/rts-soupman-alex-package \
  --force \
  --require-coverage command,snapshot,pathing,client-context
```

For a fresh beta incident, provide the exact UTC window and run id:

```bash
node scripts/capture-net-incident.mjs \
  --beta \
  --from 2026-06-30T00:16:00Z \
  --to 2026-06-30T00:42:00Z \
  --run-id alex-s-lobby-1782778605186-000004 \
  --match-id 103 \
  --out-dir /tmp/rts-beta-incident \
  --require-coverage command,snapshot,pathing,client-context
```

The beta path shells through `scripts/fly-logs.sh` with a diagnostic-only filter and bounded page
count. The generated `analysis.md` is intentionally neutral: supported, contradicted, unknown, and
next diagnostic gap sections only. Do not use it to prescribe pathing, transport, render, snapshot,
or command fixes.

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
open /tmp/rts-lag-summary/README.md
```

For a quick terminal view:

```bash
node scripts/parse-net-report-logs.mjs /tmp/rts-lag-window.jsonl
node scripts/parse-net-report-logs.mjs --format tsv /tmp/rts-lag-window.jsonl
node scripts/parse-net-report-logs.mjs --format json /tmp/rts-lag-window.jsonl
```

The markdown table is the operator-facing summary. The first screen now includes an agent digest
that names the supported diagnosis and biggest unknowns before the detailed tables. The JSON output
preserves per-match and per-player metrics, row counts, classifications, missing-data warnings, and
the `agentDigest` block for follow-up scripts. The TSV output is intentionally flat so it can be
pasted into a spreadsheet or attached to a bug report.

With `--out-dir`, the parser writes an incident package:

- `README.md`: agent-first digest, package inventory, coverage matrix, top bad windows, and timeline.
- `evidence-index.json`: source manifest, evidence classes, coverage matrix, field catalog, and
  provenance/privacy notes.
- `key-metrics.json`: stable digest JSON with classifications, one-minute timeline bands, top
  issue windows, and explicit unknowns.
- `incident-summary.md`, `incident-summary.json`, and `incident-rows.tsv`: backwards-compatible
  parser outputs.
- `client-net-rows.tsv` and `server-tick-rows.tsv`: filtered rows for spot-checking the windows
  named in the digest.

Timeline bands default to one minute and can be changed with `--timeline-band-ms`. Treat missing
snapshot projection, writer, DB summary, replay, or transport rows as "not logged or unavailable";
they are not zero-cost evidence. Keep beta incident evidence, local replay/perf harness evidence,
and synthetic stress evidence separated by the `evidenceKind` and source manifest labels rather than
averaging them together.

Classification is evidence-bounded:

`classifications[].result` is kept for older scripts, while `classifications[].status` carries the
agent-facing state: `indicated`, `contradicted`, `weak`, `unavailable`, or `unknown`. Each
classification includes `evidenceFor`, `evidenceAgainst`, and `unavailable` field notes so agents
can see which fields triggered or argued against a diagnosis.

- Server tick/scheduler pressure requires server tick, scheduler lag, slow-tick, or performance tick
  rows. Clean `server_tick_ms`, `server_lag_ms`, and `slow_tick_count` values are evidence against
  server-lag blame, not proof that every host resource was perfect.
- Server snapshot projection/compact/serialization pressure requires `performance tick summary`,
  `performance snapshot timing`, or newer `client_net_report` server lifecycle fields. Older
  incidents without those rows are reported as unavailable.
- Snapshot payload composition pressure uses client payload byte/packet-budget fields plus newer
  server payload byte totals and top section/entity-kind summaries when present. Section bytes are
  compact-frame attribution estimates, not raw snapshot bodies.
- WebSocket writer/send pressure requires writer timing, high buffered bytes, or head-of-line/backlog
  evidence. Newer `client_net_report` rows can also show server outbound pressure from multiple
  reliable messages drained before one snapshot, snapshot queue age, writer-send timing, snapshot
  send age, or latest-only snapshot slot replacement.
- Client network/snapshot delivery pressure uses RTT, bad RTT samples, snapshot jitter, snapshot gaps,
  stale/duplicate/skipped snapshot counters, and burst counters.
- Browser processing pressure uses payload size, packet-budget p95/rate, frame parse, compact decode,
  snapshot apply, prediction apply, frame work, 60 FPS budget misses, distinct renderer
  update/present timing, RAF dispatch, unattributed work, frame gaps, and FPS estimates.
- Command path pressure uses legacy acknowledged-command latency when that is all an old log has, and
  uses the newer client-send, upload/server-receipt, server room queue/handling, receipt-send,
  sim-ack, and downstream-apply milestones when present.
- Command density pressure uses `commandsIssued`, `commandBurstMax`, and server command-receipt
  counts. It is a correlation signal, not proof that commands caused later jitter.
- Prediction health uses stable disable-reason buckets, WASM replay max ms/ticks, replay-budget
  exceed counts, and predicted-snapshot coverage during late snapshot frames.

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

- `client_renderer_present`: any present-budget miss, `rendererPresentMaxMs >= 33`, or
  `rendererPresentP95Ms >= 16`; distinguishes actual Pixi/Babylon presentation from scene update.
- `client_renderer_update`: `rendererUpdateMaxMs >= 33` or `rendererUpdateP95Ms >= 16`; points at
  backend scene translation/drawing preparation before the actual present.
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
- `client_frame_work_budget`: at least one `frameWorkBudgetMissCount`; exposes misses above the
  16.67 ms 60 FPS work budget without redefining the legacy 33 ms slow-frame threshold.
- `client_raf_dispatch` and `client_frame_unattributed` distinguish pre-callback scheduling pressure
  from work inside the RAF that is not yet covered by top-level `match.*` phases.
- `client_frame_stall`: `frameGapMaxMs >= 100`, or `slowFrameCount > 0` when frame-work thresholds
  were not crossed; points at requestAnimationFrame gaps even when measured frame work was not the
  dominant issue.
- `command_density` classifies sustained report-window command totals, high short-bucket command
  density, or high server command-receipt volume before it falls through to prediction, outbound
  writer, or generic network buckets.
- `server_snapshot_lifecycle` classifies high server projection, compaction, serialization, queue
  age, or writer-send timing from the per-connection snapshot lifecycle window.
- `server_snapshot_outbound` classifies multi-reliable-before-snapshot backlog, snapshot-send-age,
  or latest-slot-replacement pressure observed by the server connection writer. One reliable message
  before a snapshot with zero send age and no slot replacement is normal ordering, not pressure.
- Existing buckets continue to separate `network_rtt`, `snapshot_gap`, `snapshot_jitter`,
  `snapshot_cadence`, `server_tick`, `server_scheduler_lag`, `websocket_backlog`, `pending_commands`,
  `prediction_correction`, `prediction_disabled`, and `wasm_budget`.
- `command_client_send_delay`, `command_upload_delay`, `command_server_parse`,
  `command_server_queue`, `command_receipt_delivery`, `command_response_delay`,
  `command_ack_apply`, and `command_rejected` classify command milestone issues before they fall
  through to generic RTT or prediction fallback buckets. Client-send delay is browser issue to
  WebSocket send acceptance; upload delay is high issue-to-receipt timing; server parse is inbound
  frame deserialize cost; server queue delay is high room-event queue or accepted-to-sim-ack timing;
  receipt delivery is reliable receipt queue/send age; response delay is high issue-to-sim-ack or
  oldest pending age; ack apply points at browser processing after the ack snapshot arrives.

`snapshot_cadence` covers `snapshotTickGapMax >= 3`, stale/duplicate/skipped snapshot counters, or
`snapshotBurstMax >= 3`. Use it to distinguish receive burst/head-of-line symptoms from high RTT:
large payload fields plus parse/decode cost point at client payload pressure, while clean payload
fields plus high RTT/jitter point at the network path.

For Fly or local logs, start with:

```bash
scripts/fly-logs.sh beta recent | rg 'client_net_report|primary_issue'
```
