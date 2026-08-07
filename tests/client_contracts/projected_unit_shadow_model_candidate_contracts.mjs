import assert from "node:assert/strict";
import {
  PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES,
  PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET,
} from "../../client/src/renderer/projected_unit_shadow_model_candidates.js";

const expectedKinds = [
  "worker",
  "panzerfaust",
  "anti_tank_gun",
  "mortar_team",
  "artillery",
  "command_car",
  "ekat",
  "golem",
];

assert.deepEqual(PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES.map((entry) => entry.kind), expectedKinds);

for (const unit of PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES) {
  assert.equal(unit.candidates.length, 2, `${unit.kind} has exactly two review candidates`);
  assert(unit.spriteEnvelope.length > 0 && unit.spriteEnvelope.width > 0);
  for (const candidate of unit.candidates) {
    assert(candidate.parts.length > 0);
    assert(
      candidate.parts.length <= PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET,
      `${candidate.id} stays within the ten-shape budget`,
    );
    for (const part of candidate.parts) {
      assert.equal(part.center.length, 3);
      assert.equal(part.size.length, 3);
      assert(part.center.every(Number.isFinite));
      assert(part.size.every((value) => Number.isFinite(value) && value > 0));
      assert(Number.isFinite(part.yaw));
    }
    const bounds = footprintBounds(candidate.parts);
    assert(bounds.length >= unit.spriteEnvelope.length * 0.5);
    assert(bounds.length <= unit.spriteEnvelope.length * 1.2);
    assert(bounds.width >= unit.spriteEnvelope.width * 0.5);
    assert(bounds.width <= unit.spriteEnvelope.width * 1.2);
  }
}

assert(!PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES.some((entry) => entry.kind === "scout_plane"));
console.log("projected unit shadow model candidate contracts passed");

function footprintBounds(parts) {
  const xs = [];
  const ys = [];
  for (const part of parts) {
    const [cx, cy] = part.center;
    const [length, width] = part.size;
    const cos = Math.cos(part.yaw);
    const sin = Math.sin(part.yaw);
    for (const dx of [-length / 2, length / 2]) for (const dy of [-width / 2, width / 2]) {
      xs.push(cx + dx * cos - dy * sin);
      ys.push(cy + dx * sin + dy * cos);
    }
  }
  return {
    length: Math.max(...xs) - Math.min(...xs),
    width: Math.max(...ys) - Math.min(...ys),
  };
}
