# Phase 0 - Contract and Current Anatomy Inventory

## Phase Status

Status: Done.

- [x] Done.

## Objective

Freeze the migration contract before adding schema, SVG parsing, or runtime rig code.

## Work Completed

- Inventoried the current procedural unit renderer in `client/src/renderer/units.js` and
  `client/src/renderer/shared.js`.
- Recorded the runtime renderer boundaries in `client/src/renderer/index.js`,
  `client/src/renderer/entities.js`, `client/src/state.js`, `client/unit-lab.js`, and
  `scripts/check-client-architecture.mjs`.
- Defined the approved normalized rig API, SVG authoring conventions, unsupported SVG behavior,
  equivalence sampling matrix, thresholds, and migration order for later phases.
- Confirmed this phase requires no runtime code, no wire protocol change, no balance change, and no
  client architecture allowlist change.

## Current Renderer Contract

The match renderer is Pixi-owned. `Renderer.render()` drives one world container from the camera,
draws entities into fixed layer containers, then overlays fog, feedback, placement, and drag
selection. Per-entity visuals are pooled by id through `_slot(poolName, id)` and hidden by `_sweep`;
unit rig work must preserve this pooling model and must not introduce live SVG DOM into the match
world.

Unit rendering currently uses two layers per entity: `unitShadows` for shadows and `units` for the
body. Selection rings and HP bars are separate pooled layers drawn after unit bodies. Shot-reveal
entities reuse `_drawUnit()` through alternate pools, then fade the reveal body and reveal shadow
above fog; rigged units must preserve that alternate-pool path and alpha behavior.

Renderer-local visual state currently lives in `_setupVisuals` and `_tankMotion`. `_setupVisuals`
tracks setup/teardown transition start times for deployed weapons. `_tankMotion` derives visual
track/wheel activity from interpolated position and facing deltas and adds owner-only oil cues for
Tanks. Rig animation may read normalized samples from these seams, but must not move authority for
movement, setup state, resources, or fog out of `GameState` and server snapshots.

`client/unit-lab.js` is a separate Canvas 2D preview for ignored JSON generation attempts. It sorts
simple shape records by numeric `layer`, draws rect/ellipse/triangle/barrel/track primitives, and
shows prompt plus animation notes. It is not the live renderer contract; later SVG preview work may
reuse the route, but core rig tests must not depend on local generation files.

## Unit Anatomy Inventory

Every rig should define semantic bounds for selection and HP placement, plus a `shadow` part or
shadow descriptor matching the current layer behavior.

| Unit kind | Current procedural anatomy that must become named rig parts |
| --- | --- |
| `worker` | Compact pentagonal body, pale facing tick, optional busy crown/chevron when `latchedNode` or building, circular shadow, infantry-style selection/HP bounds. |
| `rifleman` | Shared infantry torso polygon, head, shoulder/arm strokes, rifle stock-to-muzzle line, hand stroke, circular shadow, muzzle anchor at rifle tip. |
| `machine_gunner` | Shared infantry torso/head/arms, MG stock, receiver, receiver highlight, long shroud with perforation slots, deployable bipod legs, optional deployed muzzle cap, weapon muzzle anchor. |
| `anti_tank_gun` | Wheeled carriage axle, left/right gun tires, shield, shaded shield stripe, split trail legs and braces, barrel, breech, muzzle tick, setup/deploy geometry, circular shadow. |
| `mortar_team` | Axle, left/right tires, tow bar, bipod root and feet, tube base, cradle/body blocks, mortar tube, muzzle block, setup/deploy geometry, circular shadow. |
| `artillery` | Vehicle-scale circular shadow, split trails and deployed feet, axle, left/right tires, cradle, breech block, barrel, muzzle flash triangles/circles, recoil kick, carriage-vs-weapon facing split. |
| `scout_car` | Truck hull polygon, side running gear strips, cabin/body panels, hood/window marks, pintle mount, gunner torso/head/arms, vehicle MG stock/receiver/shroud/barrel, nose facing tick, vehicle shadow. |
| `command_car` | Truck hull polygon, side running gear strips, cabin and window panels, windshield line, nose facing tick, two command lamps/dots, breakthrough aura ring, vehicle shadow. |
| `tank` | Left/right tracks, per-side tread marks, hull polygon, inner hull shadow, nose/highlight panels, turret body, barrel, hull nose tick, fuel cue icon, vehicle shadow. |
| `ekat` | Currently falls through the default compact tool-carrying body path like Worker, with facing tick and unit shadow. Later Ekat-specific art must stay separate from Kriegsia unit assumptions. |

