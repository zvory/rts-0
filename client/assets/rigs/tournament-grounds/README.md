# Cultivator Tournament Grounds

Selected tournament-bowl candidate generated with the built-in imagegen tool using the previous
Tournament Grounds source as its style reference. The raised octagonal fighting platform,
four-tier timber spectator seating, stone stair aisles, timber railing and rear judges pavilion,
white-painted curved roof and lotus finial share their elevated 45-degree orthographic camera and
hand-painted outline style. There are no people, effects, lettering or cast ground shadows.
The building occupies a 4x4 gameplay footprint; its octagonal arena and seating sit inside that square.

`tournament-source.png` preserves the generated concept on the requested solid magenta background.
`tournament-atlas.png` contains two 384x384 frames: fixed-color base then roof/finial team-paint
layer. Runtime player tint multiplies the white paint layer. Natural stone, timber and railing
panels stay fixed. Use equal X/Y scale, `visualScale: 1.1`, `silhouetteShadow: false`, and
`omitFootprintShadow: true`, matching the existing Pavilion presentation.

Regenerate with `node scripts/art/tournament-pipeline.mjs`; verify with `--check`. The pipeline
keys magenta, removes edge spill, and downsamples with premultiplied 4x4 supersampling. Coverage
below 8% is discarded to remove faint chromakey residue. The neutral-bright paint mask is
restricted to the upper 25% of the source so the fighting platform stays natural stone.

## Generation prompt

Use the previous Tournament Grounds source as style/material/camera reference. One isolated
Cultivators RTS Tournament Grounds with a square 4x4-tile footprint, no drawn grid. Compact
octagonal tournament bowl: broad raised pale-stone fighting platform, narrow circulation aisle,
four stepped tiers of pale-stone terraces with warm timber benches around the rear and sides,
and radial stair aisles. Small rear judges pavilion with white upturned roof and lotus finial.
Wide front entrance, shallow stone steps and low timber/white-panel perimeter rail. Match the
reference's hand-painted outlines, broad readable shapes and elevated front-centered 45-degree
orthographic view. Entire building visible with margin. Solid RGB 255,0,255 background including
holes. No people, lettering, symbols, UI, watermark, scenery, effects or cast ground shadows.
