import assert from "node:assert/strict";

import { TERRAIN } from "../../client/src/protocol.js";
import {
  armMapEditorLocation,
  updateMapEditorBasePatchCount,
  updateMapEditorPendingBasePatchCount,
} from "../../client/src/map_editor_location_resources.js";
import {
  addSymmetricDraftLocations,
  authoredMapFromMaterialized,
  MAP_EDITOR_DEFAULT_OIL_PATCHES,
  MAP_EDITOR_DEFAULT_STEEL_PATCHES,
  MAP_EDITOR_SYMMETRY,
  MapEditorSession,
  updateSymmetricDraftBasePatchCount,
} from "../../client/src/map_editor_session.js";

{
  const viewport = {
    tool: null,
    armTool(tool) { this.tool = tool; },
  };
  const panel = {
    viewport,
    symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
    pendingBaseSteelPatches: MAP_EDITOR_DEFAULT_STEEL_PATCHES,
    pendingBaseOilPatches: MAP_EDITOR_DEFAULT_OIL_PATCHES,
    setStatus(message) { this.status = message; },
  };
  armMapEditorLocation(panel, "base", null, true);
  updateMapEditorPendingBasePatchCount(panel, "steelPatches", 36);
  updateMapEditorPendingBasePatchCount(panel, "oilPatches", 9);
  assert.deepEqual(viewport.tool, {
    kind: "base",
    locationIndex: null,
    add: true,
    symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
    steelPatches: 36,
    oilPatches: 9,
  }, "add-mode resource controls remain editable and carry their counts into placement");

  const draft = authoredMapFromMaterialized({
    name: "Rich symmetric bases", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [],
    baseSites: [],
  });
  const result = addSymmetricDraftLocations(draft, {
    kind: viewport.tool.kind,
    tile: { x: 8, y: 8 },
    symmetry: viewport.tool.symmetry,
    steelPatches: viewport.tool.steelPatches,
    oilPatches: viewport.tool.oilPatches,
  });
  assert.deepEqual(result, { ok: true, count: 2 });
  assert(draft.baseSites.every((site) => site.steelPatches === 36 && site.oilPatches === 9),
    "every base created by symmetry receives the requested resource counts");

  const startDraft = authoredMapFromMaterialized({
    name: "Rich symmetric starts", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [],
    baseSites: [],
  });
  assert.deepEqual(addSymmetricDraftLocations(startDraft, {
    kind: "start",
    tile: { x: 8, y: 8 },
    symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
    steelPatches: 36,
    oilPatches: 9,
  }), { ok: true, count: 2 });
  assert(startDraft.baseSites.every((site) => site.steelPatches === 36 && site.oilPatches === 9),
    "player starts created by symmetry receive the requested backing-base resources");
}

{
  const session = new MapEditorSession({ storage: null });
  session.loadAuthoredMap(authoredMapFromMaterialized({
    name: "Symmetric resource edits", description: "", size: 32,
    terrain: Array(32 * 32).fill(TERRAIN.GRASS),
    starts: [],
    baseSites: [
      { x: 8, y: 8, steelPatches: 12, oilPatches: 3 },
      { x: 8, y: 23, steelPatches: 12, oilPatches: 3 },
      { x: 16, y: 16, steelPatches: 12, oilPatches: 3 },
    ],
  }));
  const panel = {
    session,
    symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
    setStatus(message) { this.status = message; },
  };
  updateMapEditorBasePatchCount(panel, 0, "steelPatches", 30);
  updateMapEditorBasePatchCount(panel, 0, "oilPatches", 8);
  assert.deepEqual(
    session.draft.baseSites.map(({ steelPatches, oilPatches }) => ({ steelPatches, oilPatches })),
    [
      { steelPatches: 30, oilPatches: 8 },
      { steelPatches: 30, oilPatches: 8 },
      { steelPatches: 12, oilPatches: 3 },
    ],
    "resource edits update existing symmetric partners without touching unrelated bases",
  );
  assert.deepEqual(updateSymmetricDraftBasePatchCount(session.draft, {
    baseIndex: 0,
    fieldName: "unknown",
    value: 1,
    symmetry: MAP_EDITOR_SYMMETRY.HORIZONTAL,
  }), { ok: false, error: "Choose a valid base resource field." });
}

console.log("map_editor_location_resource_contracts: ok");
