# Client Frame-Work Reduction

## Status

Ready for implementation. This plan is based on a canonical 15-second
`supply-300-hellhole-stream` V8 CPU profile captured from `origin/main` at `c7dc801d` on
2026-07-15. The requested `framebuffer` name is the plan identifier; the evidence points to CPU
frame preparation rather than a new GPU framebuffer architecture.

## Current Evidence

The isolated client workload passed its 900-frame, 30 Hz, 380-entity contract with no WebSocket
or live server simulation. On this machine, `frame.work` averaged 22.5 ms with a 24 ms p95 and
33.8 ms maximum, missing the 60 FPS p95 budget by 7.33 ms. Attribution covered 99.6% of average
frame work:

- `match.renderer`: 13.9 ms average / 16 ms p95.
- `renderer.update`: 10.6 ms average / 12 ms p95.
- `renderer.units`: 8.7 ms average / 12 ms p95.
- `renderer.present`: 3.3 ms average / 4 ms p95.
- `match.fog`: 6.1 ms average / 8 ms p95.
- `match.presentationFrame`: 1.2 ms average / 2 ms p95.

The ranked CPU profile identifies the actionable self-time rather than only the coarse phases:

- `Fog._rayClear`: 21.6%.
- `sampleRigAnimation`: 9.5%.
- `Renderer._recordRenderDiagnostic`: 5.5%.
- `UnitRigInstance.update`: 4.8%.
- garbage collection: 3.6%.
- `pngAtlasRouteCoverage`: 3.5%.
- Pixi `updateTransform`: 3.0% and Pixi `render`: 2.5%.
- presentation `detach`: 3.0%.

The counters explain the rig cost: 340 units cause about 1,742 rig redraw attempts, 1,713
unchanged skips, and 13,214 route-hidden part checks every frame. The Hellhole stream intentionally
uses a full-world dev snapshot with no `visibleTiles`, so its fog cost is the client fallback path
for all non-neutral spectator sources; normal fogged clients receive a server-authoritative grid.
An attempted `supply-300-active` comparison was rejected twice because its authoritative setup
timed out, including with a release server, so its sampled timing is inadmissible and this plan
makes no production-cap claim from it.

Raw profiles, summaries, and flame graphs remain under ignored `target/client-perf/` paths. Do not
commit them or treat percentages from this machine as device certification.

## Phase Summaries

### [Phase 1 - Make Measurement Cheap and Representative](phase-1.md)

Batch high-frequency renderer diagnostics so measurement no longer performs millions of string and
map updates in the hot loop. Restore the existing authoritative `supply-300-active` setup without
weakening its assertions, then recapture both comparison lanes. This phase should improve ordinary
frames while making the evidence for the next two phases substantially less observer-distorted.

### [Phase 2 - Reuse Fog Work Across Stable Snapshots and Sources](phase-2.md)

Give client snapshots a monotonic local revision so fog is recomputed only when its authoritative
inputs change. Preserve exact fallback visibility while caching bounded per-source masks across
unchanged units and rebuilding only moved, added, removed, or terrain-invalidated sources. This
phase targets the 21.6% `_rayClear` hotspot without weakening server fog authority or changing the
wire protocol.

### [Phase 3 - Sample Each Rig Once and Apply Sparse Routes](phase-3.md)

Compile each unit kind's PNG/SVG route coverage once and remove steady-state route discovery and
excluded-part scans. Build one animation sample and render context per entity per frame, share them
across shadow, body, overlay, PNG, and SVG fallback routes, and keep transient sample storage
bounded and non-escaping. Finish with fresh client-only and admissible active-player profiles, then
stop for a measured review before planning more work.

## Whole-Plan Constraints

- Preserve server-authoritative fog, fog-filtered entity projection, and current visible/explored
  semantics. Client fallback fog remains cosmetic and cannot reveal hidden entities or become
  command authority.
- Preserve the detached, frozen `PresentationFrameV1` boundary, one Match-owned RAF, exactly one
  explicit present per frame, and last-successful-present selection semantics.
- Do not change balance, the production supply cap, snapshot cadence, protocol fields, or the
  checked-in Hellhole stream while comparing before/after client work.
- Whole-map zoom remains the representative stress view. Viewport culling does not count as fixing
  this workload.
- Keep renderer failures bounded per entity/frame. Optimization must not remove the existing
  missing-texture fallback or allow one malformed rig to stop future frames.
- Preserve teardown ownership. Any new cache or pooled buffer must be cleared on map reset and
  destroyed with its owning Match/renderer.
- Optimize current measured work before considering workers, OffscreenCanvas, a second RAF, WebGL
  framebuffer tricks, or a renderer rewrite. Those add coordination and lifecycle cost without
  addressing the current JavaScript hotspots.
- Use the same viewport, DPR, CPU throttle, duration, stream bytes, and current machine for paired
  comparisons. Absolute FPS is advisory; retained before/after summaries and ranked functions are
  the evidence.
- Use `supply-300-hellhole-stream` for the repeatable renderer ceiling and a passing
  `supply-300-active` run for claims about active prediction, production-shaped behavior, or normal
  server-visible fog. Neither substitutes for the other.

## Delivery and Handoff

Implement each phase on its own clean branch from current `origin/main`, commit it separately, push
an owned PR, arm auto-merge, and run `scripts/wait-pr.sh <pr>`. The implementing agent must verify
the phase head is reachable from `origin/main` before reporting completion or starting the next
phase. Mark the phase document done in that phase's implementation commit; when the final phase is
done, allow `scripts/agent-pr.sh` to archive this plan in its follow-up commit.

After every phase, provide a handoff that names changed contracts, focused automated checks, exact
before/after profile locations and settings, remaining top functions, the next phase, and the core
manual tests. Manual testing should cover the affected fog/rig visuals, camera movement, one normal
live match frame flow, and leave/re-enter teardown rather than an exhaustive device matrix. Each
graphics phase must use the project-local `interact` skill for one small authoritative Pixi scene,
inspect one clean PNG once, and hand off only its Tailnet Preview URL.

## Checkpoint and Deferred Backlog

After Phase 3, compare the new ranked self/inclusive functions and phase summary against this
baseline, review the result with the user, and either stop or create a fresh small plan. Do not add
executable phases here for presentation detachment, Pixi transforms/presentation, minimap work,
selection/HP overlays, frame-rate adaptation, workers, OffscreenCanvas, or remote device
certification until the new profile shows which one is next.
