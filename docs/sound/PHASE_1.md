# Phase 1 — Core engine: AudioContext, buffer cache, voice pool

The minimum that makes anything audible. No spatialization yet.

## Module layout

New file: `client/src/audio.js`. Single class `Audio` constructed in `main.js`, passed to whichever
modules need to trigger sounds (`renderer.js` for events, `input.js` for command acks, `hud.js` for
UI).

## Responsibilities

- Lazily create `AudioContext` on first user gesture (browser policy). Until then, all `play()`
  calls are no-ops (not queued — queuing pre-gesture sounds is worse than dropping them).
- Preload a manifest of `{ id, url, category }` at match start. Decode all buffers up front; do not
  start the match until ready (or show "loading audio…" if it exceeds ~500 ms).
- Maintain a **voice pool** of active `AudioBufferSourceNode`s with metadata
  `{ priority, startedAt, category, id }`. Cap at 48. On overflow, evict the lowest
  `priority - age_bonus` voice.
- Per-category `GainNode` chain → master `GainNode` → destination. Categories: `ui`, `alert`,
  `combat_self`, `combat_other`, `unit_voice`, `ambient`.
- `destroy()` closes the `AudioContext` and clears the buffer cache.

## Public API (stable seam)

```
audio.play(id, { x?, y?, priority?, category?, pitchVariance? })
audio.playUI(id, opts)            // bypasses spatialization
audio.setMasterVolume(v)
audio.setCategoryVolume(cat, v)
audio.preload(manifest) → Promise
audio.destroy()
```

`x, y` are world pixels (per the coordinate invariant). Phase 1 ignores them; phase 2 wires them up.

## Settings persistence

`localStorage` keys: `audio.master`, `audio.cat.<name>`. Default master 0.7, ambient 0.4, others
1.0. Add sliders to the in-match settings UI (`hud.js`) in the same phase — sliders without
persistence are a UX regression.

## Tab-blur mute

Listen on `document.visibilitychange`. Ramp master to 0 over 100 ms on hide, restore on show. Do not
suspend the `AudioContext` itself — resume latency is platform-dependent and adds bugs.

## Dedup & variation

A `play()` for the same `id` within a per-sound cooldown (default 60 ms) is dropped. Pitch variance
defaults to ±6% via `playbackRate`, drawn from a seeded RNG owned by the audio module.

> **Known issue (deferred to phase 3 — *Voice-line debounce*):** 60 ms is correct for gunshots
> but useless for spoken alert lines (1–2 s each). Mashing "Build" on a worker with insufficient
> supply currently overlaps "Build more supply depots" with itself on every click. Phase 3 fixes
> this by raising the per-id cooldown for `alert`/`ui`/`unit_voice` to `max(buffer.duration,
> 1500 ms)` while leaving the toast un-debounced.

## Tests

- Headless smoke (`tests/client_smoke.mjs`): mock `AudioContext` to a stub that records calls;
  assert preload completes and `play()` increments a counter. Do not assert audible output.
- Unit-ish: pool eviction test (push 50 sounds, assert pool size ≤ 48 and lowest-priority dropped).

Deliverable: every existing `Event::Notice` (and a hard-coded "click" on building-placement
confirm) plays a non-spatial sound. Volume sliders work. No regressions in `client_smoke`.

> **Known gap (deferred to phase 2 — *Combat SFX wiring*):** combat sounds are intentionally not
> wired in phase 1. `Event::Attack` is silent right now even though `combat_kar98k_*`,
> `combat_mg42_burst_*`, and `combat_tank_cannon_*` are already on disk. Combat needs positional
> audio (a tank shot on the far side of the map should not punch your ears) so the wiring waits
> for phase 2's spatialization graph. Until then: no firing audio, no tank cannons, no MG bursts.
