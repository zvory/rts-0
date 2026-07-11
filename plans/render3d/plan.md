# Production 3D Rendering Foundations Plan

## Purpose

Turn the Babylon.js proof-of-concept result into durable production foundations without beginning
a faction-wide art conversion or replacing Pixi as the default renderer. The effort establishes
renderer-neutral camera, selection, presentation, capture, ownership, asset, fog/layer, and
performance contracts, then proves them with a bounded opt-in Babylon path. When this plan ends,
future work should be able to add one capability or content slice at a time without rediscovering
the migration architecture or requiring gameplay logic to be implemented twice.

The proof-of-concept implementation branch was deleted intentionally and is not an available
dependency, reference implementation, or recovery source. The PoC observations copied into this
plan are unverified historical leads, not evidence that an executor must recover or reproduce.
Every technique must be re-derived against current `main` and the durable production contracts
created by this plan; do not attempt to restore, locate, or reuse PoC code or assets.

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
- The shared rAF path already lives in `frame_recovery.js`; fixed capture suspends/restores it but
  currently assigns synthetic time to live state while stepping room ticks. Phase 5 must split live
  admission time from detached playback and add a no-tick-step event mode before Babylon evidence;
  Babylon must call `scene.render()` from that path and never start `runRenderLoop()`.
- Orthographic representation leaks beyond rendering: match viewport checks, minimap viewport
  drawing, spatial audio, control-group framing, camera carryover, Lab inspection, observer labels,
  and input read raw `x`, `y`, `zoom`, `viewW`, or `viewH` assumptions.
- Current drag selection converts two screen corners to a world rectangle. That is equivalent only
  for the orthographic camera and is not a valid perspective marquee contract.
- `frame_entity_views.js` already builds shared per-rAF entity arrays, while the Pixi renderer still
  reads `GameState`, `ClientIntent`, camera internals, and fog directly. This is a useful incremental
  starting point, but it is not yet a renderer-neutral presentation boundary.
- The plan records an unverified PoC observation that shared particle texture disposal failed only
  on a later enter/leave cycle, that Babylon draw-call counters accumulated when its engine loop was
  bypassed, and that a realistic 200 ms effect was hard to capture asynchronously. Ownership scopes,
  per-frame metric reset, repeated lifecycle checks, and retained real-event replay are therefore
  foundation work.
- Historical PoC draw, mesh, instance, and triangle counts are intentionally omitted because no
  durable artifact supports them. Reproducible current-main scenarios and category-level counters
  must precede enforceable limits.

## Decisions Locked for Unattended Execution

- The launch selector is exactly `rtsRenderer=pixi|babylon`. Missing means `pixi`; any other value
  is an actionable pre-join error, and renderer choice is never persisted silently in settings or
  local storage.
- The experimental capability baseline is WebGL 2; lack of WebGL 2 is a bounded pre-join failure,
  not a WebGL 1/WebGPU/default-renderer fallback. Phase 6 selects the highest official stable
  Babylon patch available at implementation time, vendors only the minified UMD core and glTF
  loader plus licenses/manifest, loads core then loader sequentially from same-origin URLs, and
  records official package integrity plus file SHA-256.
- Through Phase 10, Babylon is restricted to an authoritative `/lab` launch explicitly requesting
  `rtsRenderer=babylon`; Phases 6.5 through 10 add only the controlled capabilities named in their
  phase files. Phase 10.5 is the sole gate allowed to enable experimental live, replay, and
  spectator launches, and only after its route/capability tests pass.
- The 240 ms normalized `attack`/muzzle-feedback event is the required short-event fixture for
  retained capture. An executor may substitute another already-received event only if current
  `main` removed that event and the replacement is shorter than one second, spatially
  self-contained, and documented in the durable rendering contract.
- Representative Phase 10.5 visuals are exactly: building placement footprint with valid/invalid
  state, move order line/destination plus entity-target marker, selected-unit range ring, Lab tool
  area preview, screen marquee, and the normalized attack/muzzle effect. Substitution is allowed
  only when current `main` removed the source feature, using the same bounded replacement rule as
  the short-event fixture.
