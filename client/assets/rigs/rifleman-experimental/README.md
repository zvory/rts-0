# Experimental Rifleman graphics

Enable **Use experimental graphics** in Game settings. The local preference defaults off,
persists across launches, and switches world Rifleman art without a rematch. Other unit kinds,
HUD portraits, minimap icons, and simulation rules keep their existing behavior.

The atlas contains sixteen headings, with one idle, four running, and four shot-triggered
firing/recovery poses per heading: 144 cells, 128×128 each, packed in sixteen columns.
Frames 0–15 are idle; 16–79 are running; 80–143 are firing. Firing follows the normal
420 ms Rifleman recoil cycle and weapon bearing. Running follows body bearing and displacement.
All poses use a common foot anchor. A small contact shadow grounds the oblique sprite.

The art was generated with built-in `image_gen`. `approved-medium-contact-sheet.png` is the
approved design; `run-source-*.png` and `shoot-source-*.png` retain the generated source sheets.
`running-prompts.json` and `shooting-prompts.json` preserve their prompts and reference role.

Run `node scripts/build-rifleman-experimental.mjs` to rebuild the three runtime layers and
registration metadata; add `--check` for deterministic verification. Registration removes stray
alpha components and chroma fringes, normalizes heading scale, centers the helmet horizontally,
and aligns the lowest boot with the common ground contact. The existing white art is preserved;
no magenta regeneration or keying is used for team colors.

The white-material mask uses neutral-color saturation and brightness, preserving shaded cloth
and helmet highlights while excluding skin, leather, wood, dark boots and contact pixels.
`fixed-atlas.png` and `paint-atlas.png` are source-over layers. A white team palette reconstructs
the approved neutral atlas; the shared Cultivator team-paint compositor applies each actual
owner color and caches palettes per match. Separate layer images keep each texture within
2048×2048. Atlas loading is lazy while the preference is off, and palettes follow renderer teardown.
