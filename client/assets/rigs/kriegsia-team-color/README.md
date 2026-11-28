# Kriegsia selective team paint

Approved production treatment for Rifleman, Panzerfaust, Machine Gunner, Scout Plane,
Tank, Scout Car, Rocket Truck, Artillery, Anti-Tank Gun and Mortar Team. Engineer and
Command Car retain their existing material treatments. Cultivator, Adept/Master,
Bow Novice, Ekat and Golem artwork is unchanged.

Uniforms, helmets and designated painted armor/panels use shaded magenta in the
source artwork. Skin, black hair, wooden stocks, tan launchers/backpacks, leather,
blackened steel, tires and tracks keep natural material colors. Mortar outer tubes
and muzzle rims receive team paint, with black bores and dark steel fittings.

## Sources and reproduction

The approved representative edits, exact prompts and iterative material corrections
are retained in `../roster-team-color-review/`. `animation-prompts.json` records the
exact full-pose generation prompts and references; `source/` retains the three raw
built-in image_gen responses. All requests used `transparent_background: true`;
the tool exposed no model identifier or other settings. No separate team-color
images were generated.

`references/` holds contact sheets of every original infantry frame, four columns
at 2x size. Rifleman has five frames, loaded Panzerfaust eight (including wind-up),
and Machine Gunner fifteen (including setup). Scout Plane retains its single
frame. Articulated vehicle/support-weapon component atlases cover movement,
turning, recoil, deployment and packing through the existing rig transforms.

```sh
node scripts/art/kriegsia-team-color-references.mjs
node scripts/art/kriegsia-team-color-pipeline.mjs
node scripts/art/kriegsia-team-color-pipeline.mjs --check
node tests/rig_runtime.mjs
node tests/client_contracts.mjs
```

`specs.json` records canonical original assets, approved idle cells, raw animation
inputs and immutable cell dimensions. Packing aligns each generated cell to the
corresponding original alpha bounds; frame zero uses the approved idle artwork.
Original alpha locks each silhouette. Original Rifleman/Panzerfaust firing muzzle
pixels remain fixed. The Tank roof gun is copied from its original Scout Car gun
crop into the existing attachment coordinates; anchors and sampling are unchanged.

Magenta chroma/purity smoothsteps extract a soft shaded grayscale paint layer and
complementary fixed-material layer, using the Cultivator formula. Outputs contain
neutral composite row 0, empty row 1, fixed row 2 and paint row 3. Row 1 introduces
no independent effects. `--check` compares deterministic PNG bytes without writing.

`team_paint.js` derives and caches normal batchable player textures at runtime.
`renderRows: 1` avoids allocating an unused effect row; Cultivator defaults remain
unchanged. Portraits and animated minimap parts use these same fixed and paint rows
with alpha-preserving tint matrices. Animation routing, timing, facing, scales,
anchors, rig shadows, fog and independent projectile/native effects remain owned
by the original metadata and renderer.
