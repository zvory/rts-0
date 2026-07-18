# Raster unit art handoff

Status: active visual experiment only. The checked-in tank PNG rig path is enabled for a pass-11
white-painted Tiger I no-track hull/turret/barrel experiment, Rifleman pass 02 is enabled as a
full-frame PNG strip, and Artillery uses the modular A-19 pass-02 alignment-review atlas. The
generated images are not final game art. This note records what worked, what failed, and how to
reproduce the next experiment without rediscovering the same traps.

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
asset. Pass 11 is enabled only as a local visual experiment; it repaints the pass-10 no-guide Tiger
I sheet as weathered matte white, bakes 30% lower brightness and 20% lower saturation, keeps the
separate turret/barrel structure, maps `sprite.barrel` to the original barrel animation, removes
visible guide boxes from the imagegen input, and relies on visible-alpha postprocessing for runtime
sizing. Runtime owner color intentionally tints the dimmed white source art through the semantic
atlas tint slots. It still needs component cleanup and alignment review before it should be treated
as accepted art.

Rifleman pass 02 deliberately does not use component slicing yet. It ships as a compact six-frame
full-body strip because the useful experiment is testing whether a generated infantry token reads at
RTS scale in-game. Runtime frame 0 is the idle standing frame. Frames 1-4 cycle at 12 FPS only while
the authoritative client entity state is `move`. Frame 5 is retained from the generated post-shot
source but is not wired; firing and recoil remain future work. The old SVG rifleman rig still
provides the shadow route and fallback while the PNG texture loads. Frame-strip art uses the shared default
`FRAME_STRIP_TARGET_COLOR_ADJUSTMENT` in `client/src/renderer/rigs/frame_strip_color_profile.js`
(`brightness: 170`, `saturation: 118`, `hue: 100`) and the brighter `team-light` tint slot so blue
and orange owners read on dark terrain. Rifleman pass 02 already has that baseline baked into its
checked-in runtime strip; raw frame strips such as Machine Gunner pass 01 receive the missing
delta once at client texture-load time, preserving the original generated source sheets. A strip can
override the target when that generated unit needs its own brightness match.

Default color strategy for future generated PNG unit sprites: generate team-colorable paint,
uniform, and armor regions as weathered matte white or off-white source art, not blue, gray-blue, or
any final owner color. Keep dark outlines, panel seams, equipment details, and light gray shading so
the white base stays readable before tinting. Runtime tint slots, color-profile adjustments, or
future masks then apply owner color over the neutral white base. Fixed non-team materials such as
rubber, dark weapon metal, wood, skin, and transparent chroma-key background can keep their normal
material colors; the `#ff00ff` key remains only the background key, never the unit paint.

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
- `client/assets/rigs/tank-ps1/tank-atlas.png` is the enabled pass-11 runtime atlas. It uses only
  the generated hull/body, turret/coax, and separate main-barrel cells; the reference, track
  placeholder, and unused cells are transparent so generated/default tracks are not rendered during
  this experiment. The current active variant is `pass11-white-dim30`, an imagegen repaint of the
  pass-10 no-guide sheet with baked `brightness: 70` and `saturation: 80` modulation. Hull and
  barrel use `team` tint while turret uses `team-light`, so owner team tint applies over the dimmed
  white base. The runtime sprite frames are normalized to visible component alpha bounds, not the
  full generated cell bounds.
- `client/src/renderer/rigs/tank_png_atlas.js` is generated metadata. Its `enabled` field is
  currently `true` for the pass-11 experiment.
- `client/src/renderer/rigs/png_runtime.js` and `png_routing.js` render atlas sprites in place of
  SVG pixels when an atlas is enabled and loaded.
- `client/assets/rigs/artillery-a19-pass-02/` keeps the regenerated modular A-19 source sheet, the
  alpha-converted diagnostic atlas, and its prompt summary. `artillery_png_atlas.js` maps the two
  independent trails, carriage, and barrel/recoil assembly back onto the Artillery SVG animation
  bindings. The left trail is deliberately purple and both trail crop frames carry black borders
  during pivot/origin review; neither treatment is final art direction.
