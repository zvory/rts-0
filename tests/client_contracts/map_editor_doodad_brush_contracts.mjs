import assert from "node:assert/strict";

import {
  createDoodadDragStroke,
  doodadTypeFromSelection,
  extendDoodadDragStroke,
  MAP_EDITOR_DOODAD_TYPES,
  MAP_EDITOR_TREE_MIN_SPACING,
  spacedTreePlacements,
} from "../../client/src/map_editor_doodads.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import { MAP_EDITOR_SYMMETRY, MapEditorSession } from "../../client/src/map_editor_session.js";
import { MapEditorViewport } from "../../client/src/map_editor_viewport.js";

{
  const viewport = { armTool(tool) { this.tool = tool; } };
  const panel = {
    viewport,
    selectedDoodadType: MAP_EDITOR_DOODAD_TYPES.TREE_PINE,
    selectedTreeTypes: new Set([
      MAP_EDITOR_DOODAD_TYPES.TREE_OAK,
      MAP_EDITOR_DOODAD_TYPES.TREE_PINE,
      MAP_EDITOR_DOODAD_TYPES.TREE_ALDER,
    ]),
    doodadColor: "#e8b84a",
    doodadRadius: 48,
    doodadDensity: 4,
    symmetry: MAP_EDITOR_SYMMETRY.NONE,
    render() {},
  };
  MapEditorPanel.prototype.armDoodad.call(panel, "spray");
  assert.deepEqual(viewport.tool.typeIds, [
    MAP_EDITOR_DOODAD_TYPES.TREE_OAK,
    MAP_EDITOR_DOODAD_TYPES.TREE_PINE,
    MAP_EDITOR_DOODAD_TYPES.TREE_ALDER,
  ], "the spray tool carries the complete selected tree-species set");
  assert.equal(viewport.tool.density, 4, "tree spray retains the low default density");
}

{
  const once = createDoodadDragStroke({ x: 100, y: 100 });
  const oncePlacements = [...once.placements, ...extendDoodadDragStroke(once.stroke, { x: 300, y: 100 })];
  const partitioned = createDoodadDragStroke({ x: 100, y: 100 });
  const partitionedPlacements = [...partitioned.placements];
  for (const x of [115, 160, 215, 300]) {
    partitionedPlacements.push(...extendDoodadDragStroke(partitioned.stroke, { x, y: 100 }));
  }
  assert.deepEqual(partitionedPlacements, oncePlacements,
    "tree drag placement is independent of pointer-event subdivision");
  assert.equal(MAP_EDITOR_TREE_MIN_SPACING, 64, "tree brushes keep centres at least two tile widths apart");
  assert.deepEqual(spacedTreePlacements([
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 64, y: 64 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE, x: 300, y: 300 },
  ], [
    { x: 96, y: 64 },
    { x: 128, y: 64 },
    { x: 170, y: 64 },
  ]), [
    { x: 128, y: 64 },
  ], "tree drag and spray candidates cannot overlap existing or same-stroke trees inside two tiles");
  const treeMix = [MAP_EDITOR_DOODAD_TYPES.TREE_OAK, MAP_EDITOR_DOODAD_TYPES.TREE_PINE];
  assert(treeMix.includes(doodadTypeFromSelection(treeMix, 11)),
    "procedural tree placement chooses only from the selected species");
  assert.deepEqual(new Set(Array.from({ length: 32 }, (_, index) => doodadTypeFromSelection(treeMix, index + 1))), new Set(treeMix),
    "procedural placement varies across every species in the selected tree mix");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Sprayed mixed trees");
  const updates = [];
  const viewport = {
    session,
    tool: {
      typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK,
      typeIds: [MAP_EDITOR_DOODAD_TYPES.TREE_OAK, MAP_EDITOR_DOODAD_TYPES.TREE_PINE],
      color: null,
      symmetry: MAP_EDITOR_SYMMETRY.NONE,
    },
    queueDoodadPatch(update) { updates.push(structuredClone(update)); },
  };
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [
    { x: 64, y: 96 },
    { x: 80, y: 96 },
    { x: 128, y: 96 },
  ]);
  assert.deepEqual(updates[0].upserts.map(({ x, y }) => ({ x, y })), [
    { x: 64, y: 96 },
    { x: 128, y: 96 },
  ], "mixed-tree placement filters too-close candidates");
  assert(updates[0].upserts.every(({ typeId }) => viewport.tool.typeIds.includes(typeId)),
    "mixed-tree placement uses only selected species");
  session.cancelDoodadStroke();
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Placed spacing fixture");
  session.placeDoodads([{ x: 64, y: 96 }], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  session.commitDoodadStroke();
  session.beginDoodadStroke("Placed symmetric mixed trees");
  const updates = [];
  const viewport = {
    session,
    tool: {
      typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK,
      typeIds: [MAP_EDITOR_DOODAD_TYPES.TREE_OAK, MAP_EDITOR_DOODAD_TYPES.TREE_PINE],
      color: null,
      symmetry: MAP_EDITOR_SYMMETRY.HALF_TURN,
    },
    queueDoodadPatch(update) { updates.push(structuredClone(update)); },
  };
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [{ x: 80, y: 96 }]);
  assert.deepEqual(session.draft.doodads.map(({ x, y }) => ({ x, y })), [{ x: 64, y: 96 }],
    "tree spacing rejects an entire symmetric group when any counterpart is blocked");
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [{ x: 160, y: 160 }]);
  assert.deepEqual(updates[0].upserts.map(({ x, y }) => ({ x, y })), [
    { x: 160, y: 160 }, { x: 351, y: 351 },
  ], "an unblocked symmetric tree group is placed in full");
  assert.equal(new Set(updates[0].upserts.map(({ typeId }) => typeId)).size, 1,
    "mirrored trees use the same species from the selected mix");
  session.cancelDoodadStroke();
}
