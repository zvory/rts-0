# Machine Gunner Pass 01

This pass replaces the authored SVG Machine Gunner body/weapon with a generated PNG frame strip.
The carry frames were edited from the accepted rifleman PNG sheet, then a single carry frame was
used as the reference for the deploy-only generation.

## Runtime Behavior

- Frames 0-5 are movement frames: the gunner carries an oversized MG42-style weapon across the
  body with a bulkier pack. Runtime rotates only these movement frames 90 degrees left and renders
  them at the smaller `movementWorldScale`.
- Frames 6-11 are setup/deployed frames: setup starts from a carry pose, then rotates the MG into a
  south-facing deployed pose with the bipod open.
- The frame-strip renderer maps `setting_up` and `tearing_down` progress onto frames 6-11. A fully
  `deployed` Machine Gunner holds frame 11.
- Frames 12-14 are a no-muzzle-flare firing recoil strip. They are transformed to 85% scale and
  shifted north before the production strip is downsampled to 64x64 RGBA8 cells.
- The deployed art points down/south in the sheet, so runtime setup/deployed rendering applies a
  `PI/2` forward-angle offset to align that sprite direction with authoritative `weaponFacing`.

## Source Notes

- Keep this no-feet top-down convention for future infantry passes. Earlier attempts drifted into
  front-facing boots and standing character art.
- The setup pass works better when generated separately from movement. Do not ask for movement,
  setup, and final deployed frames in one prompt unless there is a stronger reference sheet.
- For firing recoil, keep the first and last frame visually aligned with frame 11 so the deployed
  idle pose does not pop when the recoil clip starts or finishes.

## Pass 02 White Clothing Recolor

Pass 02 sends the carry, setup/deploy, and firing/recoil sheets through built-in image generation
independently. The model output is used as a semantic guide for the clothing and backpack/bedroll
material only. `scripts/art/machine-gunner-white-pipeline.mjs` maps that guide back onto the compact
15-frame runtime strip and writes an explicit approved recolor mask.

The original pass-01 compact strip is retained as `generated/machine-gunner-pass-01-prewhite-strip.png`.
The pipeline asserts that the output alpha is byte-identical, that every RGB byte outside the
approved mask is unchanged, and that no protected weapon pixel changes. This keeps the generated
recolor away from the gun and prevents silhouette or exterior-edge drift.

## Pass 03 White Material Repaint

Pass 03 expands the ImageGen-guided repaint from isolated clothing/backpack patches to the full
tintable soldier surface, including the helmet and the visible pack/bedroll material in every carry,
deployment, and recoil frame. The deterministic runtime rebuild still starts from the pass-01 strip:
the 960×64 frame sheet and every alpha byte remain unchanged, and weapon pixels stay protected.

## Pass 04 Registered White Color Transfer

Pass 04 tested selective registered color transfer and was rejected because mixing generated
material patches with the old compact sprite still behaved like a separate recolor layer.

## Pass 05 Whole-Frame White Pipeline

Pass 05 follows the Rifleman pipeline: each visible runtime pixel samples the corresponding
accepted ImageGen frame, so the body, shading, linework, and weapon form one coherent white-source
sprite. The old pass-01 sprite contributes only the exact 960×64 frame layout and byte-identical
alpha channel. The client then applies the same 70% brightness target as Rifleman and Rifleman
Panzerfaust.

## Pass 06 Rifleman-Resolution Runtime Strip

Pass 06 samples the high-resolution source alpha and accepted ImageGen color directly into 160×160
runtime cells instead of routing them through the old 64×64 strip. The deployed scale changes from
0.84 to 0.336 and the movement scale changes from 0.612 to 0.2448. These proportional changes keep
the exact full-canvas world extents (`64 × 0.84 = 160 × 0.336` and
`64 × 0.612 = 160 × 0.2448`) while providing the same 160-pixel linear source resolution used by
Rifleman. Known detached crop fragments are removed from setup frames 7 and 8.
