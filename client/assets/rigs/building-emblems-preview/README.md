# Building emblems preview

Second local image-generation pass for five production-building identifiers. Unlike the rejected
badge pass, these are bare silhouette-first emblems with no plate, border, frame, bolts, or backing
field. The Resource Depot remains unchanged.

Each emblem is retained as a standalone 256×256 alpha PNG. The selected `*-team-tint.png`
variants use flat white paint with a thick black outline. Their preview atlases append one
full-frame emblem layer after the existing base, team-tint, and silhouette-shadow frames, so the
emblems float above the building art without changing it. At runtime, the renderer multiplies the
emblem layer by the owning player's color: white becomes the faction color while black stays black.

- Barracks: one simplified M14-style rifle silhouette
- Training Centre: MG42-style machine gun without a bipod, crossed with a Panzerfaust-style launcher
- Engineering Complex: atomic symbol with eight electrons
- Vehicle Works: medium tank silhouette
- Gun Works: field-artillery cannon silhouette

The chroma sources, extracted alpha intermediates, atlas-sized overlay frames, generation prompt,
and manifest are retained under `generated/` and `metadata/`.

The selected silhouette source for Gun Works is `steelworks-emblem-v2.png`; the first cannon color
pass remains in the directory as review history. The live preview references the versioned M14
barracks and MG42/Panzerfaust training-center atlases plus the other three `*-atlas-team-tint.png`
atlases.
