import { assert } from "./assertions.mjs";
import { TERRAIN } from "../../client/src/protocol.js";
import {
  drawTerrainTile,
  TERRAIN_BLEND_MODES,
  TERRAIN_BLEND_PRESETS,
} from "../../client/src/renderer/terrain.js";
import {
  groundTransitionEdges,
  impassableEdgeDirections,
  terrainBlendRank,
  terrainMaterial,
} from "../../client/src/renderer/terrain_palette.js";

const materialMap = {
  width: 3,
  height: 2,
  terrain: [
    TERRAIN.ROAD_BARE, TERRAIN.GRASS, TERRAIN.GRAVEL_A,
    TERRAIN.ROCK, TERRAIN.WATER, TERRAIN.WATER,
  ],
};
assert(
  terrainMaterial(TERRAIN.ROAD_BARE) === terrainMaterial(TERRAIN.ROAD_HORIZONTAL),
  "road markings share one terrain blend material",
);
assert(
  terrainBlendRank(TERRAIN.ROAD_BARE) < terrainBlendRank(TERRAIN.GRASS) &&
    terrainBlendRank(TERRAIN.GRASS) < terrainBlendRank(TERRAIN.GRAVEL_A) &&
    terrainBlendRank(TERRAIN.MUD_C) < terrainBlendRank(TERRAIN.FROSTED_GROUND),
  "terrain blend precedence is stable from road through frost",
);
assert(
  groundTransitionEdges(materialMap, 0, 0, TERRAIN.ROAD_BARE).map(({ direction }) => direction).join(",") === "east",
  "higher-ranked grass creeps into neighboring road",
);
assert(
  groundTransitionEdges(materialMap, 1, 0, TERRAIN.GRASS).map(({ direction }) => direction).join(",") === "east",
  "higher-ranked gravel creeps into neighboring grass without a reverse double band",
);
assert(
  impassableEdgeDirections(materialMap, 1, 1, TERRAIN.WATER).includes("west") &&
    !impassableEdgeDirections(materialMap, 0, 1, TERRAIN.ROCK).includes("east"),
  "water owns one dark rock/water separator instead of both blockers doubling it",
);
assert(
  !impassableEdgeDirections(materialMap, 2, 1, TERRAIN.WATER).includes("west"),
  "matching impassable terrain has no internal outline",
);

class TerrainContext {
  constructor() { this.calls = []; }
  set fillStyle(value) { this.calls.push(["style", value]); }
  fillRect(...args) { this.calls.push(["rect", ...args]); }
}

const blendMap = { width: 2, height: 1, terrain: [TERRAIN.ROAD_BARE, TERRAIN.FROSTED_GROUND] };
assert(
  TERRAIN_BLEND_PRESETS["hard-chips-wide"].depth > TERRAIN_BLEND_PRESETS["hard-chips"].depth &&
    TERRAIN_BLEND_PRESETS["organic-wide"].depth > TERRAIN_BLEND_PRESETS.organic.depth,
  "wide terrain prototypes isolate transition depth as an explicit factor",
);
assert(
  TERRAIN_BLEND_PRESETS["soft-ramp"].feather && TERRAIN_BLEND_PRESETS["soft-organic"].feather,
  "soft terrain prototypes opt into color feathering",
);
const signatures = TERRAIN_BLEND_MODES.map((terrainBlendMode) => {
  const first = new TerrainContext();
  const repeated = new TerrainContext();
  drawTerrainTile(first, blendMap, 0, 0, 8, { terrainBlendMode });
  drawTerrainTile(repeated, blendMap, 0, 0, 8, { terrainBlendMode });
  assert(
    JSON.stringify(first.calls) === JSON.stringify(repeated.calls),
    `${terrainBlendMode} terrain blending is deterministic`,
  );
  return JSON.stringify(first.calls);
});
assert(new Set(signatures).size === TERRAIN_BLEND_MODES.length, "terrain blend prototypes produce distinct edge masks");
assert(
  signatures[TERRAIN_BLEND_MODES.indexOf("soft-ramp")].includes("rgba") &&
    signatures[TERRAIN_BLEND_MODES.indexOf("soft-organic")].includes("rgba"),
  "soft terrain prototypes emit translucent source colors for browser-native interpolation",
);
