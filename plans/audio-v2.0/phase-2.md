# Phase 2 - Combat Audibility Envelope and Listening Checkpoint

## Phase Status

- [ ] Not started.

## Objective

Reduce irrelevant edge and offscreen combat noise with one tighter combat-only radial profile, then
prepare a real-match checkpoint where the user/manual tester can evaluate both phases together.
Keep the implementation tied to the existing renderer-neutral listener reference distance rather
than introducing exact viewport geometry, combat-family budgets, or new camera dependencies.

## Initial Tuning Profile

Use one shared profile for both `combat_self` and `combat_other`. Let `r` be
`referenceDistancePx`, `d` be radial listener-to-emitter distance, `near = 0.4 * r`, and
`maxDistance = 1.2 * r`:

- hard drop the request when `d > maxDistance`
- otherwise compute `effective = near + max(0, d - near) * 4.0`
- compute linear distance gain as `near / max(effective, near)`
- keep gain at `1.0` through `0.4 * r`; the equation yields `0.5` gain at `0.5 * r` and about
  `0.143` gain at `1.0 * r`
- keep the existing left/right pan based on the listener reference distance
- interpolate low-pass cutoff from the existing near cutoff to the existing far cutoff using
  `t = clamp((d - near) / (maxDistance - near), 0, 1)`, so it reaches the far cutoff at the `1.2 * r`
  hard-drop boundary rather than substantially earlier
- use the same `t` for a simple distance priority penalty of `30 * t`, keeping voices inside the
  full-gain region equal and making otherwise equivalent voices lose priority monotonically as they
  approach the hard-drop boundary

These are initial listening values, not a permanent acoustic standard. Keep them as a few named
constants so one manual adjustment is easy if the integrated checkpoint finds the edge transition
clearly too aggressive or too weak.

## Work

- Select spatial parameters from the voice category or a similarly small explicit profile marker.
  Apply the new envelope only to `combat_self` and `combat_other`.
- Preserve the current default spatial profile for any non-combat spatial caller. Although current
  positioned callers are combat, do not silently redefine future notification, voice, ambient, or
  UI positioning behavior.
- Implement the exact initial gain, low-pass, hard-drop, and distance-priority equations above.
  Do not substitute another curve during implementation; tune only from the listening checkpoint.
- Ensure active spatial voices remember enough category/profile information for
  `setListener()` camera updates to recompute the same combat envelope. Preserve the current smooth
  ramp so camera pans and minimap jumps do not create zipper noise or sudden full-volume pops.
- Do not import camera, renderer, viewport, DOM, or Pixi modules into `audio.js`. Do not add
  rectangular viewport tests, screen-edge raycasts, per-weapon distances, combat-family metadata,
  or offscreen notice synthesis.
- Preserve Phase 1 notice audio as centered/non-spatial, along with its explicit duck behavior.
- Keep the global 48-voice pool and existing score-based global eviction unchanged. Do not add a
  combined combat ceiling or rifle/automatic/heavy limits in this phase.
- Update `docs/design/client-ui.md` with the combat-only profile and integrated first-pass behavior.

## Expected Touch Points

- `client/src/audio.js`
- `docs/design/client-ui.md`
- `tests/client_contracts/audio_contracts.mjs`

## Implementation Checklist

- [ ] Add a combat-only spatial profile selected without camera/renderer coupling.
- [ ] Apply the exact near, attenuation, low-pass, hard-drop, and priority equations.
- [ ] Keep default non-combat spatial behavior unchanged.
- [ ] Recompute in-flight combat voices with the same profile after listener movement.
- [ ] Keep the global pool unchanged and add no combat-family budget system.
- [ ] Add focused near/edge/drop, low-pass, priority, and listener-refresh contracts.
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

Focused contracts should cover full combat gain through `0.4 * r`, `0.5` gain at `0.5 * r`, about
`0.143` gain at `1.0 * r`, and hard drop beyond `1.2 * r`. They must assert that low-pass reaches
its far cutoff at the hard-drop boundary rather than earlier, default non-combat spatial behavior is
unchanged, active voices recompute consistently after listener movement, and the distance penalty
is zero through the near region and rises monotonically to 30 at the boundary.

## Manual Test Focus

Provide a runnable local release match or deterministic setup, plus the command or Tailnet link and
the short checklist below, so the user/manual tester can listen on their actual device. Do not use a
Lab screenshot or capture as audio evidence. The tester should use the same dense mixed fight at
camera center, near the visible edge, just offscreen, and far offscreen; check that close combat
remains satisfying, edge combat recedes, far routine fighting drops out, and camera pans/minimap
jumps do not pop or abruptly brighten active sounds. During the same fight, trigger an existing
under-attack or command-feedback voice and check that Phase 1 ducking remains clear and gradual.

This user/manual listening pass is the plan's measured checkpoint, not an automated merge gate.
Record concrete observations about notice intelligibility, perceived combat density, missing
important heavy cues, edge/offscreen noise, and camera-transition artifacts before proposing any
follow-up. If the battle remains overloaded, record whether it is general voice-pool saturation or
one sound family drowning out another; do not automatically add limits without that evidence.

## Handoff Expectations

Report the final combat profile constants, automated checks, and the exact local setup/link and
checklist prepared for the user. Leave subjective mix conclusions pending until the user/manual
tester reports whether the first pass achieved understandable existing notices, busy nearby combat,
and a useful edge/offscreen falloff. List only follow-up candidates backed by that checkpoint; a
single combined combat ceiling is the first pool-control candidate, while family budgets, asset-tail
work, normalization, limiting, and combat-event aggregation remain deferred until evidence justifies
them and the user chooses a new plan.
