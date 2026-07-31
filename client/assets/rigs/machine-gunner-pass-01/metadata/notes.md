# Machine Gunner white-chroma pipeline

The MG now follows the same whole-frame rule as Rifleman: white-painted RGB and alpha come from
the same generated artwork. Carry, deployment, and firing sheets use a magenta chroma background;
their `*-alpha.png` counterparts are the only inputs to the runtime-strip builder.

`scripts/art/machine-gunner-white-pipeline.mjs` splits those sheets into frames, keeps the complete
connected character in each cell, fits it within six pixels of a 128×128 frame, and resamples RGBA
together. It does not consult older green art, transfer masks, color guides, or compact silhouettes.

Frames 0–5 are carry movement, frame 6 repeats the first carry pose, frames 7–11 deploy, and frames
12–14 fire. The client applies the shared `team-light` tint and 70% target brightness used by the
Rifleman strip.
