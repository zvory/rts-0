## Setup

- Deals **no damage** until set up ({{ANTI_TANK_GUN_SETUP_TICKS|s}}). Packing up takes {{ANTI_TANK_GUN_TEARDOWN_TICKS|s}}.
- Once set up: {{ANTI_TANK_GUN_DEPLOYED_RANGE_TILES}}-tile range, but only within a {{ANTI_TANK_GUN_FIELD_OF_FIRE_RAD|deg}} arc.

## Damage

One shot every {{unit:anti_tank_gun.attack.cooldownTicks|s}}.

| Target | Damage |
|---|---|
| Tank — front / side / rear | {{dmg:anti_tank_gun>tank.front}} / {{dmg:anti_tank_gun>tank.side}} / {{dmg:anti_tank_gun>tank.rear}} |
| Scout Car | {{dmg:anti_tank_gun>scout_car}} |
| Infantry ({{ANTI_TANK_GUN_INFANTRY_DAMAGE_MULTIPLIER|pct}} damage) | {{dmg:anti_tank_gun>rifleman}} |
