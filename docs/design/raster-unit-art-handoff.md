# Raster unit art handoff

Status: experimental handoff only. The checked-in tank PNG rig path is deliberately disabled, and
the current generated images are not final game art. This note records what worked, what failed, and
how to reproduce the next experiment without rediscovering the same traps.

## Goal

The intended workflow is:

1. Keep authoring unit rigs as SVG, because the SVG rig already carries anchors, part ids, draw
   order, tint slots, and animation bindings.
2. Render one contact sheet containing the complete unit plus every component needed by the runtime
   rig.
3. Feed that single sheet to an image generation pass so all components are restyled together.
4. Slice the output back into a PNG atlas and metadata.
5. Let Pixi render the PNG components while the SVG rig remains authoritative for animation,
   pivots, recoil, facing, selection anchors, and route split.

The current tank prototype proves the shape of this pipeline but does not yet produce an acceptable
asset. The imagegen outputs still fail component consistency and alignment checks, so the atlas must
remain inactive until a later pass fixes those problems.

## Current files

- `scripts/art/tank-raster-pipeline.mjs` builds the tank contact sheet, writes prompts, and writes
  PNG atlas metadata.
- `client/assets/rigs/tank-ps1/tank-contact-sheet.svg` and
  `client/assets/rigs/tank-ps1/tank-contact-sheet.png` are the current semantic source sheet.
- `client/assets/rigs/tank-ps1/metadata/source-grid.json` records the cell order, source parts, and
  semantic sprite grouping.
- `client/assets/rigs/tank-ps1/metadata/prompt*.md` records the base prompt and the four Tiger I
  prompt iterations.
- `client/assets/rigs/tank-ps1/generated/` keeps generated candidates and alpha-converted copies.
- `client/assets/rigs/tank-ps1/tank-atlas.png` is currently just the disabled semantic source atlas,
  not a final generated art pass.
- `client/src/renderer/rigs/tank_png_atlas.js` is generated metadata. Its `enabled` field is
  currently `false`.
- `client/src/renderer/rigs/png_runtime.js` and `png_routing.js` are the disabled runtime path that
  can render atlas sprites in place of SVG pixels when an atlas is enabled and loaded.

## Current semantic sheet

The current tank sheet uses a 3x2 grid:

1. `reference.full` - assembled tank reference, with the SVG drop shadow and fuel cue removed.
2. `sprite.track.left` - left track assembly, including all left tread blocks.
3. `sprite.track.right` - right track assembly, including all right tread blocks.
4. `sprite.hull` - hull, nose, hull shading parts, and nose tick.
5. `sprite.turret` - turret, main barrel, and coax barrel.
6. `sprite.fuelCue` - the fuel warning cue.

The SVG is annotated with stable `part.*` ids, but those ids are not enough for image generation.
Many individual SVG parts are rectangles, lines, or tiny treads with no independent semantic
meaning. The important lesson is that each unit needs a semantic grouping layer before imagegen:
turret assembly, hull assembly, left track assembly, right track assembly, weapon assembly, crew
assembly, and so on. The grouping can be derived from existing ids for simple rigs, but it cannot be
assumed from raw SVG layers alone.

## Reproduce the current prototype

Requirements:

- Run from the repo root.
- ImageMagick must provide `magick`.
- The image generation step is manual: give the generated sheet and the prompt to the image model,
  then save the returned PNG under `client/assets/rigs/tank-ps1/generated/`.

Generate the semantic source sheet:

```bash
node scripts/art/tank-raster-pipeline.mjs make-sheet \
  --scale 3 \
  --columns 3 \
  --layout tight \
  --profile semantic
```

Refresh the base prompt:

```bash
node scripts/art/tank-raster-pipeline.mjs write-prompt
```

Use `client/assets/rigs/tank-ps1/tank-contact-sheet.png` as the input image. Start from the latest
short prompt direction rather than the most detailed prompt: strict top-down Tiger I, no shadows,
very simple low-end 3D raster rendering, anti-aliased raster shapes, not pixel art, and much less
detail than concept art.

Save each generated candidate with a pass number, for example:

```text
client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-05.png
client/assets/rigs/tank-ps1/metadata/prompt-tiger-i-pass-05.md
client/assets/rigs/tank-ps1/metadata/tiger-i-pass-05.json
```

Convert the chroma-key background to alpha before atlas wiring:

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/imagegen/scripts/remove_chroma_key.py" \
  --input client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-05.png \
  --out client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-05-alpha.png \
  --auto-key border \
  --soft-matte \
  --transparent-threshold 12 \
  --opaque-threshold 220 \
  --despill
```

Write atlas metadata disabled while evaluating:

```bash
node scripts/art/tank-raster-pipeline.mjs write-atlas \
  --sheet client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-05-alpha.png \
  --columns 3 \
  --layout tight \
  --profile semantic \
  --disabled \
  --model "built-in image generation" \
  --notes "Candidate Tiger I raster pass; disabled pending alignment and component consistency review."
