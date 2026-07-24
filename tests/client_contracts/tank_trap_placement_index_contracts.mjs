import { assert } from "./assertions.mjs";
import {
  createFootprintPlacementBlockerQuery,
  footprintPlacementBlocker,
  placementPolicyForBuilding,
} from "../../client/src/input/placement.js";
import { KIND, TERRAIN } from "../../client/src/protocol.js";

const entities = [
  { id: 101, owner: 1, kind: KIND.TANK, x: 58, y: 48, facing: 0.4 },
  { id: 102, owner: 1, kind: KIND.SCOUT_CAR, x: 130, y: 75, facing: 1.9 },
  { id: 103, owner: 1, kind: KIND.ANTI_TANK_GUN, x: 190, y: 110, facing: 0.8 },
  { id: 104, owner: 1, kind: KIND.WORKER, x: 75, y: 145 },
  { id: 105, owner: 2, kind: KIND.BARRACKS, x: 144, y: 176 },
  { id: 106, owner: 0, kind: KIND.OIL, x: 240, y: 80, remaining: 1000 },
  { id: 107, owner: 0, kind: KIND.STEEL, x: 48, y: 240, remaining: 1000 },
  { id: 108, owner: 2, kind: KIND.TANK, x: 176, y: 48, facing: 0, shotReveal: true },
  { id: 109, owner: 2, kind: KIND.TANK, x: 208, y: 48, facing: 0, visionOnly: true },
];
const allowedOverlapIds = new Set([104]);
const policy = placementPolicyForBuilding(KIND.TANK_TRAP);
const map = { width: 10, height: 10, tileSize: 32, terrain: new Array(100).fill(0) };
map.terrain[8 * map.width + 8] = TERRAIN.ROCK;

let sourceScans = 0;
const countedEntities = {
  [Symbol.iterator]() {
    sourceScans += 1;
    return entities[Symbol.iterator]();
  },
};
const indexedBlocker = createFootprintPlacementBlockerQuery(
  countedEntities,
  allowedOverlapIds,
  map,
  policy,
);
assert(sourceScans === 1, "Tank Trap placement index scans the visible entity source once");

for (let footH = 1; footH <= 2; footH++) {
  for (let footW = 1; footW <= 2; footW++) {
    for (let tileY = -1; tileY <= map.height; tileY++) {
      for (let tileX = -1; tileX <= map.width; tileX++) {
        const expected = footprintPlacementBlocker(
          entities,
          allowedOverlapIds,
          tileX,
          tileY,
          footW,
          footH,
          map,
          policy,
        );
        assert(
          indexedBlocker(tileX, tileY, footW, footH) === expected,
          `indexed Tank Trap placement preserves exact blocker semantics at ${tileX},${tileY} ${footW}x${footH}`,
        );
      }
    }
  }
}

assert(
  sourceScans === 1,
  "Tank Trap line queries reuse the spatial index instead of rescanning visible entities per preview site",
);

console.log("✅ tank_trap_placement_index_contracts.mjs: parity and one-scan contracts passed");
