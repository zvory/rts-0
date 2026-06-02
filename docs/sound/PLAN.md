# Sound System — Multi-Phase Plan

Foundation and the first 80% of a server-authoritative RTS audio system.

Audio is **client-side** (presentation). The server's only obligation is to emit fog-gated events
the client already needs. No server-side mixing, no audio in snapshots beyond what is already there.

## Guiding invariants

1. **Fog applies to audio.** Any event that triggers a sound must respect the same per-player
   visibility/ownership gate as rendering. Position-revealing audio for invisible entities is a
   cheat vector (cf. `DESIGN.md §7`). Audit on every event addition.
2. **One `AudioBuffer` per SFX, many emitters.** Never decode per playback. Never load on first
   play.
3. **Voice budget is fixed.** Cap concurrent sources (~48). Eviction is by priority, not FIFO.
4. **Audio is a pure function of (events, seeded RNG, settings).** Required for deterministic
   replays. No bare `Math.random()` in audio code.
5. **No new cross-module imports in the client.** The audio module is wired in `main.js` via DI,
   mirroring `DESIGN.md §4`.
6. **`destroy()` is mandatory.** The audio module holds an `AudioContext` and listeners; leaking
   one across rematches is a hard bug.

## Event surface (current vs needed)

Today (`server/src/protocol.rs`):

- `Attack { from, to }` — no position; client must resolve via entity table
- `Death { id, x, y, kind }` — positioned
- `Build { id, kind }` — no position; resolve via entity table
- `Notice { msg }` — UI text, non-positional

Gaps that audio will expose:

- No `unit_produced` event (production completion) — currently inferred from `prod_progress` going
  to 0. Inference is fine for v1; promote to explicit event only if it becomes painful.
- No `command_ack` channel — acknowledgments ("yes sir", "moving out") are local: the client knows
  when *it* issued a command. Keep client-local; do not add to wire.
- No projectile-impact-vs-fire distinction. `Attack` fires once per shot; that is sufficient for v1
  (one gunshot SFX). Revisit if impact sounds diverge from muzzle sounds.

Decision: **do not extend the wire protocol in phase 1**. The existing events are sufficient for
the 80%.

---

## Phase 0 — Asset & licensing baseline

Cheap, blocks nothing else. Do first so later phases have something to play.

- Pick a license-clear asset source (freesound.org CC0, Sonniss GDC packs, or commissioned).
- Establish a directory: `client/assets/sound/{ui,combat,units,buildings,ambient}/`.
- File format: **OGG Vorbis** primary, **MP3** fallback for older Safari. No WAV in production
  (size). 44.1 kHz, mono for positional SFX, stereo only for UI/music.
- Naming: `category_subject_variant.ogg` (e.g. `combat_rifle_01.ogg`, `unit_marine_ack_03.ogg`).
- Loudness: normalize to ~-16 LUFS integrated, true-peak ≤ -1 dBTP. Inconsistent loudness is the
  single biggest cause of "audio feels bad" in indie RTS.
- Commit a small placeholder set (10–15 files) so phase 1 has something to wire.

Deliverable: asset directory + 1-page `docs/sound/ASSETS.md` covering source, license, format,
loudness target.

---

## Phase 1 — Core engine: AudioContext, buffer cache, voice pool

The minimum that makes anything audible. No spatialization yet.

### Module layout

New file: `client/src/audio.js`. Single class `Audio` constructed in `main.js`, passed to whichever
modules need to trigger sounds (`renderer.js` for events, `input.js` for command acks, `hud.js` for
UI).

### Responsibilities

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

### Public API (stable seam)

```
audio.play(id, { x?, y?, priority?, category?, pitchVariance? })
audio.playUI(id, opts)            // bypasses spatialization
audio.setMasterVolume(v)
audio.setCategoryVolume(cat, v)
audio.preload(manifest) → Promise
audio.destroy()
```

`x, y` are world pixels (per the coordinate invariant). Phase 1 ignores them; phase 2 wires them up.

### Settings persistence

`localStorage` keys: `audio.master`, `audio.cat.<name>`. Default master 0.7, ambient 0.4, others
1.0. Add sliders to the in-match settings UI (`hud.js`) in the same phase — sliders without
persistence are a UX regression.

### Tab-blur mute

Listen on `document.visibilitychange`. Ramp master to 0 over 100 ms on hide, restore on show. Do not
suspend the `AudioContext` itself — resume latency is platform-dependent and adds bugs.

### Dedup & variation

