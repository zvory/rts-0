# Raster unit art handoff

Status: active visual experiment only. The checked-in tank PNG rig path is enabled for a pass-06
Tiger I body/turret experiment, but the generated images are not final game art. This note records
what worked, what failed, and how to reproduce the next experiment without rediscovering the same
traps.

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
asset. Pass 06 is enabled only as a local visual experiment; it still fails component consistency and
alignment checks, so it should not be treated as accepted art.

## Current files

- `scripts/art/tank-raster-pipeline.mjs` builds the tank contact sheet, writes prompts, and writes
  PNG atlas metadata.
- `client/assets/rigs/tank-ps1/tank-contact-sheet.svg` and
  `client/assets/rigs/tank-ps1/tank-contact-sheet.png` are the current semantic source sheet.
- `client/assets/rigs/tank-ps1/metadata/source-grid.json` records the cell order, source parts, and
  semantic sprite grouping.
- `client/assets/rigs/tank-ps1/metadata/prompt*.md` records the base prompt and Tiger I prompt
  iterations.
- `client/assets/rigs/tank-ps1/generated/` keeps generated candidates and alpha-converted copies.
- `client/assets/rigs/tank-ps1/tank-atlas.png` is the enabled pass-06 runtime atlas. It uses only
  the generated hull/body and turret/barrel cells; the top row is transparent so generated/default
  tracks are not rendered during this experiment. The runtime sprite frames are normalized to the
  visible component alpha bounds, not the full generated cell bounds.
- `client/src/renderer/rigs/tank_png_atlas.js` is generated metadata. Its `enabled` field is
  currently `true` for the pass-06 experiment.
- `client/src/renderer/rigs/png_runtime.js` and `png_routing.js` render atlas sprites in place of
  SVG pixels when an atlas is enabled and loaded.

## Current guided semantic sheet

The current tank sheet uses a 2x2 grid. Every cell has an outer guide box, an 8x8 internal guide
grid, and center marks to give imagegen stronger alignment and scale cues:

1. `reference.full` - assembled tank reference, with the SVG drop shadow and fuel cue removed.
2. `sprite.track` - one reusable straight track-link strip, rendered from the left tread blocks only
   so the model does not draw a closed track assembly or an end contour.
3. `sprite.hull` - hull, nose, hull shading parts, and nose tick.
4. `sprite.turret` - turret, main barrel, and coax barrel.

The runtime atlas metadata still exposes `sprite.track.left` and `sprite.track.right`, but both
sprites point at the same `sprite.track` source cell with different rig origins. This keeps the
contact sheet to one track art element while preserving left/right tank placement when the prototype
PNG path is tested locally. In pass 06 the active runtime alpha sheet blanks that track cell to
suppress tracks entirely. The fuel/no-oil cue is intentionally omitted from the PNG atlas and
remains SVG-only through a separate overlay route.

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
  --columns 2 \
  --layout tight \
  --profile semantic
```

Refresh the base prompt:

```bash
node scripts/art/tank-raster-pipeline.mjs write-prompt
```

Before any image generation pass, present the exact contact sheet image to the user and wait for
explicit confirmation. Do not start imagegen from a new, edited, cropped, or regenerated sheet until
the user has seen that specific sheet and approved it as the input.

Use `client/assets/rigs/tank-ps1/tank-contact-sheet.png` as the input image. Start from the latest
short prompt direction rather than the most detailed prompt: strict top-down Tiger I, no shadows,
very simple low-end 3D raster rendering, anti-aliased raster shapes, not pixel art, and much less
detail than concept art.

Save each generated candidate with a pass number, for example:

```text
client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-06.png
client/assets/rigs/tank-ps1/metadata/prompt-tiger-i-pass-06.md
client/assets/rigs/tank-ps1/metadata/tiger-i-pass-06.json
```

Convert the chroma-key background to alpha before atlas wiring:

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/imagegen/scripts/remove_chroma_key.py" \
  --input client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-06.png \
  --out client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-06-alpha.png \
  --key-color '#ff00ff' \
  --auto-key none \
  --soft-matte \
  --transparent-threshold 12 \
  --opaque-threshold 220 \
  --despill
```

Use an explicit `#ff00ff` key while the guide boxes touch the sheet border. Border auto-key can
sample the cyan guide line instead of the background.

Write atlas metadata disabled while evaluating. For imagegen sheets that keep guide edges, generated
cell dividers, or extra padding, use the normalization flags so runtime scale comes from the visible
component bounds rather than the whole cell:

```bash
node scripts/art/tank-raster-pipeline.mjs write-atlas \
  --sheet client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-06-alpha.png \
  --columns 2 \
  --layout tight \
  --profile semantic \
  --normalize-visible-bounds \
  --clear-cell-edge-alpha 16 \
  --visible-padding 2 \
  --disabled \
  --model "built-in image generation" \
  --notes "Candidate Tiger I raster pass; disabled pending alignment and component consistency review."
```

For a local no-track experiment like pass 06, also blank the reference and track cells and add an
image version so a browser reload fetches the updated atlas:

```bash
node scripts/art/tank-raster-pipeline.mjs write-atlas \
  --sheet client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-06-lowpoly-body-turret-alpha.png \
  --columns 2 \
  --layout tight \
  --profile semantic \
  --blank-cells reference.full,sprite.track \
  --normalize-visible-bounds \
  --clear-cell-edge-alpha 16 \
  --visible-padding 2 \
  --image-version pass06-normalized \
  --prompt-file client/assets/rigs/tank-ps1/metadata/prompt-tiger-i-pass-06-lowpoly.md \
  --model "built-in image generation" \
  --notes "Experimental Tiger I low-poly raster pass 06."
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
- preserve the guide boxes as alignment only; do not turn guide lines into armor seams or track
  detail
- the track cell is one reusable strip/segment of track links, not left and right full track
  assemblies

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
- `tank-tiger-i-pass-05-guided.png`: preserved the guided 2x2 sheet, removed the fuel/no-oil cue,
  and generated one reusable track-link strip. Rejected for activation because it retained guide
  lines in the alpha output and was still more detailed than the intended low-end raster style.
- `tank-tiger-i-pass-06-lowpoly.png`: active experiment. Recognizable Tiger I low-poly direction
  with separate hull/body and turret/barrel cells. The raw generated sheet included tracks, but the
  runtime alpha sheet blanks the top row so no generated or fallback tracks render. The atlas
  writer now clears generated guide-edge alpha and normalizes frames to visible alpha bounds, which
  fixes the earlier too-small render and black cell-box artifacts. Not final art: the hull has an
  open turret-ring hole and component alignment still needs review.

These candidates are useful references for what to avoid. None should be treated as accepted art.

## Validation checklist before activation

- The exact input contact sheet was shown to the user before imagegen, and the user explicitly
  confirmed that sheet before generation started.
- The output keeps the exact 2x2 grid and cell order.
- Every cell has transparent or perfectly keyable background.
- There is no drop shadow, cast shadow, contact shadow, ambient blob, floor, or ground plane.
- The tank is strict top-down, not perspective or side-biased.
- No invented loose gears, sprockets, road wheels, extra turrets, extra barrels, labels, fuel icons,
  warning symbols, or UI.
- The single track cell is a reusable track-link strip with no closed end cap or perimeter contour.
- The component cells preserve source orientation, center, approximate footprint, and pivot meaning.
- The component cells can be assembled into the complete tank reference.
- The complete tank reference did not diverge into its own independent design.
- The detail level is simple low-end raster art, not pixel art and not detailed concept art.
- Team-colorable regions remain clean enough for runtime tinting or a future mask/chroma pass.
- Guide lines are absent from any alpha sheet wired into `tank-atlas.png`, or removed before
  activation without cutting through the component art.
- The generated atlas is inspected on a dark background after alpha conversion to catch fringes.

## Next work

- Move semantic grouping out of `tank-raster-pipeline.mjs` into reusable per-unit sidecar metadata.
- Generate a component-only sheet plus a separate reference image, or make the full cell visibly
  non-authoritative so the model cannot redesign it independently.
- Add a preview that reconstructs the tank from sliced component cells and compares it against the
  reference cell.
- Add a guide-removal/deguide step or make the guides model-visible without preserving them in the
  generated atlas.
- Add alignment normalization for generated components: rotation, scale, center, and alpha bounds.
- Decide the team-color strategy. The runtime can tint atlas sprites by existing tint slots, but a
  final art pass may still need neutral grayscale masks or explicit chroma-key regions per part.
- Keep shadows out of unit sprites. If a final art direction needs shadows, render them as a
  separate deterministic game layer, not inside generated component art.
- Do another imagegen pass using the pass-04 direction but with stronger constraints that the
  component cells are the only runtime source and must assemble into the reference.
