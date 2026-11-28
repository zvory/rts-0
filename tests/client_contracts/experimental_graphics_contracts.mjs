import { RecordingGraphics } from "./pixi_fakes.mjs";
import { assertApprox } from "./assertions.mjs";
import { _drawMuzzleFlashes } from "../../client/src/renderer/muzzle_feedback.js";
import assert from "node:assert/strict";
import { applyMatchExperimentalGraphics } from "../../client/src/match_settings_toggles.js";
import { VisualEffectBackedState, VisualEffectBuffers } from "../../client/src/state_visual_effects.js";
import { EVENT } from "../../client/src/protocol.js";
import fs from "node:fs";
import { PNG } from "pngjs";
import { readExperimentalGraphicsEnabled, writeExperimentalGraphicsEnabled } from "../../client/src/experimental_graphics_settings.js";
import { frameStripFrameIndex, frameStripVisualFacing, renderFrameStripUnit } from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { RIFLEMAN_EXPERIMENTAL_FRAME_STRIP as strip } from "../../client/src/renderer/rigs/rifleman_experimental_png_strip.js";
import { experimentalRiflemanMuzzle } from "../../client/src/renderer/rigs/rifleman_experimental_png_strip.js";
import { composeTeamPaintRgba } from "../../client/src/renderer/rigs/team_paint.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { selectionProxyForEntity } from "../../client/src/input/selection_projection.js";
import { buildSettingsTabs } from "../../client/src/settings_panels.js";
import { memoryStorage, withFakeSettingsDocument, findFakeById } from "./fakes.mjs";
import { createInspectionPngPixiFactory, FakeContainer } from "../helpers/rig_inspection_pixi.mjs";

{
  const attacker = { id: 1, kind: "rifleman", x: 100, y: 200, facing: 0, weaponFacing: 0 };
  const state = {
    entityById: id => id === attacker.id ? attacker : null,
    liveMuzzleFlashes: () => [{ from: attacker.id, weaponKind: "rifleman_rifle", createdAt: 1000 }],
  };
  for (const experimental of [false, true]) {
    const graphics = new RecordingGraphics();
    _drawMuzzleFlashes.call({
      visualNow: () => 1000,
      _feedbackGfx: graphics,
      _experimentalRiflemanFrameStrip: () => experimental ? {} : null,
      _unitRenderContexts: new Map([[attacker.id, { visualScale: 0.5 }]]),
    }, state);
    const flashes = graphics.calls.filter(call => call[0] === "drawCircle");
    assert(flashes.length === (experimental ? 2 : 0),
      "experimental art needs rendered muzzle flashes; normal Rifleman art supplies its own");
    for (const flash of flashes) {
      assertApprox(flash[1], experimentalRiflemanMuzzle(attacker, 0.5).x, 0.001, "experimental flash follows the scaled muzzle x");
      assertApprox(flash[2], experimentalRiflemanMuzzle(attacker, 0.5).y, 0.001, "experimental flash follows the scaled muzzle height");
    }
  }
}

const storage = memoryStorage();
storage.removeItem = key => storage.data.delete(key);
assert.equal(readExperimentalGraphicsEnabled(storage), false);
writeExperimentalGraphicsEnabled(true, storage);
assert.equal(readExperimentalGraphicsEnabled(storage), true);
writeExperimentalGraphicsEnabled(false, storage);
assert.equal(readExperimentalGraphicsEnabled(storage), false);
assert.equal(readExperimentalGraphicsEnabled({ getItem() { throw Error("unavailable"); } }), false);
assert.doesNotThrow(() => writeExperimentalGraphicsEnabled(true, { setItem() { throw Error("unavailable"); } }));
withFakeSettingsDocument(() => {
  let enabled = false;
  const [tab] = buildSettingsTabs({ game: { experimentalGraphics: {
    state: () => ({ enabled }), onToggle: () => { enabled = !enabled; },
  } } }).filter(tab => tab.id === "game");
  const root = document.createElement("div"); tab.render(root, {});
  const checkbox = findFakeById(root, "experimental-graphics-toggle");
  assert.equal(checkbox.type, "checkbox"); assert.equal(checkbox.checked, false);
  checkbox.listeners.change(); assert.equal(enabled, true); assert.equal(checkbox.checked, true);
});

