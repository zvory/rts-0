# Phase 6 - Audio And Feedback Polish

## Phase Status

Status: done.

## Objective

Add intentional first-pass audio and feedback polish for the Panzerfaust now that the unit is
trainable and readable. This phase should remove accidental placeholder treatment and record any
remaining art or sound debt explicitly.

## Scope

- Add or map audio for:
  - Panzerfaust launch.
  - Projectile travel if the audio system supports it without spam.
  - Tank impact or miss/expired impact where server feedback permits it.
  - Conversion/recovery completion only if a sound improves clarity without becoming noisy.
  - Selection, command acknowledgement, production complete, and death only if the current infantry
    defaults are misleading.
- Tune visual feedback as needed:
  - Muzzle flash scale and placement.
  - Short tracer or projectile readability.
  - Impact effect scale and lifetime.
  - Recovery/conversion readability if Phase 4/5 manual inspection found ambiguity.
- Ensure Tank cannon, Anti-Tank Gun, Rifleman, Machine Gunner, artillery, and debug sounds/effects
  are not reused in a misleading way.
- Keep spam risk low when several Panzerfaust units fire together.
- Add contract coverage for audio/feedback mapping where practical.
- Update patch notes and deferred polish notes.

## Expected Touch Points

- `client/src/audio.js`
- `client/src/sound_manifest.js`
- `client/src/combat_audio.js`
- `client/src/alerts.js`
- `client/src/state.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/renderer/units.js`
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/renderer_feedback_contracts.mjs`
- `docs/design/client-ui.md`
- `plans/panzerfaust/checklist.md`

## Edge Cases To Cover

- Panzerfaust launch does not play Tank cannon, Rifleman, artillery, or debug audio.
- Fog-projected visual-only events do not play sounds for hidden information the recipient should
  not know.
- Multiple launches in the same second are capped, mixed, or quiet enough to avoid audio spam.
- Conversion does not double-play death, production-complete, or selection sounds.
- Replays and spectator views play only the feedback available from their projected events.
- Missing audio assets fail gracefully if local browser autoplay or asset loading blocks playback.

## Verification

- Focused client audio contract tests for Panzerfaust event-to-sound mapping and non-mapping of
  unrelated events.
- Focused renderer feedback tests for effect scale/lifetime if changed.
- `node scripts/check-client-architecture.mjs`.
- `node tests/client_contracts/audio_contracts.mjs`.
- `node tests/client_contracts/renderer_feedback_contracts.mjs` if feedback logic changes.
- `git diff --check`.

## Manual Test Focus

In a normal or lab match, fire one Panzerfaust, then fire several close together. Confirm launch and
impact feedback is readable, not confused with Tank cannon or Rifleman fire, not overly loud, and
still respects fog/replay projection behavior.

## Handoff Expectations

List every sound/effect intentionally used, every placeholder left in place, and any manual browser
or replay inspection performed. Tell Phase 7 which feedback cases still need regression coverage or
playtest attention.
