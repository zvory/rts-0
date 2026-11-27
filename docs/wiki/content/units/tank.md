---
constants: TANK_
---
## Range

- Range grows from {{unit:tank.attack.rangeTiles}} to {{TANK_STATIONARY_RANGE_MAX_TILES}} tiles while the Tank stands still, reaching full range after {{TANK_STATIONARY_RANGE_RAMP_TICKS|s}} ({{calc: TANK_STATIONARY_RANGE_RAMP_TICKS / 2|s}} with {{research:tank_plus}}).
- Moving or turning the hull resets it to {{unit:tank.attack.rangeTiles}} tiles.

## Armor

- Armored: weapons without armor penetration deal 25% (a Rifle shot does {{dmg:rifleman_rifle>tank}}).
- Takes more damage from the side and rear:

| Attacker | Front | Side | Rear |
|---|---|---|---|
| {{unit:tank}} | {{dmg:tank_cannon>tank.front}} | {{dmg:tank_cannon>tank.side}} | {{dmg:tank_cannon>tank.rear}} |
| {{unit:anti_tank_gun}} | {{dmg:anti_tank_gun>tank.front}} | {{dmg:anti_tank_gun>tank.side}} | {{dmg:anti_tank_gun>tank.rear}} |
| {{unit:panzerfaust}} | {{dmg:panzerfaust_loaded_shot>tank}} | {{dmg:panzerfaust_loaded_shot>tank}} | {{dmg:panzerfaust_loaded_shot>tank}} |

- When hit by one of these, turns its hull toward the shooter for {{TANK_ARMOR_REACTION_LOCK_TICKS|s}} (max {{TANK_ARMOR_REACTION_MAX_PIVOT_RAD|deg}}).

## Notes

- Cannon shells also deal {{TANK_HE_SPLASH_DAMAGE}} splash damage in {{TANK_HE_RADIUS_TILES}} tiles.
- The coaxial MG fires bursts: {{dmg:tank_coax>rifleman.burstMin}}–{{dmg:tank_coax>rifleman.burstMax}} damage to infantry.
- {{research:tank_plus}}: turn ×{{TANK_PLUS_TURN_MULTIPLIER}}, speed {{TANK_PLUS_SPEED_MULTIPLIER|change}}, sight +{{TANK_PLUS_SIGHT_BONUS}}, full range in half the time.
