# Phase 3 - Stress, Polish, And Docs

## Phase Status

- [ ] Done.

## Objective

Prove the decal system remains cheap and readable after large numbers of deaths, then polish the
visual behavior and document the renderer/asset contract. This phase should turn the feature from a
working client effect into a shippable visual system.

## Scope

- Add stress coverage.
  - Exercise hundreds or thousands of synthetic decal stamps without creating hundreds or thousands
    of Pixi display objects.
  - Verify old decals are not redrawn or iterated every normal render frame after stamping.
  - Verify texture update count is tied to new-death batches, not historical decal count.
  - Verify dedupe state prevents double stamping the same death id.
- Add renderer diagnostics or test hooks if Phase 1 did not already add them.
  - Total decals stamped.
  - Pending decals.
  - Texture update count.
  - Decal texture dimensions/downsample.
  - Decal layer child count.
- Tune visuals.
  - Adjust tint strength, opacity, scale ranges, and vehicle/support hull dimensions.
  - Confirm player-colored infantry marks read intentionally as paint/blood rather than accidental
    terrain noise.
  - Confirm vehicle/support marks remain blackened/scorched while still showing team color.
  - Confirm marks do not obscure unit selection rings, HP bars, placement ghosts, or combat
    feedback.
- Update docs.
  - Update `docs/design/client-ui.md` if a new renderer module, asset directory, or lifecycle rule
    should be part of the documented client surface.
  - Refresh `docs/context/client-ui.md` only if section lists or code map entries shift.
  - Mention that decals are client-only, best-effort visual state derived from transient death
    events.

## Expected Touch Points

- `client/src/renderer/decals.js` or split decal renderer helpers
- `client/src/state_ground_decals.js` or equivalent decal buffer
- Renderer diagnostics or frame profiler integration if useful
- Focused client tests under `tests/`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md` only if needed

Avoid touching:

- Server protocol and simulation code
- Match history and replay artifact code
- Broad test bundles unless a focused change makes them necessary

## Implementation Details

- Prefer a deterministic synthetic test harness over a flaky live combat setup for high-count
  stress. The test can feed normalized decals directly into the renderer helper if that proves more
  stable than forcing thousands of in-game kills.
- Keep stress instrumentation out of normal player-facing UI.
- Do not solve replay seek or reconnect persistence in this phase. Document the best-effort behavior
  instead.
- Treat texture size as a product/performance knob. If the chosen downsample makes infantry marks
  unreadable, tune it with measured memory/update cost rather than jumping to full world
  resolution.
- Make sure `Renderer.destroy()` releases every new GPU/canvas resource and ignores any late asset
  loads.

## Verification

- `node scripts/check-client-architecture.mjs`
- Focused decal stress/contract test added in this phase
- Existing focused renderer/client test touched by the implementation, if any
- `node scripts/check-docs-health.mjs` if docs change
- `git diff --check`

Do not run broad bundles by default. Let the PR `./tests/run-all.sh` gate cover full-suite
regression unless the phase touches a wider surface than expected.

## Manual Testing Focus

Run a local match or dev scenario with repeated infantry and vehicle/support deaths. Confirm marks
remain permanent for the current match, are cleared between matches, stay under fog and unit
overlays, show owner/player tint, and do not produce visible frame drops after many deaths. Use a
synthetic high-count stress route or test harness if available to inspect hundreds or thousands of
marks quickly.

## Handoff Expectations

The handoff must include the stress count tested, decal layer child count after stress, decal
texture dimensions/downsample, focused verification commands, and manual observations for
readability and FPS feel. It must also call out the accepted limitation that client-only decals are
not replay-seek or reconnect persistent.
