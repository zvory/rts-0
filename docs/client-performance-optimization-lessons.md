# Client performance optimization lessons

Use this document before proposing or implementing client renderer performance work. It records
measured wins, rejected experiments, misleading interpretations, and work that was tried locally
but never merged. It complements the benchmark contract in
[design/client-stress-tests.md](design/client-stress-tests.md) and the measurement workflow in
[perf-tracing.md](perf-tracing.md).

Last evidence audit: 2026-07-18. Runtime route counts below were regenerated from `origin/main` at
`b29a18b7`. Performance numbers are historical measurements and must be remeasured on current
`origin/main` before making a new acceptance decision.

## Start with the correct model

The source SVG definition, the animation stage used by a live route, and the Pixi objects actually
drawn are three different things.

- The **source SVG definition** contains all authored parts, anchors, bindings, and fallback pixels.
  Its size describes authoring complexity, not ordinary per-frame cost.
- A **PNG or frame-strip draw plan** determines which source parts are covered by raster sprites and
  which genuinely remain SVG. The plan and route coverage are cached.
- The **live animation stage** includes only the composite PNG anchor parts plus uncovered SVG parts.
  It is the relevant binding count for per-frame animation sampling.
- The **live Pixi route** contains the composite PNG sprites and the uncovered SVG parts. Covered
  source SVG parts are not secretly instantiated or drawn behind the PNGs.

Do not multiply a full source rig's part or binding count by the unit population and report the
result as live renderer work. That produced the incorrect claim that PNG support weapons evaluate
roughly 300 animation relationships per unit per frame.

Current live routes are:

| Kind | Runtime | PNG/frame sprites | Live SVG parts | Sampled parts | Evaluated bindings | Full source parts/bindings |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Anti-tank gun | PNG atlas | 6 | 1 shadow | 7 | 39 | 49 / 299 |
| Artillery | PNG atlas | 8 | 1 shadow + 3 muzzle effects | 12 | 80 | 54 / 332 |
| Mortar team | PNG atlas | 9 | 1 shadow | 6 | 30 | 52 / 306 |
| Scout car | PNG atlas | 2 | 1 shadow | 3 | 5 | 21 / 36 |
| Tank | PNG atlas | 5 | 1 shadow + 6 fuel/muzzle effects | 12 | 61 | 35 / 130 |
| Machine gunner | Frame strip | 1 | 1 shadow | 1 | 0 | 17 / 57 |
| Rifleman | Frame strip | 1 | 1 shadow | 1 | 0 | 6 / 9 |
| Scout plane | Frame strip | 1 | 1 shadow | 1 | 1 | 11 / 11 |
| Command car | SVG | 0 | 13 | 13 | 11 | 15 / 13 |
| Ekat | SVG | 0 | 19 | 19 | 19 | 19 / 19 |
| Golem | SVG | 0 | 4 | 4 | 2 | 4 / 2 |
| Worker | SVG | 0 | 4 | 4 | 2 | 4 / 2 |

Some packed/deployed variants and effects are resident but hidden for a given state. The table is a
route inventory, not a claim that every listed child produces pixels on every frame.

The narrowing path is owned by:

- `client/src/renderer/units.js`: selects frame-strip, PNG, or SVG rendering.
- `client/src/renderer/rigs/draw_plans.js`: caches coverage and selects sampled parts.
- `client/src/renderer/rigs/png_runtime.js`: creates only atlas sprites covered by the selected
  route and applies the sampled composite-part states.
- `client/src/renderer/rigs/animation.js`: resets the selected part states and evaluates only the
  compiled stage bindings.

The SVG definitions still matter to PNG units: they supply anchors, packed/deployed visibility,
facing, recoil, muzzle-flash state, tint, and related transforms. The opportunity is to skip or
simplify this **selected** work when evidence supports it, not to remove hundreds of SVG objects
that are not actually live.

## Canonical workload findings

Use only `fixed-roster-hellhole-stream` for fixed-roster client renderer comparisons. It exercises the
ordinary client-only decode, state, presentation, Pixi, fog, HUD, minimap, and animation-frame paths
without a WebSocket or server simulation.

