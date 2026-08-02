# Doodad sprite sources

These runtime PNG sprites were generated with the built-in OpenAI image-generation tool on
2026-08-02, using a flat `#ff00ff` chroma-key background. The project-local alpha PNGs were made
with the Codex image-generation skill's `remove_chroma_key.py` helper, then trimmed and resized
with ImageMagick. The generated originals remain outside the repository in Codex's generated-image
store.

All prompts requested an isolated, elevated three-quarter top-down, low-poly/PS1-style RTS sprite
with restrained natural colors, crisp edges, no shadow, no text, no watermark, and no scenery.
The subjects were:

- `tree-oak.png`: a mature broadleaf oak with a sturdy brown trunk and rounded olive canopy.
- `tree-pine.png`: a mature Scots pine with a narrow trunk and tiered deep-green needles.
- `tree-birch.png`: a European birch with a white-charcoal trunk and airy yellow-green canopy.
- `tree-spruce.png`: a Norway spruce with dense drooping blue-green boughs.
- `tree-aspen.png`: a Eurasian aspen with a pale trunk and loose blue-green crown.
- `tree-alder.png`: a black alder with dark forked trunks and dense rounded foliage.
- `tree-*-topdown.png`: a second version of every tree species rendered from a strict 90-degree
  vertical orthographic camera for in-game perspective comparison.
- `wildflower-single.png`: one pale five-petal flower with a short green stem and two leaves.
- `wildflower-cluster.png`: a cohesive tuft of five pale blossoms with short stems and sparse leaves.

The flower sprites use neutral petals so the runtime renderer can apply authored color tinting.
