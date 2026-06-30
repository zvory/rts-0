# Phase 5 - Coax Client Feedback And Tank Rig

## Phase Status

Status: pending.

## Objective

Render and play Tank coax shots as machine-gun-scale secondary weapon feedback instead of cannon
feedback. This phase consumes the server-emitted weapon identity from Phase 4 and should make the
coax understandable to players without changing any command UI.

## Scope

- Add a tiny gray rectangular coax barrel beside the Tank main cannon in the live Tank rig.
- Add a coax muzzle anchor or equivalent render routing so coax flashes/tracers originate from the
  small barrel rather than the main cannon muzzle.
- Teach visual-effect buffers and renderer feedback to select muzzle origin, flash size, tracer
  width, overpenetration tail scale, and recoil behavior from attack-event weapon identity.
- Make `tank_coax` shots use machine-gun combat sound, not the tank cannon sound.
- Prevent coax shots from triggering Tank cannon recoil or large Tank cannon muzzle flash.
- Preserve existing Tank cannon sound, flash, tracer, overpenetration tail, and recoil behavior.
- Keep fallback behavior safe for old/default attack events with no weapon hint.
- Add focused client tests that prove weapon identity, not attacker kind alone, controls the
  Tank cannon versus coax feedback split.
- Do not add command-card UI, range display changes, player toggles, or new settings.

## Expected Touch Points

- `client/src/combat_audio.js`
- `client/src/match_combat_audio.js`
- `client/src/state_visual_effects.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/shared.js`
- `client/src/renderer/rigs/tank_svg.js`
- `client/src/renderer/rigs/live_routing.js`
- `client/src/renderer/rigs/animation.js` if recoil routing needs weapon identity
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/state_input_contracts.mjs`
- Render/rig contract fixtures or tests touched by Tank rig changes
- `docs/design/client-ui.md`

## Edge Cases To Cover

- `tank_coax` from a Tank plays MG audio and uses small flash/tracer feedback.
- `tank_cannon` or missing/default Tank attack weapon keeps cannon audio, large flash, tracer, and
  recoil.
- Coax overpenetration visuals are small-scale and do not imply a second cannon shot.
- Shot reveals from fog still render with the correct weapon-specific feedback when enough data is
  projected, and degrade safely when only legacy/default data is present.
- Multiple coax shots over time do not create stuck looping MG audio keys or stale recoil state.
- The Tank rig still renders at normal scale, selection bounds and HP anchor stay stable, and the
  new coax barrel does not break tinting or animation routes.

## Verification

- Focused audio contract tests for Tank cannon versus Tank coax sound selection.
- Focused visual-effect/renderer contract tests for weapon-specific muzzle flash, tracer, and recoil
  state.
- Rig/schema tests or SVG fixture updates required by the Tank rig route.
- `node scripts/check-client-architecture.mjs`.
- `node tests/protocol_parity.mjs` if protocol constants/weapon ids are touched.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`.

## Manual Test Focus

Run a local dev scenario with a Tank firing cannon and coax at visible targets. Confirm the cannon
still feels heavy and the coax reads as a small MG beside the cannon barrel, including while the
Tank is moving or the cannon is reloading.

## Handoff Expectations

Describe the final client weapon-feedback mapping and the Tank rig anchors/routes added. Note any
browser/manual rendering checks performed and any remaining art polish that Phase 6 or Phase 7
should consider.
