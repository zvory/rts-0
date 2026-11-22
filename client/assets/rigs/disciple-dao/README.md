# Disciple Dao art

Approved art generated with the built-in imagegen tool, using the original
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
