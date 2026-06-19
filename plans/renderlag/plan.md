# Render Lag Optimization Plan

## Purpose

Improve client-side rendering headroom so normal matches, replays, and deterministic stress
workloads have a credible path toward 120 FPS on weaker laptops. The current measurement evidence
points first at minimap work, then repeated per-frame entity view allocation, then Pixi renderer
scaling and DOM/overlay churn. This plan turns those findings into small implementation phases that
can be measured independently without changing server authority, gameplay semantics, or network
diagnostics.

## Current Evidence

- A local Matt/Alex match-54 replay probe on the current client showed `frame.work` at 14.6 ms
  average with a 24 ms p95 bucket. In that same replay, `match.minimap` was 5.9 ms average and an
  8 ms p95 bucket, while `match.renderer` was 1.2 ms average and a 2 ms p95 bucket.
- Direct minimap subphase samples showed static terrain drawing at about 2.98 ms average and fog
  drawing at about 2.76 ms average on the 126 x 126 map. The minimap issued about 31,400
  `fillRect` calls per replay frame because it repainted terrain for every tile and then repainted
  fog for almost every hidden tile.
- The vehicle-wall stress workload showed the same shape on a smaller no-fog case: minimap terrain
  was about 3 ms per frame, renderer was about 1 ms, and the minimap was the worst phase in every
  sampled frame.
- `state.entitiesInterpolated()` is called multiple times per animation frame by fog, renderer,
  minimap, and overlay/HUD paths. At the measured entity counts this is not the largest cost, but it
  is repeated allocation and should scale poorly as entity counts grow.
- HUD selection rendering and observer analysis are not first-order blockers in the current
  workloads, but they contain frame-by-frame rebuild paths that are cheap to guard once the larger
  paint costs are addressed.

## Overall Constraints

- Stay focused on browser client FPS and rendering work. Do not change server tick rate, simulation
  semantics, command authority, snapshot cadence, or network transport to make render numbers look
  better.
- Preserve server-authoritative fog. Client changes may cache, batch, or schedule local fog drawing,
  but they must not reveal unseen entities, hide visible data, or treat client fog as authority.
- Use the existing browser perf harness and `window.__rtsPerf` evidence for before/after comparison.
  Browser numbers are machine-local evidence, not portable guarantees; do not add a hard CI gate on
  absolute FPS or frame time in this plan.
- Keep the 120 FPS frame budget in view: 8.33 ms total frame work leaves very little room for a
  3-6 ms recurring minimap cost. Each phase should report whether it improved p50, p95 bucket, max,
  and worst-phase counts for the Matt/Alex replay and vehicle-wall stress workloads.
- Do not combine per-player beta FPS reports when analyzing results. Matt's and Alex's reports are
  separate client observations; local replay or stress harness numbers are separate local browser
  measurements.
- Preserve visual behavior unless the phase explicitly states an acceptable visual tradeoff. The
  minimap should remain legible, resource blips should remain visible, fog should retain explored
  and unexplored states, and renderer failures should keep failing soft.
- Follow the client architecture rules: plain ES modules, no JS build step, PixiJS as global `PIXI`,
  dependency injection for cross-area coordination, teardown for new DOM/GPU resources, and
  `node scripts/check-client-architecture.mjs` for client module changes.
- Keep normal telemetry bounded. Local harnesses may collect detailed timing artifacts under ignored
  `target/client-perf/` directories; normal client uploads must not include raw frame arrays, raw
  entity data, replay contents, or high-cardinality labels.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Minimap Static Layer Cache](phase-1.md)

Stop repainting static minimap terrain on every animation frame. This phase should cache terrain and
static resource marks in an offscreen layer that is rebuilt only when the map, transform, size, or
style inputs change. Dynamic overlays such as entities, fog, viewport outline, and pings should keep
rendering live so minimap behavior stays recognizable while the largest static tile loop disappears.

### [Phase 2 - Minimap Fog Scheduling And Batching](phase-2.md)

Stop repainting the full minimap fog grid on every animation frame. This phase should update fog
visual data only when the visibility/exploration grids change and should batch drawing with a cached
overlay, row runs, image data, or another measured low-overhead shape. The goal is to preserve visible,
explored, and unexplored minimap states while removing the second whole-map per-frame tile loop.

### [Phase 3 - Shared Frame Entity Views](phase-3.md)

Compute the frame's common entity views once and share them across fog, renderer, minimap, HUD, and
observer paths. This phase should reduce repeated `entitiesInterpolated()` calls and allocations
without changing interpolation, prediction display, selection, or spectator behavior. The result
should make entity-count scaling cleaner before deeper renderer work.

### [Phase 4 - Pixi Renderer Scaling Pass](phase-4.md)

Reduce recurring Pixi renderer work after the minimap and frame-view wins are in place. This phase
should profile and optimize the unit, resource/building, selection/HP, rig, and feedback paths that
still dominate renderer subphases under stress. The implementation should prefer dirty updates,
reused graphics, cached static rig parts, and unchanged visual semantics over broad rewrites.

### [Phase 5 - HUD And Observer Dirty Guards](phase-5.md)

Remove remaining per-frame DOM and overlay rebuilds that do not need to run every RAF. This phase
should add signature or cadence guards to selection panel, HUD subpanels, and observer analysis
paths that currently rebuild while their inputs are unchanged. The goal is smaller GC and DOM
pressure in selected-unit, replay, and spectator views without changing player-facing controls.

### [Phase 6 - Render Budget Harness And Playbook](phase-6.md)

Make the optimization work durable and repeatable after the code changes land. This phase should
teach the browser perf harness and docs how to run the render-lag comparison suite, report advisory
120 FPS budget warnings, and preserve artifacts for human review without failing CI on
machine-specific FPS. It should also document which workloads, phase labels, and manual checks future
render optimization work must use.

## Phase Index

1. [Phase 1 - Minimap Static Layer Cache](phase-1.md)
2. [Phase 2 - Minimap Fog Scheduling And Batching](phase-2.md)
3. [Phase 3 - Shared Frame Entity Views](phase-3.md)
4. [Phase 4 - Pixi Renderer Scaling Pass](phase-4.md)
5. [Phase 5 - HUD And Observer Dirty Guards](phase-5.md)
6. [Phase 6 - Render Budget Harness And Playbook](phase-6.md)

## Non-Goals

- Do not change server simulation, pathing, combat, fog authority, command validation, or networking.
- Do not add a hard CI failure on absolute FPS, browser frame time, or Chrome trace timing.
- Do not remove minimap fog, resource visibility, viewport outline, pings, selection rings, HP bars,
  shot-reveal visuals, command feedback, or observer analysis to gain FPS.
- Do not change map size, art style, terrain palette, unit silhouettes, or gameplay-visible stats.
- Do not upload raw Chrome traces, raw frame arrays, replay data, raw entities, command payloads, or
  player-entered text from normal browser clients.
- Do not weaken client architecture checks or large-file ratchets to make optimization patches easier.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes, use:

```bash
scripts/phase-runner.sh --plan renderlag phase-1 --pr --wait
scripts/phase-runner.sh --plan renderlag phase-1 phase-2 phase-3 phase-4 phase-5 phase-6 --pr --wait
```