- `client/assets/rigs/rifleman-pass-02/` keeps the enabled rifleman pass-02 source sheet, alpha
  conversion, compact runtime strip, prompt, and manifest.
- `client/src/renderer/rigs/rifleman_png_strip.js` and
  `client/src/renderer/rigs/machine_gunner_png_strip.js` record the enabled frame-strip metadata,
  including any color adjustment already baked into the checked-in runtime strip.
- `client/src/renderer/rigs/frame_strip_runtime.js` and `frame_strip_routing.js` render full-frame
  unit strips. `frame_strip_color_profile.js` owns the shared brightness/saturation target applied
  at texture-load time when a strip is not already baked to that baseline.

## Current no-guide semantic sheet

The current tank sheet uses a 2x3 layout with no visible guide boxes, subgrid lines, center marks,
or dividers. The cell order is metadata-driven and the model sees only the magenta background plus
the component art:

1. `reference.full` - assembled no-track tank reference, with the SVG drop shadow, tracks, fuel
   cue, and muzzle-flash effect parts removed.
2. `sprite.track` - empty no-track placeholder used only so transparent PNG sprites can cover the
   SVG track/tread parts and suppress fallback track rendering.
3. `sprite.hull` - hull, nose, hull shading parts, and nose tick.
4. `sprite.turret` - turret and coax barrel, excluding the main barrel.
5. `sprite.barrel` - separate main barrel mapped to `part.barrel`, preserving the original SVG
   barrel recoil scale.
6. `unused.blank` - empty zone to keep the contact sheet rectangular.

The runtime atlas metadata still exposes `sprite.track.left` and `sprite.track.right`, but both
sprites point at the same `sprite.track` source cell with different rig origins. That cell is
intentionally blank in pass 11: it preserves PNG coverage for track source parts while suppressing
both generated tracks and fallback SVG tracks. The fuel/no-oil cue is intentionally omitted from the
PNG atlas and remains SVG-only through a separate overlay route.

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
  --profile semantic \
  --guides none
```

Refresh the base prompt:

```bash
node scripts/art/tank-raster-pipeline.mjs write-prompt
```

Before any image generation pass, present the exact contact sheet image to the user and wait for
explicit confirmation. Do not start imagegen from a new, edited, cropped, or regenerated sheet until
the user has seen that specific sheet and approved it as the input.

Use `client/assets/rigs/tank-ps1/tank-contact-sheet.png` as the input image for a new structural
generation pass. For a repaint pass like `pass11-white`, use the prior generated sheet
(`client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-10-noguide-ref.png`) as the edit target
and constrain the model to preserve the existing 2x3 layout. Start from the latest short prompt
direction rather than the most detailed prompt: strict top-down Tiger I, no shadows, very simple
low-end 3D raster rendering, anti-aliased raster shapes, not pixel art, and much less detail than
concept art.

Save each generated candidate with a pass number, for example:

```text
client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-11-white.png
client/assets/rigs/tank-ps1/metadata/prompt-tiger-i-pass-11-white.md
client/assets/rigs/tank-ps1/metadata/tiger-i-pass-11-white.json
```

Convert the chroma-key background to alpha before atlas wiring:

```bash
python "${CODEX_HOME:-$HOME/.codex}/skills/.system/imagegen/scripts/remove_chroma_key.py" \
  --input client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-11-white.png \
  --out client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-11-white-alpha.png \
  --key-color '#ff00ff' \
  --auto-key none \
  --soft-matte \
  --transparent-threshold 58 \
  --opaque-threshold 220 \
  --despill
```

Use an explicit `#ff00ff` key when the generated background has mild magenta variation. Wider
thresholds may be needed because the model sometimes adds subtle background gradients even when
asked for a flat chroma key.

