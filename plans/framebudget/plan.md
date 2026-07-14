# 300-Supply Client Frame-Budget Plan

## Purpose

Make a 300-supply cap technically evaluable by first creating one deliberately pathological,
server-authoritative Lab battle, then measuring the browser's complete Pixi frame and removing the
client work already shown to scale badly near 200 supply. This plan covers the benchmark foundation
and five agreed priorities without changing the production supply cap: single-frame ownership and
honest presentation timing; route-specific rigs; revision-cached fog; one consolidated frame-entity
path; and cached minimap, HP, selection, and occupied-trench drawing. Viewport culling is explicitly
excluded because whole-map zoom is normal play and therefore remains the representative worst case.

Phase 0 is an initial measured checkpoint that establishes the reproducible Lab stress fixture.
Three optimization phases follow before the final measured checkpoint, as required by
`docs/context/planning.md`. The five priorities are grouped by contract boundary so each phase can
land as one coherent PR instead of leaving partially migrated renderer paths. A later supply-cap
decision is outside this plan and must use the final active-player evidence rather than assuming
that either the Lab benchmark or phase completion automatically makes 300 supply safe.

## Priority Mapping

| Agreed priority | Owning phase |
| --- | --- |
| 0. Canonical 300-supply-per-side Lab hellhole and workload | [Phase 0](phase-0.md) |
| 1. One Match-owned RAF and honest Pixi-present timing | [Phase 1](phase-1.md) |
| 2. Route-specific rig construction and one animation sample per entity | [Phase 2](phase-2.md) |
| 3. Fog identity/revision caching | [Phase 3](phase-3.md) |
| 4. Remove repeated frame-entity, presentation, selection, and Pixi compatibility copying | [Phase 2](phase-2.md) |
| 5. Cache minimap blips, HP/selection geometry, and occupied-trench overlays | [Phase 3](phase-3.md) |

## Phase Summaries

### [Phase 0 - Build the 300-Supply Lab Hellhole](phase-0.md)

Add a bundled `1v1` Lab scenario with two exact 300-supply armies, every ordinary unit kind, both
players in god mode, and a minimum-spacing interleaved formation kept in sustained combat. Register
it as one canonical performance workload whose assertions cover composition, projection, codec,
stable entity count, and continuing combat before sampling. Treat the checked-in fixture as the
durable benchmark definition and its initial timing as provisional until Phase 1 owns and measures
the complete presentation frame.

### [Phase 1 - Own and Measure the Complete Frame](phase-1.md)

Make `Match` the only animation-frame owner for a match, disable Pixi's automatic ticker, and
perform exactly one explicit update and present inside the measured frame. Add bounded update,
present, RAF-dispatch, and 60 FPS work-budget evidence locally and in Mainline reports without
changing the renderer-neutral frame boundary. Establish deterministic, server-authoritative
active-player 200- and 300-supply workloads so every later phase compares the same predicted
whole-map client path.

### [Phase 2 - Remove Entity-Linear Rig and Frame-View Waste](phase-2.md)

Construct only the rig parts a route uses, share one animation sample across all routes, remove the
Tank's transparent suppression sprites, and share atlas subtextures at renderer scope. Consolidate
visual, current, and authoritative entity derivation into one source pass while preserving
prediction, authoritative fog poses, detached presentation data, and successful-present selection
semantics. Prove the structural reductions with exact object/sample/copy counts and compare the
same Phase 1 workloads before and after.

### [Phase 3 - Make Stable Layers Revision-Driven](phase-3.md)

Memoize fog grids/facades by revision and stop rebuilding unchanged fog geometry every display
frame. Cache minimap entity blips, HP/selection geometry, and occupied-trench overlays while keeping
camera motion, positions, pings, projectiles, and other genuinely animated effects frame-smooth.
Finish with repeated 200/300 active-player stress measurements at fixed CPU, viewport, and DPR
settings and report whether 300 supply actually clears the proposed 60 FPS gate.

## Overall Constraints

- Do not raise the production supply cap, change balance, alter simulation outcomes, or weaken
  server-authoritative fog in this plan.
- Preserve `PresentationFrameV1` as detached and frozen. A backend must not receive `GameState`,
  transport, input state, hidden entities, or mutable snapshot collections.
- Preserve `SelectionSceneV1` as the interaction authority for the last successfully presented
  frame. A failed update or present must not publish a new selection scene.
- Preserve the three required pose meanings: predicted/interpolated visual pose, latest current
  pose, and non-predicted authoritative fog pose.
- Pixi remains the authoritative production renderer. Keep Babylon's explicit render semantics and
  Map Editor's separately owned non-Match loop correct where shared contracts change.
