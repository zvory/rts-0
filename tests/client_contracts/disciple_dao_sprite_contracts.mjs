import assert from "node:assert/strict";
import fs from "node:fs";
import { PNG } from "pngjs";
import { KIND, STATE } from "../../client/src/protocol.js";
import { createLiveFrameStrips, liveFrameStripFor, liveFrameStripKeyForEntity } from "../../client/src/renderer/rigs/frame_strip_routing.js";
import { LIGHTNESS_PNG_FRAME_STRIP, MAGIC_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/disciple_png_strips.js";
import { WARRIOR_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/warrior_png_strip.js";
import { frameStripFrameIndex, frameStripVisualFacing } from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { liveUnitIconMarkupFor } from "../../client/src/renderer/rigs/unit_icon_sources.js";
import { createMinimapAnimation } from "../../client/src/renderer/rigs/minimap_animation.js";
import { selectionBudgetGridModel } from "../../client/src/hud_selection_panel.js";

const strips = createLiveFrameStrips();
for (const dao of [undefined, "war"]) {
  const entity = { kind: KIND.WARRIOR, dao };
  assert.equal(liveFrameStripFor(strips, liveFrameStripKeyForEntity(entity)), WARRIOR_PNG_FRAME_STRIP);
  assert.match(liveUnitIconMarkupFor(entity.kind, { dao }), /warrior-runtime-strip/);
}
const minimap = createMinimapAnimation();
for (const [dao, strip, weaponKind] of [
  ["lightness", LIGHTNESS_PNG_FRAME_STRIP, "warrior_sword"],
  ["magic", MAGIC_PNG_FRAME_STRIP, "disciple_orb"],
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
    const shot = { ...context, recoilWeaponKind: weaponKind, recoilProgress: 0.9, recoilPhase: phase };
    const frame = frameStripFrameIndex(strip, entity, shot);
    assert.equal(frame, strip.firingFrames[Math.min(index, strip.firingFrames.length - 1)]);
    assert.equal(frameStripVisualFacing(strip, entity, shot, frame), entity.weaponFacing + strip.firingFacingOffset);
  }
  assert.equal(frameStripFrameIndex(strip, entity, { ...context, recoilProgress: 0,
    recoilPhase: 1, recoilWeaponKind: weaponKind }), gait);
  assert.equal(frameStripFrameIndex(strip, { ...entity, state: STATE.IDLE }, context), 0);
  assert.match(liveUnitIconMarkupFor(entity.kind, { dao }), new RegExp(dao === "lightness" ? "lightness-melee-b-runtime-strip" : "magic-runtime-strip"));
  assert.match(liveUnitIconMarkupFor(entity.kind, { minimapPart: `dao-${dao}:frame:2` }),
    new RegExp(dao === "lightness" ? "lightness-melee-b-runtime-strip" : "magic-runtime-strip"));
  minimap.beginFrame([entity], {}, 600);
  assert.match(minimap.layers(entity)[0].part, new RegExp(`^dao-${dao}:frame:`));
  assert.ok(minimap.parts(KIND.WARRIOR).includes(`dao-${dao}:frame:${strip.frameCount - 1}`));
  assert.equal(selectionBudgetGridModel([entity]).blocks[0].dao, dao);
}
minimap.destroy();
console.log("disciple_dao_sprite_contracts: ok");
