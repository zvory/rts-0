# Roster selective-color material review — approval pending

This is a preview-only material study for Rifleman infantry, Tiger tank and A-19 artillery.
No complete animation sets are authored, and no production renderer changes are included.
The review gate precedes extending the treatment across the remaining roster and animations.
No merge or auto-merge is authorized.

## Materials

- Rifleman: uniform and helmet receive team paint. Skin, leather, rifle wood and gunmetal stay fixed.
- Tank: broad hull panels, fenders and turret armor receive paint. Gun barrel, roof gun, grilles,
  exhausts, fittings, rubber and tracks stay fixed.
- Artillery: trail panels, carriage crossbeams, shield and broad cradle/breech plates receive paint.
  Tires, axle, pivots, recoil cylinders, exposed mechanisms and gun tube stay fixed.

The current catalog also includes Machine Gunner, loaded Rifleman/Panzerfaust, Mortar Team,
Anti-Tank Gun, Scout Car, Command Car, Rocket Launcher, faction infantry and special units.
These remain outside this initial representative review; approved boundaries should be extended
through their applicable movement, fire, setup and other poses before production integration.
Cultivator variants already have their own selective material treatment.

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

## Reproduce

From a clean task worktree with `npm ci` installed:

```sh
node scripts/art/roster-team-color-review.mjs
node scripts/art/roster-team-color-review.mjs --check
node scripts/art/roster-team-color-review.mjs --install-preview
node scripts/art/capture-roster-team-color-review.mjs
node scripts/art/roster-team-color-review.mjs --restore
```

The installer uses existing packed textures, backs up thirteen renderer modules beneath ignored
`target/`, and rewrites them only for local preview. It refuses existing backups; restore verifies
both backup hashes and installed files before writing, then removes the backup. Changed renderer
files or legacy backups require manual preservation/reconciliation rather than automatic overwrite.
Restore before committing. `--check` compares generated outputs without writing them.
Packing and checking refuse an installed preview because production renderer metadata is a source input.
The capture script requires an unused Interact runtime and uses its authoritative Lab,
pauses the scene, and takes seven separate game screenshots at camera scale 1 and DPR 2, using
seven colors from the current player palette. Each color launches the same checkpoint through a temporary catalog scenario in a fresh
Lab session. The catalog bytes are restored and the temporary scenario is removed in `finally`. It closes sessions and shuts down
the owned daemon in `finally`. The comparison board copies a terrain crop of each capture at 1:1
pixel size, adds external labels, and leaves unit pixels unchanged. Evidence JSON records each
capture's color, tick, camera, pause state, subjects, readiness errors and original publication URL.

The labeled source board and shared layer PNGs are under `generated/`. Game captures and evidence
are ignored local artifacts under `target/team-color-review-captures/`, never checked into Git.
The image generator's edits have small internal line/shading differences; original alpha preserves
outer boundaries, but this is an art review rather than a claim of pixel-identical recoloring.

## Expanded full-roster review

The follow-up covers 23 appearance/rank/faction rows: all 12 Kriegsia units (including the
Command Car-launched Scout Plane), nine Cultivator rows (worker, Novice, Bow Novice, all three
Adepts and all three Masters), and Ekat/Golem. The shared worker is shown in both factions.
The new Bow Novice from current main is included with a representative selective-color idle.
Existing Command Car, Cultivator, worker and Ekat/Golem selective/vector artwork is retained.
The Golem remains the current geometric prototype artwork.

Eight additional built-in imagegen edits are preserved in `source/`, with exact prompts and
reference paths in `expanded-prompts.json`. `references/machine-gunner-carry.png` is the first
sixth-width cell of the existing carry alpha sheet; `references/bow-novice.png` is the first
256×256 cell of the current archer strip. These crops are generation references only.
`roster-team-color-review-specs.mjs` declares the production image and representative cell sizes.
Rifleman, Panzerfaust, Machine Gunner, Bow Novice and Scout Plane use one preview frame; applicable
movement/setup/fire metadata points to that representative pose temporarily. This does not
constitute animation production or animation-continuity validation. All modular atlas transforms
and normal body scale remain in the actual renderer. Machine Gunner shows the carrying pose.

The extended packer emits shared fixed/paint layers and a labeled `expanded-source-material-board.png`.
Use the same pack/check/install/restore commands above; for all units replace the capture step with:

```sh
node scripts/art/capture-full-roster-team-color-review.mjs
```

An optional `--group=kriegsia`, `--group=cultivators` or `--group=ekat` recaptures that faction
and retains the other faction evidence. Each group is arranged in a clear region of the unchanged
Classic map. Off-screen headquarters remain to keep the scene valid. The Scout Plane is launched
through the normal Command Car ability and captured during flight; its crop follows its actual
paused position. No synthetic airplane entity is spawned. The command car is separated from the
launch position before capture. New captures must not overlap other unit artwork.

Local `target/full-roster-team-color-review/` contains the 21 raw game captures, full readiness/
subject evidence, and three labeled unit-by-color sheets. The sheets copy 200-world-pixel terrain
regions at 1:1 DPR-2 pixel size. They retain ordinary game health bars and effects. They are
presentation crops of actual game pixels; they are not separate team-color illustrations.

Rebuild the labeled sheets from existing captures without launching another game:

```sh
node scripts/art/roster-team-color-review-boards.mjs
```
