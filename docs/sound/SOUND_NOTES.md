# Sound Effect Reviews

## Review Methodology

Files were played back via `afplay` on macOS and evaluated by a human listener in a single pass. Criteria:

- **Realism**: does it sound like a real-world recording, or synthetic/processed?
- **Isolation**: is the target sound clean, or does it contain bleed (voices, ambient noise, other instruments)?
- **Usability**: can the file be dropped into the game as-is, or does it require editing (trimming, normalizing)?
- **Loop/variation fitness**: for sounds played repeatedly (footsteps, engine loops), a single sample is insufficient — packs with multiple variations are required.

Files that failed any of these were deleted immediately.

## Keepers

| File | Notes |
|------|-------|
| combat_tank_cannon_01.mp3 | Fine. A bit loud. |
| units_tank_engine_loop_01.mp3 | Good. Needs opening voice ("22") cut. Use first ~10s only — tank fades out after that. |

## Rejections and Criteria

**combat_explosion_01.mp3** — Rejected. Too synthetic/8-bit. Sounds like a retro game effect, not a real explosion. Look for: recordings with real low-end rumble and pressure wave, not digitally generated blasts.

**combat_kar98k_01.mp3** — Rejected. Sounds like a foley effect from an old movie — thin, hollow crack with no body. Look for: modern bolt-action rifle recordings with realistic reverb tail and sharp transient.

**combat_mg42_burst_01.mp3** — Rejected. Poorly isolated recording: multiple overlapping guns, audible background ambience, tree/environment noise, starts with a human voice. Look for: clean isolated burst recordings, no bleed from other sources, voice-free.

**units_infantry_footstep_01/02.ogg** — Rejected. Single isolated footstep sounds. Looping one footstep is immediately detectable and sounds wrong. Look for: a footstep pack with multiple surface variations (dirt, gravel, grass) and enough samples to randomize playback without repetition fatigue.
