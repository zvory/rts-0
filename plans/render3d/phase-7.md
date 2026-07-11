# Phase 7 - Coordinates and GLB Asset Contract

## Phase Status

- [ ] Not started.

## Depends On

- Phase 6 merged with the lazy production kernel and pinned self-hosted glTF loader.

## Objective

Establish the coordinate and asset rules before real Babylon content exists. Centralize every
world/scene conversion and define a machine-validated GLB contract for visual structure, anchors,
materials, animation, provenance, and budgets. Use minimal validator fixtures only; gameplay
selection remains Phase 2 plain rules/presentation data and the representative asset remains Phase 13.

## Work

- Add the only server-world-to-Babylon coordinate module, owning world `(x,y)` pixels to scene
  ground `(x,0,z)`, inverse ground conversion, scale, height, handedness, hull/weapon facing,
  direction vectors, attachment transforms, and angle wrapping.
- Keep public projection/picking in viewport-local CSS pixels; the Babylon adapter alone handles DPR,
  canvas backing dimensions, engine hardware scaling, and render-buffer projection.
- Cover canonical points/directions, arbitrary facing, round trips, map corners, negative
  intermediates, attachments, and tolerances. Entity, terrain, effect, input, asset, and shadow
  modules import this conversion; add a ratchet against local swaps/signs/scale constants.
- Define a manifest/schema covering asset id/version, source/license/provenance, authoring/runtime
  scale, up/forward axes, handedness, ground pivot, visible bounds, visual selection/HP/turret/
  muzzle/exhaust/effect anchors, articulated parts, visible/shadow/LOD node roles, team material
  slots, texture ownership declarations, clips/looping/root-motion prohibition, and triangle/
  material/texture/bone/draw-part/shadow budgets.
- State explicitly that asset bounds/anchors are visual placement metadata only. Gameplay click,
  marquee, command targeting, and selectability use the Phase 2 semantic proxies and cannot change
  when a GLB, LOD, pivot, shadow proxy, or malformed mesh changes.
- Add a Node validator inspecting manifest and GLB structure with actionable paths. Include tiny
  valid/malformed fixtures for missing visual anchors, axes/pivot, material slots, parts/clips,
  undeclared textures, provenance, and budgets; fixtures need not be attractive or articulated
  gameplay models.
- Load only the validator fixture through the production loader to prove served paths/checksum/
  plugin configuration and fallback diagnostics. Do not create a faction or representative unit.
- Use `lab-interact` with explicit `RTS_CLIENT_DIR` to capture the minimal valid fixture at canonical
  facing/scale once and inspect the returned PNG; the artifact is pipeline evidence, not an art review.
- Update architecture/suite selection, `docs/design/client-rendering.md`, and
  `docs/design/rendering-parity.md` with actual conventions and evidence.

## Expected Touch Points

- `client/src/renderer_babylon/coordinates.js`
- `client/src/renderer_babylon/assets/` loader/manifest helpers
- checked-in GLB manifest/schema and minimal valid/malformed fixtures
- `scripts/validate-rendering-assets.mjs`
- `tests/client_contracts/babylon_coordinate_contracts.mjs`
- `tests/client_contracts/babylon_asset_contracts.mjs`
- client architecture and suite-selection rules
- durable rendering docs/parity ledger
- `plans/render3d/phase-7.md` status update in the implementation commit

## Contract Requirements

- One conversion module owns point, direction, height, scale, hull/weapon facing, and attachment math.
- Round trips state tolerances and never silently clamp invalid input.
- Asset metadata cannot participate in gameplay selection or authority.
- Required versus optional visual anchors/nodes are explicit; malformed required data falls back
  with bounded diagnostics rather than terminating the frame.
- Core and glTF runtime files continue through Phase 6's pinned loader/manifest; no second loader path.

## Explicit Exclusions

- No resource registry beyond loader fixture cleanup; Phase 8 owns it.
- No representative/faction model, AI-generated asset workflow, finished art, compression/decoder,
  broad animation system, or runtime LOD policy.
- No fog, effects, shadows, batching, or gameplay mesh picking.

## Implementation Checklist

- [ ] Centralize/test all world/scene conversion and add a local-conversion ratchet.
- [ ] Define GLB manifest/schema with provenance, visual anchors/roles, and budgets.
- [ ] Explicitly prohibit asset-authored gameplay selection geometry.
- [ ] Add actionable validator and minimal valid/malformed fixtures.
- [ ] Load the minimal fixture through the production path and test fallback diagnostics.
- [ ] Capture/inspect the minimal valid fixture through Lab Interact.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_coordinate_contracts.mjs
    node tests/client_contracts/babylon_asset_contracts.mjs
    node scripts/validate-rendering-assets.mjs --all
    node scripts/check-client-architecture.mjs
    node tests/select-suites.mjs --verify
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Inspect the minimal fixture at canonical map positions and arbitrary hull/attachment facings, at DPR
1 and a non-1 DPR/resized viewport. Force malformed/missing manifests/assets and verify actionable
fallback/readiness diagnostics with selection behavior unchanged; report the inspected artifact path.

## Handoff Expectations

Report final scale/axis/handedness/facing conventions, CSS/render-buffer handling, schema/validator
paths, runtime loader path, fixtures, fallback behavior, and selection-geometry prohibition. Include
the exact preview command/URL and inspected artifact. Name Phase 8 as next and identify backend/
shared/entity/effect/pool ownership, loader-container lifetime, late generation completion, double
release, and shared particle-texture survival.
