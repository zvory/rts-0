# Phase 2 - Permissive Combat Voice Guardrails

## Phase Status

- [ ] Not started.

## Objective

Prevent combat from occupying the entire global voice pool while keeping large fights intentionally
dense. Add simple combined and family-level guardrails that tolerate long decoded clip lifetimes and
quiet asset tails without changing attack cadence or clip playback behavior.

## Starting Limits

Keep the existing global 48-voice cap and begin with these deliberately high combat ceilings:

- 36 active combat voices total across `combat_self` and `combat_other`
- 16 active rifle-family voices
- 16 active automatic-weapon voices
- 12 active heavy/ordnance voices

Family limits intentionally sum above the combined total. These are permissive safety rails selected
because an active Web Audio source may remain alive through a quiet tail; they are not a request to
detect silence or shorten the source.

## Work

- Add the smallest audio-engine admission seam that supports:
  - one combined combat-category ceiling across self/other buses
  - one explicit coarse budget group per combat voice
  - active counts that clear automatically when a voice naturally ends or is stopped
- Do not parse sound ids to infer families. Tag the current specifications in
  `MatchCombatAudio` explicitly using three coarse groups:
  - `rifle`: Rifleman-style rifle feedback and current rifle fallbacks
  - `automatic`: Machine Gunner, Scout Car, and tank-coax machine-gun feedback
  - `heavy`: Tank/anti-tank cannon, Panzerfaust, mortar, artillery fire, and artillery landing
- Apply the same family tag to self and other combat categories so the family ceiling is truly
  combined rather than doubled by ownership.
- When the combined or family ceiling is full, reuse the existing age, distance, ownership/category,
  and caller priority score:
  - replace the worst lower-scored combat voice inside the constrained scope when the incoming voice
    is more important
  - otherwise drop the incoming combat request
  - never evict alert/UI voices merely to admit another constrained combat voice
- Keep global priority eviction as the final backstop. Spoken notices and UI should retain the
  remaining global headroom and their existing ability to outrank combat.
- Preserve all current machine-gunner voice keys and stop behavior. The budget must count the actual
  active one-shots; it must not convert them into a loop, replace them by emitter, or infer firing
  state beyond the current key lifecycle.
- Preserve the existing default 60 ms combat dedup, every caller-specific cooldown override, pitch
  variance, variants, per-sound gain, point-fire scheduling, artillery landing timer, and
  attack/weapon mapping.
- Do not add duration-aware limits, waveform analysis, silent-tail metadata, per-emitter caps,
  per-weapon settings, adaptive ceilings, or diagnostics upload.
- Update `docs/design/client-ui.md` with the combined/family ceilings, coarse family mapping, and
  score-based constrained replacement behavior.

## Expected Touch Points

- `client/src/audio.js`
- `client/src/match_combat_audio.js`
- `docs/design/client-ui.md`
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/match_shell_contracts.mjs`

## Implementation Checklist

- [ ] Add combined combat active-count admission below the global 48-voice cap.
- [ ] Add one explicit coarse budget-group field for combat voices.
- [ ] Tag every currently audible combat and positional-event specification.
- [ ] Reuse the current score for constrained replacement without creating a second scheduler.
- [ ] Prove alert/UI voices remain outside combat-family counts and retain headroom.
- [ ] Preserve cadence, variants, gains, timers, keys, and natural clip duration.
- [ ] Add focused total/family saturation and replacement tests.
- [ ] Update the client UI design document.
- [ ] Mark this phase done in this file in the implementation commit.

## Verification

- `node tests/client_contracts/audio_contracts.mjs`
- `node tests/client_contracts/match_shell_contracts.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

Focused contracts should demonstrate the 36-voice combined ceiling, each family ceiling, replacement
by a better-scored combat voice, rejection of a worse incoming voice, cleanup on natural/forced end,
and protection of alert/UI voices. They should also assert that every existing combat mapping passes
the intended family without changing scheduling or keys.

## Manual Test Focus

Run a large close-range mixed Rifleman, Machine Gunner, Scout Car, Tank, anti-tank, mortar, and
artillery fight. Confirm the result remains busy rather than sparse, automatic weapons and rifles
retain texture, heavy weapons remain recognizable, and an existing spoken notice still plays and
ducks the saturated battle. Listen specifically for an accidental cadence or lifecycle change;
there should be none.

## Handoff Expectations

Report the landed combined and family ceilings, exact family membership, constrained replacement
rule, and tests proving alert/UI headroom. Call out whether manual testing exposed obvious starvation
caused by silent tails; do not change the user's approved high-limit policy in the handoff. Tell
phase 3 which category/group metadata is available when selecting the combat spatial profile.
