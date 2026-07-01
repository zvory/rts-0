# Phase 8 - Coax Tank Rig And Feedback Origin Polish

## Phase Status

Status: done.

## Objective

Replace Phase 7's provisional Tank coax feedback origin with authored rig geometry and a transformed
coax muzzle anchor. This phase consumes the existing `weaponKind` mapping and makes the coax read as
a small barrel beside the main gun without changing command UI or server gameplay.

## Scope

- Add a tiny gray rectangular coax barrel beside the Tank main cannon in the live Tank rig.
- Add a coax muzzle anchor such as `anchor.coaxMuzzle` and route it through rig import/runtime
  metadata.
- Route attack feedback origin through transformed rig anchors where available: `tank_cannon` uses
  the main muzzle anchor, and `tank_coax` uses the coax muzzle anchor. Shot-reveal entities must use
  the same weapon-specific origin logic when the reveal carries enough rig data.
- Preserve the Phase 7 visual-effect buffer behavior that stores weapon identity per muzzle flash
  and recoil event rather than only by attacker id.
- Preserve same-tick cannon and coax feedback from one Tank without one event erasing the other.
- Route feedback by weapon identity:
  - `tank_cannon` or missing/default Tank weapon keeps cannon sound, large flash/tracer,
    overpenetration tail scale, and cannon recoil.
  - `tank_coax` uses machine-gun combat sound, small flash/tracer, small overpenetration tail scale,
    and no Tank cannon recoil.
- Make coax muzzle flash and tracer originate from the coax barrel rather than the main cannon
  muzzle.
- Use unkeyed MG burst audio for `tank_coax` unless Phase 7 already introduced a weapon-aware keyed
  lifecycle. Do not reuse Machine Gunner-only looping/key cleanup in a way that creates stuck or
  immediately stopped Tank coax audio.
- Keep fallback behavior safe for old/default attack events with no weapon hint.
- Preserve Tank selection bounds, HP anchor, tinting, animation routes, and existing cannon recoil.
- Add focused client tests proving weapon identity, not attacker kind alone, controls the Tank
  cannon versus coax feedback split.

## Out Of Scope

- No server gameplay changes.
- No command-card UI, range display, player toggle, or settings change.
- No new protocol fields beyond consuming the Phase 4 `weaponKind` field.

## Expected Touch Points

- `client/src/combat_audio.js`
- `client/src/match_combat_audio.js`
- `client/src/state_visual_effects.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/shared.js`
- `client/src/renderer/rigs/tank_svg.js`
- `client/src/renderer/rigs/live_routing.js`
- `client/src/renderer/rigs/animation.js`
- `tests/fixtures/svg/rig-vehicle.svg` if the rig fixture mirrors authored Tank parts
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/state_input_contracts.mjs`
- `tests/client_contracts/renderer_feedback_contracts.mjs`
- rig importer/runtime tests if present
- `docs/design/client-ui.md`

## Edge Cases To Cover

- `tank_coax` from a Tank plays MG audio and uses small flash/tracer feedback.
- `tank_coax` does not trigger Tank cannon recoil.
- `tank_cannon` or missing/default Tank attack weapon keeps cannon audio, large flash/tracer, and
  cannon recoil.
- Same-tick Tank cannon and coax events both produce appropriate feedback.
- Tank cannon feedback originates from the main muzzle anchor and Tank coax feedback originates from
  the coax muzzle anchor when the rig is available.
- Artillery self-reveal attack events still do not create tracers or combat audio.
- Coax overpenetration visuals are small-scale and do not imply a second cannon shot.
- Shot reveals from fog render with the correct weapon-specific feedback when enough data is
  projected, and degrade safely when only legacy/default data is present.
- Multiple coax shots over time do not create stuck looping MG audio keys or stale recoil state.
- The Tank rig still renders at normal scale and the new coax barrel does not break tinting,
  selection bounds, HP anchor, or animation routes.

## Verification

- Focused audio contract tests for Tank cannon versus Tank coax sound selection and the chosen coax
  MG audio lifecycle.
- Focused visual-effect/renderer contract tests for weapon-specific muzzle flash, tracer,
  overpenetration tail scale, recoil state, same-tick cannon/coax feedback, and rig-anchor origin.
- `node tests/rig_schema.mjs`
- `node tests/svg_rig_importer.mjs`
- `node tests/rig_runtime.mjs`
- Focused Tank rig assertion that the live/authored rig exposes `coaxMuzzle` and preserves existing
  `muzzle`/`turret` anchors.
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs` if protocol constants/weapon ids are touched.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

Run a local dev scenario with a Tank firing cannon and coax at visible targets. Confirm the cannon
still feels heavy and the coax reads as a small MG beside the cannon barrel, including while the
Tank is moving or the cannon is reloading.

## Handoff Expectations

Describe the final client weapon-feedback mapping, recoil storage behavior, Tank rig anchors/routes
added, MG audio lifecycle choice, browser/manual rendering checks performed, and factual patch-note
bullets. Note any remaining art polish that Phase 9 should consider.
