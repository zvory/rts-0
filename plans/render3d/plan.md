# Production 3D Rendering Foundations Plan

## Purpose

Turn the Babylon.js proof-of-concept result into durable production foundations without beginning
a faction-wide art conversion or replacing Pixi as the default renderer. The effort establishes
renderer-neutral camera, selection, presentation, capture, ownership, asset, fog/layer, and
performance contracts, then proves them with a bounded opt-in Babylon path. When this plan ends,
future work should be able to add one capability or content slice at a time without rediscovering
the migration architecture or requiring gameplay logic to be implemented twice.

The proof-of-concept implementation branch was deleted intentionally and is not an available
dependency, reference implementation, or recovery source. This plan uses only the current
checked-in client plus the high-level findings recorded in the PoC handoff as evidence. Every
technique must be re-derived and implemented against current `main` and the durable production
contracts created by this plan; do not attempt to restore, locate, or reuse PoC code or assets.

## Outcome

The finished foundations keep one authoritative JavaScript client with two selectable world-render
backends, only one of which is active for a match. `Match` continues to own state, interpolation,
the visual clock, and the only requestAnimationFrame loop; Pixi remains the default while Babylon
is loaded only for an explicit experimental launch. Babylon ends this plan with truthful generic
coverage and representative production assets/effects sufficient to validate fog, layering,
selection, lifecycle, capture, shadows, and batching, not with a converted faction.

## Current Evidence

- `Match` currently constructs `Camera` and the Pixi `Renderer` directly, and the app's start path
  constructs `Match` synchronously. The renderer seam therefore needs an app-owned asynchronous
  load step without moving frame ownership out of `Match`.
- The shared rAF path already lives in `frame_recovery.js`; fixed capture already suspends and
  restores that loop through an injected render clock. Babylon must call `scene.render()` from that
  path and must never start `runRenderLoop()`.
- Orthographic representation leaks beyond rendering: match viewport checks, minimap viewport
  drawing, spatial audio, control-group framing, camera carryover, Lab inspection, observer labels,
  and input read raw `x`, `y`, `zoom`, `viewW`, or `viewH` assumptions.
- Current drag selection converts two screen corners to a world rectangle. That is equivalent only
  for the orthographic camera and is not a valid perspective marquee contract.
- `frame_entity_views.js` already builds shared per-rAF entity arrays, while the Pixi renderer still
  reads `GameState`, `ClientIntent`, camera internals, and fog directly. This is a useful incremental
  starting point, but it is not yet a renderer-neutral presentation boundary.
- The PoC handoff reports that shared particle texture disposal failed only on a later enter/leave
  cycle, that Babylon draw-call counters accumulated when its engine loop was bypassed, and that a
  realistic 200 ms effect was hard to capture asynchronously. Ownership scopes, per-frame metric
  reset, repeated lifecycle checks, and retained real-event replay are therefore foundation work.
- The PoC benchmark of roughly 428 draws, 186 meshes, 81 instances, and 40k triangles is
  encouraging evidence, not a production budget. Reproducible current-main scenarios and
  category-level counters must precede enforceable limits.

## Overall Constraints

- Keep the Rust server, protocol, two-dimensional world-pixel contract, fog authority, snapshots,
  prediction, interpolation, replay data, commands, and simulation unchanged. Stop and request a
  scope decision before adding a wire field or moving gameplay authority into a renderer.
- Keep Pixi as the default and release-required path throughout this plan. A normal match must not
  download, import, parse, initialize, or retain Babylon or experimental renderer modules.
- Treat the deleted PoC implementation as nonexistent. No phase may recover it from Git history,
  reflogs, caches, old worktrees, pull-request patches, artifacts, or another clone; similar code is
  acceptable only when independently justified by the current phase contract and current code.
- Run exactly one world renderer per match. Do not build a long-lived Pixi/Babylon compositor;
  the DOM HUD and existing minimap remain shared external presentation surfaces.
- Keep `Match` as the sole animation-loop owner. Babylon renders only when the authoritative frame
  or fixed-capture path calls it; two loops are a release-blocking defect.