A `play()` for the same `id` within a per-sound cooldown (default 60 ms) is dropped. Pitch variance
defaults to ±6% via `playbackRate`, drawn from a seeded RNG owned by the audio module.

### Tests

- Headless smoke (`tests/client_smoke.mjs`): mock `AudioContext` to a stub that records calls;
  assert preload completes and `play()` increments a counter. Do not assert audible output.
- Unit-ish: pool eviction test (push 50 sounds, assert pool size ≤ 48 and lowest-priority dropped).

Deliverable: every existing `Event::Notice` (and a hard-coded "click" on building-placement
confirm) plays a non-spatial sound. Volume sliders work. No regressions in `client_smoke`.

---

## Phase 2 — Spatialization & distance

Adds the "far-off battle gets louder as you approach" behavior.

### Listener

The listener is the **camera center in world pixels**, updated each frame from `camera.js`.
Expose `audio.setListener(x, y, zoom)`. Pass `zoom` because reference distance should scale with
zoom — at max zoom-out, the whole map should still feel audible at reduced volume rather than
silent.

### Per-emitter graph

```
AudioBufferSourceNode → StereoPannerNode → BiquadFilterNode (lowpass) → category GainNode
```

Reasons for `StereoPannerNode` over `PannerNode`:

- Top-down 2D; we don't need HRTF or 3D azimuth.
- Cheaper.
- Predictable. `PannerNode`'s default model surprises people.

### Attenuation curve

Custom (not `PannerNode.distanceModel`):

```
dx = emitter.x - listener.x
dy = emitter.y - listener.y
d  = sqrt(dx*dx + dy*dy)
refDist  = 1 screen-width worth of world pixels at current zoom
maxDist  = 3 * refDist
gain     = clamp(refDist / max(d, refDist), 0, 1)   // inverse-ish, flat inside refDist
pan      = clamp(dx / refDist, -1, 1)
lpHz     = lerp(20000, 1200, clamp(d / maxDist, 0, 1))
```

Beyond `maxDist`, drop the sound entirely (do not even allocate a voice). This is the single
biggest perf win in a 200-unit fight.

### Off-screen cue

Sounds whose emitter is outside the viewport but within `maxDist` still play — that *is* the "far
battle" feature. The lowpass + reduced gain provide the muffled cue. Do not gate on viewport;
gate on `maxDist` only.

### Fog gate (critical)

The audio module trusts the snapshot: it only ever sees events the server already filtered. The
audit task here is on the **server side**:

- Confirm `Event::Attack`, `Event::Death`, `Event::Build` are only emitted to players who can see
  the relevant entities (check `game/services/` snapshot assembly).
- Add a regression test in `tests/regression.mjs`: spawn two players, hide a battle in fog for
  player A, assert player A's snapshot stream contains no events referencing the hidden entities.

If any leak is found, fix the server before this phase ships. Audio will otherwise surface fog
leaks as audible cheats.

### Resolving non-positional events

`Event::Attack { from, to }` has no `(x, y)`. The client resolves position from the entity table at
the snapshot tick:

- Prefer the attacker's position (`from`). It's the player's own unit more often than not, and
  matches the "muzzle" sound model.
- If `from` is unknown (stale id, evicted from view), fall back to `to`. If both unknown, drop the
  sound — do not play at (0, 0).

`Event::Build` resolves via `id`; same fallback rule.

### Tests

- Extend the audio stub to capture `(pan, gain, lpHz)` per play; assert distance/pan math.
- A live test in `tests/client_smoke.mjs`: load a 2-player replay, scroll the camera, assert gain
  changes monotonically with distance.

Deliverable: distance attenuation, stereo pan, off-screen lowpass. Fog regression test green.

---

## Phase 3 — Prioritization, ducking, voice management

Makes 200-unit battles tolerable.

### Priority scoring

When the voice pool is full and a new sound wants in:

```
score(voice) = base_priority[category]
             - age_ms / 1000
             - distance_penalty(d)
             + alert_bonus            // sticky for UI/alerts
```

Base priorities: `alert=100`, `ui=90`, `unit_voice=70`, `combat_self=60`, `combat_other=40`,
`ambient=10`. Evict lowest score. New sound gets in only if its score exceeds the evictee's.

### Ducking

Per-category `GainNode`s are ducked when higher-priority categories play:

- Any `alert` fires → ramp `ambient` to -12 dB and `combat_*` to -4 dB over 80 ms; restore over
  400 ms after the alert ends.
- Player-issued `ui`/`unit_voice` does **not** duck combat (would feel laggy/wrong).

### Dedup tightening

