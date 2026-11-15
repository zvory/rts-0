# Tangled barbed wire

Generated with built-in ImageGen from the existing barbed-wire SVG raster reference.
Selected candidate 5 after in-game review. Neutral steel and wood; no team tint.

Use case: stylized-concept. Asset type: single neutral barbed-wire obstacle PNG candidate for a top-down WWII RTS. Input image 1 is the ACTUAL EXISTING SOURCE ASSET, a style, geometry, scale and readability reference, not a scene to reproduce. Reimagine it as polished hand-painted low-poly / PS1-inspired game sprite, bold clean silhouette and restrained flat material shading, dark readable edges, moderately chunky steel so it survives downsampling to a 32-world-pixel tile. Maintain the reference's overhead game projection: loops read as upright narrow ellipses seen from a high overhead angle; do not use eye-level side elevation or cinematic perspective. One compact horizontal modular barrier section, isolated, centered, entirely inside canvas with margin, similar square-ish footprint to reference. Neutral obstacle like a tank trap: natural dull gray steel, slightly warm dark weathered wooden stakes when present; NO team paint, NO forced white repaint, NO faction colors. Perfectly flat uniform solid #ff00ff magenta everywhere outside the object AND through every gap/hole, easy chroma keying. No floor, grass, dirt, environment, ground/base plate, ambient backdrop variation, cast/drop shadows, glow, gradient, text, labels, UI, frame, watermark, characters, weapons, bonus objects, montage or alternative views. Small readable barbs, selective detail, prioritize negative space and silhouette over photoreal microtexture. One sprite only. Design 5 — compact irregular wire entanglement: a small asymmetrical bundle of three angular steel loops held by three short rough wooden stakes, sparse protruding hooked barbs; organized readable interlocking silhouette with two or three large open gaps, rugged improvised field obstacle, no random hairball or dense mesh.

Run `node scripts/art/barbed-wire-pipeline.mjs` to rebuild the 256×256 runtime PNG.
The sprite uses a fixed-color part, centered at (128,128), 8 pixels per world pixel,
and a 32×32 world footprint. Chroma key, edge despill, tight alpha crop, 240px
maximum silhouette extent and 4×4 supersampling match the approved preview.
The existing native footprint shadow remains; the source contains no cast shadow.
