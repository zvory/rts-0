# Phase 4 — Unit voices, command acks, ambience

The flavor layer. Cheap once phases 1–3 are solid.

## Command acknowledgments

Triggered client-side in `input.js` on command issue:

- Single-unit select → "selected" line.
- Move/attack command → "acknowledged" line, from one randomly chosen selected unit (seeded RNG).
- Per-unit cooldown ~2 s on voice lines so spam-clicking doesn't machine-gun.
- Non-positional (UI category): always centered, full volume.

## Idle ambience

A long, looping `ambient` track at low gain. Optional per-map. Loop with a small crossfade
(`AudioBufferSourceNode.loop = true` is fine; crossfade only if seams are audible).

## Building / production sounds

Keep the placement/command confirmation sound only. The client already plays the
`buildings_construction_start_01.mp3` cue when the player issues a valid build command; do not
add a second sound on `Event::Build` / construction completion.

## Death variety

`Event::Death.kind` already carries the unit kind. Map to per-kind death SFX with 2–3 variants
chosen via seeded RNG.

Deliverable: the game feels alive without you having to think about audio.
