import assert from "node:assert/strict";

import { createMapEditorPresentation } from "../../client/src/map_editor_presentation.js";
import {
  commitMapEditorSunField,
  previewMapEditorSunField,
} from "../../client/src/map_editor_sun_controls.js";
import { MapEditorViewport } from "../../client/src/map_editor_viewport.js";
import { MapEditorWorkerRenderer } from "../../client/src/renderer/map_editor_worker_renderer.js";
import { TERRAIN } from "../../client/src/protocol.js";
import {
  authoredMapFromMaterialized,
  MapEditorSession,
} from "../../client/src/map_editor_session.js";

const elevation = Array.from({ length: 16 * 16 }, (_, index) => index < 16 * 8 ? 1 : 4);
const authored = authoredMapFromMaterialized({
  name: "Relief", description: "", size: 16,
  terrain: Array(16 * 16).fill(TERRAIN.GRASS), elevation,
  sun: { azimuthDegrees: 315, elevationDegrees: 20, warmth: 70 },
  starts: [], baseSites: [], doodads: [],
});
const session = new MapEditorSession({ storage: null });
session.loadAuthoredMap(authored);
const viewport = {
  session,
  terrainRevision: 0,
  pendingTerrainUpdate: null,
  camera: { worldW: 1, setBounds() {} },
  root: { clientWidth: 800, clientHeight: 600 },
};
MapEditorViewport.prototype.rebuildTerrain.call(viewport);
assert.deepEqual(viewport.pendingTerrainUpdate.sun, authored.sun,
  "editor terrain replacement carries authored sun conditions");
assert.deepEqual(viewport.pendingTerrainUpdate.elevation, elevation,
  "editor terrain replacement carries materialized elevation");
assert.equal(MapEditorViewport.prototype.previewSunConditions.call(viewport, {
  azimuthDegrees: 90, elevationDegrees: 8, warmth: 95,
}), true);
assert.deepEqual(viewport.pendingTerrainUpdate.sun, {
  azimuthDegrees: 90, elevationDegrees: 8, warmth: 95,
}, "sun preview replaces the latest pending worker update without mutating the draft");
assert.deepEqual(session.draft.sun, authored.sun);

const previews = [];
const previewViewport = { previewSunConditions: (sun) => { previews.push(sun); return true; } };
assert.equal(previewMapEditorSunField(session, previewViewport, "elevationDegrees", 7), true);
assert.deepEqual(previews, [{ azimuthDegrees: 315, elevationDegrees: 7, warmth: 70 }]);
assert.equal(commitMapEditorSunField(session, "azimuthDegrees", 400), true);
assert.equal(session.draft.sun.azimuthDegrees, 359,
  "committing a sun control records one bounded authored-map mutation");
session.undo();
assert.equal(session.draft.sun.azimuthDegrees, 315, "sun settings participate in undo history");

let builtMap = null;
const worker = { terrainRevision: 0, renderer: {
  buildStaticMap(map) { builtMap = map; },
  updateStaticTerrainTiles() {},
} };
MapEditorWorkerRenderer.prototype._applyTerrain.call(worker, viewport.pendingTerrainUpdate);
assert.deepEqual(builtMap.sun, { azimuthDegrees: 90, elevationDegrees: 8, warmth: 95 });
assert.deepEqual(builtMap.elevation, elevation,
  "worker forwards live sun and elevation to the shared terrain renderer");

assert.throws(() => createMapEditorPresentation({
  frameId: 2,
  camera: { x: 0, y: 0, zoom: 1 },
  terrainUpdate: {
    kind: "replace", revision: 1, width: 2, height: 2, tileSize: 32,
    terrain: [0, 0, 0, 0], elevation: [0, 1], sun: null,
  },
}), /elevation shape/, "editor presentation rejects malformed relief data");

console.log("✅ map_editor_sun_contracts.mjs: authored controls and live worker lighting passed");
