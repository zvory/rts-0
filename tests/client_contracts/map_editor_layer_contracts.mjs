import assert from "node:assert/strict";

import {
  defaultMapAuthoringLayerVisibility,
  MAP_AUTHORING_LAYER,
  MAP_AUTHORING_LAYER_IDS,
  mapAuthoringDoodadLayer,
  mapAuthoringLayerVisibilityFromSelection,
} from "../../client/src/map_authoring/layers.js";
import { MapEditorViewport } from "../../client/src/map_editor_viewport.js";

assert.deepEqual(Object.keys(defaultMapAuthoringLayerVisibility()), MAP_AUTHORING_LAYER_IDS,
  "the shared UI/CLI layer vocabulary has one explicit visibility switch per authoring layer");
assert.equal(mapAuthoringDoodadLayer("tree.oak"), MAP_AUTHORING_LAYER.TREES);
assert.equal(mapAuthoringDoodadLayer("unit.tank_trap"), MAP_AUTHORING_LAYER.GAMEPLAY_DOODADS);
assert.equal(mapAuthoringDoodadLayer("wildflower.cluster"), MAP_AUTHORING_LAYER.DECORATIVE_DOODADS);
assert.deepEqual(
  Object.entries(mapAuthoringLayerVisibilityFromSelection("stealth,trees"))
    .filter(([, visible]) => visible).map(([id]) => id),
  [MAP_AUTHORING_LAYER.STEALTH, MAP_AUTHORING_LAYER.TREES],
  "layer selection isolates an exact semantic subset",
);
assert.throws(() => mapAuthoringLayerVisibilityFromSelection("forest"), /Unsupported map authoring layer/,
  "the removed Forest authoring concept is not a layer alias");

let redraws = 0;
const viewport = {
  layerVisibility: defaultMapAuthoringLayerVisibility(),
  drawOverlay() { redraws += 1; },
};
assert.equal(MapEditorViewport.prototype.setLayerVisibility.call(
  viewport, MAP_AUTHORING_LAYER.STEALTH, false,
), true);
assert.equal(viewport.layerVisibility[MAP_AUTHORING_LAYER.STEALTH], false);
assert.equal(redraws, 1, "visibility changes enqueue a fresh worker-owned editor presentation");
assert.equal(MapEditorViewport.prototype.setLayerVisibility.call(
  viewport, MAP_AUTHORING_LAYER.STEALTH, false,
), false, "reapplying the same visibility is a no-op");

console.log("✅ map_editor_layer_contracts.mjs: shared layer vocabulary and viewport switches passed");