- Define camera and selection semantics using plain world/screen data, never Babylon or Pixi
  objects. Mesh topology, LODs, shadow proxies, missing art, and asset replacement must not change
  which authoritative entities can be selected or commanded.
- Preserve server world `(x, y)` pixels at the application boundary. Put every world-to-scene
  axis, scale, height, and facing conversion in one tested renderer-owned module.
- New Babylon code consumes a renderer-neutral presentation model and already-filtered
  presentation events. It must not independently query `GameState` or infer visibility, ownership,
  selection, command intent, or replay semantics from engine objects.
- Make ownership explicit: backend/scene scope owns engines and shared GPU resources; entity and
  effect scopes own only their instances. Child teardown must not dispose shared textures,
  materials, source assets, shadow infrastructure, or pooled resources.
- Pin and self-host Babylon core, the glTF loader, and every enabled runtime module, load them
  asynchronously, and never use `document.write`. Record versions, licenses, served files,
  checksums/update procedure, optional decoder policy, and unsupported-capability behavior.
- Preserve soft failure. A missing or malformed asset, failed late import, bad entity view, effect
  error, or unsupported GPU feature must produce bounded diagnostics/fallback behavior rather than
  terminate the match frame loop.
- Preserve fixed-capture semantics. Use the injected visual clock and replay retained real,
  already-received fog-filtered presentation events; never lengthen gameplay effects or patch
  `performance.now()` globally for screenshot convenience.
- Do not convert a faction, create a broad unit/building catalog, add free camera orbit, change
  gameplay elevation, adopt WebGPU as a requirement, or make Babylon the default in this plan.
- Graphics phases must use the project-local `lab-interact` skill for a small authoritative scene,
  inspect the returned PNG once, and report its absolute path. Captures stay under
  `target/lab-interact/` and are never committed.
- Worktree preview commands must set `RTS_CLIENT_DIR=<phase-worktree>/client`; a reused release
  binary otherwise may serve the main checkout. Do not replace an existing listener merely to
  create a preview.
- Treat performance thresholds as scenario- and quality-tier-specific. Do not turn one developer
  machine's FPS or the PoC's draw count into a universal gate; do enforce structural counters and
  same-machine regressions once current-main baselines exist.
- Update the durable rendering design contract and the active parity/budget ledger in every phase
  that changes their facts. Active plan files may track execution status, but runtime tests and
  policy must read durable source-of-truth files under `docs/` or code, not archived plans.
- Run `node scripts/check-client-architecture.mjs` for every client phase and add focused pure-data
  contracts before relying on browser-only evidence. The GitHub `Main test gate` remains the
  authoritative full suite.

## Phase Summaries

### [Phase 0 - Contract, Inventory, and Baselines](phase-0.md)

Create the durable rendering architecture document, active parity ledger, and reproducible
baseline scenario definitions using current `main` and the supplied PoC findings. Inventory every
camera consumer, presentation layer, renderer-owned resource class, lifecycle surface, and capture
dependency, then distinguish required cutover parity from representative foundation coverage. This
phase changes no runtime renderer and leaves explicit decisions for later phases rather than
encoding migration policy only inside plan files.

### [Phase 1 - Semantic Camera and Projection Contract](phase-1.md)

Replace application-wide dependence on orthographic `x/y/zoom` representation with semantic camera
and projection operations while preserving the current Pixi view exactly. Migrate minimap, audio,
viewport tests, control groups, Lab inspection/focus, carryover, diagnostics, and camera navigation
to consume the contract. Lock equivalence with pure camera tests, architecture checks, client smoke,
and manual navigation/audio/minimap review before any production Babylon backend is introduced.

### [Phase 2 - Perspective-Safe Picking and Marquee Selection](phase-2.md)

Separate nullable screen-to-ground interaction from entity picking and make entity selection depend
on projected plain-data selection proxies rather than ground rectangles or render meshes. Project
candidate bounds into screen space for clicks, ctrl-select-in-viewport, ordinary marquee selection,
and Lab box tools while retaining current ownership, unit preference, budgeting, and fog-filtered
candidate rules. Prove the behavior with a fake perspective projection as well as orthographic
regression tests so this phase is independently useful before Babylon exists.