// Disabled means no experimental image load; a late load cannot switch an opted-out match.
let loads = 0, available = null;
const rendererState = {
  _visualFrameStripTextureFor() { loads++; return available; },
  _experimentalRiflemanFrameStrip: Renderer.prototype._experimentalRiflemanFrameStrip,
};
assert.equal(rendererState._experimentalRiflemanFrameStrip(), null);
assert.equal(loads, 0);
Renderer.prototype.setExperimentalGraphicsEnabled.call(rendererState, true);
assert.equal(loads, 1); assert.equal(rendererState._experimentalRiflemanFrameStrip(), null);
Renderer.prototype.setExperimentalGraphicsEnabled.call(rendererState, false);
available = { source: {} };
assert.equal(rendererState._experimentalRiflemanFrameStrip(), null);
Renderer.prototype.setExperimentalGraphicsEnabled.call(rendererState, true);
assert.equal(rendererState._experimentalRiflemanFrameStrip().texture, available);
Renderer.prototype.setExperimentalGraphicsEnabled.call(rendererState, false);
assert.equal(rendererState._experimentalRiflemanFrameStrip(), null);

const entity = { id: 1, kind: "rifleman", state: "move", x: 100, y: 200, facing: 0, hp: 45, maxHp: 45 };
const renderHarness = { _liveRigPools: { units: new Map() }, _seen: { units: new Set() },
  layers: { units: new FakeContainer() }, _rigPixiFactory: createInspectionPngPixiFactory() };
for (let direction = 0; direction < 16; direction++) {
  const angle = Math.PI / 2 + direction * Math.PI / 8;
  for (const turns of [-2, 0, 2]) {
    entity.facing = angle + turns * Math.PI * 2;
    entity.weaponFacing = entity.facing + Math.PI;
    for (let phase = 0; phase < 6; phase++) {
      const frame = direction * 16 + 2 + phase;
      const context = { now: phase * 90, frameStripMoving: true, teamColor: 0x0072b2 };
      assert.equal(frameStripFrameIndex(strip, entity, context), frame);
      assert.equal(frameStripVisualFacing(strip, entity, context), 0);
      const instance = renderFrameStripUnit(renderHarness, entity, strip, available, {
        poolName: "units", layerName: "units", renderContext: context,
      });
      assert.equal(instance.container.x, entity.x); assert.equal(instance.container.y, entity.y);
      assert.equal(instance.sprite.anchorY, 112 / 128);
      assert.equal(instance.sprite.texture.frame.x, frame % 16 * 128);
      assert.equal(instance.sprite.texture.frame.y, Math.floor(frame / 16) * 128);
    }
    entity.weaponFacing = angle;
    const animationState = { strip };
    // First shot articulates settle/ready/raise/shoulder/aim, then recoil/bolt/re-aim.
    for (const [now,pose] of [[0,8],[60,1],[100,9],[200,10],[300,11],[360,12],[500,13],[650,14],[800,11]]) {
      const context={now,frameStripMoving:false,recoilProgress:now<420?1:0,
        recoilPhase:now/420,recoilWeaponKind:"rifleman_rifle"};
      assert.equal(frameStripFrameIndex(strip, entity, context, animationState),direction*16+pose);
    }
    assert.equal(frameStripFrameIndex(strip, entity, {now:1530,frameStripMoving:false}, animationState),direction*16+15);
    assert.equal(frameStripFrameIndex(strip, entity, {now:1700,frameStripMoving:false}, animationState),direction*16);
    assert.equal(frameStripFrameIndex(strip, entity, {frameStripMoving:false}),direction*16);
  }
}
for (const instance of renderHarness._liveRigPools.units.values()) instance.destroy();
const oldProxy = selectionProxyForEntity(entity);
entity.weaponFacing = 0;
const eastMuzzle=strip.poseGeometry.muzzleByDirection[12];
assert.deepEqual(experimentalRiflemanMuzzle(entity, 0.5), { x: entity.x + eastMuzzle.x * .45 * .5, y: entity.y + eastMuzzle.y * .45 * .5 },
  "presentation muzzle offsets follow the body's scale");
