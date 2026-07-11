# Phase 1 - Semantic Camera and Projection Core

## Phase Status

- [ ] Not started.

## Depends On

- Phase 0 merged and its rendering contract/parity ledger available from `origin/main`.

## Objective

Implement the renderer-neutral camera/projection core without migrating the application-wide
consumer set yet. Preserve current Pixi camera behavior and legacy view restore while defining the
plain-data semantics a fixed elevated perspective adapter will later implement. Keep this phase
bounded to the camera, pure helpers, contracts, and durable documentation.

## Work

- Implement the Phase 0 semantic contract with plain `{x,y,heightPx}` presentation points and
  operations for nullable screen-to-ground projection, world-point projection with depth/clip
  state, projected extent/screen scale,
  viewport ground polygon and conservative world bounds, projected containment, fit/focus, pan,
  anchor-aware zoom/dolly, resize/map bounds, semantic snapshot/restore, and audio listener data.
- Preserve current world-pixel focus and zoom as private orthographic state. Existing consumers may
  remain temporarily on the named compatibility edge, and legacy persisted `{x,y,zoom}` may be
  accepted there, but no new raw consumer is allowed.
- Define every public screen coordinate and extent in viewport-local CSS pixels. DPR, canvas
  backing dimensions, DOM offsets, and future hardware scaling remain adapter-private.
- Make perspective ground/frustum intersection semantics explicit: clip to map bounds, return
  deduplicated stable clockwise world-`(x,y)` winding, return an empty polygon when no bounded
  ground is visible, and return no conservative bounds for an empty polygon.
- Define semantic snapshot/restore in player-intent terms—world focus, framing scale, and bounded
  view policy—without serializing matrices or backend nodes. Keep pitch, yaw, height, and FOV as
  future adapter configuration, not player orbit state.
- Add pure fake adapters and orthographic equivalence tests covering round trips, finite-value
  rejection, clipping, partial misses, empty ground views, polygon/bounds, anchored zoom, pan,
  clamping, fit/focus, snapshot, resize, and listener data.
- Update `docs/design/client-rendering.md` and `docs/design/rendering-parity.md` with final method
  names, units, nullability, compatibility lifetime, and Phase 1 evidence.

## Expected Touch Points

- `client/src/camera.js`
- small focused projection/view helpers next to the camera
- `client/src/camera_view_selection.js` only for semantic snapshot/fit primitives
- `tests/client_contracts/camera_projection_contracts.mjs` (create it in this phase)
- existing camera/fog contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-1.md` status update in the implementation commit

## Contract Requirements

- `groundAtScreen` or its final equivalent may return no hit; the orthographic adapter happens to
  hit, but the public contract never promises that.
- World projection reports viewport-local CSS position, positive/negative depth, and clip/visibility
  state. A bare `{x,y}` result is insufficient.
- Positive `heightPx` projects a presentation-only elevated anchor; ground commands always use
  `heightPx=0` and return authoritative world `(x,y)` only.
- `viewportGroundPolygon` is public polygon semantics. Its conservative AABB is used only where
  false-positive inclusion is explicitly acceptable.
- Projected extent/overlay sizing is independent of nominal zoom and supports constant-screen-size
  labels and selection/HP presentation.
- Raw orthographic transform state remains temporarily available only through the documented camera
  compatibility edge consumed by Phase 1.5 and Pixi.

## Explicit Exclusions

- No application-wide consumer migration; Phase 1.5 owns it.
- No Babylon dependency, backend, scene, engine, ray class, or matrix object.
- No perspective visual change, selection rewrite, protocol change, or replay-format change.

## Implementation Checklist

- [ ] Implement the semantic camera/projection core with orthographic equivalence.
- [ ] Define CSS-pixel, nullability, clipping, polygon, fit/focus, snapshot, and listener semantics.
- [ ] Preserve and document the temporary raw orthographic compatibility edge.
- [ ] Add real-orthographic and fake-perspective pure contracts.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/camera_fog_contracts.mjs
    node tests/client_contracts/camera_projection_contracts.mjs
    node scripts/check-client-architecture.mjs
    git diff --check

## Manual Test Focus

Run a normal Pixi match and exercise basic pan, zoom, resize, and camera restore to confirm the core
adapter did not change visible behavior. Review the fake-perspective contract output for nullable
ground hits, clipped/behind-camera points, partial ground views, and stable polygon winding; broad
consumer behavior remains Phase 1.5.

## Handoff Expectations

List final semantic method names, units, nullability, fake-adapter coverage, snapshot shape, and the
temporary compatibility reads. Name Phase 1.5 as next and identify the exact raw camera consumers
it must migrate without changing behavior.
