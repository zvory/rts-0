# GPU minimap experiment — 2026-10-29

This draft explores combining a two-piece tank minimap icon with GPU rendering in the existing
Pixi worker. It is not a performance improvement and should not merge in its present form.
The original recoil/MG setup PR #1718 remains unchanged and unmerged. This branch builds on its
`81a86b81d94e58dd599451fa0c319c832a8c885c` head; its main-targeted diff includes that prerequisite.

## Implementation

The main thread retains authoritative entity/fog selection, input, panel chrome and cached static
raster authoring. A Canvas-shaped recorder emits ordered image, shape and unit commands. Texture
snapshots upload by revision, with immutable icon images packed into worker atlas pages. The worker
samples current poses, batches contiguous unit runs in ParticleContainers and renders a minimap
RenderTexture in the existing WebGL context. Backing resolution remains at least twice CSS size.
Every presented frame contains current minimap state; no cadence reduction or staggered work.

Tanks use hull and turret/gun only, without track layers or full rig articulation. They retain
independent body/weapon facing and firing recoil. Other unit recoil and support-weapon setup use
the production animation adapter. Explicit minimap PNG exports retain Canvas2D rendering.

## Measurement

Canonical `fixed-roster-hellhole-stream`, authored 30 Hz stream, 1440×900 viewport, DPR 1, Chrome
156, CPU throttle 1, 15 measured seconds after warmup. Uncapped completion-paced production path,
pipeline depth two. These measure asynchronous WebGL presentation capacity, not GPU-complete
throughput or ordinary display FPS. No profiling or concurrent browser workload in headline runs.

The first paired comparison measured **118.73 → 104.13 completed presentations/s (−12.3%)**.
Main-thread frame work fell **3.8 → 2.7 ms**, but worker update rose to about **3.62 ms** and main
submission/serialization rose to about **2.15 ms**. Reducing main-thread work did not improve overall
throughput. A subsequent low-resolution fog variant measured **102.33/s**, again a regression.
The final paired rerun measured **119.20 → 106.40/s (−10.7%)**, with main-thread frame work
**3.8 → 2.6 ms** and p95 moving from the 8 ms to 4 ms histogram bucket. The smaller main-thread
numbers do not compensate for reduced end-to-end capacity. All five uncapped runs passed with drained queues and zero failed/stale/superseded presentations.
Texture-message processing is included in final worker-update timings, and worker rendering includes
the minimap render target and world composite.

The requested historical pre-fidelity comparison is preserved separately in
[minimap-animation-performance.md](minimap-animation-performance.md): `f28b4a465` before PR #1712
versus the original animation implementation, median **132.73 → 119.67/s (−9.84%)** over three pairs.
That broader comparison includes intervening main changes. This experiment instead isolates the
GPU/two-piece alternative against #1718's animation implementation.

## Visual evidence and checks

Twelve seeded deterministic ticks (102, 132, 141, 169, 254, 308, 577, 669, 682, 688, 798, 877)
were captured before/after with matching state hashes, viewport, DPR, render clock and ready assets.
Minimap images were decoded and compared exactly; **all twelve fail exact pixel parity**. This is
recorded as a failed check, not a parity pass. Intentional differences include two-piece tank art,
Pixi shape antialiasing, and nearest-filtered tile fog replacing Canvas2D's overlapping one-pixel fog
rectangles. Fog edges change across much of the map. These presentation differences require review
and are another reason to keep this prototype a draft. No authoritative visibility rules change.

An Interact Lab capture with tanks, a Machine Gunner and an Anti-Tank Gun passed readiness and shows
the minimap composited through the existing DOM panel. The initial full-resolution standalone-texture
approach and direct Sprite approach also regressed; trimming alpha bounds and batching particles did
not recover the loss. A second rendering context or worker is not justified by these measurements.

Focused contracts cover texture revision/dispatch behavior, source buffer retention during transfer,
resize uploads, command ordering/transforms, and independent tank facing/recoil. Existing minimap
input, export, renderer worker, rehydration and presentation contracts also pass. The known browser
smoke starting-steel assertion remains documented in the original report.

Local artifacts: `/tmp/minimap-gpu/pair`, `/tmp/minimap-gpu/tile-fog`,
`/tmp/minimap-gpu/final-pair`, and `/tmp/minimap-gpu/parity`. PNG captures and raw benchmark JSON
are local diagnostics, not committed assets. Reproduce with:

```sh
RTS_CLIENT_PERF_UNCAPPED=1 node scripts/client-perf-harness.mjs \
  --workload fixed-roster-hellhole-stream --seconds 15 --viewport 1440x900 --dpr 1
```

A future attempt should first reduce the minimap scene cloning/serialization and worker update cost,
then rerun the same benchmark. Exact pose caching remains deferred. GPU batching and simple tank
art are compatible, but compatibility alone does not make this implementation faster.