Shared helper anatomy in `shared.js` that should map to reusable rig parts or part primitives:
rotated rectangles, free rotated rectangles, rotated polygons, oriented capsule tires, track tread
segments, facing wedges, vehicle shadows, infantry base, gun tires, recoil vectors, and lighten/tint
variants.

## Animation Inputs and Renderer-Local State

The normalized animation sampler must accept these current inputs explicitly:

- `kind`, `id`, `owner`, `x`, `y`, `state`, `facing`, and optional `weaponFacing`.
- Setup state from `setupState` with `packed`, `setting_up`, `deployed`, and `tearing_down`, plus
  renderer-derived `prongFactor` and `barrel` from `_deployedWeaponSetupVisual()`.
- Recoil progress from `state.weaponRecoil(id, kind, now)`, converted through
  `weaponRecoilOffset(kind, progress)`.
- Vehicle motion from `_tankMotionVisual()`: `leftPhase`, `rightPhase`, per-side direction,
  `activity`, `lowOil`, and `oilStarved`.
- Owner tint from `_ownerColors()` / `_tintFor()` and player palette fallback.
- Worker busy state from `latchedNode` or `state === build`.
- Command Car `breakthroughTicks` for the aura ring.
- Shot-reveal lifetime fields `shotRevealCreatedAt` and `shotRevealExpiresAt` for alternate layer
  alpha.
- Resource context for Tank oil cues through `state.resources.oil`.
- Fog and visibility remain outside the rig definition; rigs receive only already-visible or
  shot-reveal entity views from the existing renderer.

## Normalized Rig API and Ownership Boundaries

Later phases should implement the APIs already named in the overall plan with these ownership
rules:

- `validateRigDefinition(definition)` is pure, returns normalized data or structured errors, and
  never creates Pixi objects.
- `compileSvgRig(svgText, metadata)` parses authoring SVG into the normalized definition and never
  creates Pixi objects.
- `sampleRigAnimation(definition, entity, renderContext)` is pure enough for unit tests. It returns
  per-part transform, alpha, tint-slot, and visibility samples plus semantic anchors.
- `createUnitRigInstance(kind, definition, pixiFactory)` owns the Pixi container and one child per
  compiled part. It does not read global renderer state.
- `UnitRigInstance.update(entity, renderContext)` applies sampled transforms, tint, alpha, and
  visibility. It may receive renderer-local setup/motion samples in `renderContext`, but may not
  mutate `GameState`, camera, fog, selection, or protocol objects.
- `renderContext` is narrow: `{ now, teamColor, recoilProgress, recoilPx, setupVisual,
  vehicleMotion, selected, damaged, shotRevealAlpha, visibility, mapTileSize }` plus unit-kind
  fields already present in entity snapshots.
- The live renderer chooses legacy or rigged drawing per kind behind a temporary migration switch
  until Phase 8. The selection ring, HP bar, fog, feedback, and command overlays remain outside the
  unit rig instance.

## SVG Authoring Conventions

Approved SVG source files must be deterministic authoring inputs, not unchecked runtime assets.

- Root `<svg>` must define `viewBox`, `data-rts-rig-kind`, `data-rts-rig-version`, and
  `data-rts-origin="center"`. Unit-local coordinates use world pixels with `(0,0)` at the entity
  origin and positive x as forward for the unrotated authored pose.
- Semantic parts are named with stable ids such as `part.body`, `part.head`, `part.rifle`,
  `part.mg.receiver`, `part.mg.shroud`, `part.bipod.left`, `part.track.left`, `part.track.right`,
  `part.turret`, `part.barrel`, `part.wheel.left`, `part.wheel.right`, `part.fuelCue`,
  `part.busy`, `part.breakthroughRing`, and `part.shadow`.
- Anchors use ids under `anchor.*`: at minimum `anchor.origin`, `anchor.selection`, and
  `anchor.hp`; weapon units also need `anchor.muzzle`, and support weapons need carriage or bipod
  anchors as appropriate.
- Bounds use ids under `bounds.*`: `bounds.selection`, `bounds.hp`, and optional
  `bounds.pixelTolerance` for test-only expected extents.
- Tintable geometry must declare `data-rts-tint="team"`, `data-rts-tint="team-light"`, or
  `data-rts-tint="neutral"`. Literal fills are allowed only for fixed material colors such as
  shadow, tires, barrel metal, highlights, and muzzle flash.
- Allowed geometry in Phase 3 importer: `<g>`, `<path>` with normalized absolute path commands,
  `<polygon>`, `<polyline>`, `<rect>`, `<circle>`, `<ellipse>`, `<line>`, and `<metadata>`.
