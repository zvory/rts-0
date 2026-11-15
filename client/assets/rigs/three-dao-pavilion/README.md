# Three Dao Pavilion

Approved concept: a circular stone study dais with three scripture scrolls, a central reading
table, timber uprights, and a three-lobed white canopy. Generated with the built-in image tool,
using the Cultivator Nexus as the style reference: elevated front-centered orthographic view,
about 45 degrees above horizontal, painterly RTS outlines, warm wood and neutral stone.
The source has a solid magenta background with no people, scene, lettering, glow or cast shadow.

`pavilion-source.png` preserves the selected concept in 8-bit PNG encoding. `pavilion-atlas.png`
is the approved 3x3 in-game preview derivative: two square 384x384 frames, fixed-color base then
white canopy/finial paint overlay. Runtime player tint multiplies the overlay; wood, stone and
parchment stay fixed. Equal X/Y scale preserves the source proportions. The rectangular fallback
footprint shadow is disabled, matching the Nexus.

Regenerate with `node scripts/art/pavilion-pipeline.mjs`; check with `--check`. The pipeline keys
magenta and removes spill before premultiplied 4x4 supersampling. Its low-saturation/high-value
paint mask is restricted to the upper 46% of the source to exclude scrolls and the stone dais.

The approved gameplay footprint is 3x3 (previously 3x2); other rules and research are unchanged.
