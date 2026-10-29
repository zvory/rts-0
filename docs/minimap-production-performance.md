# Production minimap performance — 2026-10-29

The Canvas minimap now shows weapon recoil and Machine Gunner setup/teardown while removing
redundant frame serialization and eager diagnostic sorting. Two genuinely transparent tank atlas
layers are excluded before image preparation and pose compilation. The visible hull, turret and
independently recoiling barrel remain; world track marks are unchanged. The GPU experiment in
PR #1719 is not part of this change. This delivery carries forward and supersedes PR #1718.

## Matched historical comparison

The requested historical baseline is `f28b4a465`, the first parent before PR #1712's articulated
Tank/Anti-Tank Gun minimap fidelity change. The candidate source is `5ccfd7c64`, based on current
main `913c0ac18`, including the reviewed animation implementation from `81a86b81d` and the
production worker-boundary fixes. Documentation-only follow-ups do not affect these measurements.

Canonical `fixed-roster-hellhole-stream`, identical checked-in stream and private server binary,
Chrome156, 1440×900 viewport, DPR1, CPU throttle1, ordinary terrain shadows. Each fresh browser
rebuilds the match, pauses the stream, delivers through tick90, warms ready assets and renderer at
that state, and resumes authored30Hz playback. All headline windows cover **tick90→540** over
15 measured seconds. Three alternating before/after pairs; no profiler or concurrent browser test.
Uncapped two-frame production pipeline, acknowledged asynchronous WebGL completion. These are
capacity measurements, not display FPS or GPU-complete timings. No entity, fog, animation or
presentation cadence is reduced. Timing/counter histories are still recorded on every event.

| Pair | Before presentations/s | Production presentations/s | Change |
|---|---:|---:|---:|
| 1 |126.53|140.73|+11.22%|
| 2 |127.53|144.53|+13.33%|
| 3 |128.13|143.27|+11.82%|
| Median |**127.53**|**143.27**|**+12.34%**|

All six runs have zero failed, superseded, stale or pending presentations after draining, and no
page errors. Submission CPU falls from about **1.62ms to0.70–0.71ms**. Main `frame.work` average
rises from2.8ms to3.2–3.3ms because the new minimap animations do more work; both p95 values remain
in the4ms histogram bucket. Complete presentation capacity nevertheless improves because redundant
copying and statistics work in worker-response callbacks is removed. `frame.work` alone does not
account for that callback work and is not the acceptance metric by itself.

A separate matched15-second reference pair compares the original animation implementation
(`81a86b81d`) at117.87/s with production at143.40/s (+21.66%). That single pair supports the
mechanism but is not a repeated headline estimate. The historical comparison also includes
intervening main changes; it does not isolate any single earlier PR's cost. The old unmatched-window
measurements remain in [minimap-animation-performance.md](minimap-animation-performance.md) as
historical evidence, not as the final production comparison.

## Ownership and diagnostics

The assembler already supplies detached immutable records. Dynamic packets create separate
sanitized frame/layer/grid wrappers and let synchronous `postMessage` perform the cross-thread
clone, avoiding a second full local clone. Transferred grids and durable/static-map records retain
their existing copy boundaries. Contract tests transfer actual buffers, mutate the worker-side
result and prove retained/frozen sources remain unchanged; queue, supersession, capture and
rehydration tests also pass.

Diagnostic histories and counters remain current, while sorting happens only when diagnostics are
read. A stable getter serves current counters, timings and ages. Tests cover resets, fatal failures,
replacement matches and teardown; the global final record does not retain a destroyed host.

The tank atlas declares its track overlay cell blank. A test decodes the actual PNG and verifies
both track sprites have zero alpha while the hull, turret and barrel contain pixels. The minimap
stage drops from5 parts/16 bindings to3 parts/10 bindings and avoids two draws per tank. This is
exact removal of empty work, not lower-frequency or lower-fidelity presentation.

## Validation and reproduction

Full client contracts, minimap input contracts, all13 minimap export/icon tests, architecture,
source-size, docs and whitespace checks pass. Twelve seeded ticks (10,19,48,98,157,238,259,518,
600,621,693,864) compare exactly in fresh-browser world captures against the original animation
implementation. The capture-only tread clock is driven by source ticks because the existing world
tread layer otherwise reads real time outside the fixed animation clock; live gameplay is unchanged.

Minimap before/after captures use one entity array, viewport/DPR, clock, Canvas context and ready
image cache, comparing the original full atlas stage with the filtered stage. All12 ticks match
exact decoded pixels, repeated in two fresh sessions (24 paired captures). A separate100-pose
numeric comparison also matches every retained tank transform. Independent-browser minimap exports
initially differed because the first blue hull's derived SVG sprite bitmap was different (other
cached sprites, entity records and transforms matched). Those runs failed the identical-asset
precondition and are retained as diagnostics; they are not reported as parity passes. Sharing the
ready decoded bitmap cache isolates the actual render optimization without forcing another Canvas
backend in production.

An authoritative Interact Lab scene with blue/orange tanks, a Machine Gunner and an Anti-Tank Gun
passes readiness and was visually inspected with the normal HUD/minimap. The scene is closed and
its private daemon stopped. Full PR gate results are recorded before merge. No simulation,
authority, balance or protocol behavior changes.

The fixed-start driver uses existing production workload/setup/presentation harness helpers; it
changes only the warmup/start position. Before measurement it calls `preparePresentationWindow`
with the stream paused at frame90, then resumes with
`startedAt = now - (frameIndex - 1) * 1000 / tickRateHz` and schedules the next frame after one
interval. `runUncappedPresentationWindow(page,15000)` measures and drains the ordinary pipeline.
Source checkouts are selected with `RTS_CLIENT_DIR`, and actual Git source ids are recorded above
because the shared server's `/version` label is not a client-source identity.

Raw run JSON: `/tmp/minimap-production-perf/`; complete local comparison driver:
`/tmp/minimap-production-benchmark.mjs`. The final PNG comparisons are under
`/tmp/minimap-production-parity/`. Standard non-fixed-start corroboration can be run from either
checkout with:

```sh
RTS_CLIENT_PERF_UNCAPPED=1 node scripts/client-perf-harness.mjs \
  --workload fixed-roster-hellhole-stream --seconds 15 --viewport 1440x900 --dpr 1
```