const newProxy = selectionProxyForEntity(entity, 32, null, true);
assert.deepEqual(newProxy.footprint, oldProxy.footprint, "marquee and collision presentation stay grounded");
assert.equal(newProxy.directClickFootprint.kind, "polygon");
assert.ok(newProxy.directClickFootprint.points.some(point => point.y < entity.y - 35), "upright torso is clickable");
assert.equal(selectionProxyForEntity({ ...entity, visionOnly: true }, 32, null, true), null);

const read = name => PNG.sync.read(fs.readFileSync(new URL(`../../client/assets/rigs/rifleman-experimental-v3/${name}-atlas.png`, import.meta.url)));
const [neutral, base, paint] = ["neutral", "fixed", "paint"].map(read);
assert.equal(neutral.width, 2048); assert.equal(neutral.height, 2048);
assert.equal(base.width, neutral.width); assert.equal(paint.height, neutral.height);
const white = composeTeamPaintRgba(base.data, paint.data, 0xffffff);
const blue = composeTeamPaintRgba(base.data, paint.data, 0x0072b2);
const orange = composeTeamPaintRgba(base.data, paint.data, 0xd55e00);
const counts = Array.from({ length: 256 }, () => ({ visible: 0, colored: 0, fixed: 0 }));
for (let y = 0; y < neutral.height; y++) for (let x = 0; x < neutral.width; x++) {
  const i = (y * neutral.width + x) * 4, frame = Math.floor(y / 128) * 16 + Math.floor(x / 128);
  if (neutral.data[i + 3] < 20) continue;
  counts[frame].visible++;
  assert.ok(Math.abs(white[i + 3] - neutral.data[i + 3]) <= 1, "mask compositing preserves alpha coverage");
  for (let c = 0; c < 3; c++) assert.ok(Math.abs(white[i + c] - neutral.data[i + c]) <= 1,
    "white palette reconstructs approved source colors");
  assert.equal(blue[i + 3], orange[i + 3]);
  if (paint.data[i + 3] === 0) {
    counts[frame].fixed++;
    assert.deepEqual(blue.slice(i, i + 4), orange.slice(i, i + 4), "untinted materials ignore team color");
  } else if (Math.abs(blue[i] - orange[i]) > 20) counts[frame].colored++;
  const bounds=strip.poseGeometry.frameBounds[frame];
  if (y % 128 >= bounds.y+bounds.height-2) for (let c=0;c<3;c++) {
    assert.ok(Math.abs(blue[i+c]-orange[i+c])*blue[i+3]/255<=2, "boot sole color stays fixed within two composited PNG levels");
  }
}
for (const [frame, count] of counts.entries()) {
  assert.ok(count.visible > 300 && count.colored > 40 && count.fixed > 40, `complete fixed/paint split in frame ${frame}`);
}
console.log("experimental_graphics_contracts: default-off persistence, live routing, 16-direction run/fire, hit areas and material masks passed");

// Presentation delay is scoped to opt-in Rifleman shots; other weapons and simulation
// event times retain their existing behavior with either setting.
for (const enabled of [false,true]) {
 const effects=new VisualEffectBuffers();
 effects.applySnapshotEvents([
  {e:EVENT.ATTACK,from:1,to:2,weaponKind:"rifleman_rifle"},
  {e:EVENT.ATTACK,from:3,to:2,weaponKind:"machine_gunner_mg"},
 ],1000,()=>null,enabled);
 assert.equal(effects.muzzleFlashes[0].createdAt,enabled?1350:1000);
 assert.equal(effects.muzzleFlashes[1].createdAt,1000);
 assert.equal(effects.weaponRecoilById.get(1).startedAt,1000);
}
{
 const state=new VisualEffectBackedState();state.entityById=()=>null;
 const match={state};applyMatchExperimentalGraphics(match,true);
 state.applySnapshotVisualEvents([{e:EVENT.ATTACK,from:1,to:2,weaponKind:"rifleman_rifle"}],1000);
 assert.equal(state.muzzleFlashes[0].createdAt,1350);
 applyMatchExperimentalGraphics(match,false);
 assert.equal(state.muzzleFlashes[0].createdAt,1000,"opting out restores pending shot timing immediately");
 state.applySnapshotVisualEvents([{e:EVENT.ATTACK,from:1,to:2,weaponKind:"rifleman_rifle"}],2000);
 assert.equal(state.muzzleFlashes[1].createdAt,2000);
}
await import("../../scripts/check-rifleman-sixteen.mjs");
