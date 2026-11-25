import assert from "node:assert/strict";
import {
  gfxFillStrokePath,
  gfxStrokePaths,
} from "../../client/src/renderer/native_graphics.js";
import { drawMagicAnchor } from "../../client/src/renderer/magic_anchor_effect.js";
import { drawRotatedRectOutline } from "../../client/src/renderer/shared.js";
import { _drawPanzerfaustImpacts } from "../../client/src/renderer/panzerfaust_feedback.js";
import { _drawAbilityObjects } from "../../client/src/renderer/feedback.js";
import { EKAT_MAGIC_ANCHOR_RADIUS_TILES, DISCIPLE_SHIELD_DURATION_TICKS, DISCIPLE_SHIELD_RADIUS_TILES } from "../../client/src/config.js";
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

// Shield lifetime maxima are public with the visible field.
for (const [expiresIn, expectedWidth] of [
  [DISCIPLE_SHIELD_DURATION_TICKS, 56],
  [DISCIPLE_SHIELD_DURATION_TICKS / 2, 28],
  [0, 0],
  [DISCIPLE_SHIELD_DURATION_TICKS * 2, 56],
  [-1, 0],
]) {
  const g = new RecordingGraphics();
  _drawAbilityObjects.call({ _abilityObjectGfx: g, _renderClock: { now: () => 0 } }, {
    abilityObjects: [{ id: 1, kind: "shield", x: 100, y: 200, expiresIn, ownerState: { durationTicks: DISCIPLE_SHIELD_DURATION_TICKS } }],
  });
  const rects = callsNamed(g, "drawRect");
  assert.deepEqual(rects[0], ["drawRect", 71, 200 - 32 * DISCIPLE_SHIELD_RADIUS_TILES - 3, 58, 6], "black backing below HP row");
  if (expectedWidth > 0) {
    assert.deepEqual(rects[1], ["drawRect", 72, 200 - 32 * DISCIPLE_SHIELD_RADIUS_TILES - 2, expectedWidth, 4]);
    assert(g.calls.some(call => call[0] === "beginFill" && call[1] === 0x9a9a9a));
  } else {
    assert.equal(rects.length, 1, "expired shields have an empty bar");
  }
}

// The same shield redraws its current pool independently of its lifetime.
for (const [hp, expectedWidth] of [[150, 56], [75, 28], [37.5, 14], [0, 0], [300, 56], [-1, 0]]) {
  const g = new RecordingGraphics();
  _drawAbilityObjects.call({ _abilityObjectGfx: g, _renderClock: { now: () => 0 } }, {
    abilityObjects: [{ id: 1, kind: "shield", x: 100, y: 200,
      expiresIn: DISCIPLE_SHIELD_DURATION_TICKS / 2, ownerState: { hp, maxHp: 150, durationTicks: DISCIPLE_SHIELD_DURATION_TICKS } }],
  });
  const rects = callsNamed(g, "drawRect");
  assert.deepEqual(rects[0], ["drawRect", 71, 200 - 32 * DISCIPLE_SHIELD_RADIUS_TILES - 9, 58, 6], "HP backing sits above the timer");
  if (expectedWidth > 0) {
    assert.deepEqual(rects[1], ["drawRect", 72, 200 - 32 * DISCIPLE_SHIELD_RADIUS_TILES - 8, expectedWidth, 4]);
    assert(g.calls.some(call => call[0] === "beginFill" && call[1] === 0x6eeaff));
  } else {
    assert.equal(rects.length, 3, "depleted HP has no fill");
  }
  assert.deepEqual(rects.at(-1), ["drawRect", 72, 200 - 32 * DISCIPLE_SHIELD_RADIUS_TILES - 2, 28, 4], "damage does not affect the timer");
}

// A damaged Master shield uses its cast-time maxima for both public bars.
{
  const g = new RecordingGraphics();
  _drawAbilityObjects.call({ _abilityObjectGfx: g, _renderClock: { now: () => 0 } }, {
    abilityObjects: [{ id: 1, kind: "shield", x: 100, y: 200,
      expiresIn: 150, ownerState: { hp: 150, maxHp: 300, durationTicks: 300 } }],
  });
  const fills = callsNamed(g, "drawRect").filter(call => call[4] === 4);
  assert.equal(fills.length, 2);
  assert(fills.every(call => call[3] === 28), "both Master bars show half remaining");
}

console.log("native_graphics_contracts: ok");

// Enemy snapshots intentionally omit private ownerState; every public effect must match.
{
  const object = { id: 9, kind: "magicAnchor", x: 150, y: 170, expiresIn: 120 };
  const owner = new RecordingGraphics();
  const enemy = new RecordingGraphics();
  const radius = 32 * EKAT_MAGIC_ANCHOR_RADIUS_TILES;
  _drawAbilityObjects.call({ _abilityObjectGfx: owner }, {
    abilityObjects: [{ ...object, ownerState: { radius } }],
  });
  _drawAbilityObjects.call({ _abilityObjectGfx: enemy }, { abilityObjects: [object] });
  assert.deepEqual(enemy.calls, owner.calls, "enemy anchor circles and particles match owner geometry");
  assert(enemy.calls.some(call => call[0] === "drawCircle" && call[3] === radius));
}
