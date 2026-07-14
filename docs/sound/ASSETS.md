# Sound Assets

See `SOUND_NOTES.md` for full review notes, rejection reasons, and source tracking policy.
See `COMBAT_EXAMPLES.md` for the downloaded CC0 mortar, artillery, tank-hit, and anti-tank-hit example
sets and their source links.
See `REVIEW_PROCESS.md` for the one-by-one audition process and verdict format.

## Format

- All current files: MP3 (OGG primary format deferred — no OGG sources found yet)
- Sample rate: 44.1 kHz, mono
- No WAV in production

## Loudness target

Normalize to −16 LUFS integrated, true-peak ≤ −1 dBTP before shipping. Placeholder files are not normalized.

## Naming convention

`category_subject_variant.ext` — e.g. `combat_kar98k_03.mp3`, `units_tank_engine_idle_03.mp3`

## ElevenLabs voice lines — NON-COMMERCIAL ONLY

Generated on ElevenLabs free plan. **Cannot be used in any commercial release.** Must be replaced
with licensed recordings before any commercial distribution.

| File | Line | Voice | Plan |
|------|------|-------|------|
| `alert/alert_under_attack_01.mp3` | "You Are Under Attack" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `alert/alert_supply_low_01.mp3` | "Build More Supply Depots" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `alert/alert_steel_low_01.mp3` | "Not Enough Steel" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `alert/alert_oil_low_01.mp3` | "Not Enough Oil" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `alert/alert_out_of_range_01.mp3` | "Too Far" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `ui/ui_victory_01.mp3` | "You Win" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `ui/ui_defeat_01.mp3` | "You Lose" | Commander Brake – Strict & Dominant | ElevenLabs free |
| `alert/alert_cannot_build_01.mp3` | "Cannot Build There" | Commander Brake – Strict & Dominant | ElevenLabs free |

## Current keepers

| File | Role | Source | License |
|------|------|--------|---------|
| `combat/combat_kar98k_02.mp3` | Kar98k variation | unknown | unknown |
| `combat/combat_kar98k_03.mp3` | Kar98k primary (trimmed, bolt removed) | unknown | unknown |
| `combat/combat_kar98k_03_with_bolt_action.mp3` | Kar98k original (bolt retained) | unknown | unknown |
| `combat/combat_mg42_burst_02.mp3` | MG42 primary — clean isolated burst | unknown | unknown |
| `combat/combat_mg42_burst_03.mp3` | MG42 backup | unknown | unknown |
| `combat/combat_panzerfaust_launch_01.mp3` | Panzerfaust launch — generated first-pass cue; replace with reviewed recording before final audio pass | local procedural ffmpeg generation, Phase 6 | project-generated |
| `combat/combat_panzerfaust_impact_01.mp3` | Panzerfaust tank impact/miss-expire cue — generated first-pass cue; replace with reviewed recording before final audio pass | local procedural ffmpeg generation, Phase 6 | project-generated |
| `combat/combat_mortar_launch_04.mp3` | Mortar launch — selected from CC0 examples; too long, trim before final polish | https://opengameart.org/content/25-cc0-bang-firework-sfx | CC0 |
| `combat/combat_artillery_fire_05.mp3` | Artillery fire — selected from CC0 examples | https://opengameart.org/content/25-cc0-bang-firework-sfx | CC0 |
| `combat/combat_artillery_landing_01.mp3` | Artillery incoming whistle and landing blast — user-selected focused-whistle mix, trimmed to 7.20 s after the audible decay | user-provided local source | unknown |
| `combat/combat_distant_bed_01.mp3` | First-pass fixed 12 s distant-combat bed; low/high-pass filtered, reverberant, and compositionally static so the global activity signal reveals no live battle details | Project derivative of `combat_artillery_fire_05`, `combat_mortar_launch_04`, and `combat_panzerfaust_launch_01` | project derivative (CC0 + project-generated sources) |
| `combat/combat_tank_cannon_01.mp3` | Tank cannon (a bit loud) | unknown | unknown |
| `combat/combat_tank_cannon_06.mp3` | Tank cannon primary — extracted from US Army video, some compression artifacts | https://freesound.org/people/qubodup/sounds/161343/ | CC0 |
| `units/units_tank_engine_idle_03.mp3` | Tank engine idle primary — clean, long, authentic | https://freesound.org/people/C-V/sounds/565598/ | unknown |
| `buildings/buildings_construction_start_01.mp3` | Construction start — all buildings | https://freesound.org/people/dr19/sounds/353907/ | CC0 |
| `ui/ui_countdown_drei_01.mp3` | Pre-match countdown: Drei | user-provided local file | unknown |
| `ui/ui_countdown_zwei_01.mp3` | Pre-match countdown: Zwei | user-provided local file | unknown |
| `ui/ui_countdown_eins_01.mp3` | Pre-match countdown: Eins | user-provided local file | unknown |

### Attribution required

- `combat_tank_cannon_09.mp3` (not yet downloaded): CC-BY 4.0 — credit YleArkisto / yle.fi. Source: https://freesound.org/people/YleArkisto/sounds/332935/

## Gaps

- Sources unknown for most keepers — must resolve before ship
- License unknown for `units_tank_engine_idle_03.mp3` — verify freesound.org/people/C-V/sounds/565598/
- Grass infantry footstep pack: need 6–10 clean variations on grass surface
- Tank engine running (moving): https://freesound.org/people/C-V/sounds/565597/ (same session as idle_03)
- No explosion sounds (all candidates rejected — see SOUND_NOTES)
- Replace generated Panzerfaust launch/impact cues with reviewed realistic recordings before final
  audio polish
- `ambient/` directory is empty
