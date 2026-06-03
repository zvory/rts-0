# Phase 3 — Prioritization, ducking, voice management

Makes 200-unit battles tolerable.

## Priority scoring

When the voice pool is full and a new sound wants in:

```
score(voice) = base_priority[category]
             - age_ms / 1000
             - distance_penalty(d)
             + alert_bonus            // sticky for UI/alerts
```

Base priorities: `alert=100`, `ui=90`, `unit_voice=70`, `combat_self=60`, `combat_other=40`,
`ambient=10`. Evict lowest score. New sound gets in only if its score exceeds the evictee's.

## Ducking

Per-category `GainNode`s are ducked when higher-priority categories play:

- Any `alert` fires → ramp `ambient` to -12 dB and `combat_*` to -4 dB over 80 ms; restore over
  400 ms after the alert ends.
- Player-issued `ui`/`unit_voice` does **not** duck combat (would feel laggy/wrong).

## Dedup tightening

Per-sound cooldown is now per-`(id, listener-distance-bucket)`. 40 marines firing in the same tick
produce 1–3 gunshots, not 40, but two squads on opposite sides of the map both fire normally.

### Voice-line debounce (alerts/UI)

Phase 1's 60 ms dedup is way too short for spoken lines (1–2 s each). Mashing **Build** on a
worker with insufficient supply spams "Build more supply depots" once per click — unusable.

Rules:

- Per-id cooldown for `category in {alert, ui, unit_voice}` defaults to **`max(buffer.duration,
  1500 ms)`**. A 2.3 s line cannot retrigger for at least 2.3 s.
- Identical alert text within the cooldown window is dropped silently (no queue — the player
  already heard it).
- A *different* alert id within the cooldown plays normally (steel-low does not block supply-low).
- Optional: per-category global cooldown of 400 ms so two distinct alerts fired in the same tick
  don't talk over each other. Implemented as a thin scheduler in `audio.js`, not the voice pool.
- Toast text on the screen is **not** debounced — visual repetition is fine and signals "yes the
  game heard your click." Only the audio side is muted.

Decoupling the toast from the sound is the key insight: the player needs the visual feedback to
know the input registered, but the voice line is per-event, not per-input.

## Alert + minimap coupling

`Event::Notice` with `msg` starting with `"alert:"` (proposed convention — confirm with server
side) plays the alert SFX **and** pings the minimap at the event's resolved position. If the
notice has no position, pulse the minimap border instead. Coordinate with `minimap.js` via DI from
`main.js`.

Open question: should `Notice` carry an optional `(x, y)` and a `severity` field? Probably yes,
but that is a wire change — defer to phase 4 only if alert UX demands it.

## Tests

- Pool-saturation test: enqueue 200 sounds in a tick, assert ≤48 active, and that the 48 retained
  have the highest priorities.
- Ducking test: fire an alert, assert ambient gain drops then recovers.

Deliverable: battles sound dense but not muddy; alerts cut through.
