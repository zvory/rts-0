# Phase 0 — Asset & licensing baseline

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