Write atlas metadata disabled while evaluating. For no-guide sheets, use visible-alpha
normalization so runtime scale comes from the generated component bounds rather than the whole cell:

```bash
node scripts/art/tank-raster-pipeline.mjs write-atlas \
  --sheet client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-11-white-alpha.png \
  --columns 2 \
  --layout tight \
  --profile semantic \
  --normalize-visible-bounds \
  --clear-cell-edge-alpha 0 \
  --visible-padding 0 \
  --disabled \
  --model "built-in image generation" \
  --notes "Candidate Tiger I raster pass; disabled pending alignment and component consistency review."
```

For a local no-track experiment like pass 11, also blank the reference, track-placeholder, and
unused cells, then add an image version so a browser reload fetches the updated atlas:

```bash
node scripts/art/tank-raster-pipeline.mjs write-atlas \
  --sheet client/assets/rigs/tank-ps1/generated/tank-tiger-i-pass-11-white-alpha.png \
  --columns 2 \
  --layout tight \
  --profile semantic \
  --blank-cells reference.full,sprite.track,unused.blank \
  --normalize-visible-bounds \
  --clear-cell-edge-alpha 0 \
  --visible-padding 0 \
  --world-scale 1.2 \
  --brightness 70 \
  --saturation 80 \
  --image-version pass11-white-dim30 \
  --prompt-file client/assets/rigs/tank-ps1/metadata/prompt-tiger-i-pass-11-white.md \
  --model "built-in image generation + chroma-key cleanup + ImageMagick dimming pass" \
  --notes "Experimental Tiger I white-painted no-track no-guide raster pass 11 with 1.2x world-scale compensation, 30% lower brightness, and 20% lower saturation, using runtime team tint over the white base."
```

Only omit `--disabled` for a local experiment after the validation checklist passes. Omit
`--semantic-paint-tint-slot` when the white source art should still receive owner team tint; reserve
the option for a future pass that deliberately bakes final paint colors into the generated image.
Do not commit an enabled atlas unless the component cells can actually reconstruct the tank.

## Prompt lessons

Use concise prompts. Over-specified realism prompts pushed the model toward tiny hatches, grilles,
scratches, bolts, and independent redesigns. The better direction is:

- strict top-down orthographic view
- Tiger I silhouette first
- long rectangular hull
- no tracks in the active no-track experiment
- square flat-sided turret
- long centered barrel as a separate component cell, not merged into the turret
- very early low-end 3D raster graphics
- anti-aliased raster shapes, not pixel art
- only a few broad shade values per part
- team-colorable unit paint, uniforms, and armor generated as matte white or off-white source art,
  not blue/gray-blue or final owner color
- no shadows of any kind
- no labels, text, insignia, loose parts, or extra cells
- preserve cell order, centers, scale, and orientation
- do not show guide boxes or grid lines to the model; keep sizing and anchoring in metadata and
  postprocessing
- for no-track passes, the track cell is an empty runtime suppressor, not track art

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

Visible guide boxes helped layout but hurt the art. They repeatedly came back as baked guide-color
slivers, black frame remnants, or grid-inspired seams. Pass 10 removes visible guides from the
imagegen input and lets atlas postprocessing do the sizing.

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
- `tank-tiger-i-pass-06-lowpoly.png`: saved experiment. Recognizable Tiger I low-poly direction
  with separate hull/body and turret/barrel cells. The raw generated sheet included tracks, but the
  runtime alpha sheet blanks the top row so no generated or fallback tracks render. The atlas
  writer now clears generated guide-edge alpha and normalizes frames to visible alpha bounds, which
  fixes the earlier too-small render and black cell-box artifacts. Not final art: the hull has an
  open turret-ring hole and component alignment still needs review.
