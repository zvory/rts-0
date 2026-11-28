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

The installer uses existing packed textures, backs up twelve renderer modules beneath ignored
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

## Expanded Kriegsia review and material corrections

Scope is Kriegsia only. Cultivators (including Bow Novice, Adepts and Masters) and Ekat/Golem
retain their existing production artwork and renderer metadata. The former Bow Novice study and
its temporary override have been removed at the user's request. Historical full-roster captures
are reference evidence; they do not authorize changes to those factions.

Seven additional built-in imagegen edits are preserved in `source/`, with exact prompts and
reference paths in `expanded-prompts.json`. `references/machine-gunner-carry.png` is the first
sixth-width cell of the existing carry alpha sheet. `roster-team-color-review-specs.mjs` declares
production images, representative sizes and selected authoring revisions.

Mortar v3 paints the entire outer tube and muzzle rim with team color; the bore is black.
Its axle, mounting collars, pivots and wheel hubs are dark blackened steel, and tires black rubber.
Anti-Tank Gun v2 similarly replaces pale/silver/coppery exposed fittings with dark neutral steel.
There is no white paint. Small shaded steel highlights remain gray. Both exact revision prompts
and their references are retained in `material-corrections-v2.json` and
`mortar-barrel-correction-v3.json`. The packer uses mortar v3 and anti-tank gun v2.

Rifleman, Panzerfaust, Machine Gunner and Scout Plane use one representative preview frame;
applicable movement/setup/fire metadata temporarily points to that pose. These material previews
are not complete animation sets or animation-continuity validation. Modular atlas transforms and
normal body scale remain in the actual renderer. Machine Gunner shows its carrying pose.

The same pack/check/install/restore commands above apply. All twelve temporarily patched renderer
files restore byte-for-byte, and no Cultivator/Ekat source file is among them. Existing Command Car
selective artwork remains unchanged.

The full-roster capture tool can inspect unchanged factions as references, or recapture Kriegsia only:

```sh
node scripts/art/capture-full-roster-team-color-review.mjs --group=kriegsia
node scripts/art/roster-team-color-review-boards.mjs
```

Existing game screenshot sheets copy 200-world-pixel regions at 1:1 DPR-2 pixel size. The Scout Plane
is launched through its normal Command Car ability and its crop follows its actual paused position.
The blue overview and color sheets are crops of actual game pixels, never separate AI team-color
illustrations. All local capture evidence remains under ignored `target/`.

For a live Lab review, serve an isolated copy of the installed preview client through
`RTS_CLIENT_DIR`, then restore the task checkout. The ordinary Lab allows spawning, movement,
combat, pausing, stepping and setup review. Material-only preview poses remain a limitation until
full animation production is approved. Live previews expire after one hour and must terminate their
owned server/supervisor and remove their copied client assets. They do not restart automatically.