### [Phase 3 - Renderer-Neutral Presentation Frame](phase-3.md)

Build a least-privilege renderer frame from the existing frame-local cache, fog, selection,
remembered state, client intent, and overlays without exposing mutable `GameState` or hidden
authoritative variants to a backend. Keep Pixi working through one named compatibility adapter and
define a tested synchronous borrowed-frame lifetime for large revisioned grids plus detached data
for ordinary records. Preserve frame ordering and current visuals while making `render(frame)` the
only contract available to future Babylon code.

### [Phase 4 - Authoritative Presentation Event Contract](phase-4.md)

Normalize already-received fog-filtered presentation events before any renderer consumes them,
including stable identity, deterministic seed, finite lifetime, layering, and reset semantics.
Capture every authorized pose, position, anchor, and facing required for later presentation at
receipt time so a retained event never resolves an old entity id against future state. Prove
deduplication and reconciliation across prediction, pause, replay seek, Lab reset, capture, and
rematch while leaving event visuals in Pixi unchanged.

### [Phase 5 - Deterministic Capture and Retained Event Replay](phase-5.md)

Generalize the existing fixed-capture path into a renderer-neutral lifecycle without creating a
second loop or changing ordinary real-time systems. Freeze a detached presentation revision,
retain a bounded history of real normalized events, and replay one at monotonic non-negative visual
offsets with payload and seed preserved. Demonstrate deterministic short-effect frames,
error/cancel cleanup, rAF restoration, and repeated teardown without lengthening production effect
lifetimes.

### [Phase 6 - Lazy Backend Loading and Lifecycle Kernel](phase-6.md)

Preload the selected renderer factory and every pinned self-hosted Babylon runtime module before
joining an experimental room so `START` handling and listener installation remain synchronous.
Preserve a Babylon-free default static graph and network path, render a bounded production kernel
only through `scene.render()` from `Match`, and expose backend-neutral reset, resize, capture,
diagnostics, failure, and destroy hooks. Validate static and browser loading absence, stale-load
cancellation, unsupported graphics, freeze/rematch, and repeated enter/leave cycles in a controlled
Lab/no-fog route, keeping normal live/replay Babylon blocked until Phase 9 proves secrecy.

### [Phase 7 - Coordinates and GLB Asset Contract](phase-7.md)

Centralize server-world-to-Babylon point, height, direction, scale, and facing conversion, with
round-trip tests and no ad hoc corrections elsewhere. Establish a machine-validated GLB manifest
for visual pivots/anchors, articulated parts, team materials, clips, LOD/shadow roles, provenance,
and budgets while explicitly excluding gameplay selection geometry. Validate the contract with
minimal validator fixtures only; the sole production-representative asset remains a later gate.

### [Phase 8 - Renderer-Owned Resource Registry](phase-8.md)

Implement explicit backend, shared-asset, entity-instance, effect-instance, pool, and shadow-
resource ownership scopes with live diagnostics and generation-safe asynchronous loading. Prove
that child disposal cannot destroy shared meshes, textures, materials, shaders, loader containers,
or shadow resources, including the later-effect particle-texture failure mode from the PoC handoff.
Exercise malformed/missing fallback, repeated entity/effect/reset/rematch disposal, and idempotent
root destruction before fog or broad effects allocate shared resources.

### [Phase 9 - Authoritative Fog and Reveal Secrecy](phase-9.md)

Implement semantic layer categories plus current visible/explored fog, remembered buildings,
vision-only intel, and explicit shot/event reveals in the Babylon kernel. Add programmatic no-leak
assertions covering geometry, picking proxies, diagnostics, particles, labels, and future shadow
admission, not merely mesh visibility. Use a deterministic fog-edge scene to prove replay,
spectator, Lab reset, resize, capture, and rematch behavior before adding interaction overlays.

### [Phase 10 - Core Interaction and Overlay Spine](phase-10.md)