- `tank-tiger-i-pass-06-lowpoly-bright-body-turret-alpha.png`: saved no-imagegen brightness pass.
  Derived from pass 06 with `--brightness 130 --saturation 105`, retaining the same normalized
  component frames and no-track behavior while making the tank read more clearly on the map.
- `tank-tiger-i-pass-06-lowpoly-brighter-body-turret-alpha.png`: saved no-imagegen brightness pass.
  Derived from pass 06 with `--brightness 145 --saturation 108` immediately before restarting the
  contact sheet around a separate barrel cell.
- `tank-tiger-i-pass-07-separated.png`: saved experiment. Regenerated from a no-track 2x3 sheet
  with separate hull/body, turret/coax, and main-barrel cells. The active runtime atlas blanks the
  reference, track placeholder, and unused cells; uses `--ignore-guide-bounds` so guide boxes do not
  drive sprite scale; and maps `sprite.barrel` to `part.barrel`, restoring the original barrel recoil
  scaling. Not final art: some guide-line edge artifacts are baked into the generated cell imagery,
  and component alignment still needs review.
- `tank-tiger-i-pass-08-tiger1.png`: saved experiment. Regenerated from the separated 2x3 no-track
  sheet with a prompt explicitly naming a 1940s Tiger I / Panzerkampfwagen VI Tiger Ausf. E and a
  separate 88mm barrel. The runtime atlas keeps transparent track coverage, maps `sprite.barrel` to
  `part.barrel`, and changes the barrel tint slot to `team-light` so it tints with the turret. Not
  final art: the generated hull has heavy side armor shapes that may read like track guards and
  should be inspected in-game.
- `tank-tiger-i-pass-09-outline.png`: saved experiment. Regenerated from the separated 2x3 no-track
  sheet with the attached top-down Tiger I image used as silhouette reference only. The prompt asks
  for less detail, a stepped/chamfered non-perfect-rectangular hull, black RTS-readable outlines,
  and a barrel with the same team-color material as the hull. The generated barrel cell was flipped
  horizontally before atlas writing so the muzzle and pivot match the original rig. Not final art:
  the side armor blocks should be inspected in-game because they may still read like track guards.
- `tank-tiger-i-pass-10-noguide-ref.png`: saved experiment and source for pass 11. Regenerated from
  a no-guide 2x3 sheet
  and the attached top-down Tiger I reference as the subject input. The runtime atlas uses 1.2x
  world-scale compensation so the no-track raster art recovers the old SVG tank's visual mass, uses
  visible-alpha normalization only, with no guide masking or cell-edge clearing. The client applies
  an additional non-destructive 5% runtime brightness lift. This pass validates the pipeline
  direction because guide artifacts are gone, but it is not final art: the generated hull still
  carries rear/side fixtures from the reference that may read as unwanted track hardware.
- `tank-tiger-i-pass-11-white.png`: active experiment. Imagegen repaint of the pass-10 no-guide
  sheet that preserves the 2x3 component layout while changing the armor to weathered matte white.
  The active runtime atlas version is `pass11-white-dim30`, with 30% lower brightness and 20% lower
  saturation baked in. It still uses 1.2x world-scale compensation, visible-alpha
  normalization only, and transparent reference/track/unused cells. Runtime owner tint applies over
  the dimmed white source art through the semantic hull, turret, and barrel tint slots. Not final
  art: component alignment and readability still need inspection if this stays active beyond local
  preview.
- `rifleman-pass-02`: active experiment. Generated as the best of five one-shot full-frame
  rifleman strips after simplifying the prompt around strict nadir view, hidden legs, shoulder-line
  rifle pose, and RTS-scale detail. It is enabled as a full-frame strip rather than a semantic
  component atlas. The active runtime strip is brightened from the generated pass using
  the shared frame-strip target and `team-light` tint. Not final art: the anatomy and weapon hold
  still need another art pass, and the current runtime has no firing/recoil frame.