The 2026-07-18 investigation measured 10.8-11.6 ms average frame work at CPU throttle 1, 23.0-24.4
ms at throttle 2, and 40.6-52.0 ms at throttle 4. The 4x cells reproduce the reported roughly 20
FPS failure class. Across 1024x768, 1440x900, and 1920x1080 at DPR 1 and 2, frame work barely moved.

Treat that as evidence of a CPU/scene-processing problem, not a texture-fill or render-resolution
problem. Texture compression and lower DPR may still improve download time, first-use stalls, GPU
memory, or a different device-specific bottleneck, but they are not the leading steady-state FPS
intervention without new contradictory evidence.

One canonical flame graph from the same audit measured 10.6 ms average frame work, including about
8.0 ms in the renderer, 4.5 ms in renderer update, 3.5 ms in synchronous Pixi present, 2.0 ms in the
unit phase, and 1.3 ms in presentation-frame assembly. Nested and self CPU percentages overlap; do
not add them together as independent savings.

The first 2026-07-19 Pixi v8.19.0 cutover mechanically preserved v7 immediate-mode drawing and
introduced a repeatable regression on this same workload. Two v7 baseline runs measured 9.2-9.6 ms
average and 12 ms p95 frame work. Four unoptimized v8 runs measured 10.4-11.2 ms average and 16 ms
p95. The cause was not extra scene work: per-primitive `fill`/`stroke` calls and per-frame HP-bar
`Graphics` clears dirtied v8 contexts and repeatedly rebuilt retained render instructions.

The corrected local port uses explicit native v8 paths for every continuous line, subpath, filled
outline, and joined polyline. A four-cell control on current main separated migration correctness
from general retained-geometry work. Three alternating runs per cell measured:

- stock v7 (`6cfb35a4`): 9.20 ms average, 12 ms p95;
- v7 plus equivalent retained HP/coalesced fog (`3be60d92`): 8.43 ms average, 12 ms p95;
- correctness-only v8 (`7928ac70`): 10.43 ms average, 16 ms p95; and
- v8 plus the same general optimizations (`a6f7df7c`): 9.23 ms average, 12 ms p95.

Therefore the native correctness-only migration cost remained 13.4% average with a 33% p95
increase. The final optimized comparison remained 9.5% slower than equivalently optimized v7,
although p95 returned to 12 ms. Do not use the 9.23-versus-9.20 unoptimized comparison as evidence
of parity. The general optimization pairs matched exactly across the same 16 deterministic decoded
RGBA ticks; v7 versus v8 changed 11.83% of pixels on average from engine rasterization and was
manually reviewed on nonblank battle frames.

Render groups were measured rather than assumed: a world group measured 9.8 ms average, an HP-only
group 10.2 ms, and grouping the volatile layers 10.5 ms with a 26.7 ms maximum. All were rejected.
Shared `GraphicsContext` resources remain a separate follow-up experiment; retain one only if
complete frame work improves without changing frame state, timing, or reviewed pixels.

## Experiment ledger

### Procedural grass: GPU pixel work and accepted specialization

The 2026-11-23 investigation supersedes the July CPU-only diagnosis for the newly released grass.
At 2056×1204 CSS pixels, three rotated canonical Hellhole capacity runs per historical revision
measured roughly 134 worker acknowledgments/s before the grass release versus 93 afterward at
DPR2. DPR1 stayed around 134–137/s. Timing-disabled controls reproduced the high-DPR loss.
These are uncapped, pipelined WebGL acknowledgments, not monitor FPS or GPU-completed frames.

The pre-atlas style 9 already ran on the GPU as a mesh; per-frame JavaScript mainly updated its time uniform.
Its fragment evaluated nine neighboring clumps with three blade masks each, plus
hash/noise, bending, shadows, cloud and wind-light math. `floor(vWorld/1.25)` quantizes artwork
coordinates without reducing native fragment invocations. DPR2 supplies four times the physical
pixels. A hypothetical full-grass 4112×2408 draw has up to 267 million blade evaluations per
frame; actual visible grass coverage is smaller. One draw call does not imply cheap shader work.