- The final representative GLB is a repository-authored neutral tracked-vehicle fixture generated
  by a checked-in deterministic script. It has a hull, turret, independently articulated barrel,
  team-color slot, muzzle/selection/HP anchors, visible bounds, and shadow proxy; it is
  production-representative in contract complexity and budget shape, not a claim of final art. Its
  semantic hull is exactly 50.4 world px long by 28.8 world px wide. Executors must not search for,
  download, or generate third-party/AI art for this gate.
- Experimental quality tier ids are exactly `off`, `low`, `medium`, and `high`. Vegetation density
  factors are `0`, `0.30`, `0.60`, and `1.00`; Phase 12.5 starts shadows at map/caster/update values
  `0/0/never`, `512/32/every 4th frame`, `1024/64/every 2nd frame`, and `2048/128/every frame` and
  may only reduce optional work when measured budgets require it.
- The committed benchmark schema lives at `scripts/rendering-benchmark.schema-v1.json`; only
  generated reports live under `target/rendering-benchmarks/`. Verification commands naming a new
  test or script require the phase to create that file if it does not yet exist.
- Every new pure client contract must be imported by `tests/client_contracts.mjs` in the creating
  phase. Every new browser assertion must either join an existing `client_smoke`/Lab smoke path or
  be registered in `tests/run-all.sh` and the authoritative CI shard; a directly invoked orphan
  test is not completion evidence.
- Dedicated `browser_babylon_*` and rendering integration commands own a private server/browser and
  tear them down, while accepting an injected runner URL when the authoritative suite supplies one.
  They fail rather than skip when Chrome, WebGL 2, selected backend, or readiness is unavailable.
- Graphics evidence defaults to seed `3303`, viewport `1000x700` at DPR 1, clean presentation,
  capture name `render3d-p<phase-with-dots-replaced-by-dashes>`, and the phase-selected backend.
  Before capture, the executor records an exact Lab scenario or mutation command sequence plus
  readiness assertions for backend id, no fallback/error, expected subjects, and stable presentation/
  view generation; a phase may override these values only when its file names an exact alternative.
- Public contract names may be adjusted to fit current code, but behavior, ownership, units, and
  safety rules may not be weakened. Choose the smallest coherent name/API, record it in
  `docs/design/client-rendering.md`, and continue without requesting product input.
- The semantic layer ids and back-to-front order are exactly `staticGround`,
  `persistentGroundMark`, `fogGatedWorld`, `rememberedWorld`, `belowFogIntel`, `currentFog`,
  `aboveFogReveal`, `tacticalFeedback`, and `screenOverlay`. Phase 3 freezes the descriptors,
  Phase 4 assigns events to them, and Phases 9/9.5 implement Babylon fog/order behavior without
  redefining the enum.
- Renderer-neutral projected points are `{x, y, heightPx}` with `heightPx=0` on the authoritative
  plane. Positive `heightPx` is presentation-only height expressed in world-pixel scale for
  semantic proxies/anchors; it never enters the wire, simulation, pathing, or command coordinates.
- Large terrain/fog data crosses the renderer boundary as a revisioned immutable `GridSnapshot`
  accessor with no exposed mutable typed array. It may provide indexed reads and copy into a
  backend-owned staging buffer; the assembler creates a new snapshot only when the source revision
  changes, and fixed capture pins that immutable revision. There is no unenforceable borrowed-array
  lifetime contract.

## Unattended Executor Contract

At the start of every phase, read the merged durable rendering contract and parity ledger, confirm
the declared dependency is reachable from `origin/main`, and inventory current code only within
the phase's named surfaces. Expected touch points are a bounded search area, not permission to
rewrite every listed file; prefer focused collaborators and update the list in the handoff when
current `main` has moved a responsibility.

An executor may make ordinary implementation choices—module names, pure-helper decomposition,
test fixture layout, Babylon stable patch version, and equivalent local API shape—when all locked
contracts remain true. It must not stop merely because an expected file does not exist, a planned
test must be added, or an implementation detail was not preselected.

Stop and return a structured `blocked` handoff only when at least one of these is true:

- satisfying the phase requires a protocol, Rust server, simulation, replay-format, or gameplay-
  authority change;
- an external dependency or asset lacks a reproducible official source, checksum/integrity record,
  or repository-compatible license;
- current merged behavior contradicts a security/ownership invariant and the smallest correction
  materially exceeds the phase;
- a required browser/GPU/Lab artifact cannot be produced after pure-data and nonvisual checks pass,
  with the exact missing capability and recovery command recorded; or
- focused verification, the commit hook, PR automation, or the authoritative merge gate fails and
  cannot be repaired within the phase.

Do not absorb work assigned to the next phase. If a convenience refactor is not needed for the
current acceptance gate, ledger it and leave it for its owner.

Before marking a phase done, map every implementation-checklist item to a named command/assertion
in the structured handoff. Manual review is evidence only for visual readability, composition, and
artifact inspection; secrecy, authority, lifecycle counts, targeting classifications, capture
clocks, pool reset, counter reset, budgets, and teardown are automated blocking assertions.

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
- Lab Interact remains the agent's scene setup/capture tool, not a mobile interface. When useful for
  informal review, a graphics handoff may also expose the same local scene through a verified
  Tailscale URL so the user can pan and pinch-zoom from a phone; this convenience is not acceptance
  evidence and Tailscale unavailability does not block a phase.
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
baseline scenario definitions using current `main` and the unverified historical leads copied into
this plan. Inventory every camera consumer, presentation layer, renderer-owned resource class,
lifecycle surface, and capture
dependency, then distinguish required cutover parity from representative foundation coverage. This
phase changes no runtime renderer and freezes every later product/design choice or blocks.

### [Phase 1 - Semantic Camera and Projection Core](phase-1.md)

Add the renderer-neutral semantic camera/projection API while preserving the current Pixi camera
implementation exactly. Prove orthographic equivalence, nullable ground hits, projection depth and
clipping, viewport polygons, semantic navigation, fit/focus, snapshot, and listener data in pure
contracts. Keep consumers on the compatibility edge until the next phase so this phase remains a
bounded foundation change.

### [Phase 1.5 - Navigation and Minimap Migration](phase-1.5.md)

Move live/replay camera gestures and minimap footprint/recenter behavior onto the merged semantic
contract. Preserve Pixi behavior and CSS-pixel/DPR handling while carrying only a bounded temporary
allowlist for consumers owned by Phase 1.75. Prove the high-frequency navigation path before
changing app-shell, audio, or Lab surfaces.

### [Phase 1.75 - Shared Camera Consumer Closure](phase-1.75.md)

Migrate audio, viewport alerts, control groups, app/replay carryover, visual profiles, Lab,
observer, and diagnostics to semantic camera data. Version the public snapshot/tooling shape and
close the raw-read ratchet with no shared consumer exception. Finish the complete camera migration
before selection semantics change.

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
authoritative variants to a backend. Define locked layer descriptors, revisioned immutable grid
snapshots, and detached ordinary records beside the current Pixi call path. Prove assembly order,
least privilege, and replay/Lab/reset semantics before runtime cutover.

### [Phase 3.5 - Pixi Presentation Cutover](phase-3.5.md)

Make one assembled `render(frame)` call the only backend seam from Match and keep Pixi working
through one named compatibility adapter. Move non-event destructive consumption into shared
reconciliation and ratchet every temporary legacy read. Prove Pixi runtime, capture, replay, Lab,
soft-error, and rematch equivalence before event normalization.

### [Phase 4 - Authoritative Presentation Event Contract](phase-4.md)

Normalize already-received fog-filtered presentation events before any renderer consumes them,
including stable identity, deterministic seed, finite lifetime, layering, and reset semantics.
Capture every authorized pose, position, anchor, and facing required for later presentation at
receipt time so a retained event never resolves an old entity id against future state. Prove
deduplication and reconciliation across prediction, pause, replay seek, Lab reset, capture, and
rematch while leaving event visuals in Pixi unchanged.

### [Phase 5 - Deterministic Capture and Retained Event Replay](phase-5.md)

