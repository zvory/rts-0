| Tank stat | Before | After |
|---|---|---|
| Turn rate | ×1 | ×{{TANK_PLUS_TURN_MULTIPLIER}} |
| Speed | {{unit:tank.speedPxPerTick|speed}} | {{calc: unit:tank.speedPxPerTick * TANK_PLUS_SPEED_MULTIPLIER|speed}} |
| Sight | {{unit:tank.sightTiles}} tiles | {{calc: unit:tank.sightTiles + TANK_PLUS_SIGHT_BONUS}} tiles |
| Time to full range ({{TANK_STATIONARY_RANGE_MAX_TILES}} tiles) | {{TANK_STATIONARY_RANGE_RAMP_TICKS|s}} | {{calc: TANK_STATIONARY_RANGE_RAMP_TICKS / TANK_PLUS_RANGE_RAMP_RATE|s}} |