Per-sound cooldown is now per-`(id, listener-distance-bucket)`. 40 marines firing in the same tick
produce 1–3 gunshots, not 40, but two squads on opposite sides of the map both fire normally.

### Alert + minimap coupling

`Event::Notice` with `msg` starting with `"alert:"` (proposed convention — confirm with server
side) plays the alert SFX **and** pings the minimap at the event's resolved position. If the
notice has no position, pulse the minimap border instead. Coordinate with `minimap.js` via DI from
`main.js`.

Open question: should `Notice` carry an optional `(x, y)` and a `severity` field? Probably yes,
but that is a wire change — defer to phase 4 only if alert UX demands it.

### Tests

- Pool-saturation test: enqueue 200 sounds in a tick, assert ≤48 active, and that the 48 retained
  have the highest priorities.
- Ducking test: fire an alert, assert ambient gain drops then recovers.

Deliverable: battles sound dense but not muddy; alerts cut through.

---

## Phase 4 — Unit voices, command acks, ambience

The flavor layer. Cheap once phases 1–3 are solid.

### Command acknowledgments

Triggered client-side in `input.js` on command issue:

- Single-unit select → "selected" line.
- Move/attack command → "acknowledged" line, from one randomly chosen selected unit (seeded RNG).
- Per-unit cooldown ~2 s on voice lines so spam-clicking doesn't machine-gun.
- Non-positional (UI category): always centered, full volume.

### Idle ambience

A long, looping `ambient` track at low gain. Optional per-map. Loop with a small crossfade
(`AudioBufferSourceNode.loop = true` is fine; crossfade only if seams are audible).

### Building / production sounds

Tie to inferred production-complete (snapshot diff: a `prod_progress` reaching the kind's build
time, or the unit appearing). Positional, at the building's location, `combat_other` category.

### Death variety

`Event::Death.kind` already carries the unit kind. Map to per-kind death SFX with 2–3 variants
chosen via seeded RNG.

Deliverable: the game feels alive without you having to think about audio.

---

## Phase 5 — Replay determinism & polish

Last mile. Only worth doing if replays are in scope.

- Replace any remaining `Math.random()` in audio code with the seeded stream.
- Replay player must feed the audio module the same event stream + same seed; verify audio output
  byte-identical between two replays (assert via the test stub's call log).
- Add a `--mute` query-string flag to the self-play replay route (`/dev/selfplay?replay=…&mute`)
  for headless debugging.
- Settings polish: per-category sliders, mute toggle (M key), audio device selector if Web Audio
  exposes one on the target browsers (Chrome does via `setSinkId`).

Deliverable: replays sound the same on every machine; debugging flags exist.

---

## What is explicitly out of scope (for now)

- Music system (dynamic tracks, combat stingers). Important eventually; not in the 80%.
- HRTF / 3D audio. Top-down RTS does not benefit.
- Server-side audio mixing or audio in snapshots beyond existing events.
- Voice chat. Different problem entirely (WebRTC, not Web Audio).
- Reverb / convolution. Diminishing returns vs CPU for a 2D top-down game.

## Risks & open questions

1. **`Event::Attack` has no position.** Phase 2 resolves via entity table; works in practice but
   needs validation under heavy fog churn (attacker may have just left vision). Measure in phase 2;
   if frequent, add `x, y` to `Attack` (wire change, mirror in `client/src/protocol.js` and bump
   any version field per `DESIGN.md §2`).
2. **Fog leaks via audio.** Phase 2 includes the audit. If any leak is found, server fix is a
   blocker, not a follow-up.
3. **Loudness drift across asset sources.** Phase 0 LUFS target must be enforced — it is the
   cheapest fix and the most commonly skipped.
4. **`AudioContext` resume on iOS Safari.** Stricter than other browsers. Test on real device before
   declaring phase 1 done; do not trust desktop Safari as a proxy.
5. **Replay determinism interacts with browser audio scheduling jitter.** "Determinism" means same
   logical play calls, not sample-accurate output. Good enough.

## Sequencing summary

| Phase | Blocks?              | Effort   | User-visible win                  |
|-------|----------------------|----------|-----------------------------------|
| 0     | Phase 1              | 0.5 day  | none (assets ready)               |
| 1     | All later phases     | 2 days   | UI sounds, volume sliders         |
| 2     | Phase 3              | 2 days   | spatial audio, far-battle effect  |
| 3     | none                 | 1.5 days | battles sound clean, alerts cut   |
| 4     | none                 | 1.5 days | unit voices, ambience             |
| 5     | replay feature       | 1 day    | deterministic replay audio        |

Total: ~8.5 engineer-days for the 80%.
