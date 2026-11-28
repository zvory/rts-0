import assert from "node:assert/strict";
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
    for (let phase = 0; phase < 4; phase++) {
      const frame = 16 + direction * 4 + phase;
      const context = { now: phase * 100, frameStripMoving: true, teamColor: 0x0072b2 };
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
    for (let phase = 0; phase < 4; phase++) {
      assert.equal(frameStripFrameIndex(strip, entity, { frameStripMoving: true, recoilProgress: 1,
        recoilPhase: phase / 4, recoilWeaponKind: "rifleman_rifle" }), 80 + direction * 4 + phase,
      "a real rifle shot follows weapon bearing and overrides locomotion");
    }
    assert.equal(frameStripFrameIndex(strip, entity, { frameStripMoving: false }), direction);
  }
}
for (const instance of renderHarness._liveRigPools.units.values()) instance.destroy();
const oldProxy = selectionProxyForEntity(entity);
entity.weaponFacing = 0;
assert.deepEqual(experimentalRiflemanMuzzle(entity, 0.5), { x: entity.x + 12, y: entity.y - 15 },
  "presentation muzzle offsets follow the body's scale");
const newProxy = selectionProxyForEntity(entity, 32, null, true);
assert.deepEqual(newProxy.footprint, oldProxy.footprint, "marquee and collision presentation stay grounded");
assert.equal(newProxy.directClickFootprint.kind, "polygon");
assert.ok(newProxy.directClickFootprint.points.some(point => point.y < entity.y - 35), "upright torso is clickable");
assert.equal(selectionProxyForEntity({ ...entity, visionOnly: true }, 32, null, true), null);

const read = name => PNG.sync.read(fs.readFileSync(new URL(`../../client/assets/rigs/rifleman-experimental/${name}-atlas.png`, import.meta.url)));
const [neutral, base, paint] = ["neutral", "fixed", "paint"].map(read);
assert.equal(neutral.width, 2048); assert.equal(neutral.height, 1152);
assert.equal(base.width, neutral.width); assert.equal(paint.height, neutral.height);
const white = composeTeamPaintRgba(base.data, paint.data, 0xffffff);
const blue = composeTeamPaintRgba(base.data, paint.data, 0x0072b2);
const orange = composeTeamPaintRgba(base.data, paint.data, 0xd55e00);
const counts = Array.from({ length: 144 }, () => ({ visible: 0, colored: 0, fixed: 0 }));
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
  if (y % 128 >= 96) assert.equal(paint.data[i + 3], 0, "boot contact pixels stay fixed-color");
}
for (const [frame, count] of counts.entries()) {
  assert.ok(count.visible > 300 && count.colored > 40 && count.fixed > 40, `complete fixed/paint split in frame ${frame}`);
}
console.log("experimental_graphics_contracts: default-off persistence, live routing, 16-direction run/fire, hit areas and material masks passed");
