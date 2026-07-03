# Rifleman Backpack Hidden-Legs Experiment

This folder archives a non-production rifleman sprite-sheet generation pass. The art is good enough
to keep as evidence for future prompt and atlas work, but it is not wired into runtime rendering and
should not be treated as final production art.

## What Worked

- Use strict camera language: "strict nadir", "zenith orthographic", "plan-view", and "map token".
- State that the sprite must remain believable when rotated to any direction in-engine.
- Hide anatomy the model struggles to represent in rotation-safe top-down form. In this pass, a
  bulky backpack/bedroll plus gas-mask canister hides the lower body so the model does not try to
  draw side-view running legs.
- Ask for running/moving frames to read through shoulder movement, torso twist, pack sway, and rifle
  bob instead of visible leg motion.

## What Still Needs Work

- The generated sheet did not meet the square-cell layout target. It is six equal-width 362x724
  cells, so do not treat it as a ready atlas without explicit cropping or normalization.
- The frames are too similar to be a strong production run cycle without additional editing or a
  more controlled rig.
- Some equipment and arm details vary frame to frame.
- The sheet is an experiment only; it is not referenced by client code.

## Tooling Notes

- No project scripts were changed for this experiment.
- The alpha sheet was produced from a flat magenta background with the imagegen chroma-key helper.
- The animated previews were derived from six equal-width cells at 12 FPS.