Add truthful instance-compatible generic entity fallbacks plus the smallest selection/HP,
placement, order/target, tactical ground, Lab/observer, real finite effect, and screen-marquee paths
needed to validate the architecture. Exercise the real Babylon perspective camera for clicks,
entity targeting, marquee selection, ground commands, minimap footprint, audio listener, replay,
spectator, Lab, capture, resize, and rematch. Record long-tail Pixi overlays and unit-specific
presentation as explicit ledger backlog rather than expanding this phase into full parity.

### [Phase 11 - Batching, Pools, and Benchmark Harness](phase-11.md)

Create reproducible quiet, dense-placeholder, active-effect, fog/overlay, and lifecycle scenarios
with stable JSON reports under `target/`. Implement shared mesh/material instance policies and
bounded effect pools, then reset Babylon counters explicitly and report current-frame draw calls,
meshes/instances, triangles, materials/textures, particles, timings, and registry state. Calibrate
provisional structural and same-device regression budgets without making machine-dependent FPS a
CI gate.

### [Phase 12 - Vegetation, Shadows, and Quality Tiers](phase-12.md)

Add instanced vegetation driven by one shared shader time uniform with explicit instance world-
matrix support and no per-plant JavaScript animation. Implement a measured directional shadow
manager with caster admission, proxies, map resolution/update policy, and deliberate quality-tier
degradation, then integrate both paths into the benchmark harness. Prove shadow stability,
resource cleanup, and expected relative counter changes without committing to cascades or final
hardware targets without evidence.

### [Phase 13 - Representative GLB and Foundation Gate](phase-13.md)

Validate exactly one neutral production-representative articulated vehicle or structure through the
real manifest, loader, anchors, team material, animation/part, shadow proxy, fallback, registry,
capture, and budget pipeline. Run the full named scenario and repeated lifecycle evidence, update
the durable contract and parity/budget ledgers, and issue a content-expansion `go`, `revise`, or
`stop` recommendation. Do not make Babylon default, convert a faction, or delete/freeze Pixi; those
remain separately reviewed future plans.

## Phase Index

1. [Phase 0 - Contract, Inventory, and Baselines](phase-0.md)
2. [Phase 1 - Semantic Camera and Projection Contract](phase-1.md)
3. [Phase 2 - Perspective-Safe Picking and Marquee Selection](phase-2.md)
4. [Phase 3 - Renderer-Neutral Presentation Frame](phase-3.md)
5. [Phase 4 - Authoritative Presentation Event Contract](phase-4.md)
6. [Phase 5 - Deterministic Capture and Retained Event Replay](phase-5.md)
7. [Phase 6 - Lazy Backend Loading and Lifecycle Kernel](phase-6.md)
8. [Phase 7 - Coordinates and GLB Asset Contract](phase-7.md)
9. [Phase 8 - Renderer-Owned Resource Registry](phase-8.md)
10. [Phase 9 - Authoritative Fog and Reveal Secrecy](phase-9.md)
11. [Phase 10 - Core Interaction and Overlay Spine](phase-10.md)
12. [Phase 11 - Batching, Pools, and Benchmark Harness](phase-11.md)
13. [Phase 12 - Vegetation, Shadows, and Quality Tiers](phase-12.md)
14. [Phase 13 - Representative GLB and Foundation Gate](phase-13.md)

## Phase Gates and Ordering

- Implement phases serially. A later phase may use only contracts already merged by its declared
  dependencies; do not carry an unmerged worktree assumption into another phase.
- Phase 0 is the decision record for contract names, parity categories, scenarios, and provisional
  measurement policy. If implementation evidence invalidates it, update the durable document in
  the responsible phase rather than silently diverging.
- Phases 1 and 2 must merge before production Babylon camera/input integration. Phases 3 and 4
  must merge before Babylon renders live state/events, and Phase 5 must merge before Babylon-
  specific effect capture is accepted as evidence.
- Phase 6 must demonstrate hidden default dependency loading and lifecycle correctness before
  Phase 7 introduces assets. Phase 8's ownership tests must merge before Phase 9 creates shared fog
  textures or Phase 10 creates particles.