- Allowed transforms: `translate`, `rotate`, `scale`, and matrix transforms only when finite and
  decomposable by the importer. Nested transforms must be flattened into normalized part-local
  transforms.
- Unsupported features fail closed with structured errors: scripts, foreignObject, external hrefs,
  filters, masks, clip paths, gradients, patterns, CSS animations, percentage units, non-finite
  values, duplicate ids, missing required anchors, and unknown required metadata.

## Equivalence Spec

The temporary harness should compare legacy and rigged output while both renderers exist. It should
prefer semantic measurements and bounded pixel diffs rather than brittle full screenshots.

Sampling matrix:

- Facings: `0`, `PI/2`, `PI`, `3PI/2`.
- Weapon facings for split-facing units: same as hull, `+PI/4`, `-PI/2`, and `PI`.
- Recoil progress: `0`, `0.35`, `1`.
- Setup states for MG, AT gun, Mortar, and Artillery: packed, setting up at 50%, deployed, tearing
  down at 50%.
- Vehicle motion: idle, forward, reverse, pivot left, pivot right, with tread phases fixed to
  deterministic values.
- Team colors: first palette color and a high-contrast alternate palette color.
- Visibility paths: normal visible unit and shot reveal at fresh, mid-life, and fade-out alpha.
- Unit-specific samples: Worker busy on/off, Tank normal/low-oil/oil-starved, Command Car
  breakthrough ring on/off.

Semantic measurements:

- Part anchor positions for muzzle, origin, selection center, HP anchor, track centers, wheel
  centers, deployed feet, turret center, barrel tip, fuel cue, busy marker, and breakthrough ring.
- Overall local bounds, shadow bounds, selection ring geometry, HP-bar top reference, and draw-layer
  membership.
- Per-part transform values after animation sampling, including hull-vs-weapon rotation and recoil
  offsets.

Pixel thresholds for Phase 1 baselines and later rig comparison:

- Deterministic viewport: 256x256 world-pixel fixture per unit, camera zoom `1`, device pixel ratio
  `1`, `PIXI.settings.SCALE_MODE = NEAREST`, antialias disabled, transparent background.
- Static non-recoil poses: at least `98.5%` identical alpha-weighted pixels within the measured
  unit bounds, with no unmatched opaque cluster larger than `12` pixels.
- Animated/recoil/deploy poses: at least `96%` identical alpha-weighted pixels within the measured
  unit bounds, with no unmatched opaque cluster larger than `24` pixels.
- Alpha drift: per-pixel alpha may differ by up to `0.04` for shadows and shot-reveal fades, and
  up to `0.02` for opaque body parts.
- Semantic anchors are stricter than pixels: muzzle, HP, selection, wheel, track, and deployed-foot
  anchors must be within `1.5` world px, and selection/HP bounds within `2` world px.
- A migrated rig may differ from legacy only when the phase author records a deliberate art
  correction in that phase, keeps the semantic anchors within tolerance, and updates the temporary
  baseline in the same commit.

## Migration Order

Use the plan order unless a later phase discovers a blocker:

1. Worker/Engineer first, because it exercises body tint, facing tick, busy state, shadow, selection,
   and HP placement without weapon-facing separation.
2. Rifleman, then Machine Gunner, to validate shared infantry parts before deployable support
   weapons.
3. Anti-Tank Gun, Mortar Team, and Artillery, to validate setup/deploy animation, carriage geometry,
   recoil, and muzzle anchors.
4. Scout Car and Command Car, to validate vehicle body rigs, wheeled/running gear, gunner parts,
   and breakthrough ring.
5. Tank, to validate tracks, turret-vs-hull facing, tread motion, recoil, and fuel cues.
6. Ekat-specific art only after the active-ability visual contract is stable; until then, preserve
   the current fallback-body behavior.

## Phase Split Notes

No plan split or order change is required. Later phases should keep the unit-lab preview optional
and avoid changing wire protocol, balance mirrors, or server authority for this SVG migration.

## Implementation Checklist

- [x] Inventory unit kinds and procedural visual subparts.
- [x] Inventory animation inputs and renderer-local visual state.
- [x] Define SVG authoring conventions and unsupported SVG behavior.
- [x] Define normalized rig API and ownership boundaries.
- [x] Define equivalence sampling matrix and thresholds.
- [x] Record any phase split or order changes needed before implementation.

## Verification

- Docs-only phase.
- Run `git diff --check`.

## Manual Test Focus

None required. This phase does not change runtime behavior.

## Handoff Expectations

Next executor should start Phase 1 by building the temporary legacy visual oracle from this
inventory. The oracle should exercise the sampling matrix above against current procedural drawing
before any rig renderer exists.
