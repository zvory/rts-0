# Machine Gunner Pass 01

This pass replaces the authored SVG Machine Gunner body/weapon with a generated PNG frame strip.
The carry frames were edited from the accepted rifleman PNG sheet, then a single carry frame was
used as the reference for the deploy-only generation.

## Runtime Behavior

- Frames 0-5 are movement frames: the gunner carries an oversized MG42-style weapon across the
  body with a bulkier pack.
- Frames 6-11 are setup/deployed frames: setup starts from a carry pose, then rotates the MG into a
  south-facing deployed pose with the bipod open.
- The frame-strip renderer maps `setting_up` and `tearing_down` progress onto frames 6-11. A fully
  `deployed` Machine Gunner holds frame 11.
- The deployed art points down/south in the sheet, so runtime setup/deployed rendering applies a
  `PI/2` forward-angle offset to align that sprite direction with authoritative `weaponFacing`.

## Source Notes

- Keep this no-feet top-down convention for future infantry passes. Earlier attempts drifted into
  front-facing boots and standing character art.
- The setup pass works better when generated separately from movement. Do not ask for movement,
  setup, and final deployed frames in one prompt unless there is a stronger reference sheet.