Separate live admission time from detached playback time without creating a second loop or changing
ordinary real-time systems. Preserve simulation-timeline fixed capture and add a distinct frozen
event mode that never steps ticks, using bounded real event history and explicit offsets. Prove
capture purity, deterministic short-effect frames, cleanup, and rAF restoration without extending
production lifetimes.

### [Phase 6 - Lazy Backend Loading](phase-6.md)

Add the exact `rtsRenderer` selector, an app-owned pre-join backend resolver, and pinned self-hosted
Babylon core/glTF runtime files without constructing a Babylon engine. Inject the existing Pixi
backend through the same factory seam and prove the default static graph and browser timeline load
no Babylon code or bytes. Cover cancellation, stale completion, dependency integrity, invalid
selection, and synchronous `START` construction before graphics lifecycle work begins.

### [Phase 6.5 - Babylon Lifecycle Kernel](phase-6.5.md)

Construct the smallest controlled-Lab Babylon engine, scene, canvas, fixed perspective adapter,
and presentation bundle behind the Phase 6 resolver. Render only through `scene.render()` when
`Match` calls it and cover partial failure, resize, reset, capture, freeze, destroy, and two full
enter/leave cycles. Keep fog-dependent categories and all normal live/replay/spectator Babylon
routes blocked while producing the first backend-neutral Lab capture.

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
or shadow resources, including the later-effect particle-texture failure mode recorded as an
unverified historical observation in this plan. Exercise malformed/missing fallback, repeated
entity/effect/reset/rematch disposal, and idempotent root destruction before fog or broad effects
allocate shared resources.

### [Phase 9 - Visibility and Fog Core](phase-9.md)

Implement the locked terrain/current-visibility/client-explored fog core with revisioned
registry-owned resources. Add view-generation clearing for fog, SelectionScene, and recipient
diagnostics while keeping normal routes blocked. Prove controlled replay/spectator/Lab/reset/
capture/rematch lifecycle before memory and reveals are admitted.

### [Phase 9.5 - Memory, Reveal, and Secrecy Gate](phase-9.5.md)

Add remembered buildings, below-fog intel, and explicit above-fog reveals with the full generation
reset set. Prove end-to-end absence of never-authorized sentinel ids/positions through a mandatory
real two-recipient server fixture, not only fake frames. Capture the deterministic fog-edge scenario
and keep normal routes blocked until Phase 10.5.

### [Phase 10 - Generic Entities and Perspective Interaction](phase-10.md)

Render every received current entity kind through truthful shared generic fallbacks and add semantic
selection/HP presentation. Exercise every entity-target and nullable ground-target path against the
real Babylon perspective adapter while keeping asset geometry out of selection authority. Validate
minimap, audio, control groups, replay/spectator no-control policy, Lab, resize, capture, and rematch
without adding the overlay/effect catalog or unlocking normal routes.

### [Phase 10.5 - Overlay, Effect, and Route Gate](phase-10.5.md)

Add one complete placement, order/target, tactical ground, screen-marquee, Lab/observer overlay,
and real finite normalized-event effect path. Prove fog, capture, resource, and lifecycle behavior
for those representative categories in controlled Lab and browser contracts. Only after those
checks pass, enable explicitly requested experimental live, replay, and spectator Babylon routes
while preserving role-appropriate command policy.

### [Phase 11 - Benchmark Harness and Counter Semantics](phase-11.md)

Create the five stable pre-vegetation scenarios, committed JSON schema, launcher, metadata, warmup,
sampling, and teardown contract. Reset Babylon counters once per authoritative Match frame and
prove current-frame versus cumulative semantics before optimizing the scene. Record reproducible
unoptimized baselines and structural invariants, but defer routing and pool budgets to Phase 11.5.

### [Phase 11.5 - Batching, Pools, and Provisional Budgets](phase-11.5.md)

Classify generic content into unique, hardware-instance, thin-instance, merged-static, or pool
routes and enforce shared source/material/texture keys. Add bounded effect-pool capacity, overflow,
complete return-state reset, and lifecycle diagnostics using the resource registry. Re-run the
stable scenarios to set provisional structural and same-device regression budgets without making
machine-dependent FPS a CI gate.