- Whole-map zoom is the performance target. Do not claim viewport culling or a hidden-camera test as
  evidence that 300 supply is safe.
- Do not blanket-throttle combat effects. Use revision/dirty caching for stable data and retain
  display-rate animation for camera movement, entity positions, pings, projectiles, fades, smoke,
  and muzzle effects where their current semantics require it.
- Performance comparisons must use identical checked-in workloads, seeds, viewports, DPRs, CPU
  throttles, durations, and repeat counts. Chrome CPU throttling is a same-machine stress control,
  not a hardware-identical model of a player's PC.
- Keep the Phase 0 scenario and workload descriptor unchanged after its baseline is accepted. If a
  unit roster or scenario contract must change, version the fixture and re-establish its baseline
  rather than silently rewriting the workload under later phases.
- Phase 0's Lab route exercises normal authoritative snapshots and compression but is
  spectator-shaped and prediction-free. Use it for comparative stress; use Phase 1's active-player
  workload for a later production cap decision.
- Keep traces, screenshots, and benchmark outputs under ignored `target/` paths. Do not commit
  generated PNG captures or performance artifacts.
- Update `docs/design/client-rendering.md`, `docs/design/client-ui.md`, `docs/perf-tracing.md`, and
  protocol documentation in the phase that changes their owned contracts.
- Rendering changes must use the project-local `interact` skill and `interact lab` commands for one
  small authoritative scene, one clean Pixi capture, and one inspected Tailnet Preview URL.

## Measurement and Decision Rules

- Phase 0 proves that the canonical Lab hellhole is exact, repeatable, sustained, and measurable;
  its pre-Phase-1 timing is provisional.
- Phase 1 records the honest baseline; it is not expected to improve frame time.
- Phase 2 and Phase 3 must retain comparable before/after artifacts for both the 200- and
  300-supply active-player workloads.
- Local structural tests are hard gates: one present per owned frame, one rig animation sample per
  entity, route-only object construction, one frame-entity source pass, stable fog revisions, and no
  unchanged geometry redraw.
- The final proposed performance gate is end-to-end `frame.work` p95 at or below 16.67 ms in the
  checked-in 300-supply active-player workload at the agreed weak-PC proxy cell, with zero sustained
  below-60 windows attributable to client work. Prefer approximately 12 ms p95 on the unthrottled
  reference cell to leave useful scheduling and GC margin.
- The serious matrix must include 1x, 2x, and 4x CPU stress, DPR 1 and 2, and default and large
  viewports with at least three repeats. The designated local weak-CPU proxy gate is 4x CPU,
  default viewport, DPR 1; the other 4x cells remain required diagnostic evidence and must not be
  omitted or relabeled.
- If the final gate fails, report the first failing cell and top measured phase; do not raise the
  cap or add speculative follow-up phases to this plan.

## Implementation and Handoff Process

- Implement one phase at a time from fresh `origin/main` in a clean `/tmp/rts-worktrees` worktree on
  a `zvorygin/` branch.
- Mark the phase document Done in that phase's implementation commit.
- Run the focused verification named by the phase, then run
  `scripts/agent-pr.sh --verification "<focused checks and evidence passed>"`.
- Push an owned PR, arm auto-merge, run `scripts/wait-pr.sh <pr>`, and wait until the PR is merged and
  its head is reachable from `origin/main` before starting the next phase.
- After each phase, provide a handoff message describing changed contracts, exact verification and
  measurement artifacts, known caveats, what the next agent should do, and the core manual tests to
  repeat. Manual testing should cover the phase's core features rather than an exhaustive matrix.
- When Phase 3 marks every phase Done, allow `scripts/agent-pr.sh` to archive this plan in the final
  phase PR as defined by the repository workflow.

For unattended execution after approval:

```bash
scripts/phase-runner.sh --plan framebudget phase-0 --pr --wait
scripts/phase-runner.sh --plan framebudget phase-1 --pr --wait
scripts/phase-runner.sh --plan framebudget phase-2 --pr --wait
scripts/phase-runner.sh --plan framebudget phase-3 --pr --wait
```

## Deferred Backlog

- Production supply-cap or balance changes after the final evidence is reviewed.
- Device-lab certification on named low-end hardware; the checked-in CPU/DPR matrix is comparative
  evidence, not certification.
- LOD, viewport culling, Web Workers, snapshot-rate changes, GPU instancing, or a wholesale Pixi
  helper rewrite unless final traces show one is still necessary.
- Blanket combat-effect throttling or visual simplification not supported by a measured phase.
