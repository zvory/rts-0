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
| One Match-owned rAF and visual clock | complete | missing | Babylon Phase 4. |
| Semantic camera/projection | complete | missing | Shared contract complete; Babylon perspective Phase 4. |
| Perspective-safe selection/marquee/ground hits | complete | missing | Shared `SelectionSceneV1`; Babylon projection use Phases 4–5. |
| Detached fog-filtered presentation frame | complete | missing | Shared/Pixi complete; Babylon consumption Phase 4. |
| Default Pixi and explicit lazy Babylon selector | complete | missing | Phase 4. |
| Static ground and generic scene | complete | missing | Phase 4 Lab kernel. |
| Current/explored fog, memory, intel, and reveals | complete | missing | Phase 5 plus two-recipient secrecy check. |
| Generic entities, selection/HP, basic command feedback | complete | missing | Phase 5 playable slice. |
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

## Evidence policy

Each remaining phase records only the commands and inspected Lab capture that cover its central
risk. Manual review is appropriate for visual readability; automated checks remain required for
authority/secrecy, projection agreement, and loop/teardown ownership.

After Phase 5, a real playtest decides whether Phase 6 remains the next priority. After Phase 6,
archive the plan. Missing/deferred rows create no automatic phase and do not authorize a default
switch.
