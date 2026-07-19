import assert from "node:assert/strict";
import {
  gfxFillStrokePath,
  gfxStrokePaths,
} from "../../client/src/renderer/native_graphics.js";
import { drawMagicAnchor } from "../../client/src/renderer/magic_anchor_effect.js";
import { drawRotatedRectOutline } from "../../client/src/renderer/shared.js";
import { _drawPanzerfaustImpacts } from "../../client/src/renderer/panzerfaust_feedback.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

function callsNamed(graphics, name) {
  return graphics.calls.filter(([call]) => call === name);
}

{
  const g = new RecordingGraphics();
  gfxStrokePaths(g, [
    [[0, 0], [10, 0], [10, 10]],
    [[20, 20], [30, 30]],
  ], 3, 0xabcdef, 0.75);
  assert.deepEqual(g.calls, [
    ["moveTo", 0, 0],
    ["lineTo", 10, 0],
    ["lineTo", 10, 10],
    ["moveTo", 20, 20],
    ["lineTo", 30, 30],
    ["lineStyle", 3, 0xabcdef, 0.75],
  ], "multiple subpaths commit one v8 stroke, preserving continuous joins and move boundaries");
}

{
  const g = new RecordingGraphics();
  gfxFillStrokePath(g, [[0, -5], [4, 0], [0, 5], [-4, 0]], {
    fill: { color: 0xc7d07a, alpha: 0.18 },
    stroke: { width: 2.2, color: 0xc7d07a, alpha: 0.88 },
  });
  assert.equal(callsNamed(g, "closePath").length, 1, "closed shape explicitly closes its path");
  assert.deepEqual(g.calls.at(-2), ["beginFill", 0xc7d07a, 0.18]);
  assert.deepEqual(g.calls.at(-1), ["lineStyle", 2.2, 0xc7d07a, 0.88]);
}

{
  const g = new RecordingGraphics();
  drawMagicAnchor(g, { id: 9, x: 40, y: 50, expiresIn: 12 }, 48);
  const diamondFill = g.calls.findIndex((call) =>
    call[0] === "beginFill" && call[1] === 0xc7d07a && call[2] === 0.18);
  assert(diamondFill > 0, "Magic Anchor commits the diamond fill");
  assert.equal(g.calls[diamondFill - 1][0], "closePath", "Magic Anchor closes before filling");
  assert.equal(g.calls[diamondFill + 1][0], "lineStyle", "Magic Anchor strokes the same path after filling");
}

{
  const g = new RecordingGraphics();
  drawRotatedRectOutline(g, 10, 20, 12, 8, Math.PI / 5, 2, 0xff00aa, 0.6);
  assert.equal(callsNamed(g, "moveTo").length, 1);
  assert.equal(callsNamed(g, "lineTo").length, 4);
  assert.equal(callsNamed(g, "lineStyle").length, 1,
    "rotated outline is one joined native v8 stroke");
}

{
  const g = new RecordingGraphics();
  _drawPanzerfaustImpacts.call({
    _feedbackGfx: g,
    visualNow: () => 100,
  }, {
    livePanzerfaustImpacts: () => [{ id: 1, x: 20, y: 30, seed: 7, createdAt: 0 }],
  });
  const joinedRingStroke = g.calls.some((call, index) => call[0] === "lineStyle" &&
    call[1] === 3 && callsNamed({ calls: g.calls.slice(Math.max(0, index - 12), index) }, "lineTo").length === 10);
  assert(joinedRingStroke, "Panzerfaust jagged ring commits one joined stroke after all segments");
}

console.log("native_graphics_contracts: ok");
