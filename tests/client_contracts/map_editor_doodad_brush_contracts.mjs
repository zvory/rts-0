import assert from "node:assert/strict";

import {
  createDoodadSprayStroke,
  doodadTypeFromSelection,
  extendDoodadSprayStroke,
  MAP_EDITOR_MAX_DOODADS,
  MAP_EDITOR_DOODAD_TYPES,
  MAP_EDITOR_MAX_SPRAY_DENSITY,
} from "../../client/src/map_editor_doodads.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import { MAP_EDITOR_SYMMETRY, MapEditorSession } from "../../client/src/map_editor_session.js";
import { MapEditorViewport } from "../../client/src/map_editor_viewport.js";

{
  const viewport = { armTool(tool) { this.tool = tool; } };
  const panel = {
    viewport,
    selectedDoodadType: MAP_EDITOR_DOODAD_TYPES.TREE_PINE,
    doodadColor: "#e8b84a",
    doodadRadius: 48,
    doodadDensity: 4,
    symmetry: MAP_EDITOR_SYMMETRY.NONE,
    render() {},
  };
  MapEditorPanel.prototype.armDoodad.call(panel, "spray");
  assert.equal(viewport.tool.typeId, MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE,
    "arming doodad placement cannot retain a raw tree selection outside the catalog");
  assert.deepEqual(viewport.tool.typeIds, [MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE],
    "the doodad tool falls back to a placeable palette object");
  assert.equal(viewport.tool.density, 4, "wildflower spray retains the selected density");
  MapEditorPanel.prototype.armDoodad.call(panel, "remove");
  assert.equal(viewport.tool.mode, "place", "the palette no longer arms the removed box-selection tool");
}

{
  const dense = createDoodadSprayStroke({ x: 100, y: 100 }, {
    radius: 256,
    density: MAP_EDITOR_MAX_SPRAY_DENSITY,
    seed: 23,
  });
  assert.equal(dense.stroke.density, MAP_EDITOR_MAX_SPRAY_DENSITY,
    "the spray sampler accepts the full editor density range");
  assert(extendDoodadSprayStroke(dense.stroke, { x: 356, y: 100 }).length > 100,
    "the expanded density range produces substantially denser strokes");
  const treeMix = [MAP_EDITOR_DOODAD_TYPES.TREE_OAK, MAP_EDITOR_DOODAD_TYPES.TREE_PINE];
  assert(treeMix.includes(doodadTypeFromSelection(treeMix, 11)),
    "procedural tree placement chooses only from the selected species");
  assert.deepEqual(new Set(Array.from({ length: 32 }, (_, index) => doodadTypeFromSelection(treeMix, index + 1))), new Set(treeMix),
    "procedural placement varies across every species in the selected tree mix");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.draft.doodads = [
    { id: 1, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK, x: 32, y: 32 },
    { id: 3, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_PINE, x: 96, y: 32 },
  ];
  session.beginDoodadStroke("Placed mixed batch");
  const added = session.placeDoodadRecords([
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE, x: 160, y: 32 },
    { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER, x: 224, y: 32 },
  ]);
  assert.deepEqual(added.map(({ id, typeId }) => ({ id, typeId })), [
    { id: 2, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_SPRUCE },
    { id: 4, typeId: MAP_EDITOR_DOODAD_TYPES.TREE_ALDER },
  ], "mixed-species batches retain smallest-free stable ids across existing holes");
  session.cancelDoodadStroke();
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
    { x: 64, y: 96 }, { x: 80, y: 96 }, { x: 128, y: 96 },
  ], "tree spray accepts dense candidates instead of imposing a local spacing cap");
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [{ x: 64, y: 96 }]);
  assert.deepEqual(updates[1].upserts.map(({ x, y }) => ({ x, y })), [{ x: 64, y: 96 }],
    "a repeated tree spray adds another doodad at the same location");
  assert(updates[0].upserts.every(({ typeId }) => viewport.tool.typeIds.includes(typeId)),
    "mixed-tree placement uses only selected species");
  session.cancelDoodadStroke();
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Placed density fixture");
  session.placeDoodads([
    { x: 64, y: 96 },
    { x: 96, y: 96 },
  ], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  session.commitDoodadStroke();
  session.beginDoodadStroke("Sprayed symmetric mixed trees");
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
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [
    { x: 80, y: 96 },
    { x: 431, y: 415 },
  ]);
  assert.deepEqual(updates[0].upserts.map(({ x, y }) => ({ x, y })), [
    { x: 80, y: 96 }, { x: 431, y: 415 },
  ], "a dense tree spray keeps both symmetry partners once even when its sampled points overlap");
  assert.equal(new Set(updates[0].upserts.map(({ typeId }) => typeId)).size, 1,
    "mirrored trees use one species from the selected mix");
  MapEditorViewport.prototype.placeDoodadPoints.call(viewport, [{ x: 160, y: 160 }]);
  assert.deepEqual(updates[1].upserts.map(({ x, y }) => ({ x, y })), [
    { x: 160, y: 160 }, { x: 351, y: 351 },
  ], "a repeated symmetric spray adds another complete group");
  assert.equal(new Set(updates[1].upserts.map(({ typeId }) => typeId)).size, 1,
    "mirrored trees use the same species from the selected mix");
  session.cancelDoodadStroke();
}

{
  const viewport = {
    doodadPointerMode: "place",
    doodadLastWorld: { x: 40, y: 50 },
    placeDoodadPoints() { throw new Error("place must not continue while dragging"); },
  };
  MapEditorViewport.prototype.continueDoodadPointer.call(viewport, { x: 100, y: 120 });
  assert.deepEqual(viewport.doodadLastWorld, { x: 100, y: 120 },
    "place is click-only; moving the pointer does not add a drag trail");
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.draft.doodads = Array.from({ length: MAP_EDITOR_MAX_DOODADS - 1 }, (_, index) => ({
    id: index + 1,
    typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK,
    x: index % 512,
    y: Math.floor(index / 512),
  }));
  session.beginDoodadStroke("Sprayed near cap");
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
  assert.deepEqual(updates, [], "the doodad cap does not admit half a symmetry group");
  assert.equal(session.draft.doodads.length, MAP_EDITOR_MAX_DOODADS - 1);
  session.cancelDoodadStroke();
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 16, playerCount: 1 });
  session.beginDoodadStroke("Placed symmetric erase fixture");
  session.placeDoodads([
    { x: 80, y: 96 }, { x: 431, y: 415 }, { x: 256, y: 96 },
  ], { typeId: MAP_EDITOR_DOODAD_TYPES.TREE_OAK });
  session.commitDoodadStroke();
  session.beginDoodadStroke("Erased symmetric doodads");
  const updates = [];
  const viewport = {
    session,
    tool: { radius: 4, symmetry: MAP_EDITOR_SYMMETRY.HALF_TURN },
    queueDoodadPatch(update) { updates.push(structuredClone(update)); },
  };
  MapEditorViewport.prototype.eraseDoodadsAt.call(viewport, { x: 80, y: 96 });
  assert.deepEqual(updates, [{ removedIds: [1, 2] }],
    "erase mirrors its brush through the active doodad symmetry");
  assert.deepEqual(session.draft.doodads.map(({ id }) => id), [3],
    "symmetric erase preserves doodads outside either brush position");
  session.cancelDoodadStroke();
}