- Phase 9 is the fog/secrecy risk gate. Phase 10 validates core interaction/layer architecture;
  long-tail parity remains ledgered rather than silently joining its scope.
- Phase 11 establishes batching and measurement before Phase 12 adds vegetation/shadows and before
  Phase 13 admits a representative asset. Phase 13 may not recommend content expansion if fog-edge
  capture, teardown, truthful fallback, or correctly reset metrics remain unresolved.
- After Phase 13 merges, perform a manual final review of the durable contract, parity ledger,
  benchmark artifacts, and inspected Lab captures. Planning and final review are not delegated to
  the phase runner; create a separate content/parity plan only after that review accepts the
  foundations.

## Required Evidence Across the Plan

- Default Pixi launch with no Babylon module or dependency request.
- Semantic camera contract coverage plus a fake-perspective selection suite.
- One authoritative local scene for each graphics phase, captured with Lab Interact and inspected
  once by the implementing agent.
- Deterministic captures of a real short-lived event at specified visual-clock offsets.
- Fog-edge evidence covering visible, explored, unseen, remembered, and above-fog reveal cases.
- Backend diagnostics that distinguish current-frame from cumulative engine counters, emitted by
  `node scripts/rendering-benchmark.mjs --backend babylon --scenario <id> --output
  target/rendering-benchmarks/<id>.json` after Phase 11 creates that stable command.
- Reproducible `quiet`, `dense-placeholders`, `active-effects`, `fog-overlays`, `lifecycle`, and
  `vegetation-shadows` scenarios, with viewport, DPR, browser/backend, settings, median/p95 frame
  time, and category counters.
- At least two enter/leave cycles in Phase 6, targeted repeated resource/effect cycles in Phases 8
  through 10, and a longer bounded lifecycle cycle in Phase 13.
- A parity-ledger update in every phase describing `complete`, `representative`, `placeholder`,
  `missing`, `external shared surface`, and `intentionally deferred` items without treating a
  placeholder as visual parity.

## Non-Goals

- No faction-wide unit or building conversion and no finished art catalog.
- No deletion, freeze, or default replacement of Pixi.
- No server-side 3D coordinates, terrain elevation gameplay, physics, renderer-owned collision,
  selection authority, pathfinding, visibility, or combat results.
- No free orbit/cinematic camera, mobile-control redesign, WebGPU requirement, global postprocess
  program, cascaded-shadow commitment, or final hardware support matrix.
- No requirement to reproduce every Pixi overlay in Babylon during this plan; Phases 9 and 10 prove
  the semantic fog/layer/interaction spine and leave explicit ledger work for future vertical slices.
- No universal FPS promise derived from one workstation or from the disposable PoC scene.

## Implementation and Handoff Process

Each phase is implemented in a clean task-specific worktree and committed on its own
`zvorygin/*` branch. In a manual executor workflow, the implementing/delivery agent runs focused
checks, marks the phase file done in the implementation commit, runs
`scripts/agent-pr.sh --verification "..."`, and then runs `scripts/wait-pr.sh <pr>`. When invoked
through `scripts/phase-runner.sh --pr --wait`, the inner executor only implements, verifies, marks,
and commits the phase; the outer runner owns push, PR creation/update, auto-merge, waiting, and
ancestry verification as required by its executor prompt. Neither workflow may report completion
or begin the next phase until the PR is definitely merged and its head is reachable from
`origin/main`.

After every phase, the implementing agent must provide a handoff stating what changed, which
durable contracts or ledger entries moved, what the next phase should do, which core features
should be manually tested, what evidence was collected, and any exact blocker or deferred risk.
Manual testing notes should cover the phase's core behavior rather than attempt an exhaustive
matrix. Graphics-phase handoffs also include the inspected Lab Interact PNG path and the exact
`RTS_CLIENT_DIR` preview command or URL used.

After approval, executor passes may use the maintained phase runner with an explicit first phase:

    scripts/phase-runner.sh --plan render3d phase-0 --pr --wait

For a later serial range, remember that `--from` is exclusive; name the first phase explicitly
when inclusion matters.
