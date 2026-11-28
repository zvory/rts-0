# Approved roster material review archive

The representative and expanded material treatment was approved for production,
including the final mortar barrel correction. Production animations, masks and
reproduction instructions now live in `../kriegsia-team-color/README.md`.
This directory retains the original review sources, exact prompts and intermediate
corrections as provenance. Cultivator and Ekat artwork is outside this treatment.

## Materials

- Rifleman: uniform and helmet receive team paint. Skin, leather, rifle wood and gunmetal stay fixed.
- Tank: broad hull panels, fenders and turret armor receive paint. Gun barrel, roof gun, grilles,
  exhausts, fittings, rubber and tracks stay fixed.
- Artillery: trail panels, carriage crossbeams, shield and broad cradle/breech plates receive paint.
  Tires, axle, pivots, recoil cylinders, exposed mechanisms and gun tube stay fixed.

Expanded sources include Machine Gunner, Panzerfaust, Mortar Team, Anti-Tank Gun,
Scout Car, Rocket Truck and Scout Plane. Final production uses mortar v3 (painted
outer tube) and Anti-Tank Gun v2 (dark steel) as named in the review specifications.

## Provenance and processing

`source/*.png` are built-in `image_gen` material edits, requested with transparent backgrounds.
`prompts.json` contains each exact prompt. Each edit used exactly one reference:

- Rifleman: `client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/no-pack/idle-alpha.png`
- Tank: `client/assets/rigs/tank-ps1/tank-atlas.png`
- Artillery: `client/assets/rigs/artillery-a19-pass-03/generated/artillery-a19-components-pass-03-alpha.png`

The generator exposed no model identifier or further settings. Its three raw responses are retained
unchanged. No artwork is AI-generated separately for player colors.

The packer rotates/bounds-aligns the Rifleman to its production idle cell. Vehicle and artillery
retain their atlas layout. Original production alpha clips each edit, removes generated background
and glow, and locks the silhouettes/transparency. Magenta chroma and purity smoothsteps extract
soft grayscale shaded paint and complementary fixed-material layers using the Cultivator formula.
The existing roof gun is copied into an unused tank atlas region for the temporary review texture.
The packed four-row format is consumed by the existing `team_paint.js` cache. Normal production
brightness/saturation, world scale, anchors, articulated transforms and shadow rigs are retained.
This study does not validate animation continuity, fog, portraits, minimap or independent combat
VFX; those belong to the implementation after approval.

## Reproduce the archived material packing

```sh
node scripts/art/roster-team-color-review.mjs
node scripts/art/roster-team-color-review.mjs --check
```

The former temporary preview installer is retired. Production metadata now routes
to the complete shared artwork/masks directly. Capture scripts render that current
production artwork using Interact; temporary scene manifests are restored in finally.