### [Phase 12 - Instanced Vegetation](phase-12.md)

Add deterministic quality-tier vegetation through the merged instance policy with one shared
visual-clock shader uniform. Prove non-identity instance world matrices participate in final vertex
position and that no per-plant JavaScript animation exists. Add a stable vegetation scenario and
measure tier deltas, fog behavior, capture, and cleanup before shadows complicate the evidence.

### [Phase 12.5 - Shadows and Quality Tiers](phase-12.5.md)

Add a bounded camera-fitted directional-shadow manager with explicit caster/proxy admission,
resolution, update cadence, and registry ownership. Integrate deliberate tier degradation and the
final `vegetation-shadows` scenario while proving hidden entities cannot cast or affect diagnostics.
Measure off/on and tier deltas, lifecycle stability, and tactical readability without adding
cascades or a universal hardware gate.

### [Phase 13 - Representative GLB Integration](phase-13.md)

Generate the locked repository-authored neutral tracked-vehicle GLB reproducibly and validate its
manifest, anchors, articulation, team material, fallback, resource, effect, shadow, and budget
paths. Measure it against the generic fallback in the same scenario/device/tier and prove
deterministic regeneration. Leave full-suite scenario/lifecycle collection and the final foundation
gate to Phase 13.5.

### [Phase 13.5 - Foundation Evidence Gate](phase-13.5.md)

Run every stable scenario and a true ten-cycle same-page Match lifecycle with exact automated
baseline assertions. Audit the durable contract/parity ledger and require every content-expansion
gate to have current command-backed evidence. If any mandatory gate fails, return `blocked` without
marking the phase done; if all pass, mark it done and let the owned PR lifecycle archive the
evidence, leaving `go/revise/stop` to the manual final review.

## Phase Index

1. [Phase 0 - Contract, Inventory, and Baselines](phase-0.md)
2. [Phase 1 - Semantic Camera and Projection Core](phase-1.md)
3. [Phase 1.5 - Navigation and Minimap Migration](phase-1.5.md)
4. [Phase 1.75 - Shared Camera Consumer Closure](phase-1.75.md)
5. [Phase 2 - Perspective-Safe Picking and Marquee Selection](phase-2.md)
6. [Phase 3 - Renderer-Neutral Presentation Frame](phase-3.md)
7. [Phase 3.5 - Pixi Presentation Cutover](phase-3.5.md)
8. [Phase 4 - Authoritative Presentation Event Contract](phase-4.md)
9. [Phase 5 - Deterministic Capture and Retained Event Replay](phase-5.md)
10. [Phase 6 - Lazy Backend Loading](phase-6.md)
11. [Phase 6.5 - Babylon Lifecycle Kernel](phase-6.5.md)
12. [Phase 7 - Coordinates and GLB Asset Contract](phase-7.md)
13. [Phase 8 - Renderer-Owned Resource Registry](phase-8.md)
14. [Phase 9 - Visibility and Fog Core](phase-9.md)
15. [Phase 9.5 - Memory, Reveal, and Secrecy Gate](phase-9.5.md)
16. [Phase 10 - Generic Entities and Perspective Interaction](phase-10.md)
17. [Phase 10.5 - Overlay, Effect, and Route Gate](phase-10.5.md)
18. [Phase 11 - Benchmark Harness and Counter Semantics](phase-11.md)
19. [Phase 11.5 - Batching, Pools, and Provisional Budgets](phase-11.5.md)
20. [Phase 12 - Instanced Vegetation](phase-12.md)
21. [Phase 12.5 - Shadows and Quality Tiers](phase-12.5.md)
22. [Phase 13 - Representative GLB Integration](phase-13.md)
23. [Phase 13.5 - Foundation Evidence Gate](phase-13.5.md)

## Phase Gates and Ordering

- Implement phases serially. A later phase may use only contracts already merged by its declared
  dependencies; do not carry an unmerged worktree assumption into another phase.
