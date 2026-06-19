# FPS Measurement Campaign

## Purpose

Create the measurement foundation for a client FPS improvement campaign. The Matt/Alex beta lag
incident proved that the existing client health reports can identify low FPS and large frame gaps,
but they cannot yet explain which frame phase consumed the time. This plan stops at measurement:
phase attribution, permanent aggregate upload through the existing Fly log path, and a repeatable
local browser harness.

## Overall Constraints

- Do not optimize renderer, HUD, fog, minimap, prediction, or state code in this plan. If a phase finds
  an obvious issue, record it in the handoff for a later optimization plan.
- Keep measurement overhead low enough to leave enabled during normal matches. Avoid per-frame console
  logging, unbounded arrays, raw entity dumps, raw command logs, or high-cardinality labels in uploaded
  reports.
- Treat browser reports as advisory and untrusted. They are for diagnosis, not gameplay authority.
- Prefer the existing WebSocket `ClientNetReport` path for permanent upload. Browser clients should
  send bounded aggregates to the server; the server should emit structured logs that Fly already
  preserves.
- Any protocol field added to `ClientNetReport` must update the Rust DTO, JavaScript builder/consumer,
  structured server logging, protocol design docs, and focused protocol/client tests together.
- Local traces and Chrome artifacts may be detailed, but committed code should keep generated artifacts
  under ignored target directories such as `target/client-perf/`.
- Performance tests should begin as reporting and regression evidence, not exact FPS gates. CI and
  laptops vary too much for a hard "must hit 60 FPS" threshold until the harness has stable baselines.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with auto-merge
  armed, then waited on until GitHub reports the PR merged and the phase head is reachable from
  `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Frame Phase Profiler](phase-1.md)

Add low-overhead browser-side phase timing around the existing match frame loop and renderer phase
boundaries. The output should explain slow frames in terms of concrete client phases such as state
interpolation, prediction visual advance, fog update, Pixi entity drawing, HUD update, minimap draw,
and overlay work. This phase should expose local summaries through the existing debug surface without
changing the wire protocol or Fly log payload yet.

### [Phase 2 - Permanent Client Perf Reports](phase-2.md)

Promote the stable Phase 1 aggregates into the existing `ClientNetReport` upload and structured server
log path. The server should log notable browser frame/render issues with enough bounded fields to
separate network lag, server lag, prediction budget pressure, and local paint cost. This phase should
update protocol mirrors, docs, and tests so future lag incidents automatically leave useful Fly log
evidence.

### [Phase 3 - Browser Perf Harness](phase-3.md)

Add a repeatable local headless-Chrome harness that loads fixed workloads, collects the Phase 1/2
summaries, and writes machine-readable artifacts. The harness should include the preserved Matt/Alex
replay path plus one or more deterministic stress/dev workloads, but it should not require manual
console copying. This phase should make CI or local runs report performance evidence without turning
machine-sensitive FPS into a brittle required gate.

## Phase Index

1. [Phase 1 - Frame Phase Profiler](phase-1.md)
2. [Phase 2 - Permanent Client Perf Reports](phase-2.md)
3. [Phase 3 - Browser Perf Harness](phase-3.md)

## Non-Goals

- Do not implement rendering optimizations, allocation reductions, fog/minimap caching, or HUD
  throttling in this plan.
- Do not add a hard CI failure on absolute FPS, frame time, or Chrome trace timing yet.
- Do not add a separate browser-to-Fly ingestion service.
- Do not upload raw Chrome traces, raw entity snapshots, player names, command payloads, or replay
  contents from normal browser clients.
- Do not replace existing server-side `RTS_PERF` tracing or the four-AI server perf harness.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes, use:

```bash
scripts/phase-runner.sh --plan fps phase-1 --pr --wait
scripts/phase-runner.sh --plan fps phase-1 phase-2 phase-3 --pr --wait
```