An isolated full-screen material probe (three rotated rounds, 48 valid GPU timer queries each)
measured default-grass median p50s of 9.056 ms at DPR1 and 15.575 ms at DPR2 on an Apple M5 Pro.
It omits units, relief lighting, water and simulation: do not substitute it for game frame time.
Removing only the blade loop, retaining grass resources and geometry, substantially restored
whole-client capacity. Real asynchronous GPU-fence controls confirmed GPU-path cost; their
deliberate wait removes CPU/GPU overlap, so they are conservative diagnostic floors.

The accepted change specializes the layer's fixed technique as a GLSL constant. Initial clean
paired runs improved DPR2 whole-client capacity from 92.5 to 118.0 acknowledgments/s, about 28%.
This recovers roughly half the loss and remains below the pre-grass baseline. Uniform style
branches do not imply divergent pixels; compiler pruning, folding and resource reduction are
plausible explanations, not measured register/occupancy results. The tiny decoded-pixel change
was explicitly reviewed and accepted in the [rendering ledger](design/rendering-parity.md).

Shipping validation on `54322c704` repeated three timing-disabled DPR2 pairs: baseline
90.88/91.00/91.25 versus specialized 116.25/116.25/116.50 acknowledgments/s (medians 91.00 and
116.25, +27.75%). Every cell preserved submitted/dispatched/completed equality and zero lost,
superseded, failed or stale frames. The fresh 12 seeded full-buffer comparisons changed
98–121 pixels/frame, maximum channel delta 1/255, no alpha changes; asset/error, input, nonblank
and changing-sequence checks passed. Exact equality failed and was accepted, not relabeled a pass.
All twelve grass styles compiled, drew and tore down in a real worker/WebGL smoke check.
Fourteen capacity cells were retained, including six DPR1 controls. Later DPR1 and fence cells
overlapped unrelated compilation/live rendering, so they do not establish a low-DPR gain or a
clean fence-throughput improvement. The fence pair's retained GPU-pass p50s were 8.763 versus
7.318 ms; these last-128-query summaries are diagnostic intervals, not whole-window frame times.

GPU occupancy and hardware ALU/texture limiters were not measured. Neither overall spare capacity
nor saturation can be inferred from JavaScript CPU idle time. Normal gameplay returns presentation
metadata, not a CPU readback of the image. Additional JavaScript workers or draw calls do not
automatically increase the parallelism of an already broad fragment draw.

