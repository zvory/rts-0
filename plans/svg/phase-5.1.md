# Phase 5.1 - Transparent Pixel Buffer Harness

## Phase Status

- [ ] Not implemented.

## Objective

Create a deterministic transparent-buffer visual comparison harness for legacy procedural units and
rigged units, so unit art migration has a mechanical pixel gate instead of manual visual review.

## Why This Phase Exists

Phase 5 let a visually wrong Worker rig pass because the checks were mostly semantic measurements:
anchors, bounds, tint, busy visibility, and routing. Those checks could not see that the body
silhouette changed, the outline disappeared, and the wrong part rotated. This phase creates the
lowest-level mechanical oracle: render legacy and rig output through Pixi into transparent pixel
buffers and compare the RGBA data directly.

## Work

- Add a focused headless-browser/Pixi fixture path that can render one unit sample into a fixed-size
  transparent canvas without the full match HUD, terrain, fog, or lobby flow.
- Render both the legacy procedural path and the normalized rig runtime path through Pixi, not raw
  browser SVG DOM, so the comparison covers the actual runtime renderer.
- Add reusable pixel-buffer comparison helpers for RGBA buffers: alpha-weighted matching ratio, max
  per-pixel RGBA distance, opaque mismatch count, mismatch bounding boxes, and connected mismatch
  cluster size.
- Save failure artifacts only when a comparison fails: `legacy.png`, `rig.png`, `diff.png`, and a
  compact JSON report under an ignored test-artifact directory.
- Wire the harness to the existing Phase 1 sample matrix and thresholds, but start with Worker-only
  samples so this phase proves the harness without requiring all rigs to exist.
- Document how to run the focused harness locally and how to inspect failure artifacts when a later
  executor needs to debug a failing conversion.

## Expected Touch Points

- `tests/` visual harness script and helpers.
- `tests/fixtures/svg/` fixture or baseline metadata, if needed.
- `client/` fixture page or test-only module only if the browser needs a stable Pixi entry point.
- `.gitignore` for generated failure artifacts.
- `plans/svg/phase-5.1.md`.

## Implementation Checklist

- [ ] Add deterministic transparent Pixi render fixture for legacy and rig paths.
- [ ] Add RGBA buffer comparison helpers with threshold reporting.
- [ ] Add failure artifact writing for legacy/rig/diff PNGs plus JSON report.
- [ ] Add Worker-only composition comparison samples covering facings, team tint, and busy state.
- [ ] Keep generated visual artifacts out of git except intentional baseline metadata.
- [ ] Run verification and record exact results.

## Verification

- Focused transparent-buffer visual harness command for Worker samples.
- Existing rig schema/importer/runtime tests if touched.
- `node scripts/check-client-architecture.mjs` if client modules are added or import boundaries
  change.
- `git diff --check`.

## Manual Test Focus

No gameplay manual test is required for this phase. If a failure artifact is produced during local
debugging, manually confirm the artifact files open and show transparent legacy, rig, and diff
images.

## Handoff Expectations

Report the harness command, the Worker samples covered, the exact pixel thresholds used, and the
artifact directory. State whether the harness compares Pixi-vs-Pixi runtime output and whether it
is ready for named part-level comparison in Phase 5.2.
