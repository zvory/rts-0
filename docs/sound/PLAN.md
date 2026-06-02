# Sound System — Multi-Phase Plan

Foundation and the first 80% of a server-authoritative RTS audio system.

Audio is **client-side** (presentation). The server's only obligation is to emit fog-gated events
the client already needs. No server-side mixing, no audio in snapshots beyond what is already there.

## Phases

- [Phase 0 — Asset & licensing baseline](PHASE_0.md)
- [Phase 1 — Core engine: AudioContext, buffer cache, voice pool](PHASE_1.md)
- [Phase 2 — Spatialization & distance](PHASE_2.md)
- [Phase 3 — Prioritization, ducking, voice management](PHASE_3.md)
- [Phase 4 — Unit voices, command acks, ambience](PHASE_4.md)
- [Phase 5 — Replay determinism & polish](PHASE_5.md)

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
