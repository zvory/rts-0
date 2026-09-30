import assert from "node:assert/strict";
import { ClientIntent } from "../../client/src/client_intent.js";
import { KIND } from "../../client/src/protocol.js";
import { _refreshPointTargetPreview } from "../../client/src/input/commands.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";
import { drawTankPointPreview } from "../../client/src/renderer/tank_point_preview.js";
import { updateMinimapPointPreview } from "../../client/src/minimap_point_preview.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

const intent = new ClientIntent();
const selected = [
  { id: 1, owner: 1, kind: KIND.TANK, hp: 100, x: 100, y: 100 },
  { id: 2, owner: 1, kind: KIND.TANK, hp: 100, x: 300, y: 200 },
  { id: 3, owner: 2, kind: KIND.TANK, hp: 100, x: 900, y: 900 },
  { id: 4, owner: 1, kind: KIND.WORKER, hp: 100, x: 800, y: 800 },
  { id: 5, owner: 1, kind: KIND.TANK, hp: 0, x: 700, y: 700 },
];
const state = { playerId: 1, selectedEntities: () => selected };
const view = (extra = {}) => buildRendererFeedbackView(state, { clientIntent: intent, ...extra });
intent.beginCommandTarget("pointTanks");
_refreshPointTargetPreview.call({ clientIntent: intent, mouse: { x: 10, y: 20 },
  state, camera: { projectionSnapshot: () => ({ groundAtScreen: () => ({ x: 400, y: 450 }) }) },
  _groundAtScreen() { throw new Error("Must use the current camera projection"); },
});
assert.deepEqual(view().pointTargetPreview, { originX: 200, originY: 150, mouseX: 400, mouseY: 450 });
selected[0].x = 200;
assert.equal(view().pointTargetPreview.originX, 250, "centroid follows moving selected tanks");
assert.equal(view({ previewSurface: "minimap" }).pointTargetPreview, null);
updateMinimapPointPreview(intent, 600, 700);
assert.equal(view({ previewSurface: "minimap" }).pointTargetPreview.mouseX, 600);
const g = new RecordingGraphics();
drawTankPointPreview(g, view().pointTargetPreview);
assert(g.calls.some((c) => c[0] === "moveTo" && c[1] === 250 && c[2] === 150));
assert(g.calls.some((c) => c[0] === "lineTo" && c[1] === 600 && c[2] === 700));
assert.equal(g.calls.filter((c) => c[0] === "lineTo").length, 6, "outlined shaft and arrowhead");
const zero = new RecordingGraphics();
drawTankPointPreview(zero, { originX: 1, originY: 2, mouseX: 1, mouseY: 2 });
assert(zero.calls.some((c) => c[0] === "drawCircle"));
assert(!zero.calls.some((c) => c[0] === "lineTo"));
intent.endCommandTarget();
assert.equal(intent.pointTargetPreview, null);
assert.equal(view().pointTargetPreview, null);
intent.beginCommandTarget("pointTanks");
intent.updatePointTargetPreview({ mouseX: 10, mouseY: 20 });
intent.beginCommandTarget("move");
assert.equal(view().pointTargetPreview, null);
intent.beginCommandTarget("pointTanks");
intent.updatePointTargetPreview({ mouseX: 10, mouseY: 20 });
assert.equal(view({ selectedEntities: [] }).pointTargetPreview, null);
console.log("point_preview_contracts: ok");
