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

### Grass shader specialization (2026-11-23)

The selected grass technique is fixed for a layer's lifetime; changing it already rebuilds the
layer. Its procedural fragment shader now declares that technique as a compile-time constant,
allowing the compiler to simplify irrelevant style paths. Animation, clump density, artwork
coordinates, blending, and presentation cadence remain unchanged.

This is an explicitly accepted small rasterization change, not exact pixel parity. After reviewing
matching before/after/difference WebMs, the user authorized shipping the measured prototype.
The 180 consecutive canonical Hellhole frames reviewed at 4112×2408 changed 38–136 pixels per
frame (84.94 mean, at most 0.0014%); every changed RGB channel differed by exactly 1/255 and
alpha matched. Inputs, camera, DPR, animation time and ready assets were identical. Counts were
computed from original decoded RGBA, before video scaling/compression; the highlighted difference
video deliberately enlarged and brightened changes. This approval is specific to specialization,
not a general tolerance for future rendering changes.

A fresh 12-seeded-tick shipping comparison against `54322c704`, using the same full physical
buffer dimensions and fixed 120000 ms visual clock, changed 98–121 pixels/frame, maximum RGB
channel delta 1/255, with no alpha differences. Capture health, identical inputs, nonblank content
and changing-sequence checks passed; exact equality intentionally remains failed.

See [client performance lessons](../client-performance-optimization-lessons.md) for the measured
cost and limitations. Local diagnostic artifacts are not checked-in benchmark fixtures.

### Animated grass atlas (2026-11-23)

After reviewing synchronized scene, close-up and difference WebMs for three grass approaches, the
user selected the animated atlas for the default style 9. It replaces procedural neighboring
clump reconstruction with eight baked bending poses, a wind/cloud lookup and current-frame pose
blending. Terrain-relief lighting, tint, premultiplied opacity, grass-only coverage and every-frame
animation remain. Softer silhouettes, a repeating pattern and simpler clump shadows/light fronts
are intentional visual changes.

The reviewed 191-frame comparison against `1a4abeba8` used identical render histories, state,
camera, DPR, ready assets and animation time at 4112×2408 physical pixels. Atlas artwork changed
85.64–89.02% of image pixels (grass covers most of the view), with mean RGB MAE 9.732/255,
maximum channel delta 172 and no alpha changes. These are original decoded-RGBA measurements,
not video-compression differences or a perceptual-quality score. The user explicitly accepted
this atlas appearance; exact equality with the old procedural artwork is not claimed.

Shipping validation on base `88255f48f` compared the new default to the reviewed atlas prototype
at 12 seeded deterministic ticks, with identical sparse render histories and full physical-buffer
readback. Every decoded pixel matched, and asset/error/input/nonblank/changing-sequence checks
passed. Thus the implementation retains the selected prototype's appearance. A separate
SwiftShader/Vulkan check rendered all candidate approaches successfully; physical PC GPU
performance remains unmeasured.
