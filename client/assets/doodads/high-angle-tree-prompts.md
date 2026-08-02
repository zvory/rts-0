# Accepted high-angle tree generation

Generated on 2026-08-02 with the built-in OpenAI image-generation tool. Each generation used the
previous accepted three-quarter species sprite as Image 1 and the rejected strict vertical sprite
as Image 2, with this shared direction:

> Generate one new Eastern Front-native tree at an intermediate high camera angle exactly between
> Image 1 and Image 2. Use a near-orthographic view with the optical axis about 40–45 degrees away
> from vertical top-down. The canopy should dominate, but part of the trunk and canopy front edge
> must remain visible. Match Image 1's early-3D/painted RTS raster aesthetic and restrained summer
> palette. Center one complete tree with generous padding on a perfectly uniform `#ff00ff`
> chroma-key background. No flowers, additional trees, ground, grass, shadows, scenery, text,
> labels, watermarks, gradients, or magenta in the tree.

Species-specific direction:

- English/pedunculate oak: broad irregular lobed crown and sturdy branching trunk.
- Scots pine: open irregular high crown, rounded needle clusters, and visible reddish upper trunk;
  never a dense spruce silhouette.
- Silver birch: airy yellow-green crown, white bark with dark markings, and delicate branching.
- Norway spruce: dense blue-green cone, layered drooping boughs, and short visible trunk.
- Eurasian aspen: narrow softly rounded pale blue-green crown, fine leaves, and slender gray trunk.
- Black alder: compact uneven dark-green crown with several dark low branching stems.

The built-in outputs were keyed with `remove_chroma_key.py` using border auto-key detection, soft
matte, despill, transparent threshold 12, and opaque threshold 220. Each accepted tree was then
trimmed, proportionally fit inside 118 × 118 pixels, bottom-centered on a transparent 128 × 128
runtime canvas, and wired under its existing `tree.<species>` id.
