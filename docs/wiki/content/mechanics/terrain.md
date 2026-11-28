---
title: Terrain and vision
constants: MAP_TERRAIN_, ROAD_, SLOW_MOVEMENT_, UPHILL_, DOWNHILL_, MAX_ELEVATION_, CONCEALMENT_, DAMAGE_REDUCTION_TILE_, ELEVATION_
order: 30
---
## Movement

| Terrain | Speed |
|---|---|
| Open ground | ×1 |
| Road | ×{{ROAD_MOVEMENT_SPEED_MULTIPLIER}} |
| Slow tiles | ×{{SLOW_MOVEMENT_TILE_SPEED_MULTIPLIER}} |
| Uphill | ×{{UPHILL_MOVEMENT_SPEED_MULTIPLIER}} |
| Downhill | ×{{DOWNHILL_MOVEMENT_SPEED_MULTIPLIER}} |

Rock and water can't be crossed.

## Line of sight

- Rock blocks sight. Water doesn't.
- High ground: +1 tile of sight per {{ELEVATION_LEVELS_PER_SIGHT_TILE}} levels of elevation, up to +{{MAX_ELEVATION_SIGHT_BONUS_TILES}}.

## Forests and concealment

Forests (and any other tiles the map marks as concealing) hide units inside them.

- Sight reaches at most {{CONCEALMENT_SIGHT_DEPTH_TILES}} tiles into concealing tiles.
- A hidden enemy is revealed when one of your units comes within {{CONCEALMENT_CLOSE_DETECTION_RANGE_TILES}} tiles of it.
- It stays revealed for {{CONCEALMENT_DETECTION_PERSIST_TICKS|s}} after contact ends.

## Smoke

| | Radius | Duration |
|---|---|---|
| Smoke | {{SMOKE_CLOUD_RADIUS_TILES}} tiles | {{SMOKE_CLOUD_DURATION_TICKS|s}} |
| With {{research:smoke_plus}} | {{SMOKE_PLUS_CLOUD_RADIUS_TILES}} tiles | {{SMOKE_PLUS_CLOUD_DURATION_TICKS|s}} |

Inside smoke, units only see each other within {{SMOKE_MELEE_VISIBILITY_RANGE_TILES}} tile.
