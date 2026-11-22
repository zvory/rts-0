# Disciple Dao art

Original approved ranged art generated with the built-in imagegen tool, using the original
`warrior-placeholder-pass-01/generated/warrior-source.png` Disciple as camera,
identity and painted-style reference. Every source uses a magenta matte and
white/grayscale cloth for runtime team tinting.

- `source/concepts.png`: approved Lightness A in row 1 (oversized bow, quiver,
  streaming headband); five columns are idle, three walking poses, attack-ready.
- `source/lightness-attack.png`: the same approved archer, four sequential
  poses: draw, release, follow-through, recovery. Keep the large bow and costume.
- `source/magic-motion.png`: approved Magic C hood and mantle, 4×2 grid. Row 1
  is idle and three arms-back runs, hands held behind the crown and mantle
  streaming back. Row 2 is windup, palm release, follow-through, recovery.

All poses use strict orthographic overhead camera with crown and shoulders only,
no face, frontal torso or feet. The frames preserve body scale and align the crown
pivot, rather than independently fitting each flowing silhouette.

Regenerate with `node scripts/art/disciple-dao-pipeline.mjs`; verify deterministic
outputs with the same command plus `--check`. The Node/pngjs pipeline removes the
magenta matte and spill, downsamples with premultiplied 4×4 area samples and packs
256-pixel cells. It also generates the projectile from
`client/assets/effects/disciple-arrow/arrow-source.png`.

The arrow generation prompt requested one isolated horizontal arrow pointing right:
light-gray shaft, small white pointed metal head, two white feathers, crisp dark
outline, overhead camera, pure magenta background, no glow/trail/shadow/text. It is
cropped and reduced to 128 pixels wide; rendering uses a shared texture at 22 world
pixels long. The orb is fixed-cost geometry rather than another bitmap or emitter.

Damage and shot timing remain server-authoritative. The four-frame attack art is
shot-triggered, not an idle loop; visual projectile release follows at 100 ms.

## Melee Lightness revision

Lightness now uses the base Disciple’s 0.75-tile sword weapon with moving attacks,
existing speed, and Dash. The current archer source files and generated strip above
are preserved unchanged; `archive/lightness-archer/` also contains an explicit copy
for reuse. Runtime uses the selected candidate B katana strip, generated from
`source/lightness-melee-b-motion.png`: idle, three running poses, and four attack poses. Magic remains unchanged.

`source/lightness-melee-candidates/` contains three built-in imagegen concepts and
the exact prompt set. These are design candidates, not animation strips:
A (twin streamers/open guard), B (single scarf/diagonal blade),
and C (compact two-handed guard). Candidate B is selected. Its standing scarf rests against the back; running poses stream it behind. The exact animation prompt is in `source/lightness-melee-b-prompt.txt`.

The running poses now use `source/lightness-melee-b-windy-run.png`, with a longer
wind-whipped scarf sweeping through three distinct curves. Standing and attack frames
remain byte-identical to the initial candidate B pass. The run prompt is preserved in
`source/lightness-melee-b-windy-run-prompt.txt`.

## Armored War Yari

War uses approved candidate B: broad angular eastern armor and a straight Yari spear.
`source/war-yari-motion.png` supplies idle and two walking poses; only its first three
cells are consumed. `source/war-yari-lunge.png` supplies anticipation, drive, full
extension and recovery. The built-in imagegen lunge prompt is preserved beside the sources.
The deterministic pipeline packs seven 256-pixel cells in `generated/war-yari-runtime-strip.png`,
normalizes crown pivots at the approved body scale, and leaves room for the full spear.
Runtime scale 0.8 keeps the body larger than the other Disciples. Shot events drive
four attack frames over 400 ms, with a smooth 16-world-pixel local-forward visual step.
War alone has 2.75-tile weapon reach, matching the approximately 95-world-pixel ready tip
once ordinary body radius/combat slack and typical target size are accounted for.
The step never changes simulation coordinates, collision or damage timing.
