# Machine Gunner Pass 01

This folder archives a first machine-gunner PNG generation pass based on the production rifleman
style reference. It is intentionally not wired into runtime rendering.

## What Worked

- The generated poses stay upright, including the setup/deployed frames.
- Frames 1-6 read as carried movement: the machine gun stays close to the chest and is not firing.
- Frames 7-9 form a simple setup progression, with the gun pointing forward and the bipod visible
  in the final ready frames.
- The art style is close to the accepted rifleman PNG strip: compact top-down token, dark outline,
  muted field-grey uniform, backpack/bedroll gear, and readable weapon silhouette.

## What Still Needs Work

- The raw image is a 2172x724 contact sheet with nine visual poses, but the poses are not exact
  equal cells. Do not treat the source sheet as directly slice-ready.
- The transition from carried to ready is abrupt because this pass asks for only three setup frames.
  A future pass may need more intermediate frames or hand-authored interpolation if deployment
  should play slowly.
- The final deployed pose is closer to a standing braced/hip-fire stance than a true bipod-rested
  firing posture, which matches the current request but may need review in motion.

## Tooling Notes

- Generated with the built-in image_gen tool using the production rifleman PNG sheet as a visible
  style reference.
- The alpha sheet was produced from the generated magenta background with the imagegen chroma-key
  helper and a one-pixel edge contraction to reduce fringe.
- The 96px review strip and 8 FPS WebP loop are derived from connected-component crops, not equal
  source-cell crops.