```

Only omit `--disabled` for a local experiment after the validation checklist passes. Do not commit
an enabled atlas unless the component cells can actually reconstruct the tank.

## Prompt lessons

Use concise prompts. Over-specified realism prompts pushed the model toward tiny hatches, grilles,
scratches, bolts, and independent redesigns. The better direction is:

- strict top-down orthographic view
- Tiger I silhouette first
- long rectangular hull
- wide straight parallel tracks
- square flat-sided turret
- long centered barrel
- very early low-end 3D raster graphics
- anti-aliased raster shapes, not pixel art
- only a few broad shade values per part
- no shadows of any kind
- no labels, text, insignia, loose parts, or extra cells
- preserve grid, cell order, centers, scale, and orientation

The prompt must say that the complete tank cell is a reference only. Runtime truth should come from
the component cells. A generated complete tank that does not match the generated components should
be rejected even if it looks good.

## What failed

Raw exploded SVG layers were technically faithful but visually useless for imagegen. The treads and
small pieces looked like unrelated rectangles, so the model had no context for what they were.

A compact/reused sheet was worse. Because it did not show every real component with context, the
model invented sprockets, gears, road wheels, and other exposed mechanisms that were not present in
the source SVG.

The original full-tank reference included a drop shadow. That made the whole sheet read like a
rendered object on a floor and encouraged shadow artifacts. The current semantic reference removes
the drop shadow and the fuel cue.

The model treated the complete tank and the components as separately designed objects. Several
passes had a plausible full tank in the top-left cell, but the generated component cells would not
assemble into that tank. This is the main unsolved issue.

Detailed prompts produced too much detail. Tiger I pass 01 and pass 03 had a better tank read, but
they were closer to detailed concept sprites than the requested simple low-end raster look.

The best style direction so far was pass 04: simpler, lower-detail, broad shapes. It still was not
usable because orientation and component consistency drifted.

The current prototype does not have automatic validation. Human inspection caught layout,
orientation, detail, shadow, and component consistency failures after generation. Future work needs
cheap checks before any generated atlas can be enabled.

## Candidate pass log

- `tank-imagegen-pass-01-unsliced.png`: rejected. It was not a faithful exploded SVG sheet and
  included invented detail.
- `tank-imagegen-pass-02.png`: rejected. The compact/primitive sheet produced hallucinated
  mechanisms and did not preserve source semantics.
- `tank-tiger-i-pass-01.png`: recognizable strict top-down Tiger I direction, but too detailed.
- `tank-tiger-i-pass-02.png`: more polygonal and silhouette-focused, but still had extra surface
  treatment and did not solve component consistency.
- `tank-tiger-i-pass-03.png`: smoother raster graphics direction, but too realistic and too busy.
- `tank-tiger-i-pass-04.png`: closer to the desired simple early-3D raster style, but not slice-ready
  because orientation/scale and component-to-reference consistency drifted.

These candidates are useful references for what to avoid. None should be treated as accepted art.

## Validation checklist before activation

- The output keeps the exact 3x2 grid and cell order.
- Every cell has transparent or perfectly keyable background.
- There is no drop shadow, cast shadow, contact shadow, ambient blob, floor, or ground plane.
- The tank is strict top-down, not perspective or side-biased.
- No invented loose gears, sprockets, road wheels, extra turrets, extra barrels, labels, or UI.
- The component cells preserve source orientation, center, approximate footprint, and pivot meaning.
- The component cells can be assembled into the complete tank reference.
- The complete tank reference did not diverge into its own independent design.
- The detail level is simple low-end raster art, not pixel art and not detailed concept art.
- Team-colorable regions remain clean enough for runtime tinting or a future mask/chroma pass.
- The generated atlas is inspected on a dark background after alpha conversion to catch fringes.

## Next work

- Move semantic grouping out of `tank-raster-pipeline.mjs` into reusable per-unit sidecar metadata.
- Generate a component-only sheet plus a separate reference image, or make the full cell visibly
  non-authoritative so the model cannot redesign it independently.
- Add a preview that reconstructs the tank from sliced component cells and compares it against the
  reference cell.
- Add alignment normalization for generated components: rotation, scale, center, and alpha bounds.
- Decide the team-color strategy. The runtime can tint atlas sprites by existing tint slots, but a
  final art pass may still need neutral grayscale masks or explicit chroma-key regions per part.
- Keep shadows out of unit sprites. If a final art direction needs shadows, render them as a
  separate deterministic game layer, not inside generated component art.
- Do another imagegen pass using the pass-04 direction but with stronger constraints that the
  component cells are the only runtime source and must assemble into the reference.
