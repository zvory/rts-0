import assert from "node:assert/strict";
import { KIND } from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { buildSelectionScene } from "../../client/src/input/selection_projection.js";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { HudSelectionPanel } from "../../client/src/hud_selection_panel.js";

const novices = [
  { id: 51, dao: null },
  { id: 52, chosenDao: "war" },
  { id: 53, kind: KIND.WAR_ADEPT, dao: "war" },
  { id: 54, kind: KIND.WAR_ADEPT, dao: "war" },
  { id: 55, kind: KIND.MAGIC_ADEPT, dao: "magic" },
  { id: 56, kind: KIND.LIGHTNESS_ADEPT, dao: "lightness" },
].map((unit, i) => ({ owner: 1, kind: KIND.WARRIOR, x: 100 + i * 40, y: 100, ...unit }));
const state = {
  playerId: 1,
  selection: new Set(),
  entityById: (id) => novices.find((unit) => unit.id === id),
  selectedEntities() { return novices.filter((unit) => this.selection.has(unit.id)); },
  setSelection(ids) { this.selection = new Set(ids); },
};
const input = Object.create(Input.prototype);
input.state = state;
input.selectionScene = buildSelectionScene({
  entities: novices,
  projection: createOrthographicProjectionSnapshot({
    x: 0, y: 0, zoom: 1, worldW: 2048, worldH: 2048, viewW: 640, viewH: 480,
  }, 1920),
});
const panel = new HudSelectionPanel(null, state);
for (const [index, expected] of [[0, [51, 52]], [1, [52, 51]], [2, [53, 54]], [4, [55]], [5, [56]]]) {
  const novice = novices[index];
  input._commitClickSelection({ x: novice.x, y: novice.y }, false, true);
  assert.deepEqual([...state.selection], expected,
    "world same-type selection separates each Dao and ignores chosen future Dao");
  for (const modifiers of [{ ctrlKey: true }, { metaKey: true }]) {
    state.setSelection(novices.map((unit) => unit.id));
    panel._applySelectionClick(novice.id, modifiers);
    assert.deepEqual([...state.selection], expected.slice().sort((a, b) => a - b),
      "HUD same-type selection separates each Dao and ignores chosen future Dao");
  }
}
panel.destroy();
console.log("dao_selection_contracts: world and HUD same-type selection passed");
