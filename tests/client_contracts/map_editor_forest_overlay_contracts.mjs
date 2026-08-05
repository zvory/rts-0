import assert from "node:assert/strict";

import { MapEditorSession } from "../../client/src/map_editor_session.js";
import { FOREST_DOODAD_ID_BASE } from "../../client/src/map_authoring/forests.js";

const tile = { x: 8, y: 8 };
const session = new MapEditorSession({ storage: null });
session.initializeBlank({ size: 16, playerCount: 2 });
session.beginOverlayStroke("Painted independent effects");
session.paintOverlayTiles([tile], {
  concealment: true,
  noVehicle: true,
  noBuilding: true,
  damageReduction: true,
  slowMovement: true,
});
assert.equal(session.commitOverlayStroke(), true);
session.beginOverlayStroke("Painted forest");
session.paintForestTiles([tile], true);
assert.equal(session.commitOverlayStroke(), true);
session.beginOverlayStroke("Erased forest");
session.paintForestTiles([tile], false);
assert.equal(session.commitOverlayStroke(), true);
for (const field of ["concealmentTiles", "noVehicleTiles", "noBuildingTiles", "damageReductionTiles", "slowMovementTiles"]) {
  assert.deepEqual(session.exportMap()[field], [tile],
    `erasing forest preserves the independently authored ${field} overlay that overlapped it`);
}

const forest = [];
for (let y = 2; y < 14; y += 1) for (let x = 2; x < 14; x += 1) forest.push({ x, y });
const collisionSession = new MapEditorSession({ storage: null });
collisionSession.initializeBlank({ size: 16, playerCount: 2 });
collisionSession.beginOverlayStroke("Painted forest");
collisionSession.paintForestTiles(forest, true);
collisionSession.commitOverlayStroke();
const generatedBeforeNoop = collisionSession.exportMap().doodads;
collisionSession.beginOverlayStroke("Repainted forest");
assert.deepEqual(collisionSession.paintForestTiles(forest, true), []);
assert.equal(collisionSession.commitOverlayStroke(), false);
assert.deepEqual(collisionSession.exportMap().doodads, generatedBeforeNoop,
  "a no-op forest stroke preserves generated trees");
const collidingManual = collisionSession.exportMap();
const generatedCollision = collidingManual.doodads.find((doodad) => doodad.id > FOREST_DOODAD_ID_BASE);
Object.assign(generatedCollision, { x: 200, y: 200 });
collisionSession.loadAuthoredMap(collidingManual);
assert(collisionSession.exportMap().doodads.some((doodad) => (
  doodad.id === generatedCollision.id && doodad.x === 200 && doodad.y === 200
)), "a manual tree whose high id collides with a forest tile is not claimed as generated");
collisionSession.beginOverlayStroke("Erased forest");
collisionSession.paintForestTiles(forest, false);
collisionSession.commitOverlayStroke();
assert(collisionSession.exportMap().doodads.some((doodad) => (
  doodad.id === generatedCollision.id && doodad.x === 200 && doodad.y === 200
)), "forest erase preserves a colliding manual tree");
assert.deepEqual(collisionSession.exportMap().doodads, [generatedCollision],
  "forest erase does not leave a repaired-id duplicate of the colliding generated tree");
