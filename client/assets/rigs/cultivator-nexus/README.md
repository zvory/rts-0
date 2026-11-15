# Cultivator lotus Nexus

Approved concept: a compact octagonal cultivation-sect headquarters with two tiers of
white-painted lotus-petal roof panels, a lotus-bud finial, warm timber structure and natural
stone foundation. Elevated front-centered orthographic view, approximately 45 degrees above
horizontal. Generated with the built-in image tool using existing building art as references.
Square solid magenta background in the source; no people, text, scene or effects.

The selected generation is preserved as `nexus-source.png` with 8-bit PNG encoding.
`nexus-atlas.png` is the exact approved in-game preview derivative: two 384×384 frames,
fixed-color base followed by a white-surface paint overlay. Pixi multiplies the paint layer by
the owning player's actual color. There is no emblem or shadow frame; the building atlas
explicitly disables the rectangular fallback footprint shadow. Wood and lower stone stay fixed.

Regenerate with `node scripts/art/nexus-pipeline.mjs`; verify reproducibility with `--check`.
The pipeline keys magenta, removes edge spill before premultiplied 4×4 supersampling, and
uses the approved low-saturation/high-value mask above 72% image height for the roof paint.
This is a presentation variant of `resource_depot` for Cultivators, not a new entity kind.
