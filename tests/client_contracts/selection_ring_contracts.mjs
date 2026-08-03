// Selection-ring transform contracts imported by ../client_contracts.mjs.

import { assert, assertApprox } from "./assertions.mjs";
import { KIND } from "../../client/src/protocol.js";
import { _drawSelectionAndHp } from "../../client/src/renderer/entities.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

function drawSelectedRing(entity) {
  const ringGfx = new RecordingGraphics();
  _drawSelectionAndHp.call(
    {
      _slot() {
        return ringGfx;
      },
      _ringRadius() {
        return { rx: 20, ry: 12, cy: 2 };
      },
      _hpBarSlot() {
        return {};
      },
      _hpBar() {},
    },
    entity,
    new Set([entity.id]),
    { playerId: 1 },
  );
  return ringGfx;
}

const facing = Math.PI / 3;
const tankRing = drawSelectedRing({
  id: 70, owner: 1, kind: KIND.TANK, x: 100, y: 100, facing,
});
const riflemanRing = drawSelectedRing({
  id: 71, owner: 1, kind: KIND.RIFLEMAN, x: 140, y: 100, facing,
});

assert(tankRing.rotation === facing, "vehicle selection ovals rotate with body facing");
assert(riflemanRing.rotation === 0, "infantry selection ovals remain screen-aligned");
assertApprox(tankRing.x, 100, 0.001,
  "vehicle selection rotation keeps the ring horizontally centered on the entity");
assertApprox(tankRing.y, 102, 0.001,
  "vehicle selection rotation keeps the ground-projection offset screen-aligned");
const tankEllipses = tankRing.calls.filter((call) => call[0] === "drawEllipse");
assert(
  tankEllipses.length === 2
    && tankEllipses.every((call) => call[1] === 0 && call[2] === 0),
  "vehicle selection ovals rotate around their own center",
);