- `machine-gunner-pass-01`: active experiment. Generated carry and setup/deployed sheets are kept
  in raw color, and the client applies a per-unit `brightness: 145`, `saturation: 118`, `hue: 100`
  frame-strip target at texture-load time so its runtime brightness can be tuned against Rifleman
  and Tank without destructively rewriting the source art.
- `mortar-png-pass-01`: active experiment. Generated as a three-cell M2 4.2-inch-inspired wheeled
  mortar sheet: assembled reference, carriage/frame/wheels component, and separate tube component.
  The checked-in alpha source is routed through `mortar_team_png_atlas.js`; the carriage and tube
  components receive runtime `team-light` tint with a dimmed saturation/brightness adjustment, tire
  overlays remain fixed-color, and the tube still follows the stronger SVG weapon recoil binding
  independently from the carriage recoil.
- `artillery-a19-pass-02`: active alignment experiment. Regenerated as four disconnected A-19
  components: two support arms, a frame/wheel carriage, and a separate elevated barrel with an
  oversized recoil assembly with a clearly visible indirect-fire elevation. The SVG rig remains
  authoritative for setup visibility, carriage and weapon facing, recoil, muzzle flash, and anchors.
  Runtime review colors the left arm purple and
  renders a black rectangle around each arm's complete crop frame so subsequent rotation feedback
  can refer to the actual image footprint and mounting origin. The frame treatment and fixed trail
  colors are temporary diagnostics, not accepted production presentation.

These candidates are useful references for what to avoid. None should be treated as accepted art.

## Validation checklist before activation

- The exact input contact sheet was shown to the user before imagegen, and the user explicitly
  confirmed that sheet before generation started.
- The output keeps the exact 2x3 grid and cell order.
- Every cell has transparent or perfectly keyable background.
- There is no drop shadow, cast shadow, contact shadow, ambient blob, floor, or ground plane.
- The tank is strict top-down, not perspective or side-biased.
- No invented loose gears, sprockets, road wheels, extra turrets, extra barrels, labels, fuel icons,
  warning symbols, or UI.
- In no-track passes, the track placeholder remains empty and transparent in the active atlas while
  still covering track/tread source parts so SVG tracks do not fall back.
- The main cannon is in its own component cell and maps to `part.barrel`, not `part.turret`.
- The generated barrel sprite uses a team tint slot when the active art direction expects owner tint
  over the generated base, or a fixed tint slot when the art direction expects baked final paint.
- The component cells preserve source orientation, center, approximate footprint, and pivot meaning.
- The component cells can be assembled into the complete tank reference.
- The complete tank reference did not diverge into its own independent design.
- The detail level is simple low-end raster art, not pixel art and not detailed concept art.
- Team-colorable regions use a clean matte white/off-white base with gray shading so runtime tinting
  or a future mask/chroma pass can recolor them predictably.
- Guide lines are absent from the source sheet and sampled runtime frames.
- The generated atlas is inspected on a dark background after alpha conversion to catch fringes.

## Next work

- Move semantic grouping out of `tank-raster-pipeline.mjs` into reusable per-unit sidecar metadata.
- Generate a component-only sheet plus a separate reference image, or make the full cell visibly
  non-authoritative so the model cannot redesign it independently.
- Add a preview that reconstructs the tank from sliced component cells and compares it against the
  reference cell.
- Keep the no-guide sheet as the default. If guides are temporarily reintroduced for debugging, do
  not use that sheet for image generation.
- Add alignment normalization for generated components: rotation, scale, center, and alpha bounds.
- Keep the white-source team-color strategy as the default. The runtime can tint atlas sprites by
  existing tint slots, but a final art pass may still need neutral masks or explicit chroma-key
  regions per part for finer color separation.
- Keep shadows out of unit sprites. If a final art direction needs shadows, render them as a
  separate deterministic game layer, not inside generated component art.
- Do another imagegen pass using the pass-04 direction but with stronger constraints that the
  component cells are the only runtime source and must assemble into the reference.