- Phase 0 is the decision record for contract names, parity categories, scenarios, and provisional
  measurement policy. If implementation evidence invalidates it, update the durable document in
  the responsible phase rather than silently diverging.
- Phases 1, 1.5, 1.75, and 2 must merge before production Babylon camera/input integration. Phases
  3, 3.5, and 4 must merge before Babylon renders live state/events, and Phase 5 must merge before
  Babylon-specific effect capture is accepted as evidence.
- Phase 6 must prove dependency loading and default absence before Phase 6.5 creates an engine, and
  Phase 6.5 must prove lifecycle correctness before Phase 7 introduces assets. Phase 8's ownership
  tests must merge before Phase 9 creates shared fog textures or Phase 10.5 creates particles.
- Phases 9 and 9.5 are the fog/secrecy risk gate, but neither unlocks normal routes. Phase 10 proves
  current-entity and perspective interaction coverage; Phase 10.5 proves the representative
  overlay/effect spine and is the only route-unlock gate.
- Phase 11 establishes reproducible measurement and counter semantics before Phase 11.5 changes
  batching or pools. Phases 12 and 12.5 add vegetation and shadows separately before Phase 13
  admits a representative asset. Phase 13.5 blocks rather than completes if fog-edge capture,
  teardown, truthful fallback, or correctly reset metrics remain unresolved.
- After Phase 13.5 merges, perform a manual final review of the durable contract, parity ledger,
  benchmark evidence, and captures. Because worktree `target/` artifacts are disposable, regenerate
  the final fog-edge/representative-asset captures and all-scenario reports from current
  `origin/main` using the recorded commands before deciding. Planning and final review are not
  delegated to the phase runner. That review records `go`, `revise`, or `stop`; `revise` creates a
  new remediation plan from the archived evidence, while `go` may create a content/parity plan.

## Required Evidence Across the Plan

- Default Pixi launch with no Babylon module or dependency request.
- Semantic camera contract coverage plus a fake-perspective selection suite.
- One authoritative local scene for each graphics phase, captured with Lab Interact and inspected
  once by the implementing agent. The ledger records the capture manifest SHA-256 and exact
  reproduction command/sequence; image bytes remain ignored and disposable.
- Deterministic captures of a real short-lived event at specified visual-clock offsets.
- Fog-edge evidence covering visible, explored, unseen, remembered, and above-fog reveal cases.
- Backend diagnostics that distinguish current-frame from cumulative engine counters, emitted by
  `node scripts/rendering-benchmark.mjs --backend babylon --scenario <id> --output
  target/rendering-benchmarks/<id>.json` after Phase 11 creates that stable command.
- Reproducible `quiet`, `dense-placeholders`, `active-effects`, `fog-overlays`, `lifecycle`,
  `vegetation`, `vegetation-shadows`, and `representative-asset` scenarios, with viewport, DPR,
  browser/backend, settings, median/p95 frame time, and category counters.
- At least two enter/leave cycles in Phase 6.5, targeted repeated resource/effect cycles in Phases 8
  through 10.5, and a ten-cycle same-page lifecycle gate in Phase 13.5.
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
- No requirement to reproduce every Pixi overlay in Babylon during this plan; Phases 9/9.5 and
  10/10.5 prove the semantic fog/layer/interaction spine and leave explicit ledger work for future
  vertical slices.
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
`RTS_CLIENT_DIR` preview command or URL used, plus capture manifest SHA-256 and a reproduction
sequence suitable for rerunning after worktree cleanup. When an interactive remote check is useful,
include the verified Tailscale preview URL as an optional convenience.

Phase 13.5 is evidence-complete only when every mandatory gate passes. A failed gate is a structured
`blocked` result and the phase status remains not started; this prevents automatic archival from
hiding required remediation. The separate manual final review owns the product decision and may
create a new remediation plan from the archived evidence after a successful evidence merge.

After approval, executor passes may use the maintained phase runner with an explicit first phase:

    scripts/phase-runner.sh --plan render3d phase-0 --pr --wait

For a later serial range, remember that `--from` is exclusive; name the first phase explicitly
when inclusion matters.