Further experiments could cache invariant clump/root data in lookup textures, or share shading
within the existing quantized artwork cells while still updating animation every frame. Both
trade arithmetic for texture/pass cost and require fresh performance and exact-pixel checks;
they are unimplemented hypotheses. Apple's primary documentation explains
[specialization](https://developer.apple.com/documentation/metal/using-function-specialization-to-build-pipeline-variants),
[shader subsystem bottlenecks](https://developer.apple.com/documentation/xcode/reducing-shader-bottlenecks),
and [occupancy](https://developer.apple.com/documentation/xcode/finding-your-metal-apps-gpu-occupancy).

Two diagnostic pitfalls matter: Chromium's WebGL `finish()` implementation only flushes commands,
so the old `GPU_COMPLETE` switch is not evidence of GPU completion; use an actual sync fence when
that boundary is required. Also, DPR2 pixel capture must read `gl.drawingBufferWidth/Height`, not
Pixi's logical renderer dimensions. The investigation's external parity helper corrected the
readback on both sides and passed a 12-frame zero-difference self-control at full resolution.

### Animated grass atlas: accepted visual/performance tradeoff

The subsequent three-approach experiment compared cached clump inputs, an animated texture atlas,
and a simpler two-blade tuft shader against the shipped specialized shader on `1a4abeba8`.
The user reviewed the scene/close-up/difference WebMs and selected the atlas. Default style 9 now
bakes eight bending poses once and blends neighboring poses every frame with a wind lookup;
the simpler appearance is explicitly accepted in the [rendering ledger](design/rendering-parity.md).
Cloud/front simplification later proved visibly repetitive and was reverted to the original
world-space noise and irregular curved fronts, without reverting blade baking. The measurements
below describe the original atlas version, not the corrected atmospheric shading. A narrower
post-correction GPU comparison is recorded in the rendering ledger; it retains most of the
procedural-to-atlas material improvement without claiming updated whole-game performance.
Other Lab styles, including experimental matting style 12, retain their existing appearance.

On Apple M5 Pro, the isolated all-grass draw's DPR2 median across three per-run GPU-query p50s
changed from 14.794 to 1.265 ms (91.45% lower). Each cell required 48 valid asynchronous queries
after eight warmups, with no pending/dropped/disjoint results. This excludes simulation, units and
terrain-relief lighting and is not a whole-game frame time. RGBA8 atlas/wind textures total
2,162,688 bytes (2.0625 MiB), versus the old default ground texture's 0.25 MiB. Constructor timing
does not establish GPU-completed startup/first-use cost.

Three rotated, quiet-host canonical Hellhole pairs at 2056×1204 CSS/DPR2 improved median uncapped
worker acknowledgment capacity from 115.63 to 131.63/s (+13.84%). Initial runs overlapping unrelated
Rust builds were retained but excluded from that primary comparison. Four fresh shipping-validation
cells on `88255f48f` reproduced medians 115.63 versus 131.94/s (+14.1%). All cells passed health and
final queue checks. These are pipelined acknowledgments, not monitor FPS; remaining client work
limits the reference machine once grass becomes cheap.

The atlas uses portable WebGL shader/texture operations, without a Metal-specific path. A separate
SwiftShader/Vulkan check supplies compiler/API coverage, not physical Nvidia/AMD/Intel performance
evidence. Texture/ALU balance and gains on those GPUs remain unmeasured. Do not extrapolate the
Mac percentages to players' PCs or present CPU software-renderer timings as hardware GPU results.

### Retained HP/status geometry: rejected and reverted

The experiment reduced HP graphics clears by 96.1%, from 133.4 to 5.2 per frame. That dramatic
counter change produced only:

- 20% improvement in `renderer.selectionHp`, below its 40% gate;
- 1.8% improvement in complete `frame.work`, below its 2% gate; and
- 2.3% improvement in renderer update + present, below its 3% gate.

Exact pixel parity passed, but the runtime cache was removed because it did not buy enough complete
frame time. Do not propose retained HP geometry again based only on the large clear count. Revisit it
only if the HP path, workload composition, or measured phase cost changes materially.

The rejection and measurements existed only in an uncommitted `framewins-phase1` worktree as of the
audit. Main's active framewins plan did not contain the result.

### Shared entity preparation: one failed checkpoint and one stranded accepted result

A narrow first checkpoint that cached frame entity views saved only 0.025 ms and failed its gate.
That result does not establish that every shared-preparation design is ineffective.

The subsequent combined design performed one entity traversal and shared one safely detached
preparation across presentation and interaction consumers. Its five-sample median changed:

- target phases from 2.027 to 1.598 ms/frame (-21.2%, -0.429 ms); and
- complete frame work from 11.283 to 10.537 ms (-6.6%, -0.746 ms).

The original exact-parity claim was invalid because the stranded driver read a cleared WebGL
framebuffer and compared black images. After repairing the driver to read pixels in the render task,
warm assets, rebuild the match, and replay from frame zero, the recovered implementation passed a
fresh 16-tick exact decoded-RGBA comparison. It also reduced sampled allocation by 3.8% and kept
retained heap within the parent spread. However, the implementation and acceptance note were never committed,
pushed, or opened as a PR. The old worktree was already 207 commits behind `origin/main` at the
2026-07-18 audit. Recover the design and tests deliberately; do not blindly merge the stale files.

### Rig hot-loop cleanup: merged win

[PR #1062](https://github.com/zvory/rts-0/pull/1062) batched rig diagnostics, reused mutable animation
stages, compiled fallback stages lazily, cached draw planning, and preserved inactive-pool cleanup.
Its paired Hellhole evidence changed average/p95/max frame work from 13.2/17/23.1 ms to
11.8/16/20.2 ms while preserving exact pixels. This work is already in main; do not propose it as
unimplemented.

It made selected animation work cheaper. It did **not** add a complete unchanged-pose fast path, so
an evidence-backed pose signature remains a distinct possible experiment.

### Display-object flattening: wrapper removal was not enough

[PR #1041](https://github.com/zvory/rts-0/pull/1041) flattened frame-strip display objects and passed
an eight-round same-browser regression check, but recorded no meaningful performance win. It was
subsequently reverted by [PR #1045](https://github.com/zvory/rts-0/pull/1045).

A separate unmerged single-part SVG flattening prototype measured 12.7 versus 12.6 ms in its local
before/candidate flame graphs and was reverted. Treat that difference as noise, not a win.

Removing a container while retaining the same children is not equivalent to baking many visible
parts into one sprite. Any future node-count experiment must report the actual reduction in live
Pixi children and complete frame work.

### Fog experiments: keep server and client work separate

[PR #1053](https://github.com/zvory/rts-0/pull/1053) sampled ordinary authoritative server fog at 15
Hz and improved its strict Hellhole **server simulation** throughput by about 46%. It is merged, but
it is not a client FPS optimization.

Client dirty-row fog rendering has not been implemented. The measured client fog phase was only
about 0.3 ms/frame in the 2026-07-18 audit, so it is a valid exact-work reduction with a small
ceiling, not a leading route to doubling FPS.

## Do not repeat these mistakes

- Do not treat full SVG source parts or bindings as live PNG-route work.
- Do not claim covered SVG parts are drawn behind PNG sprites; inspect cached route coverage and
  uncovered steps first.
- Do not infer a per-kind hotspot from shared `sampleRigAnimationInto` flame-graph frames. Add
  bounded per-kind measurements or controlled ablations.
- Do not recommend retained HP geometry solely because clear counters are large; the measured full
  frame result already failed.
- Do not call simple wrapper flattening "baking" or assume it materially reduces Pixi traversal.
- Do not present server fog throughput as client render throughput.
- Do not lead with texture compression, DPR reduction, or viewport reduction for steady-state FPS
  while the resolution matrix remains flat.
- Do not add inclusive flame-graph percentages, nested phase timings, and self time as if they were
  disjoint.
- Do not accept background-tab, unfocused-window, cold-load, or browser-throttled runs. The valid
  workload requires an uninterrupted visible and focused tab after warmup.
- Do not improve the benchmark by reducing entity reconciliation, fog, animation, or overlay
  cadence, presenting stale state, or moving unaccounted work outside the measured frame.

Deliberate lower-fidelity art is different from stale presentation. A simplified sprite or
zoom-selected LOD can still be chosen and positioned every frame from current state.

## Reasonable next experiments

Prioritize experiments by their measured complete-frame ceiling, not by alarming source counts.

1. Reconstruct the accepted shared-preparation design on current main and repeat the five-sample
   comparison.
2. Add bounded per-kind counters/timers around selected stage sampling, PNG/SVG instance update, and
   resident/visible Pixi children. This is required before naming the worst unit kind.
3. Prototype a true unchanged-pose fast path. Continue updating the entity's outer position every
   frame, but bypass selected internal stage sampling only when every pose-relevant input is equal.
4. If reduced fidelity is acceptable, test a genuinely low-piece tank or command-car route. A useful
   prototype collapses actual child sprites/parts; it does not merely remove a wrapper.
5. Consider one-sprite idle/deployed poses or a current-state zoom LOD for complex units. Keep
   facing, position, selection, HP, and gameplay feedback current every frame.
6. Reprofile before spending a phase on dirty-row fog, empty-overlay guards, or other sub-millisecond
   work.

Every accepted rendering optimization should record the base/head commits, complete `frame.work`,
renderer update/present, the directly targeted phase/counters, repeated-run method, and any explicit
fidelity tradeoff. Preserve rejected results here so a later agent does not rediscover them from
scratch.
