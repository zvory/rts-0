---
constants: PANZERFAUST_
---
## Rocket

A Rifleman with one disposable anti-armor rocket.

- {{PANZERFAUST_DAMAGE}} damage, {{PANZERFAUST_ARMOR_PENETRATION|pct}} armor penetration, {{PANZERFAUST_RANGE_TILES}}-tile range.
- Aims for {{PANZERFAUST_WINDUP_TICKS|s}} ({{METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS|s}} with {{research:methamphetamines}}); the rocket flies for {{PANZERFAUST_TRAVEL_TICKS|s}}.
- Fires at vehicles, War Adepts, and War Masters. Ignores Tank facing.
- After firing, fights as a normal Rifleman.

| Target | Rocket damage | Target HP |
|---|---|---|
| Tank | {{dmg:panzerfaust_loaded_shot>tank}} | {{unit:tank.hp}} |
| Scout Car | {{dmg:panzerfaust_loaded_shot>scout_car}} | {{unit:scout_car.hp}} |
| Command Car | {{dmg:panzerfaust_loaded_shot>command_car}} | {{unit:command_car.hp}} |
| Rocket Truck | {{dmg:panzerfaust_loaded_shot>rocket_launcher}} | {{unit:rocket_launcher.hp}} |
