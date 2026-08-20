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

for (const [symmetry, guide] of [
  [MAP_EDITOR_SYMMETRY.DIAGONAL_MAIN_FLIP, { x0: 0, y0: 0, x1: 256, y1: 256 }],
  [MAP_EDITOR_SYMMETRY.DIAGONAL_ANTI_FLIP, { x0: 0, y0: 256, x1: 256, y1: 0 }],
]) {
  assert.deepEqual(
    new Set(symmetricMapTiles(8, [{ x: 1, y: 2 }], symmetry).map(({ x, y }) => `${x},${y}`)),
    new Set(["1,2", "6,5"]),
    "diagonal with flipped copy produces one oppositely oriented partner",
  );

  assert.deepEqual(
    mapEditorSymmetryGuideLines(8, symmetry),
    [guide],
    "diagonal with flipped copy displays its selected diagonal authoring boundary",
  );
}

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
