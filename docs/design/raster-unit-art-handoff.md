# Raster unit art handoff

Status: active production contract. PNG art is authoritative for the units listed below. Their old
SVG depictions, SVG fixtures, contact sheets, and vector fallback paths have been retired.

## Authority boundary

The client intentionally supports both asset types:

- Worker, Golem, Command Car, and Ekat are still SVG-authored units. Buildings are also
  SVG-authored. Keep the shared SVG schema, importer, animation sampler, Pixi runtime, and those
  source definitions.
- Rifleman, loaded Panzerfaust, Machine Gunner, Anti-Tank Gun, Mortar Team, Artillery, Scout Car,
  Scout Plane, and Tank are PNG-backed units. Do not add SVG depictions or SVG fallbacks for them.

PNG-backed units still need normalized rig data for anchors, selection and HP bounds, animation
bindings, semantic part routing, recoil, shadows, and native effects. That data lives in
`client/src/renderer/rigs/raster_rig_definitions.js`; it is not generated from or backed by SVG art.
The shared animation and Pixi routing machinery consumes both SVG-compiled definitions and
raster-native definitions.

A configured PNG unit must have its texture. Missing frame-strip or atlas textures fail closed and
surface a renderer error instead of silently displaying obsolete vector art.

## Production files

- `client/src/renderer/rigs/raster_rig_definitions.js` owns normalized metadata for PNG-backed
  units.
- `client/src/renderer/rigs/*_png_strip.js` and `frame_strip_runtime.js` own full-frame strips for
  Rifleman, loaded Panzerfaust, Machine Gunner, and Scout Plane.
- `client/src/renderer/rigs/*_png_atlas.js`, `png_routing.js`, and `png_runtime.js` own component
  atlases for Anti-Tank Gun, Mortar Team, Artillery, Scout Car, and Tank.
- `client/assets/rigs/<unit>/` keeps generated PNG sources, production derivatives, prompts, and
  manifests needed to understand or reproduce the checked-in raster art.
- `client/src/renderer/rigs/worker_svg.js`, `vehicle_svg.js`, and `building_svg.js` contain the
  SVG-authored units and buildings that remain supported.

The Tank's legacy vector contact-sheet pipeline is deliberately not part of the repository. Its
checked-in PNG atlas and explicit raster metadata are the production source.

## Raster art rules

- Generate team-colorable paint, uniform, and armor regions as weathered matte white or off-white.
  Keep outlines, panel seams, equipment, and neutral shading readable before tinting.
- Keep fixed materials such as rubber, dark weapon metal, wood, and skin in their intended colors.
  Magenta may be used only as a removable background key.
- Keep shadows out of generated unit sprites. Use the deterministic native shadow route.
- Keep independent moving assemblies in separate atlas sprites when recoil, setup, facing, or
  pivots differ. Full-frame strips are appropriate when component slicing does not add useful
  motion or tint control.
- Browser-facing textures use 8-bit PNG channels and neither dimension may exceed 2048 pixels.
  `node scripts/check-deploy-assets.mjs` enforces that deployment contract.
- Production metadata records explicit frame geometry, origins, pixels-per-unit or world scale,
  draw order, tint slots, and image versions. Do not infer these values from removed vector art.

## Current notable assets

- Tank uses the enabled pass-11 white-painted Tiger I hull, turret/coax, and separate barrel atlas.
  The track cells are intentionally transparent. Raster metadata preserves separate turret facing,
  barrel recoil and muzzle flashes.
- Anti-Tank Gun and Artillery use modular atlases so setup and weapon-recoil assemblies can move
  independently; Mortar Team uses its atlas only for weapon recoil.
- Scout Car uses a team-palette atlas selected by owner.
- Rifleman, loaded Panzerfaust, Machine Gunner, and Scout Plane use full-frame strips with explicit
  runtime scale, facing, animation-frame, and tint metadata.

## Validation

For changes to production raster rigs, run the focused contracts that cover the changed asset plus:

```bash
node tests/rig_schema.mjs
node tests/rig_runtime.mjs
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
node scripts/check-deploy-assets.mjs
```

Review raster changes at RTS scale in the Lab. A raster unit rendering without its PNG is a defect;
do not restore an SVG copy to conceal the texture or metadata failure.
