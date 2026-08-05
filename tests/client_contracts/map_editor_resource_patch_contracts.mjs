import assert from "node:assert/strict";

import { mapEditorResourcePatches } from "../../client/src/map_editor_resource_patches.js";

const draft = {
  width: 32,
  height: 32,
  terrain: Array(32).fill(".".repeat(32)),
  startLocations: [{ x: 8, y: 8 }],
  baseSites: [{ x: 8, y: 8, steelPatches: 12, oilPatches: 3 }],
};
const patches = mapEditorResourcePatches(draft);
assert.equal(patches.filter(({ kind }) => kind === "steel").length, 12,
  "editor resource stand-ins mirror the authored Steel count");
assert.equal(patches.filter(({ kind }) => kind === "oil").length, 3,
  "editor resource stand-ins mirror the authored Oil count");
assert.deepEqual(patches.filter(({ kind }) => kind === "oil").map(({ x, y }) => [x, y]), [
  [144, 144], [176, 80], [80, 176],
], "editor Oil stand-ins mirror server tile offsets for the north-west base");

const withoutOil = structuredClone(draft);
withoutOil.baseSites[0].oilPatches = 0;
assert.equal(mapEditorResourcePatches(withoutOil).length, 12,
  "editing a base patch count changes the next editor presentation");
