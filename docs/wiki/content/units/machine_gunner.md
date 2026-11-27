---
constants: MG_
---
## Burst fire

- Fires {{MG_BURST_BULLETS}} bullets per attack. The first is aimed at the target; the others spread up to ±{{MG_HALF_SPREAD_RAD|deg}}.
- Each hit deals {{MG_RAY_DAMAGE_MULTIPLIER|pct}} of {{unit:machine_gunner.attack.dmg}}. Hits on one target are added before armor.
- Per burst vs a Rifleman: {{dmg:machine_gunner_mg>rifleman.burstMin}} (1 hit) to {{dmg:machine_gunner_mg>rifleman.burstMax}} (all hits).
- Spread bullets can hit other enemies in their path, so bursts do more against packed groups.

## Setup

- Sets up in {{MACHINE_GUNNER_SETUP_TICKS|s}}; {{METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS|s}} with {{research:methamphetamines}}.
- {{research:methamphetamines}} also makes it move ×{{METHAMPHETAMINES_SPEED_MULTIPLIER}} as fast.
