# Phase 3 - Combat Audibility Envelope and Listening Checkpoint

## Phase Status

- [ ] Not started.

## Objective

Reduce irrelevant edge and offscreen combat noise with one tighter combat-only radial profile, then
prepare a real-match checkpoint where the user/manual tester can evaluate all three phases together.
Keep the implementation tied to the existing renderer-neutral listener reference distance rather
than introducing exact viewport geometry or new camera dependencies.

## Initial Tuning Targets

Use one shared profile for both `combat_self` and `combat_other`:

- full gain through `0.4 * referenceDistancePx`
- retain the current `4.0` far-distance effect multiplier after that near region, producing no more
  than `0.5` linear gain at `0.5 * referenceDistancePx` and no more than `0.15` linear gain at
  `1.0 * referenceDistancePx`
- hard drop beyond `1.2 * referenceDistancePx`
- retain current left/right pan, distance low-pass, and short in-flight parameter ramps

These are initial listening values, not a permanent acoustic standard. Keep them as a few named
constants so one manual adjustment is easy if the integrated checkpoint finds the edge transition
clearly too aggressive or too weak.

## Work

- Select spatial parameters from the voice category or a similarly small explicit profile marker.
  Apply the new envelope only to `combat_self` and `combat_other`.
- Preserve the current default spatial profile for any non-combat spatial caller. Although current
  positioned callers are combat, do not silently redefine future notification, voice, ambient, or
  UI positioning behavior.
- Replace the current combat flat-through-one-reference-distance / drop-at-three-reference-distances
  behavior with the initial targets above. Retain monotonically decreasing gain and low-pass cutoff
  after the near region.
- Ensure active spatial voices remember enough category/profile information for
  `setListener()` camera updates to recompute the same combat envelope. Preserve the current smooth
  ramp so camera pans and minimap jumps do not create zipper noise or sudden full-volume pops.
- Keep distance-based priority penalties consistent with the tighter profile so a far voice does not
  occupy guarded combat capacity ahead of a materially nearer voice.
- Do not import camera, renderer, viewport, DOM, or Pixi modules into `audio.js`. Do not add
  rectangular viewport tests, screen-edge raycasts, per-weapon distances, or offscreen notice
  synthesis.
- Preserve Phase 1 notice audio as centered/non-spatial and preserve Phase 2 combined/family limits.
- Update `docs/design/client-ui.md` with the combat-only profile and integrated first-pass behavior.

## Expected Touch Points

- `client/src/audio.js`
- `docs/design/client-ui.md`
- `tests/client_contracts/audio_contracts.mjs`

## Implementation Checklist

- [ ] Add a combat-only spatial profile selected without camera/renderer coupling.
- [ ] Apply the initial near, attenuation, and hard-drop targets.
- [ ] Keep default non-combat spatial behavior unchanged.
- [ ] Recompute in-flight combat voices with the same profile after listener movement.
- [ ] Align distance priority with the tighter envelope.
- [ ] Add focused near/edge/drop and listener-refresh contracts.
- [ ] Prepare a runnable integrated dense-battle listening checkpoint and focused checklist for the
  user/manual tester.
- [ ] Update the client UI design document.
- [ ] Mark this phase done in this file in the implementation commit.

## Verification

- `node tests/client_contracts/audio_contracts.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

Focused contracts should cover combat gain in the near region, substantial attenuation around one
reference distance, hard drop beyond the chosen maximum, unchanged default non-combat spatial
behavior, and consistent recomputation for active voices after listener movement. They must also
assert that the combat distance penalty is monotonic, so an otherwise equivalent nearer voice
outranks a farther one under Phase 2 admission pressure.

## Manual Test Focus

Provide a runnable local release match or deterministic setup, plus the command or Tailnet link and
the short checklist below, so the user/manual tester can listen on their actual device. Do not use a
Lab screenshot or capture as audio evidence. The tester should use the same dense mixed fight at
camera center, near the visible edge, just offscreen, and far offscreen; check that close combat
remains satisfying, edge combat recedes, far routine fighting drops out, and camera pans/minimap
jumps do not pop or abruptly brighten active sounds. During the same fight, trigger an existing
under-attack or command-feedback voice and check that Phase 1 ducking remains clear and gradual while
Phase 2 limits leave the battle busy rather than hollow.

This user/manual listening pass is the plan's measured checkpoint, not an automated merge gate.
Record concrete observations about notice intelligibility, perceived combat density, missing
important heavy cues, edge/offscreen noise, and camera-transition artifacts before proposing any
follow-up; do not automatically create more phases from speculative concerns.

## Handoff Expectations

Report the final combat profile constants, automated checks, and the exact local setup/link and
checklist prepared for the user. Leave subjective mix conclusions pending until the user/manual
tester reports whether the first pass achieved understandable existing notices, bounded but busy
combat, and a useful edge/offscreen falloff. List only follow-up candidates backed by that
checkpoint; asset-tail work, normalization, limiting, or combat-event aggregation remain deferred
until the user chooses a new plan.
