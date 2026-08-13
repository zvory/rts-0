import assert from "node:assert/strict";

import {
  MAP_EDITOR_SYMMETRY,
  mapEditorSymmetrySupported,
  symmetricMapTiles,
  symmetricTerrainTiles,
} from "../../client/src/map_editor_session.js";
import {
  mapEditorSymmetryGuideLines,
} from "../../client/src/map_editor_viewport.js";
import { TERRAIN } from "../../client/src/protocol.js";

assert.deepEqual(
  symmetricMapTiles(8, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.QUADRANT_MIRROR),
  [{ x: 1, y: 2 }, { x: 1, y: 5 }, { x: 6, y: 2 }, { x: 6, y: 5 }],
  "quadrant mirror reflects into both adjacent quadrants and double-mirrors the opposite quadrant",
);

assert.deepEqual(
  mapEditorSymmetryGuideLines(8, MAP_EDITOR_SYMMETRY.QUADRANT_MIRROR),
  mapEditorSymmetryGuideLines(8, MAP_EDITOR_SYMMETRY.RADIAL),
  "both four-way modes divide the editor into four authoring regions",
);

assert.equal(
  mapEditorSymmetrySupported({ width: 12, height: 8 }, MAP_EDITOR_SYMMETRY.QUADRANT_MIRROR),
  true,
);
assert.deepEqual(
  symmetricMapTiles({ width: 12, height: 8 }, [{ x: 1, y: 2 }], MAP_EDITOR_SYMMETRY.QUADRANT_MIRROR),
  [{ x: 1, y: 2 }, { x: 1, y: 5 }, { x: 10, y: 2 }, { x: 10, y: 5 }],
  "quadrant mirrors remain valid on rectangular maps because neither reflection transposes axes",
);

assert.deepEqual(
  symmetricTerrainTiles(
    8,
    [{ x: 1, y: 2 }],
    TERRAIN.ROAD_DIAGONAL_NW_SE,
    MAP_EDITOR_SYMMETRY.QUADRANT_MIRROR,
  ),
  [
    { x: 1, y: 2, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NW_SE },
    { x: 1, y: 5, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NE_SW },
    { x: 6, y: 2, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NE_SW },
    { x: 6, y: 5, paintTerrainCode: TERRAIN.ROAD_DIAGONAL_NW_SE },
  ],
  "quadrant mirror reflects directional markings in adjacent copies and double-reflects the opposite copy",
);

console.log("map_editor_symmetry_contracts: ok");
