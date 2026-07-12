# Rendering parity and experiment ledger

This ledger records current capability and focused evidence. It is not a checklist requiring
Babylon to reproduce every Pixi feature before pre-alpha play.

## Status values

- `shared external` — intentionally outside the world renderer.
- `complete` — the backend supports the full capability named by the row.
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
| Detached fog-filtered presentation frame | complete | complete | Babylon reads only `PresentationFrameV1`. |
| Default Pixi and explicit lazy Babylon selector | complete | complete | `rtsRenderer=babylon` is live-player/Lab-only and lazy; replay/spectator matches stay Pixi. |
| Static ground and generic scene | complete | representative | Phase 4 authoritative Lab kernel capture. |
| Current/explored fog, memory, intel, and reveals | complete | complete | Revisioned grids, separated presentation layers, and two-recipient secrecy check. |
| Generic entities, selection/HP, basic command feedback | complete | placeholder | Shared generic geometry/materials plus selection, bars, marquee, move/target, and placement cues. |
| HUD, minimap, lobby, panels, audio, control groups | shared external | shared external | Existing shared surfaces. |
| One representative trusted asset | complete | missing | Phase 6 representative, not catalog parity. |
| One normalized finite attack effect | placeholder | missing | Phase 6 representative event spine. |
| Long-tail effects/overlays and faction art | complete | deferred | Future playtest-driven content plans. |
| Replay/spectator Babylon routes | complete | deferred | Future product need. |
| Benchmarks, pools, vegetation, shadows, quality tiers | n/a | deferred | Add only from measured need. |
| Babylon default / Pixi removal | complete | deferred | Separate reviewed rollout decision. |

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

Presentation-frame, Pixi-adapter, renderer-feedback, Lab capture, architecture, and browser-smoke
contracts prove one detached frame assembly, exact layers/grids, one `render(frame)` call, private
Pixi compatibility reads, non-destructive decal reconciliation, bounded render failure, and
successful-frame selection publication.

### `P4-babylon-kernel`

Selector, fixed-perspective, coordinate, backend lifecycle, client architecture, and Lab browser
contracts prove lazy opt-in loading, shared scene/selection projection, centralized world/scene
conversion, Match-owned presentation, resize, bounded failure, and idempotent cleanup. Babylon
at that checkpoint drew only the authoritative map bounds and bounded visible generic primitives;
fog and playable feedback were intentionally left to Phase 5.

### `P5-playable-fog-interaction`

Babylon kernel and real two-recipient contracts prove revisioned current/explored fog, explicit
memory/intel/reveal categories, aggregate-only diagnostics, generic entity state, minimal command
feedback, and absence of a never-authorized sentinel from the presentation frame, scene, selection
candidates, and diagnostics. Shared perspective selection remains mesh-independent; live-player
and Lab routes are explicit while Pixi stays the default and owns replay/spectator rendering.

## Evidence policy

Each remaining phase records only the commands and inspected Lab capture that cover its central
risk. Manual review is appropriate for visual readability; automated checks remain required for
authority/secrecy, projection agreement, and loop/teardown ownership.

After Phase 5, a real playtest decides whether Phase 6 remains the next priority. After Phase 6,
archive the plan. Missing/deferred rows create no automatic phase and do not authorize a default
switch.
