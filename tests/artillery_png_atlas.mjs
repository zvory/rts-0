#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KIND, SETUP, STATE } from "../client/src/protocol.js";
import { ARTILLERY_PNG_RIG_ATLAS } from "../client/src/renderer/rigs/artillery_png_atlas.js";
import { liveRigRoutesFor } from "../client/src/renderer/rigs/live_routing.js";
import { pngAtlasRouteCoverage } from "../client/src/renderer/rigs/png_runtime.js";
import { createLivePngRigAtlases } from "../client/src/renderer/rigs/png_routing.js";
import { compileSvgRig } from "../client/src/renderer/rigs/svg_importer.js";
import { ARTILLERY_RIG_SVG } from "../client/src/renderer/rigs/support_svg.js";
import { fakeAtlasTexture, makeRigRenderer } from "./helpers/rig_renderer_harness.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

assert.equal(createLivePngRigAtlases().get(KIND.ARTILLERY), ARTILLERY_PNG_RIG_ATLAS);
assert.match(ARTILLERY_PNG_RIG_ATLAS.image, /artillery-a19-pass-03/);
const result = compileSvgRig(ARTILLERY_RIG_SVG, { expectedKind: KIND.ARTILLERY });
assert.equal(result.ok, true, JSON.stringify(result.errors));
const definition = result.definition;
const [, unitRoute] = liveRigRoutesFor(KIND.ARTILLERY);
assert.deepEqual(
  pngAtlasRouteCoverage(definition, ARTILLERY_PNG_RIG_ATLAS, unitRoute).missingParts,
  ["part.art.flashCone", "part.art.flashCore", "part.art.flashGlow"],
);

const spriteById = new Map(ARTILLERY_PNG_RIG_ATLAS.sprites.map((sprite) => [sprite.id, sprite]));
const deployedLeftTrail = spriteById.get("sprite.art.leftTrail.deployed");
const deployedRightTrail = spriteById.get("sprite.art.rightTrail.deployed");
const deployedCarriage = spriteById.get("sprite.art.carriage.deployed");
const deployedBarrel = spriteById.get("sprite.art.barrelAssembly.deployed");
for (const sprite of [deployedLeftTrail, deployedRightTrail, deployedCarriage, deployedBarrel]) {
  assert.ok(sprite);
}
assert.deepEqual(
  [deployedLeftTrail.frame, deployedRightTrail.frame].map(({ x, y, w, h }) => [x, y, w, h]),
  [[110, 190, 573, 228], [110, 574, 573, 228]],
);
const atlasImageSize = readPngDimensions(`client${ARTILLERY_PNG_RIG_ATLAS.image.split("?")[0]}`);
for (const sprite of ARTILLERY_PNG_RIG_ATLAS.sprites) {
  const { frame } = sprite;
  assert.ok(frame.x >= 0 && frame.y >= 0);
  assert.ok(frame.x + frame.w <= atlasImageSize.width && frame.y + frame.h <= atlasImageSize.height);
  assert.ok(frame.originX >= 0 && frame.originX < frame.w);
  assert.ok(frame.originY >= 0 && frame.originY < frame.h);
}
assert.ok(deployedLeftTrail.sourceParts.includes("part.art.foot.left.deployed"));
assert.ok(deployedRightTrail.sourceParts.includes("part.art.foot.right.deployed"));
assert.ok(deployedBarrel.sourceParts.includes("part.art.cradle.deployed"));
assert.equal(deployedCarriage.sourceParts.includes("part.art.cradle.deployed"), false);
assert.equal(deployedLeftTrail.tintSlot, "#a05cff");
assert.equal(deployedRightTrail.tintSlot, "fixed");
assert.equal(ARTILLERY_PNG_RIG_ATLAS.grid.diagnostics.trailFrameStroke, "#000000");

const entity = {
  id: 47,
  kind: KIND.ARTILLERY,
  owner: 1,
  x: 32,
  y: 44,
  facing: 0,
  weaponFacing: 0,
  setupState: SETUP.DEPLOYED,
  state: STATE.IDLE,
};
const renderer = makeRigRenderer();
renderer._liveRigDefinitionsByKind = new Map([[KIND.ARTILLERY, definition]]);
renderer._livePngRigAtlasesByKind = new Map([[
  KIND.ARTILLERY,
  { ...ARTILLERY_PNG_RIG_ATLAS, enabled: true },
]]);
renderer._livePngRigAtlasTextures = new Map([[KIND.ARTILLERY, fakeAtlasTexture()]]);
renderer._deployedWeaponSetupVisual = () => ({ prongFactor: 1, frameProgress: 1, barrel: true });
renderer._drawUnit(entity, new Map([[1, 0x336699]]), {
  playerId: 1,
  selection: new Set(),
  weaponRecoil: () => 0,
});

const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
assert.equal(typeof unit.matchesPngAtlasRig, "function");
const leftTrail = unit.parts.get("sprite.art.leftTrail.deployed").display;
const rightTrail = unit.parts.get("sprite.art.rightTrail.deployed").display;
const carriage = unit.parts.get("sprite.art.carriage.deployed").display;
const barrel = unit.parts.get("sprite.art.barrelAssembly.deployed").display;
for (const display of [leftTrail, rightTrail, carriage, barrel]) {
  assert.equal(display.visible, true);
  assert.equal(display.alpha, 1);
}
assert.equal(leftTrail.tint, 0xa05cff);
assert.equal(rightTrail.tint, 0xffffff);
assert.ok(leftTrail.rotation > 0);
assert.ok(rightTrail.rotation < 0);
assert.ok(Math.abs(leftTrail.scaleX - 1 / 15) < 0.001);
assert.ok(Math.abs(barrel.scaleX - 1 / 10.4) < 0.001);
assert.ok(Math.abs(barrel.scaleY - 1 / 12) < 0.001);
assert.ok(Math.abs(barrel.rotation - 0.44) < 0.001);

console.log("artillery PNG atlas contract passed");

function readPngDimensions(repoRelativePath) {
  const buffer = fs.readFileSync(path.join(repoRoot, repoRelativePath));
  assert.equal(buffer.toString("hex", 0, 8), "89504e470d0a1a0a");
  assert.equal(buffer.toString("ascii", 12, 16), "IHDR");
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}
