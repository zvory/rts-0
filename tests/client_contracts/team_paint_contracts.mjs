import assert from 'node:assert/strict';
import fs from 'node:fs';
import { PNG } from 'pngjs';
import { composeTeamPaintRgba, loadTeamPaintTexture, teamPaintTextureFor, destroyTeamPaintTextures } from '../../client/src/renderer/rigs/team_paint.js';
import { createLiveFrameStrips } from '../../client/src/renderer/rigs/frame_strip_routing.js';
import { liveUnitIconMarkupFor } from '../../client/src/renderer/rigs/unit_icon_sources.js';
import { renderFrameStripUnit } from '../../client/src/renderer/rigs/frame_strip_runtime.js';
import { installFakePixi } from './pixi_fakes.mjs';

const fixed = new Uint8ClampedArray([180, 120, 80, 255, 0, 0, 0, 0, 30, 40, 50, 90]);
const paint = new Uint8ClampedArray([0, 0, 0, 0, 210, 210, 210, 255, 180, 180, 180, 80]);
const red = composeTeamPaintRgba(fixed, paint, 0xff0000), blue = composeTeamPaintRgba(fixed, paint, 0x0000ff);
assert.deepEqual([...red.slice(0, 4)], [...blue.slice(0, 4)], 'fixed skin/weapon materials never inherit player color');
assert.deepEqual([...red.slice(4, 8)], [210, 0, 0, 255]);
assert.deepEqual([...blue.slice(4, 8)], [0, 0, 210, 255]);
assert.equal(red[11], blue[11], 'soft paint coverage retains alpha independently of hue');
assert.equal(red[11], Math.round((80 / 255 + 90 / 255 * (1 - 80 / 255)) * 255));
assert.throws(() => composeTeamPaintRgba(fixed, paint.slice(0, 4), 0xff0000));

for (const strip of createLiveFrameStrips().values()) {
  if (!strip.teamPaint) continue;
  const png = PNG.sync.read(fs.readFileSync(new URL('../../' + strip.source.runtimeStrip, import.meta.url)));
  const oldPath = strip.source.runtimeStrip.replace('cultivator-team-color', 'cultivator-golden');
  const original = PNG.sync.read(fs.readFileSync(new URL('../../' + oldPath, import.meta.url)));
  const w = png.width, h = strip.frameHeight, rowBytes = w * h * 4;
  assert.equal(png.height, h * strip.sheetRows);
  assert.deepEqual(png.data.subarray(0, rowBytes * 2), original.data, 'original poses and gold effects remain byte-identical');
  const base = png.data.subarray(rowBytes * 2, rowBytes * 3), mask = png.data.subarray(rowBytes * 3);
  const composite = composeTeamPaintRgba(base, mask, 0x268ad1);
  for (let frame = 0; frame < strip.frameCount; frame++) {
    let visible = 0, colored = 0, unchanged = 0;
    for (let y = 0; y < h; y++) for (let x = frame * strip.frameWidth; x < (frame + 1) * strip.frameWidth; x++) {
      const i = (y * w + x) * 4;
      if (composite[i + 3] > 100) visible++;
      if (mask[i + 3] > 100) colored++;
      if (base[i + 3] > 100 && mask[i + 3] === 0) unchanged++;
      assert.equal(mask[i], mask[i + 1]); assert.equal(mask[i], mask[i + 2]);
    }
    assert.ok(visible > 100 && colored > 20 && unchanged > 20, `complete fixed and paint layers for ${strip.image} frame ${frame}`);
  }
  const icon = liveUnitIconMarkupFor(strip.unit, { daoMaster: /master/.test(strip.image) });
  // Direct type routing is checked below for every faction fighter.
  if (icon) assert.ok(icon.includes('<svg'));
}
for (const kind of ['warrior', 'wind_adept', 'war_adept', 'magic_adept']) {
  for (const daoMaster of [false, true]) {
    const icon = liveUnitIconMarkupFor(kind, { daoMaster });
    assert.match(icon, /data-unit-icon-component="fixed"/);
    assert.match(icon, /data-unit-icon-component="team-paint"/);
    assert.match(icon, /feColorMatrix/, 'HUD and minimap tint only the paint layer, preserving edge alpha');
  }
}

