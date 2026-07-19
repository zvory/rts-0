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

Use only `supply-300-hellhole-stream` for supply-scale client renderer comparisons. It exercises the
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

## Experiment ledger

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
