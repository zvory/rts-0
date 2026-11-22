# Proven Grounds

Generated with the built-in imagegen tool using the Cultivator Nexus, Three Dao Pavilion,
and Tournament Grounds source PNGs as style references. The tall open gate and unequal pillars
give it a distinct silhouette. `proven-source.png` preserves the generated source; the atlas
uses two 384×384 frames: fixed stone/timber, then team paint on the upper beam and pillar caps.

Regenerate the atlas with `node scripts/art/proven-pipeline.mjs`; verify with `--check`.
Chromakey, spill removal and premultiplied supersampling follow the existing faction pipeline.

Final generation prompt:

Use case: stylized-concept. Create ONE final game building sprite for a Cultivator RTS building named Proven Grounds. The three attached images are style references ONLY: 1 Nexus, 2 Three Dao Pavilion, 3 Tournament Grounds. Match their isometric/front three-quarter camera, hand-painted low-detail game illustration, dark clean outlines, warm timber, pale gray stone, ivory white painted surfaces, and crisp silhouette. NEW UNIQUE SILHOUETTE: tall open rectangular training gate, two unequal-height massive timber-and-stone uprights joined by a high ivory-painted horizontal beam, forming a big empty opening, above a small square stepped exercise courtyard with a simple practice dummy on one side. The footprint is compact square; unmistakably a training hall gate. Keep the upper silhouette dramatically open and angular. NO pagoda roof, NO lotus finial, NO three-lobed canopy, NO circular arena, NO roofed house. White painted broad gate beam and upper pillar caps will become team-color regions in game; pale stone steps and brown wood stay neutral. Single isolated complete building centered in square image with 6% margin on all sides. Flat solid pure magenta #ff00ff background for chromakey, including all openings. No scenery, no ground plane beyond building's own base, no text, no people, no watermark. Lighting upper left; comparable detail and scale to references.
