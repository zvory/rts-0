import { TERRAIN } from "./protocol.js";

export const MAP_EDITOR_GROUND_PALETTE = Object.freeze([
  [TERRAIN.GRASS, "Grass"],
  [TERRAIN.GRAVEL_A, "Gravel A — Slate"],
  [TERRAIN.GRAVEL_B, "Gravel B — Limestone"],
  [TERRAIN.GRAVEL_C, "Gravel C — Chalk"],
  [TERRAIN.DIRT_A, "Dirt A — Loam"],
  [TERRAIN.DIRT_B, "Dirt B — Red Clay"],
  [TERRAIN.DIRT_C, "Dirt C — Dry Ochre"],
  [TERRAIN.MUD_A, "Mud A — Churned"],
  [TERRAIN.MUD_B, "Mud B — Waterlogged"],
  [TERRAIN.MUD_C, "Mud C — Clay"],
  [TERRAIN.FROSTED_GROUND, "Frosted Ground"],
]);

export const MAP_EDITOR_FEATURE_PALETTE = Object.freeze([
  [TERRAIN.ROCK, "Stone"],
  [TERRAIN.WATER, "Water"],
  [TERRAIN.ROAD_BARE, "Road — bare"],
  [TERRAIN.ROAD_HORIZONTAL, "Road — horizontal"],
  [TERRAIN.ROAD_VERTICAL, "Road — vertical"],
  [TERRAIN.ROAD_DIAGONAL_NW_SE, "Road — diagonal ↘"],
  [TERRAIN.ROAD_DIAGONAL_NE_SW, "Road — diagonal ↙"],
]);
