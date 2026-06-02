# Sound Assets

## Sources

| Source | License | Attribution required |
|---|---|---|
| OpenGameArt.org | CC0 | No |
| GfxSounds.com | GfxSounds Standard (royalty-free commercial) | No |
| archive.org — USC Cinema GOLD TAPE series | CC0 1.0 Universal | No |

## Attribution

None required.

## Format

- Primary: OGG Vorbis (positional SFX, engine loops)
- Fallback: MP3 (where OGG source was unavailable)
- Sample rate: 44.1 kHz, mono
- No WAV in production

## Loudness target

Normalize to −16 LUFS integrated, true-peak ≤ −1 dBTP before shipping. Placeholder files are not normalized.

## Naming convention

`category_subject_variant.ogg` — e.g. `combat_kar98k_01.mp3`, `units_tank_engine_loop_01.ogg`

## Placeholder files

| File | Description | License |
|---|---|---|
| `combat/combat_kar98k_01.mp3` | Single rifle shot (bolt-action) | CC0 |
| `combat/combat_mg42_burst_01.mp3` | Automatic gunfire burst (USC Cinema GOLD TAPE 33) | CC0 |
| `combat/combat_tank_cannon_01.mp3` | Tank cannon single shot | GfxSounds Standard |
| `combat/combat_explosion_01.mp3` | Close explosion | CC0 |
| `units/units_infantry_footstep_01.ogg` | Infantry footstep variant 1 | CC0 |
| `units/units_infantry_footstep_02.ogg` | Infantry footstep variant 2 | CC0 |
| `units/units_tank_engine_loop_01.mp3` | M-4 Sherman tank engine (USC Cinema GOLD TAPE 15) | CC0 |

## Gaps (placeholder quality only — replace before ship)

- No authentic Kar98k recording; current file is a generic bolt-action shot
- No authentic MG42; current file is a generic LMG burst
- No German infantry voice acknowledgments
- `ui/`, `buildings/`, `ambient/` directories are empty
