import assert from "node:assert/strict";

import { STATS } from "../../client/src/config.js";
import { KIND } from "../../client/src/protocol.js";
import { mapEditorResourcePatches } from "../../client/src/map_editor_resource_patches.js";
import { MAP_EDITOR_SYMMETRY } from "../../client/src/map_editor_session.js";
import { MapEditorViewport } from "../../client/src/map_editor_viewport.js";

const TILE_SIZE = 32;

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

const denseDraft = {
  width: 64,
  height: 64,
  terrain: Array(64).fill(".".repeat(64)),
  startLocations: [{ x: 21, y: 5 }],
  baseSites: [{ x: 21, y: 5, steelPatches: 36, oilPatches: 9 }],
};
for (const layout of [draft, denseDraft]) {
  const { footW, footH } = STATS[KIND.RESOURCE_DEPOT];
  const site = layout.baseSites[0];
  const minX = (site.x - Math.floor(footW / 2)) * TILE_SIZE;
  const minY = (site.y - Math.floor(footH / 2)) * TILE_SIZE;
  const maxX = minX + footW * TILE_SIZE;
  const maxY = minY + footH * TILE_SIZE;
  const resources = mapEditorResourcePatches(layout);
  assert.equal(resources.length, site.steelPatches + site.oilPatches);
  for (const patch of resources) {
    const dx = patch.x - Math.max(minX, Math.min(maxX, patch.x));
    const dy = patch.y - Math.max(minY, Math.min(maxY, patch.y));
    assert(dx * dx + dy * dy > (TILE_SIZE / 2) ** 2,
      `${patch.kind} patches must clear the Resource Depot, including dense fields near map edges`);
  }
}

const withoutOil = structuredClone(draft);
withoutOil.baseSites[0].oilPatches = 0;
assert.equal(mapEditorResourcePatches(withoutOil).length, 12,
  "editing a base patch count changes the next editor presentation");

const splitDraft = structuredClone(draft);
splitDraft.ground = splitDraft.terrain;
splitDraft.features = Array(32).fill(".".repeat(32));
delete splitDraft.terrain;
const blockedFeatureRow = [...splitDraft.features[4]];
blockedFeatureRow[4] = "~";
splitDraft.features[4] = blockedFeatureRow.join("");
assert.deepEqual(
  mapEditorResourcePatches(splitDraft).filter(({ kind }) => kind === "oil").map(({ x, y }) => [x, y]),
  [[144, 112], [208, 80], [80, 176]],
  "editor Oil stand-ins avoid impassable semantic features in split Map Editor drafts",
);

const viewport = {
  session: {
    draft: { width: 16, height: 16, terrain: Array(16).fill("."), baseSites: [], startLocations: [] },
    mapOverlay: () => ({ starts: [], bases: [] }),
  },
  symmetry: MAP_EDITOR_SYMMETRY.NONE,
  terrainRevision: 1,
  overlayRevision: 0,
  resourcePatchRevision: -1,
  resourcePatches: [],
  selectedBaseIndex: null,
  siteRecord: MapEditorViewport.prototype.siteRecord,
  resourcePatchRecords: MapEditorViewport.prototype.resourcePatchRecords,
  paintPreviewRecord: () => null,
};
MapEditorViewport.prototype.drawOverlay.call(viewport);
assert.equal(viewport.pendingOverlay.revision, 1);
assert.equal(viewport.resourcePatchRevision, 1,
  "Map Editor resource stand-ins are cached against terrain/base-data revisions");
const initialResourcePatches = viewport.pendingOverlay.resourcePatches;
MapEditorViewport.prototype.drawOverlay.call(viewport);
assert.equal(viewport.pendingOverlay.resourcePatches, initialResourcePatches,
  "unrelated overlay redraws reuse deterministic resource placement records");
viewport.terrainRevision += 1;
MapEditorViewport.prototype.drawOverlay.call(viewport);
assert.notEqual(viewport.pendingOverlay.resourcePatches, initialResourcePatches,
  "terrain/base-data revisions invalidate cached resource placement records");
assert(Array.isArray(viewport.pendingOverlay.gridPaths),
  "Map Editor grid lines cross as detached paths for the Pixi owner");
