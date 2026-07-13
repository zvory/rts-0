# Phase 1 - Existing Notice Policy and Mix Ducking

## Phase Status

- [ ] Not started.

## Objective

Give existing server notices one match-owned presentation seam and make current spoken notices cut
through the battle mix. Preserve the current vocabulary and surfaces while stopping repeated
under-attack hits from independently driving toast and minimap spam.

## Starting Behavior

- `Match.handleNotice()` currently owns notice text, severity, toast, minimap, replay/spectator
  suppression, viewport suppression, sound lookup, and playback options inline.
- Under-attack voice is deduplicated inside the persistent app-owned `Audio` engine by a 960 px
  bucket and 10-second cooldown, but repeated events still overwrite the toast and add minimap
  pings.
- Only voices played in the `alert` category duck the mix. Current ducking reaches combat at about
  -4 dB and ambient at about -12 dB, attacks in about 80 ms, and restores in 400 ms after the final
  alert buffer ends.

## Work

- Add one focused match-owned presenter, such as `client/src/match_notice_presenter.js`, and inject
  only the collaborators needed to reproduce current notice behavior:
  - toast callback
  - minimap ping/border-pulse surface
  - audio engine
  - replay/spectator state
  - viewport predicate
- Make `Match.handleSnapshotEvents()` delegate current `EVENT.NOTICE` handling to that presenter.
  Keep `Match.handleNotice()` as a thin compatibility delegation only if existing callers or tests
  benefit from it.
- Keep notice text-to-sound selection with notice presentation policy if that falls out naturally
  from the extraction, but do not create another abstraction or make helper relocation a condition
  of shipping the behavior fixes.
- Keep ordinary command/info notices deliverable on each player action. Apply incident dedup only to
  `UNDER_ATTACK_ID`; do not generalize it to other `alert:` ids or build a scheduler for all
  messages.
- Move under-attack bucket/cooldown ownership to the match-scoped presenter so incident state cannot
  leak across rematches. Start from the existing 960 px bucket and 10-second cooldown.
- Make that presenter the sole under-attack cooldown owner. Remove the persistent `Audio` engine's
  special under-attack bucket/cooldown behavior, and schedule each presenter-admitted under-attack
  voice with no second generic spoken cooldown (`cooldownMs: 0` or an equivalent explicit bypass).
  Do not leave a second audio-engine gate that can suppress a distinct accepted location.
- Use one match-scoped admission decision for the existing under-attack surfaces:
  - the first accepted incident toasts and pings, and speaks only when it is outside the viewport
  - repeats in the same bucket and cooldown suppress toast, ping, and voice together
  - a geographically distinct bucket remains eligible
- Preserve the current first-event behavior: an in-viewport under-attack event may toast and ping
  while skipping voice. That accepted in-view event still consumes the shared incident cooldown;
  replay and live-spectator clients may toast and ping but never play player notice audio.
- Make ducking explicit on the scheduled voice, with `alert` category behavior remaining a backward-
  compatible default. Do not add manifest metadata, a sidechain graph, or a second mixer.
- Route every existing spoken server notice selected by the presenter as a ducking voice even when
  the wire severity is informational. Keep visual severity and minimap eligibility based on the
  existing wire message; audio priority must not fabricate a gameplay alert.
- Use these first-pass mixer values:
  - combat duck: -10 dB
  - ambient duck: -12 dB
  - attack: 0.08 seconds
  - release: 2.0 seconds after the last ducking voice ends
- Preserve depth counting so overlapping ducking voices cannot restore combat early. Do not change
  category slider defaults or expose the constants as user-facing settings.
- Update `docs/design/client-ui.md` with the presenter ownership, existing-notice scope, incident
  behavior, and ducking contract.

## Expected Touch Points

- `client/src/match_notice_presenter.js` or an equivalently focused match-owned module
- `client/src/match.js`
- `client/src/alerts.js` only if notice helpers move naturally with the presenter
- `client/src/audio.js`
- `docs/design/client-ui.md`
- `tests/client_contracts/audio_contracts.mjs`
- `tests/client_contracts/match_replay_contracts.mjs`
- focused presenter coverage, preferably
  `tests/client_contracts/match_notice_presenter_contracts.mjs`
- `tests/client_contracts.mjs` if a new contract file is added

## Implementation Checklist

- [ ] Extract existing server-notice presentation from `Match` without adding notice types.
- [ ] Add one match-scoped under-attack admission decision shared by toast, minimap, and voice.
- [ ] Remove the audio engine's persistent under-attack cooldown and bypass generic spoken cooldown
      for presenter-admitted under-attack voices.
- [ ] Make duck intent explicit while preserving existing `alert` callers.
- [ ] Apply the deeper combat duck and two-second release.
- [ ] Preserve replay, spectator, viewport, toast, minimap, and ordinary info-notice behavior.
- [ ] Add focused contracts for presenter routing, incident dedup, and nested duck restoration.
- [ ] Update the client UI design document.
- [ ] Mark this phase done in this file in the implementation commit.

## Verification

- `node tests/client_contracts/audio_contracts.mjs`
- focused notice-presenter contract coverage, whether added as a new file or placed in an existing
  contract
- `node tests/client_contracts/match_replay_contracts.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

The presenter coverage must directly assert shared toast/ping/voice incident dedup, an accepted
in-view incident that stays silent and consumes the shared cooldown, and replay/spectator silence.
It must also schedule two different location buckets less than 1.5 seconds apart and prove that both
voices play, demonstrating that no persistent or generic audio-engine cooldown overrides the
presenter's decision.

## Manual Test Focus

Run a local match with dense fighting audible and trigger existing spoken notices, especially under
attack and current invalid/insufficient-action lines. Confirm combat drops promptly, the voice is
clear, and combat returns over roughly two seconds after the line finishes. Confirm repeated hits in
one location do not continually refresh toast/ping/voice, a separate location still presents, an
in-view first hit remains silent and suppresses repeats for that incident, and replay/spectator
notice audio remains suppressed.

## Handoff Expectations

Report the new presenter boundary, which existing messages it owns, the incident bucket/cooldown
and shared-admission semantics, and the final duck constants. State whether any compatibility
delegation remains in `Match`, list the focused tests, and tell phase 2 to preserve the notice
headroom and explicit duck behavior while tightening the combat distance profile.
