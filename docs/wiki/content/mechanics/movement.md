---
title: Movement
order: 40
constants: STUCK_, TOLERANT_ARRIVAL_, SIDESTEP_, STATIC_BLOCKED_, ARRIVE_RADIUS_, VEHICLE_WAYPOINT_
---
## Speeds

| Unit | Speed |
|---|---|
| {{unit:worker}} | {{unit:worker.speedPxPerTick|speed}} |
| {{unit:rifleman}} | {{unit:rifleman.speedPxPerTick|speed}} |
| {{unit:panzerfaust}} | {{unit:panzerfaust.speedPxPerTick|speed}} |
| {{unit:machine_gunner}} | {{unit:machine_gunner.speedPxPerTick|speed}} |
| {{unit:mortar_team}} | {{unit:mortar_team.speedPxPerTick|speed}} |
| {{unit:anti_tank_gun}} | {{unit:anti_tank_gun.speedPxPerTick|speed}} |
| {{unit:scout_car}} | {{unit:scout_car.speedPxPerTick|speed}} |
| {{unit:command_car}} | {{unit:command_car.speedPxPerTick|speed}} |
| {{unit:tank}} | {{unit:tank.speedPxPerTick|speed}} |
| {{unit:rocket_launcher}} | {{unit:rocket_launcher.speedPxPerTick|speed}} |
| {{unit:warrior}} | {{unit:warrior.speedPxPerTick|speed}} |
| {{unit:bow_novice}} | {{unit:bow_novice.speedPxPerTick|speed}} |
| {{unit:wind_adept}} | {{unit:wind_adept.speedPxPerTick|speed}} |
| {{unit:war_adept}} | {{unit:war_adept.speedPxPerTick|speed}} |
| {{unit:magic_adept}} | {{unit:magic_adept.speedPxPerTick|speed}} |

## What changes speed

- **Terrain:** roads, slow tiles and slopes; see {{page:mechanics/terrain}}.
- **Speed bonuses:** the {{unit:command_car}}'s aura (×{{COMMAND_CAR_AURA_SPEED_MULTIPLIER}}), {{ability:breakthrough}}, {{research:methamphetamines}} and the Shop's {{ability:haste}} item. Bonuses don't stack; see each page for which one wins.
- **Barbed Wire:** infantry on it move ×0.7 as fast; see {{building:barbed_wire}}.
- **Masters:** Cultivator Masters move ×{{DAO_MASTER_SPEED_MULTIPLIER}} as fast.

## Vehicles and obstacles

- Vehicles ({{units:movementBody=VehicleBody}}) have a turning body and can't pass {{building:tank_trap|Tank Traps}}. Infantry walk through traps.
- Finished Tank Traps and Barbed Wire belong to no one, so they also stop or hurt your own units.

The values below are the movement code's arrival, unsticking and repathing settings. Turn on **Show code** to see the function that uses each one.
