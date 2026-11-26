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

### Cultivator attack playback mix

The four Disciple recordings retain their original dynamics and encoded levels. Playback in
`match_combat_audio.js` compensates per cue rather than forcing short, sparse transients to a
whole-file LUFS target. Relative to the previous shared 0.35 gain:

| Cue | Playback gain | Increase | Loudest 50 ms RMS after playback gain |
|-----|---------------|----------|---------------------------------------|
| Spear | 0.525 | +3.52 dB | −12.18 dBFS |
| Magic orb | 0.665 | +5.58 dB | −11.32 dBFS |
| Sword stab | 0.84 | +7.60 dB | −11.39 dBFS |
| Metallic sword strike | 1.05 | +9.54 dB | −12.65 dBFS |

Measured from decoded mono PCM at 44.1 kHz in consecutive 2,205-sample windows, before
category, master, pitch variation, and spatial processing. For comparison, the unchanged tank
cannon at gain 1.4 reaches −4.49 dBFS by the same measure, retaining about 7–8 dB of attack
headroom over Cultivators. All four Cultivator decoded true peaks remain below −1 dBTP after
these playback gains. These are mix reference measurements, not equal perceived loudness
claims across different timbres or combat situations.

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

Disciple attack cues were selected by the user from ElevenLabs API auditions on 2026-11-22.
See [DISCIPLE_ATTACKS.json](DISCIPLE_ATTACKS.json) for source/production hashes, generation model,
audition file names, and decoded levels. Account-plan licensing remains unverified. These short
transients retain their dynamics and stay below −16 LUFS where necessary to keep peaks ≤ −1 dBTP.

| File | Role | Source | License |
|------|------|--------|---------|
| `combat/combat_disciple_orb_01.mp3` | Dao of Magic orb launch; selected revised orb audition | https://api.elevenlabs.io/v1/sound-generation | ElevenLabs account terms; plan unverified |
| `combat/combat_disciple_spear_01.mp3` | Dao of Wind spear thrust; selected original spear audition | https://api.elevenlabs.io/v1/sound-generation | ElevenLabs account terms; plan unverified |
| `combat/combat_disciple_sword_stab_01.mp3` | Base / Dao of War vs infantry; selected variation 1, sword stab | https://api.elevenlabs.io/v1/sound-generation | ElevenLabs account terms; plan unverified |
| `combat/combat_disciple_sword_strike_01.mp3` | Base / Dao of War vs vehicles and emplacements; original metallic strike repurposed by user | https://api.elevenlabs.io/v1/sound-generation | ElevenLabs account terms; plan unverified |
| `combat/combat_kar98k_02.mp3` | Kar98k variation | unknown | unknown |
| `combat/combat_kar98k_03.mp3` | Kar98k primary (trimmed, bolt removed) | unknown | unknown |
| `combat/combat_kar98k_03_with_bolt_action.mp3` | Kar98k original (bolt retained) | unknown | unknown |
| `combat/combat_mg42_burst_02.mp3` | MG42 primary — clean isolated burst | unknown | unknown |
| `combat/combat_mg42_burst_03.mp3` | MG42 backup | unknown | unknown |
| `combat/combat_panzerfaust_launch_01.mp3` | Panzerfaust launch — generated first-pass cue; replace with reviewed recording before final audio pass | local procedural ffmpeg generation, Phase 6 | project-generated |
| `combat/combat_panzerfaust_impact_01.mp3` | Panzerfaust tank impact/miss-expire cue — generated first-pass cue; replace with reviewed recording before final audio pass | local procedural ffmpeg generation, Phase 6 | project-generated |
| `combat/combat_mortar_launch_04.mp3` | Mortar launch — selected from CC0 examples; too long, trim before final polish | https://opengameart.org/content/25-cc0-bang-firework-sfx | CC0 |
| `combat/combat_mortar_impact_01.mp3` | Mortar impact — first 76 ms removed; normalized to the project −16 LUFS target with a ≤ −1 dBTP ceiling | user-provided local source | unknown |
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
- No additional mortar explosion variations; this first impact cue is user-provided and its source/license needs verification
- Replace generated Panzerfaust launch/impact cues with reviewed realistic recordings before final
  audio polish
- `ambient/` directory is empty

## Selected Dao ability cues

The user selected Silk Gust (Dash), Resonant Shell (Shield), and magical portal candidate 1
(Anchor). These ElevenLabs-generated cues use the existing combat buses; Anchor sustains only
while its visible world object exists and is mastered below the cast cues. Exact submitted prompts,
API/model settings, raw sources, processing, and decoded levels are recorded in
[DAO_ABILITIES.json](DAO_ABILITIES.json). Commercial rights remain unverified under the account
terms. No rejected auditions are shipped in the runtime manifest.

### Vector Field — Silk Current

Selected by the user on 2026-11-26. The complete 10-second wind uses overlapping interior
sections with unequal offsets and durations. The production mono MP3 targets −20 LUFS, subject
to a −2 dBTP ceiling and a loudest 50 ms at least 3 dB below the Dash and Shield cast cues;
playback uses the same 0.7 gain and spatial combat buses as Anchor. See
[VECTOR_FIELD.json](VECTOR_FIELD.json) for exact prompts, source/production hashes, user verdict,
processing commands, and decoded levels; `scripts/prepare-vector-field-audio.py` rebuilds the
normalization. The raw and selected audition are retained in `sources/vector-field/`.
ElevenLabs account was free at generation time; commercial rights are not established.
