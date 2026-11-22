# Cultivator Tournament Grounds

Generated with the built-in imagegen tool using the Cultivator Nexus and Three Dao Pavilion
source images as visual references. The open stone fighting arena, timber railing, rear pavilion,
white-painted curved roof and lotus finial share their elevated 45-degree orthographic camera and
hand-painted outline style. There are no people, effects, lettering or cast ground shadows.
The building occupies a 3x3 gameplay footprint; its circular arena sits inside that square.

`tournament-source.png` preserves the generated concept on the requested solid magenta background.
`tournament-atlas.png` contains two 384x384 frames: fixed-color base then roof/finial team-paint
layer. Runtime player tint multiplies the white paint layer. Natural stone, timber and railing
panels stay fixed. Use equal X/Y scale, `visualScale: 1.1`, `silhouetteShadow: false`, and
`omitFootprintShadow: true`, matching the existing Pavilion presentation.

Regenerate with `node scripts/art/tournament-pipeline.mjs`; verify with `--check`. The pipeline
keys magenta, removes edge spill, and downsamples with premultiplied 4x4 supersampling. Coverage
below 8% is discarded to remove faint chromakey residue. The neutral-bright paint mask is
restricted to the upper 38% of the source so the fighting platform stays natural stone.

## Generation prompt

One isolated RTS Tournament Grounds for the Cultivators faction. Use Nexus and Dao Scripture
Pavilion only as style, material and camera references. A compact open-air martial tournament
arena on a 3x3 footprint: broad empty stone platform, low timber railing with white-painted panels,
a small rear pavilion with white curved roof, warm timber posts and lotus-bud finial, open front
entry and shallow steps. The arena is the dominant shape. Entire building centered with margin,
front-centered orthographic elevated 45-degree view, equal X/Y scale, relatively low detail and
broad readable shapes. Flat solid RGB 255,0,255 background, including holes behind posts. No
people, scene, text, symbols, effects, glow, cast ground shadows, watermark or UI.
