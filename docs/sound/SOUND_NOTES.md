# Sound Effect Reviews

## Source Tracking Policy

Every kept file must have its source URL recorded here. Do not add a file to Keepers without a source. This is required for license verification and for finding matching sounds from the same recording session.

## Review Methodology

Files were played back via `afplay` on macOS and evaluated by a human listener in a single pass. Criteria:

- **Realism**: does it sound like a real-world recording, or synthetic/processed?
- **Isolation**: is the target sound clean, or does it contain bleed (voices, ambient noise, other instruments)?
- **Usability**: can the file be dropped into the game as-is, or does it require editing (trimming, normalizing)?
- **Loop/variation fitness**: for sounds played repeatedly (footsteps, engine loops), a single sample is insufficient — packs with multiple variations are required.

Files that failed any of these were deleted immediately.

## Keepers

| File | Notes | Source |
|------|-------|--------|
| combat_tank_cannon_01.mp3 | Fine. A bit loud. | unknown |
| combat_kar98k_02.mp3 | Good. Keep as variation. | unknown |
| combat_kar98k_03.mp3 | Primary. Trimmed to 1.35s — bolt action removed. Original retained as combat_kar98k_03_with_bolt_action.mp3. | unknown |
| combat_mg42_burst_02.mp3 | Excellent. Clean isolated burst, no bleed. Primary — use in game. | unknown |
| combat_mg42_burst_03.mp3 | Very good. Backup only. | unknown |
| units_tank_engine_idle_03.mp3 | Excellent. Clean, long, authentic idle. Primary tank idle. Pair with _running (565597) from same source. | https://freesound.org/people/C-V/sounds/565598/ |
| combat_tank_cannon_06.mp3 | Acceptable. Not top quality — extracted from US Army video, some compression artifacts — but usable as primary until something better is found. | https://freesound.org/people/qubodup/sounds/161343/ |
| combat_tank_cannon_09.mp3 | Backup only. Authentic 1958 Nagra field recording of a Finnish 76mm KT cannon (caliber close to Panzer IV 75mm KwK). Good character but recording age is audible. Contains multiple shots; trim individual shots as needed. CC-BY 4.0 — credit YleArkisto / yle.fi. | https://freesound.org/people/YleArkisto/sounds/332935/ |

## Rejections and Criteria

**units_tank_engine_loop_01.mp3** — Rejected.

**units_tank_engine_idle_02.mp3** — Rejected. Deleted.

**combat_explosion_04.mp3** — Not needed. Retained on disk.

**combat_explosion_05.mp3** — Not needed. Retained on disk.

**combat_explosion_01.mp3** — Rejected. Too synthetic/8-bit. Sounds like a retro game effect, not a real explosion. Look for: recordings with real low-end rumble and pressure wave, not digitally generated blasts.

**combat_explosion_02.mp3** — Rejected. Too large-scale — sounds like a massive detonation. We want the sharp bark of tank or anti-tank fire, not a bomb blast.

**combat_explosion_03.mp3** — Rejected. Low quality, old movie sound effect character.

**combat_kar98k_01.mp3** — Rejected. Sounds like a foley effect from an old movie — thin, hollow crack with no body. Look for: modern bolt-action rifle recordings with realistic reverb tail and sharp transient.

**combat_kar98k_04.mp3** — Rejected. Heavy background noise bleed.

**combat_mg42_burst_01.mp3** — Rejected. Poorly isolated recording: multiple overlapping guns, audible background ambience, tree/environment noise, starts with a human voice. Look for: clean isolated burst recordings, no bleed from other sources, voice-free.

**combat_mg42_burst_04.mp3** — Rejected. Clean isolation but sounds distant and muffled — lacks presence.

**combat_mg42_burst_05.mp3** — Rejected. Too harsh/clipped character.

**combat_tank_cannon_07.mp3** — Rejected. qubodup "Artillery Gunfire" extracted from US Army video. Deleted.

**combat_tank_cannon_08.mp3** — Rejected. qubodup 120mm mortar shot. Deleted.

**combat_tank_cannon_02.mp3** — Rejected.

**combat_tank_cannon_03.mp3** — Rejected. Clipped at end. Deleted. Background voices audible; includes sound of an autoloader mechanism.

**units_infantry_footstep_01/02.ogg** — Rejected. Single isolated footstep sounds. Looping one footstep is immediately detectable and sounds wrong. Look for: a footstep pack with multiple surface variations (dirt, gravel, grass) and enough samples to randomize playback without repetition fatigue.

**units_infantry_footstep_grass_01–10** — Rejected. Entire pack too thumpy. Not representative of infantry on grass.

**units_infantry_footstep_sand_01–06.ogg** — Rejected. Sand surface not needed.

**units_infantry_footstep_stone_07–12.ogg** — Rejected. Stone surface not needed; grass is the target surface.

**combat_tank_cannon_03.mp3** — Low quality keeper. Good character but clipped at the end. Backup only; do not use as primary.

**combat_tank_cannon_04.mp3** — Rejected. Heavy background noise.

**combat_tank_cannon_05.mp3** — Rejected. Heavy background noise.

**units_tank_engine_idle_01.mp3** — Rejected. Too much turbine noise.

**units_tank_engine_idle_04.mp3** — Rejected. Decent but outclassed by _03.

## Gaps (still needed)

- Grass infantry footstep pack: need clean, light steps on grass, 6–10 variations
- Tank engine running (moving): download https://freesound.org/people/C-V/sounds/565597/ to match idle_03
- Sources unknown for most new keepers — track down before ship
