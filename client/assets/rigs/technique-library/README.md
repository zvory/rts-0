# Technique Library

Generated with the built-in ImageGen tool from the Portal (`proven-source.png`), Three Dao
Pavilion and Cultivator Nexus source images. A white-painted single-storey scroll library,
open shelves and central reading lectern on a stone foundation; solid magenta source background.

`library-source.png` preserves the generation. `library-atlas.png` contains two 384×384 frames:
fixed-color base and white roof/finial team-paint overlay. Timber, parchment and stone stay fixed.
The roof paint mask follows the raised center eave (32.5–43% image height), excluding the gable. Runtime uses equal X/Y scaling and omits the
rectangular footprint shadow, following the Pavilion/Nexus.

Regenerate with `node scripts/art/library-pipeline.mjs`; verify with `--check`. The pipeline keys
magenta and removes spill before premultiplied 4×4 supersampling. Visual approval is pending.

## Exact generation prompt

Use case: stylized-concept. Asset type: isolated RTS building sprite, Technique Library for the Cultivators faction. Input images are reference images only: image 1 Portal for timber and white painted beam details, image 2 Three Dao Pavilion for scrolls and stone dais, image 3 Nexus for painterly rendering and white curved roofs. Create a distinct compact single-storey sect library on a square stone platform, warm timber posts, a broad white-painted gently curved roof, open front revealing orderly shelves of rolled parchment technique scrolls and a central reading lectern with an open scroll. Recognizably a library, smaller and simpler than the Nexus, more enclosed than the Pavilion. Elevated front-centered orthographic camera approximately 45 degrees above horizontal, same game sprite style, crisp painted outlines and readable chunky shapes. Roof and decorative upper panels painted neutral white for runtime team tint; wood warm brown, stone neutral gray, parchment cream. Entire building centered and fully visible with margin in square image. Solid flat pure magenta #ff00ff background, no gradient, no cast shadow, no ground scene, no people, no lettering, no text, no glow, no watermark.
