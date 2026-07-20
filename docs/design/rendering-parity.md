# Rendering parity and experiment ledger

This ledger records current capability and focused evidence. It is not a checklist requiring
Babylon to reproduce every Pixi feature before pre-alpha play.

## Status values

- `shared external` — intentionally outside the world renderer.
- `complete` — the backend supports the full capability named by the row.
- `partial` — a useful subset exists, but named gameplay cases are still absent.
- `representative` — one bounded real example validates the architecture.
- `placeholder` — truthful generic coverage, not visual parity.
- `missing` — not implemented.
- `deferred` — intentionally outside the current plan.

## Current capability

| Capability | Pixi | Babylon | Evidence / owner |
| --- | --- | --- | --- |
| One Match-owned rAF and visual clock | complete | complete | Babylon calls `scene.render()` only from the Match frame seam. |
| Semantic camera/projection | complete | complete | Fixed perspective and scene-camera coefficients share one snapshot. |
| Perspective-safe selection/marquee/ground hits | complete | complete | Shared `SelectionSceneV1` receives the selected camera projection. |
| Detached fog-filtered dynamic presentation | complete | complete | Both adapters read only structured-cloneable `PresentationFrameV2`; Pixi reconstructs private projection/grid helpers. |
| Babylon backend construction isolation | n/a | missing | Match currently passes Babylon the broad mutable Pixi-oriented source bag even though the adapter uses only its profiler hook. |
| Default Pixi and explicit lazy Babylon selector | complete | complete | `rtsRenderer=babylon` is live-player/Lab-only and lazy; replay/spectator matches stay Pixi. |
| Narrow detached static-map delivery | complete | missing | Babylon is passed a broad source bag containing a static-map callback but has no safe narrow contract and does not consume the data. |
| Render-worker message vocabulary | complete | n/a | `RenderWorkerMessageV1` separates initialization, map, durable, revisioned, frame, and control lifetimes; Phase 2 starts no production worker. |
| Worker-decodable Pixi assets | complete | n/a | Ground-decal SVG sources generate one checked-in PNG atlas; Pixi raster paths use OffscreenCanvas and fetch/createImageBitmap or Pixi Assets. |
| Map Editor detached Pixi boundary | complete | n/a | The viewport emits `MapEditorPresentationV1`; only the Pixi adapter owns display objects and renderer internals. |
| Terrain and resource-site readability | complete | missing | Babylon draws one flat map-bounds plane and does not present terrain classes or static resource sites. |
| Current/explored fog, memory, intel, and reveals | complete | complete | Revisioned grids, separated presentation layers, and two-recipient secrecy check. |
| Generic entity presence, team, selection, HP, and progress | complete | placeholder | Shared boxes and bars truthfully cover every received entity but are not kind-readable. |
| Existing PNG/WebP/sprite-sheet/SVG reuse as flat art | complete | missing | Playable catch-up prefers directly reusable checked-in art on Babylon billboards/planes, with primitive fallback. |
| Unit/building kind, facing, weapon, and setup readability | complete | missing | Babylon does not visibly distinguish most kinds and ignores weapon-facing/setup presentation. |
| Entity-backed ability, weapon, and fuel-status cues | complete | missing | Active auras, loaded/active states, and low-oil feedback have no Babylon representation. |
| Basic move/attack/build interaction feedback | complete | partial | Marquee, selected order lines, command/attack markers, and placement footprints work; rallies, ranges/arcs, and specialized target/setup previews are absent. |
| Trenches, smoke, and ability objects | complete | missing | These gameplay-significant frame records are currently ignored by Babylon. |
| Existing command, ability, and combat feedback catalog | complete | missing | Smoke, mortar, artillery, panzerfaust, muzzle, miss, resource, support-weapon, ability, and Lab feedback still render only in Pixi. |
| HUD, minimap, lobby, panels, audio, control groups | shared external | shared external | Existing shared surfaces. |
| New/re-authored art, full rig/animation parity, cosmetic decals, and observer/debug overlays | complete | deferred | Direct reuse of existing flat art is in scope; broader fidelity work waits unless playtest evidence shows a blocker. |
| Replay/spectator Babylon routes | complete | deferred | Future product need; these routes stay Pixi and must not load or depend on Babylon. |
| Benchmarks, pools, vegetation, shadows, quality tiers | n/a | deferred | Add only from measured need. |
| Babylon live/Lab default with explicit Pixi rollback | complete | missing | Final step after a real playtest and a no-selector live canary; selector/fallback checks must prove Pixi routes do not load Babylon. |
| Pixi removal | complete | deferred | Keep Pixi for rollback and replay/spectator until a later product decision. |

## Completed shared-boundary evidence

### `P1-camera`

`node tests/client_contracts/camera_projection_contracts.mjs` proves semantic camera operations,
CSS-pixel projection, nullable ground hits, snapshots, and Pixi orthographic equivalence.

### `P1.5-navigation-minimap` and `P1.75-shared-camera`

Minimap/input, audio, replay, Lab, control-group, and architecture contracts prove shared consumers
no longer rely on raw orthographic camera representation.

### `P2-perspective-selection`

`node tests/client_contracts/selection_projection_contracts.mjs` proves fog-filtered detached
selection proxies, fake-perspective click/marquee behavior, nullable ground hits, stable admission,
and mesh-independent targeting.

### `P3-presentation-frame` and `P3.5-pixi-cutover`

Presentation-frame, coordinator, Pixi-adapter, renderer-feedback, Lab capture, architecture, and
browser-smoke contracts prove one detached frame assembly, exact layers/grids, one `render(frame)`
call, private Pixi compatibility reads, revision-exact durable decal retention, bounded lifecycle
failure, and acknowledged-presented-frame selection publication.

### `P4-babylon-kernel`

Selector, fixed-perspective, coordinate, backend lifecycle, client architecture, and Lab browser
contracts prove lazy opt-in loading, shared scene/selection projection, centralized world/scene
conversion, Match-owned presentation, resize, bounded failure, and idempotent cleanup. Babylon
at that checkpoint drew only the authoritative map bounds and bounded visible generic primitives;
fog and playable feedback were intentionally left to Phase 5.

### `P5-fog-interaction-slice`

Babylon kernel and real two-recipient contracts prove revisioned current/explored fog, explicit
memory/intel/reveal categories, aggregate-only diagnostics, generic entity state, minimal command
feedback, and absence of a never-authorized sentinel from the presentation frame, scene, selection
candidates, and diagnostics. Shared perspective selection remains mesh-independent; live-player
and Lab routes are explicit while Pixi stays the default and owns replay/spectator rendering. This
proved the control and secrecy slice; it did not make terrain, entity identity, gameplay zones, or
the existing feedback catalog readable enough for a normal live-renderer cutover.

## Evidence policy

Each remaining phase records only the commands and inspected Lab capture that cover its central
risk. Manual review is appropriate for visual readability; automated checks remain required for
authority/secrecy, projection agreement, and loop/teardown ownership.

The active plan in `plans/render3d/` owns the flat-art-or-primitive playable catch-up and live-player
cutover. Missing or deferred rows outside that plan create no automatic phase, and visual fidelity
is not a cutover requirement.
