import assert from "node:assert/strict";
import fs from "node:fs";
import { PNG } from "pngjs";
import { KIND, STATE } from "../../client/src/protocol.js";
import { createLiveFrameStrips, liveFrameStripFor, liveFrameStripKeyForEntity } from "../../client/src/renderer/rigs/frame_strip_routing.js";
import { LIGHTNESS_PNG_FRAME_STRIP, MAGIC_PNG_FRAME_STRIP, WAR_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/disciple_png_strips.js";
import { WARRIOR_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/warrior_png_strip.js";
import { frameStripFrameIndex, frameStripVisualFacing, frameStripSpriteOffset } from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { liveUnitIconMarkupFor } from "../../client/src/renderer/rigs/unit_icon_sources.js";
import { createMinimapAnimation } from "../../client/src/renderer/rigs/minimap_animation.js";
import { circularBodyRadius, pointHitsBodyCircle } from "../../client/src/input/placement.js";
import { selectionProxyForEntity } from "../../client/src/input/selection_projection.js";
import { selectionBudgetGridModel } from "../../client/src/hud_selection_panel.js";

import { entityAttackRangeTiles, DAO_WAR_RANGE_TILES, DAO_WAR_BODY_SCALE, entityBodyRadius } from "../../client/src/config.js";
assert.equal(entityAttackRangeTiles({ kind: KIND.WARRIOR, dao: "war" }), DAO_WAR_RANGE_TILES);
assert.equal(DAO_WAR_RANGE_TILES, 1.75);
assert.equal(entityBodyRadius({ kind: KIND.WARRIOR, dao: "war" }), 13.5 * DAO_WAR_BODY_SCALE);
assert.equal(entityBodyRadius({ kind: KIND.WARRIOR }), 13.5);
const bodySubject = { id: 75, kind: KIND.WARRIOR, dao: "war", x: 100, y: 100 };
assert.equal(circularBodyRadius(bodySubject), 13.5 * DAO_WAR_BODY_SCALE);
assert.equal(pointHitsBodyCircle(bodySubject, 110, 100, 0), false);
assert.equal(pointHitsBodyCircle({ ...bodySubject, dao: undefined }, 110, 100, 0), true);
assert.equal(selectionProxyForEntity(bodySubject).footprint.radiusPx, 13.5 * DAO_WAR_BODY_SCALE);
const rustUpgrades = fs.readFileSync(new URL("../../server/crates/rules/src/balance/upgrades.rs", import.meta.url), "utf8");
for (const [name, value] of [["DAO_WAR_BODY_SCALE", DAO_WAR_BODY_SCALE], ["DAO_WAR_RANGE_TILES", DAO_WAR_RANGE_TILES]]) {
  assert.equal(Number(rustUpgrades.match(new RegExp(`pub const ${name}: f32 = ([0-9.]+);`))[1]), value);
}

assert.ok(Math.abs(WAR_PNG_FRAME_STRIP.worldScale - 0.56) < 1e-8);
assert.equal(entityAttackRangeTiles({ kind: KIND.WARRIOR }), 0.75);
const strips = createLiveFrameStrips();
for (const dao of [undefined]) {
  const entity = { kind: KIND.WARRIOR, dao };
  assert.equal(liveFrameStripFor(strips, liveFrameStripKeyForEntity(entity)), WARRIOR_PNG_FRAME_STRIP);
  assert.match(liveUnitIconMarkupFor(entity.kind, { dao }), /warrior-runtime-strip/);
}
const minimap = createMinimapAnimation();
for (const [dao, strip, weaponKind] of [
  ["lightness", LIGHTNESS_PNG_FRAME_STRIP, "warrior_sword"],
  ["magic", MAGIC_PNG_FRAME_STRIP, "disciple_orb"],
  ["war", WAR_PNG_FRAME_STRIP, "warrior_sword"],
]) {
  const entity = { id: 90, kind: KIND.WARRIOR, dao, state: STATE.MOVE,
    x: 10, y: 10, facing: 0.3, weaponFacing: 1.2 };
  assert.equal(liveFrameStripFor(strips, liveFrameStripKeyForEntity(entity)), strip);
  const png = PNG.sync.read(fs.readFileSync(new URL(`../../${strip.source.runtimeStrip}`, import.meta.url)));
  assert.equal(png.width, strip.frameWidth * strip.frameCount);
  assert.equal(png.height, strip.frameHeight);
  const context = { now: 600, frameStripMoving: true };
  const gait = frameStripFrameIndex(strip, entity, context);
  assert.ok(strip.movementFrames.includes(gait));
  assert.equal(frameStripVisualFacing(strip, entity, context, gait), entity.facing - Math.PI / 2);
  // Attack state alone never invents shots; a different weapon's recoil cannot
  // accidentally trigger an archer or caster sequence after evolution.
  assert.equal(frameStripFrameIndex(strip, { ...entity, state: STATE.ATTACK }, { now: 600 }), 0);
  assert.equal(frameStripFrameIndex(strip, entity, { ...context, recoilProgress: 1, recoilPhase: 0,
    recoilWeaponKind: "disciple_arrow" }), gait);
  for (const [index, phase] of [0, 0.25, 0.5, 0.75].entries()) {
    const shot = { ...context, recoilWeaponKind: weaponKind, recoilProgress: 0.9,
      recoilPhase: phase * (strip.firingFrameHoldPhase || 1) };
    const frame = frameStripFrameIndex(strip, entity, shot);
    assert.equal(frame, strip.firingFrames[Math.min(index, strip.firingFrames.length - 1)]);
    assert.equal(frameStripVisualFacing(strip, entity, shot, frame), entity.weaponFacing + strip.firingFacingOffset);
  }
  assert.equal(frameStripFrameIndex(strip, entity, { ...context, recoilProgress: 0,
    recoilPhase: 1, recoilWeaponKind: weaponKind }), gait);
  assert.equal(frameStripFrameIndex(strip, { ...entity, state: STATE.IDLE }, context), 0);
  assert.match(liveUnitIconMarkupFor(entity.kind, { dao }), new RegExp(dao === "lightness" ? "lightness-melee-b-runtime-strip" : dao === "war" ? "war-yari-runtime-strip" : "magic-runtime-strip"));
  assert.match(liveUnitIconMarkupFor(entity.kind, { minimapPart: `dao-${dao}:frame:2` }),
    new RegExp(dao === "lightness" ? "lightness-melee-b-runtime-strip" : dao === "war" ? "war-yari-runtime-strip" : "magic-runtime-strip"));
  minimap.beginFrame([entity], {}, 600);
  assert.match(minimap.layers(entity)[0].part, new RegExp(`^dao-${dao}:frame:`));
  assert.ok(minimap.parts(KIND.WARRIOR).includes(`dao-${dao}:frame:${strip.frameCount - 1}`));
  assert.equal(selectionBudgetGridModel([entity]).blocks[0].dao, dao);
}
// The step is presentation-only, bounded, and returns exactly to the authoritative pivot.
const warEntity = { id: 91, kind: KIND.WARRIOR, dao: "war", x: 100, y: 100, facing: Math.PI / 2 };
const unchanged = structuredClone(warEntity);
for (const [phase, expected] of [[0, 0], [0.2, -2.88], [0.5, 16], [0.7, 16], [1, 0]]) {
  const context = { recoilProgress: 1, recoilPhase: phase * WAR_PNG_FRAME_STRIP.firingFrameHoldPhase,
    recoilWeaponKind: "warrior_sword" };
  const frame = frameStripFrameIndex(WAR_PNG_FRAME_STRIP, warEntity, context);
  const offset = frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, frame, context);
  assert.ok(Math.abs(offset.y - expected * DAO_WAR_BODY_SCALE) < 1e-8);
}
assert.equal(frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, 5).y, 0);
assert.equal(frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, 0, { recoilProgress: 1, recoilPhase: 0.5 }).y, 0);
assert.deepEqual(warEntity, unchanged);
minimap.destroy();
console.log("disciple_dao_sprite_contracts: ok");
