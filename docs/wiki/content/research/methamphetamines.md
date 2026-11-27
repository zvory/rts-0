## Effects

| Unit | Effect |
|---|---|
| Rifle infantry ({{units:rifleInfantry}}) | Cooldown {{unit:rifleman.attack.cooldownTicks|s}} → {{METHAMPHETAMINES_RIFLE_ATTACK_COOLDOWN_TICKS|s}} (DPS {{calc: unit:rifleman.attack.dmg / (unit:rifleman.attack.cooldownTicks / TICK_HZ)|2}} → {{calc: unit:rifleman.attack.dmg / (METHAMPHETAMINES_RIFLE_ATTACK_COOLDOWN_TICKS / TICK_HZ)|2}}); speed {{METHAMPHETAMINES_SPEED_MULTIPLIER|change}} |
| {{unit:machine_gunner}} | Speed {{METHAMPHETAMINES_SPEED_MULTIPLIER|change}}; setup {{MACHINE_GUNNER_SETUP_TICKS|s}} → {{METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS|s}} |
| {{unit:panzerfaust}} | Rocket aim {{PANZERFAUST_WINDUP_TICKS|s}} → {{METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS|s}} |

## Notes

- Speed bonuses don't stack. Rifle infantry keep ×{{METHAMPHETAMINES_SPEED_MULTIPLIER}} even near a {{unit:command_car}} (×{{COMMAND_CAR_AURA_SPEED_MULTIPLIER}}) or during Breakthrough! (×{{BREAKTHROUGH_BASE_SPEED_MULTIPLIER}}).
- Machine Gunners take the Command Car bonus when one is near, and the Methamphetamines bonus otherwise.
