# Experimental Rifleman, version 3

Available only through Settings → Game → Use experimental graphics. The persisted
preference stays off by default. The atlas explicitly opts into the upright
billboard treatment when the experimental setting tilts the battlefield. Disabling it restores the regular Rifleman;
health-bar visibility, portraits, minimap icons and simulation rules stay separate.

Sixteen headings use sixteen poses each (256 total), with 128×128 cells in a
2048×2048 atlas. The order starts at screen south (pi/2), then advances by pi/8:
south, SSW, SW, WSW, west, WNW, NW, NNW, north, NNE, NE, ENE, east, ESE, SE, SSE.
The pose order is idle, ready, six running poses, settle, raise, shoulder, aim,
recoil, bolt, re-aim and lower. Frames are heading * 16 + pose.

The standing sole-contact midpoint is (64,112). Running uses a stable family ground
point and belt registration; it does not center on lifted boots. Cardinal artwork
retains the approved pixels. New families target the approved standing height with
a 21–24px helmet-size guard. Generated geometry owns heading-specific HP clearance
and muzzle positions. The first shot's 350ms shoulder windup and flash delay are
experimental presentation only; damage and server combat timing are unchanged.

```sh
node scripts/build-rifleman-sixteen.mjs
node scripts/build-rifleman-sixteen.mjs --check
node scripts/check-rifleman-sixteen.mjs
node tests/client_contracts/experimental_graphics_contracts.mjs
```

Selected built-in image_gen inputs live in `sources/`. `prompts.json` records exact
prompts, actual generator reference paths, portable selected source paths and the
original generated output paths for the twelve new headings. `reference-prompts.json`
retains cardinal generation history and the selected south prompt. West's approved
carry/run and braced firing sources were reused without regeneration. Rejected
first passes are retained in the local prototype, not this runtime asset family.

Fixed RGB, paint and alpha are split at source resolution, then area-filtered in
premultiplied form. The generator rejects clipping and never applies an old alpha
outline to regenerated RGB. `coverage-atlas.png`, registration, geometry and approved
cardinal hashes provide reproducible verification. Only the three runtime atlas
images are included in the deployment context; raw authoring sources remain in Git.

The new set covers idle, locomotion and rifle handling; additional death or special
ability art is outside this experimental revision.
