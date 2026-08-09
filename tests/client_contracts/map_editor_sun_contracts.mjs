import assert from "node:assert/strict";

import { createMapEditorPresentation } from "../../client/src/map_editor_presentation.js";
import {
  commitMapEditorSunField,
  disableMapEditorSun,
  enableMapEditorSun,
  previewMapEditorSunDirectionField,
  previewMapEditorSunField,
} from "../../client/src/map_editor_sun_controls.js";
import {
  mapEditorSunDirectionPreview,
  MapEditorViewport,
} from "../../client/src/map_editor_viewport.js";
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
const directionPreviews = [];
const directionViewport = {
  previewSunDirection: (degrees) => directionPreviews.push(degrees),
  previewSunConditions: (sun) => { previews.push(sun); return true; },
};
assert.equal(previewMapEditorSunDirectionField(session, directionViewport, 90), true);
assert.deepEqual(directionPreviews, [90], "direction input publishes the temporary map guide");
assert.equal(previews.at(-1).azimuthDegrees, 90, "the guide and live lighting use the same azimuth");
assert.deepEqual(mapEditorSunDirectionPreview({ width: 100, height: 80 }, 0), {
  fromX: 1600, fromY: 1280, toX: 1600, toY: 819.2,
  azimuthDegrees: 0, label: "Sun source · 0° N",
}, "zero degrees points from map centre toward north");
const eastGuide = mapEditorSunDirectionPreview({ width: 100, height: 80 }, 90);
assert.equal(eastGuide.toX, 2060.8);
assert(Math.abs(eastGuide.toY - 1280) < 1e-9, "90 degrees points east without vertical drift");
assert.equal(commitMapEditorSunField(session, "azimuthDegrees", 400), true);
assert.equal(session.draft.sun.azimuthDegrees, 359,
  "committing a sun control records one bounded authored-map mutation");
session.undo();
assert.equal(session.draft.sun.azimuthDegrees, 315, "sun settings participate in undo history");

const flatSession = new MapEditorSession({ storage: null });
flatSession.initializeBlank({ size: 16, playerCount: 2 });
assert.equal(enableMapEditorSun(flatSession), true, "flat maps can opt into authored sunlight");
assert.deepEqual(flatSession.draft.sun, {
  azimuthDegrees: 315, elevationDegrees: 35, warmth: 25,
});
assert.equal(flatSession.exportMap().elevation, undefined, "flat sunlight does not force an elevation payload");
assert.deepEqual(flatSession.exportMap().sun, flatSession.draft.sun, "flat authored sunlight survives export");
assert.equal(disableMapEditorSun(flatSession), true, "flat authored sunlight can be removed");
assert.equal(flatSession.draft.sun, null);

flatSession.beginElevationStroke("Raised ridge");
assert.deepEqual(flatSession.paintElevationTiles([{ x: NaN, y: 3 }, { x: 2, y: Infinity }], 4), [],
  "malformed elevation coordinates are ignored without disturbing the active stroke");
assert.deepEqual(flatSession.paintElevationTiles([{ x: 2, y: 3 }], 4), [{ x: 2, y: 3, level: 4 }]);
assert.equal(flatSession.commitElevationStroke(), true);
assert.equal(flatSession.materialized().elevation[3 * 16 + 2], 4,
  "elevation painting materializes the selected height level");
assert.deepEqual(flatSession.draft.sun, {
  azimuthDegrees: 315, elevationDegrees: 35, warmth: 25,
}, "the first relief edit initializes valid sunlight");
assert.equal(disableMapEditorSun(flatSession), false, "relief maps retain their required sunlight");
flatSession.undo();
assert.equal(flatSession.draft.elevation.length, 0, "elevation strokes participate in undo history");
assert.equal(flatSession.draft.sun, null, "undo restores the pre-relief sunlight state atomically");

let builtMap = null;
const worker = { terrainRevision: 0, renderer: {
  previewStaticTerrain(map) { builtMap = map; },
  updateStaticTerrainTiles() {},
} };
MapEditorWorkerRenderer.prototype._applyTerrain.call(worker, viewport.pendingTerrainUpdate);
assert.deepEqual(builtMap.sun, { azimuthDegrees: 90, elevationDegrees: 8, warmth: 95 });
assert.deepEqual(builtMap.elevation, elevation,
  "worker forwards live sun and elevation through the layer-preserving terrain preview path");

assert.throws(() => createMapEditorPresentation({
  frameId: 2,
  camera: { x: 0, y: 0, zoom: 1 },
  terrainUpdate: {
    kind: "replace", revision: 1, width: 2, height: 2, tileSize: 32,
    terrain: [0, 0, 0, 0], elevation: [0, 1], sun: null,
  },
}), /elevation shape/, "editor presentation rejects malformed relief data");

console.log("✅ map_editor_sun_contracts.mjs: authored controls and live worker lighting passed");
