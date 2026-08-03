# Rendering decision and experiment ledger

## Current decision

As of 2026-08-02, PixiJS is the sole shipped world renderer. The Babylon.js experiment is
abandoned and its runtime, dependency loader, renderer selector, Interact options, styles, and
dedicated tests have been removed.

The attempted catch-up was determined to be hopeless at a reasonable scope: Babylon remained far
behind the gameplay readability, effects, asset routing, and tooling already present in Pixi, and
closing that gap would consume disproportionate effort without improving the authoritative 2D game.
Maintaining a second 3D engine would also put the client into ongoing optimization hell, adding
frame-time and input-latency risk precisely where responsiveness matters most and making acceptable
performance on terrible laptops substantially harder. Pixi is sufficient for this game's visual
and interaction needs, so future renderer work should improve the Pixi worker path instead of
reviving a parallel 3D backend.

The renderer-neutral presentation, selection, and camera seams remain because they enforce fog and
input authority, make worker ownership explicit, and keep Pixi testable. Their continued existence
is not a commitment to another renderer.

## Current capability

| Capability | Status | Evidence / owner |
| --- | --- | --- |
| Sole Pixi module-worker path | complete | Live, replay, spectator, Lab, fixed capture, stress, and Map Editor use the same WebGL-only worker host. |
| One Match-owned rAF and visual clock | complete | The worker presents only submitted frames and never owns gameplay timing. |
| Semantic camera and mesh-independent selection | complete | Camera projection and `SelectionSceneV1` keep input independent of display objects. |
| Detached fog-filtered presentation | complete | `PresentationFrameV2` carries only admitted, structured-cloneable layers and grids. |
| Worker-decodable assets and deterministic capture | complete | Asset readiness, worker lifecycle contracts, and exact sampled-tick comparisons cover the production path. |

## Retained historical evidence

### `P1-camera` through `P3.5-pixi-cutover`

Camera, minimap/input, selection, presentation-frame, coordinator, Pixi-adapter, renderer-feedback,
Lab capture, architecture, and browser-smoke contracts established the semantic camera,
mesh-independent input, detached presentation, bounded lifecycle failure, and
acknowledged-presented-frame selection publication now used by Pixi.

### `renderworker-phase-3`

Worker-host lifecycle/message contracts, route browser checks, the architecture ratchet, independent
main/worker CPU profiles, the canonical Hellhole stream, and deterministic decoded-RGBA samples
proved the atomic Pixi cutover. The host bounds work to one in-flight plus one latest pending frame,
keeps decals independently durable, publishes selection only for the acknowledged frame, orders
resize/reset/teardown, and reads fixed-capture pixels in the presenting worker task.

### Retired Babylon checkpoints

The archived `P4-babylon-kernel` and `P5-fog-interaction-slice` work proved that an opt-in 3D
backend could consume detached, fog-filtered presentation and share semantic selection without
becoming gameplay authority. It still rendered only generic primitives and a small feedback slice;
terrain, entity identity, production art, gameplay zones, effects, tooling, and performance parity
remained far behind Pixi. Those checkpoints are historical architectural evidence only, not a
baseline, compatibility promise, or invitation to restore the removed engine.

The abandoned catch-up plan is preserved under `plans/archive/render3d/` with the decision record.
