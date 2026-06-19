# Phase 5.2 - Named Part Capture and Part-Level Gates

## Phase Status

- [x] Done.

## Objective

Add a named-part capture contract so each legacy procedural visual part can be compared against the
matching SVG rig part before the full unit composition is compared.

## Why This Phase Exists

Whole-unit pixel diffs are necessary, but they can leave an LLM with a broad failure and too much
search space. The Worker miss was easier to describe as part failures: the body primitive changed,
the body outline was absent, and the facing binding was attached to the body instead of only the
line. This phase breaks the visual problem into named parts so future executors can fix one
mechanical mismatch at a time.

## Work

- Introduce a test-only visual capture API for legacy unit drawing that records stable part names
  around draw operations, such as `worker.shadow`, `worker.body`, `worker.facingTick`, and
  `worker.busyIndicator`.
- Keep the capture API out of production gameplay state. The normal renderer should still draw the
  same visuals; tests may either call extracted part draw helpers directly or enable a test-only
  recorder.
- Add a rig-side part filter that can render one normalized rig part or a named group of rig parts
  into the same transparent fixture used by Phase 5.1.
- Compare legacy part output to rig part output with stricter thresholds than full-unit composition.
  Simple primitives such as Worker body, outline, facing line, and shadow should have very small
  tolerances.
- Add diagnostic failure output that names the failing unit, sample, and part before writing the
  same legacy/rig/diff artifacts as the composition harness.
- Start with Worker part mappings and make the mapping format reusable for infantry, support
  weapons, and vehicles in later phases.

## Expected Touch Points

- `client/src/renderer/units.js` or extracted unit part helpers.
- `client/src/renderer/rigs/runtime.js` or a test-only rig part renderer helper.
- Visual harness tests from Phase 5.1.
- Worker SVG fixture/source if part ids need normalization.
- `plans/svg/phase-5.2.md`.

## Implementation Checklist

- [x] Add stable Worker legacy part names for shadow, body, facing tick, and busy indicator.
- [x] Add rig-side single-part or part-group render support for tests.
- [x] Add Worker part mapping from legacy part names to rig part ids.
- [x] Add part-level pixel comparison tests with stricter per-part thresholds.
- [x] Preserve normal live renderer behavior outside test capture.
- [x] Run verification and record exact results.

## Verification Results

- `node tests/rig_runtime.mjs` passed.
- `node scripts/check-client-architecture.mjs` passed.
- `node --check tests/transparent_unit_pixels.mjs && node --check client/src/renderer/units.js && node --check client/src/renderer/rigs/runtime.js` passed.
- `node tests/transparent_unit_pixels.mjs --parts-only --expect-failures --no-artifacts` was attempted after linking the exact lockfile-hash `tests/node_modules` cache; this sandbox blocked the fixture server with `listen EPERM 127.0.0.1` before the harness could run.
- `node tests/transparent_unit_pixels.mjs --expect-failures --no-artifacts` hit the same sandbox `listen EPERM 127.0.0.1` fixture-server blocker.

## Verification

- Worker part-level visual comparison command.
- Worker full-composition visual comparison command from Phase 5.1.
- Existing rig runtime tests if rig filtering or runtime code changes.
- `node scripts/check-client-architecture.mjs` if client module boundaries change.
- `git diff --check`.

## Manual Test Focus

No gameplay manual test is required for this phase. If a part comparison fails during development,
open the generated artifacts only to debug the implementation; pass/fail must come from the
mechanical comparison result.

## Handoff Expectations

List the Worker part names, their matching rig part ids, and the per-part thresholds. State whether
the part capture is reusable for later unit kinds and whether any legacy draw code was extracted or
renamed.
