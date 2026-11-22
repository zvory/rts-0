import assert from "node:assert/strict";
import { drawDiscipleShield } from "../../client/src/renderer/disciple_shield_effect.js";

function capture(hp, now = 0) {
  const operations = [];
  const g = new Proxy({}, { get: (_, name) => (...args) => {
    operations.push([name, ...args]);
    return g;
  } });
  drawDiscipleShield(g, { id: 7, x: 200, y: 220, ownerState: hp === undefined ? null : { hp } }, 96, now);
  return operations;
}
const rects = operations => operations.filter(([name]) => name === "rect");
assert.equal(rects(capture(undefined)).length, 0, "views without team-private HP must not invent a bar");
const full = rects(capture(300));
const half = rects(capture(150));
assert.equal(full.length, 2, "full shields show their pool immediately");
assert.equal(half[1][3], full[1][3] / 2, "half the absorption pool fills half the bar");
assert.deepEqual(rects(capture(900)), full, "over-capacity values cannot overflow the bar");
assert.equal(rects(capture(-1)).length, 1, "empty pools keep only the background");
assert.deepEqual(capture(150, 1000), capture(150, 1000), "fixed visual clocks reproduce the same shimmer");
assert.notDeepEqual(capture(150, 1000), capture(150, 1500), "the shimmer rotates as visual time advances");
console.log("disciple_shield_contracts: ok");
