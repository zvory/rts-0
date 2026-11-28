import assert from "node:assert/strict";
import fs from "node:fs";
import { PNG } from "pngjs";
import { KIND, STATE } from "../../client/src/protocol.js";
import { createLiveFrameStrips, liveFrameStripFor, liveFrameStripKeyForEntity } from "../../client/src/renderer/rigs/frame_strip_routing.js";
import { BOW_NOVICE_PNG_FRAME_STRIP, WIND_PNG_FRAME_STRIP, MAGIC_PNG_FRAME_STRIP, WAR_PNG_FRAME_STRIP,
  WIND_MASTER_PNG_FRAME_STRIP, MAGIC_MASTER_PNG_FRAME_STRIP, WAR_MASTER_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/disciple_png_strips.js";
import { PRESENTATION_ENTITY_FIELDS } from "../../client/src/presentation/entity_snapshot.js";
import { WARRIOR_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/warrior_png_strip.js";
import { frameStripFrameIndex, frameStripVisualFacing, frameStripSpriteOffset } from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { liveUnitIconMarkupFor } from "../../client/src/renderer/rigs/unit_icon_sources.js";
import { createMinimapAnimation } from "../../client/src/renderer/rigs/minimap_animation.js";
import { circularBodyRadius, pointHitsBodyCircle } from "../../client/src/input/placement.js";
import { buildFormationLinePreview } from "../../client/src/input/formation_line.js";
import { selectionProxyForEntity } from "../../client/src/input/selection_projection.js";
import { selectionBudgetGridModel } from "../../client/src/hud_selection_panel.js";

import { entityAttackRangeTiles, DAO_WAR_RANGE_TILES, DAO_WAR_BODY_SCALE, entityBodyRadius } from "../../client/src/config.js";
assert.equal(entityAttackRangeTiles({ kind: KIND.WAR_ADEPT, dao: "war" }), DAO_WAR_RANGE_TILES);
assert.equal(DAO_WAR_RANGE_TILES, 1.75);
assert.equal(entityBodyRadius({ kind: KIND.WAR_ADEPT, dao: "war" }), Math.fround(26 * Math.fround(DAO_WAR_BODY_SCALE)));
assert.equal(entityBodyRadius({ kind: KIND.WARRIOR }), 17);
const bodySubject = { id: 75, kind: KIND.WAR_ADEPT, dao: "war", x: 100, y: 100 };
assert.equal(circularBodyRadius(bodySubject), Math.fround(26 * Math.fround(DAO_WAR_BODY_SCALE)));
assert.equal(pointHitsBodyCircle(bodySubject, 120, 100, 0), false);
assert.equal(pointHitsBodyCircle({ ...bodySubject, kind: KIND.WARRIOR, dao: undefined }, 120, 100, 0), false);
assert.equal(selectionProxyForEntity(bodySubject).footprint.radiusPx, Math.fround(26 * Math.fround(DAO_WAR_BODY_SCALE)));
const formationPoints = [{ x: 300, y: 300 }, { x: 400, y: 300 }];
const formationUnits = Array.from({ length: 3 }, (_, index) =>
  ({ ...bodySubject, id: index + 1, y: 100 + index * 40 }));
assert.ok(buildFormationLinePreview(formationPoints, formationUnits).slots.every(slot => slot.y === 300));
assert.ok(buildFormationLinePreview(formationPoints, formationUnits.map(unit =>
  ({ ...unit, kind: KIND.WARRIOR, dao: undefined }))).slots.every(slot => slot.y === 300));
const rustUpgrades = fs.readFileSync(new URL("../../server/crates/rules/src/balance/upgrades.rs", import.meta.url), "utf8");
for (const [name, value] of [["DAO_WAR_BODY_SCALE", DAO_WAR_BODY_SCALE], ["DAO_WAR_RANGE_TILES", DAO_WAR_RANGE_TILES]]) {
  assert.equal(Number(rustUpgrades.match(new RegExp(`pub const ${name}: f32 = ([0-9.]+);`))[1]), value);
}

assert.ok(Math.abs(WAR_PNG_FRAME_STRIP.worldScale - 0.56) < 1e-8);
assert.equal(entityAttackRangeTiles({ kind: KIND.WARRIOR }), 0.75);
const strips = createLiveFrameStrips();
for (const dao of [undefined]) {
  const entity = { kind: ({ wind: KIND.WIND_ADEPT, war: KIND.WAR_ADEPT, magic: KIND.MAGIC_ADEPT })[dao] || KIND.WARRIOR, dao };
  assert.equal(liveFrameStripFor(strips, liveFrameStripKeyForEntity(entity)), WARRIOR_PNG_FRAME_STRIP);
  assert.match(liveUnitIconMarkupFor(entity.kind, { dao }), /warrior-runtime-strip/);
}
const minimap = createMinimapAnimation();
for (const [dao, strip, weaponKind] of [
  ["wind", WIND_PNG_FRAME_STRIP, "warrior_sword"],
  ["magic", MAGIC_PNG_FRAME_STRIP, "disciple_orb"],
  ["war", WAR_PNG_FRAME_STRIP, "warrior_sword"],
]) {
  const entity = { id: 90, kind: ({ wind: KIND.WIND_ADEPT, war: KIND.WAR_ADEPT, magic: KIND.MAGIC_ADEPT })[dao] || KIND.WARRIOR, dao, state: STATE.MOVE,
    x: 10, y: 10, facing: 0.3, weaponFacing: 1.2 };
  assert.equal(liveFrameStripFor(strips, liveFrameStripKeyForEntity(entity)), strip);
  const png = PNG.sync.read(fs.readFileSync(new URL(`../../${strip.source.runtimeStrip}`, import.meta.url)));
  assert.equal(png.width, strip.frameWidth * strip.frameCount);
  assert.equal(png.height, strip.frameHeight * (strip.sheetRows || (strip.attackTrail ? 2 : 1)));
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
  assert.equal(frameStripFrameIndex(strip, { ...entity, state: STATE.IDLE }, context), gait,
    "automatic pursuit walks even while the public order state is idle");
  assert.equal(frameStripFrameIndex(strip, { ...entity, state: STATE.IDLE },
    { ...context, frameStripMoving: false }), 0);
  assert.match(liveUnitIconMarkupFor(entity.kind, { dao }), new RegExp(dao === "wind" ? "wind-melee-b-runtime-strip" : dao === "war" ? "war-yari-runtime-strip" : "magic-runtime-strip"));
  assert.match(liveUnitIconMarkupFor(entity.kind, { minimapPart: `frame:2` }),
    new RegExp(dao === "wind" ? "wind-melee-b-runtime-strip" : dao === "war" ? "war-yari-runtime-strip" : "magic-runtime-strip"));
  minimap.beginFrame([entity], {}, 600);
  assert.match(minimap.layers(entity)[0].part, /^frame:/);
  assert.ok(minimap.parts(entity.kind).includes(`frame:${strip.frameCount - 1}`));
  assert.equal(selectionBudgetGridModel([entity]).blocks[0].dao, dao);
}
// Authored torso centers and shoulder edges must stay inside the same circle used
// by placement, formation spacing, and click selection, at every facing.
for (const [dao, strip, cx, cy, halfWidth, halfDepth] of [
  [undefined, WARRIOR_PNG_FRAME_STRIP, 58, 80, 48, 32],
  ["wind", WIND_PNG_FRAME_STRIP, 128, 140, 48, 40],
  ["magic", MAGIC_PNG_FRAME_STRIP, 128, 140, 64, 52],
  ["war", WAR_PNG_FRAME_STRIP, 128, 146, 32, 28],
]) {
  const subject = { ...bodySubject, kind: ({ wind: KIND.WIND_ADEPT, war: KIND.WAR_ADEPT, magic: KIND.MAGIC_ADEPT })[dao] || KIND.WARRIOR, dao };
  const offset = frameStripSpriteOffset(strip, 0);
  for (let facing = 0; facing < Math.PI * 2; facing += Math.PI / 4) {
    const angle = frameStripVisualFacing(strip, { ...subject, facing }, {}, 0);
    for (const [x, y] of [[cx, cy], [cx - halfWidth, cy], [cx + halfWidth, cy],
      [cx, cy - halfDepth], [cx, cy + halfDepth]]) {
      const lx = (x - strip.frameWidth / 2) * strip.worldScale + offset.x;
      const ly = (y - strip.frameHeight / 2) * strip.worldScale + offset.y;
      const wx = subject.x + lx * Math.cos(angle) - ly * Math.sin(angle);
      const wy = subject.y + lx * Math.sin(angle) + ly * Math.cos(angle);
      assert.ok(pointHitsBodyCircle(subject, wx, wy, 0), `${dao || "novice"} torso at ${facing}`);
      assert.ok(Math.hypot(wx - subject.x, wy - subject.y) <=
        selectionProxyForEntity(subject).footprint.radiusPx);
    }
  }
}
// The step is presentation-only, bounded, and returns exactly to the authoritative pivot.
const warEntity = { id: 91, kind: KIND.WAR_ADEPT, dao: "war", x: 100, y: 100, facing: Math.PI / 2 };
const unchanged = structuredClone(warEntity);
for (const [phase, expected] of [[0, 0], [0.2, -2.88], [0.5, 16], [0.7, 16], [1, 0]]) {
  const context = { recoilProgress: 1, recoilPhase: phase * WAR_PNG_FRAME_STRIP.firingFrameHoldPhase,
    recoilWeaponKind: "warrior_sword" };
  const frame = frameStripFrameIndex(WAR_PNG_FRAME_STRIP, warEntity, context);
  const offset = frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, frame, context);
  assert.ok(Math.abs(offset.y - (WAR_PNG_FRAME_STRIP.originLateralPx + expected * DAO_WAR_BODY_SCALE)) < 1e-8);
}
assert.equal(frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, 5).y, WAR_PNG_FRAME_STRIP.originLateralPx);
assert.equal(frameStripSpriteOffset(WAR_PNG_FRAME_STRIP, 0, { recoilProgress: 1, recoilPhase: 0.5 }).y, WAR_PNG_FRAME_STRIP.originLateralPx);
assert.deepEqual(warEntity, unchanged);
assert.ok(PRESENTATION_ENTITY_FIELDS.includes("daoMaster"), "rank survives the worker presentation handoff");
for (const [dao, master, adept] of [["wind", WIND_MASTER_PNG_FRAME_STRIP, WIND_PNG_FRAME_STRIP],
  ["war", WAR_MASTER_PNG_FRAME_STRIP, WAR_PNG_FRAME_STRIP],
  ["magic", MAGIC_MASTER_PNG_FRAME_STRIP, MAGIC_PNG_FRAME_STRIP]]) {
  const unit = { id: 123, kind: ({ wind: KIND.WIND_ADEPT, war: KIND.WAR_ADEPT, magic: KIND.MAGIC_ADEPT })[dao], dao, daoMaster: true, state: STATE.IDLE, x: 0, y: 0 };
  assert.equal(strips.get(liveFrameStripKeyForEntity(unit)), master);
  assert.equal(master.worldScale, adept.worldScale * 1.2);
  assert.equal(entityBodyRadius(unit), entityBodyRadius({ ...unit, daoMaster: false }) * 1.2);
  const png = PNG.sync.read(fs.readFileSync(new URL(`../../${master.source.runtimeStrip}`, import.meta.url)));
  assert.equal(png.width, 8 * master.frameWidth);
  assert.equal(png.height, master.frameHeight * (master.sheetRows || (master.attackTrail ? 2 : 1)));
  for (let frame = 0; frame < 8; frame++) {
    let opaque = 0;
    for (let y = 0; y < master.frameHeight; y++) for (let x = 0; x < master.frameWidth; x++) {
      const alpha = png.data[(y * png.width + frame * master.frameWidth + x) * 4 + 3];
      if (alpha) opaque++;
      if (x === 0 || x === master.frameWidth - 1 || y === 0 || y === master.frameHeight - 1) assert.equal(alpha, 0, `${dao} frame ${frame} fits its cell`);
    }
    assert.ok(opaque > 500, `${dao} frame ${frame} is not blank`);
  }
  assert.match(liveUnitIconMarkupFor(unit.kind, { dao, daoMaster: true }), new RegExp(`${dao}-master-runtime-strip`));
  assert.match(liveUnitIconMarkupFor(unit.kind, { minimapPart: `master:frame:2` }), new RegExp(`${dao}-master-runtime-strip`));
  minimap.beginFrame([unit], {}, 600);
  assert.match(minimap.layers(unit)[0].part, /^master:frame:/);
  assert.ok(minimap.parts(unit.kind).includes(`master:frame:7`));
  assert.equal(selectionBudgetGridModel([unit]).blocks[0].label, `${dao[0].toUpperCase() + dao.slice(1)} Master`);
}
minimap.destroy();
console.log("disciple_dao_sprite_contracts: ok");

// The preserved archer art is a live first-class kind, including its shot-driven frames.
assert.equal(liveFrameStripFor(strips, KIND.BOW_NOVICE), BOW_NOVICE_PNG_FRAME_STRIP);
assert.equal(entityAttackRangeTiles({ kind: KIND.BOW_NOVICE }), 5);
assert.equal(entityBodyRadius({ kind: KIND.BOW_NOVICE }), 17);
assert.match(liveUnitIconMarkupFor(KIND.BOW_NOVICE), /disciple-dao.*wind-runtime-strip/);
const bowPng = PNG.sync.read(fs.readFileSync(new URL(`../../${BOW_NOVICE_PNG_FRAME_STRIP.source.runtimeStrip}`, import.meta.url)));
assert.equal(bowPng.width, 8 * 256);
assert.equal(bowPng.height, 256);
for (const [index, phase] of [0, 0.25, 0.5, 0.75].entries()) {
  assert.equal(frameStripFrameIndex(BOW_NOVICE_PNG_FRAME_STRIP, { kind: KIND.BOW_NOVICE }, {
    now: 600, recoilProgress: 0.9, recoilPhase: phase, recoilWeaponKind: "disciple_arrow",
  }), 4 + index);
}