// Exercise worker-only decoding, palette reuse/color changes, neutral glow source and teardown.
const saved = Object.fromEntries(['fetch', 'createImageBitmap', 'OffscreenCanvas'].map(key => [key, globalThis[key]]));
const pixels = new Uint8ClampedArray([100, 100, 100, 255, 255, 210, 0, 128, 180, 120, 80, 255, 200, 200, 200, 128]);
let bitmapClosed = false;
class Canvas {
  constructor(width, height) { this.width = width; this.height = height; this.data = new Uint8ClampedArray(width * height * 4); }
  getContext() {
    const c = this;
    return { drawImage(image) { c.data.set(image.data); }, getImageData(x, y, w, h) { return { data: c.data.slice((y * c.width + x) * 4, (y * c.width + x) * 4 + w * h * 4) }; }, createImageData(w, h) { return { data: new Uint8ClampedArray(w * h * 4) }; }, putImageData(image) { c.data.set(image.data); } };
  }
}
try {
  globalThis.fetch = async () => ({ ok: true, blob: async () => ({}) });
  globalThis.createImageBitmap = async () => ({ width: 1, height: 4, data: pixels, close() { bitmapClosed = true; } });
  globalThis.OffscreenCanvas = Canvas;
  const created = [];
  const pixi = { Texture: { from(canvas) { const texture = { source: { resource: canvas }, destroy(source) { this.destroyed = source; } }; created.push(texture); return texture; } } };
  const original = await loadTeamPaintTexture(pixi, { image: '/asset.png', frameWidth: 1, frameHeight: 1, frameCount: 1, sheetRows: 4, teamPaint: { baseRow: 2, paintRow: 3 } }, { brightness: 100, saturation: 100, hue: 100 });
  assert.ok(bitmapClosed);
  const r = teamPaintTextureFor(original, 0xff0000), b = teamPaintTextureFor(original, 0x0000ff);
  assert.equal(teamPaintTextureFor(original, '#ff0000'), r, 'repeated entities reuse one palette texture');
  assert.notEqual(r, b, 'owner color changes select a different palette');
  assert.equal(created.length, 3, 'neutral and two shared palettes; no per-entity texture');
  assert.deepEqual([...r.source.resource.data.slice(4)], [...pixels.slice(4, 8)], 'gold attack pixels never receive player tint');
  assert.deepEqual([...original.source.resource.data.slice(0, 4)], [...pixels.slice(0, 4)], 'item glow keeps the neutral source');
  const restorePixi = installFakePixi();
  try {
    const strip = { frameWidth: 1, frameHeight: 1, frameCount: 1, teamPaint: { baseRow: 2, paintRow: 3 } };
    const renderer = { _liveRigPools: { units: new Map() }, _seen: { units: new Set() }, layers: { units: new PIXI.Container() } };
    const draw = (id, silhouetteShadow, teamColor) => renderFrameStripUnit(renderer,
      { id, kind: 'warrior', x: 0, y: 0 }, strip, original,
      { poolName: 'units', layerName: 'units', silhouetteShadow, renderContext: { teamColor } });
    const body = draw(1, false, 0xff0000);
    const shadow = draw(2, true, 0xff0000);
    assert.equal(body.texture, r);
    assert.equal(shadow.texture, body.texture, 'shadow alpha follows the authored body, rather than the original neutral pose');
    assert.equal(shadow.sprite.tint, 0x000000);
    assert.equal(body.itemGlowFrames[0].resource.source, original.source, 'item bloom retains its neutral source');
    const recolored = draw(1, false, 0x0000ff);
    assert.equal(recolored.texture, b);
    assert.equal(body._destroyed, true, 'owner color changes release the prior instance');
    for (const instance of renderer._liveRigPools.units.values()) instance.destroy();
  } finally { restorePixi(); }
  destroyTeamPaintTextures(original);
  destroyTeamPaintTextures(original);
  assert.ok(r.destroyed && b.destroyed, 'all derived GPU resources are released exactly with their match source');
} finally {
  for (const [key, value] of Object.entries(saved)) { if (value === undefined) delete globalThis[key]; else globalThis[key] = value; }
}
console.log('team_paint_contracts: ok');
