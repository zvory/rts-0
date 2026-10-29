import assert from 'node:assert/strict';
import { MinimapGpuContext } from '../../client/src/minimap_gpu_context.js';
import { minimapTexturePacket, validateMinimapImage } from '../../client/src/renderer/minimap_wire.js';
import { createMinimapAnimation } from '../../client/src/renderer/rigs/minimap_animation.js';
import { KIND } from '../../client/src/protocol.js';
import { MinimapGpuAssets } from '../../client/src/minimap_gpu_assets.js';
import { MinimapGpuBridge } from '../../client/src/minimap_gpu_bridge.js';
import { MinimapGpuTextures } from '../../client/src/renderer/minimap_gpu_textures.js';
import { Minimap } from '../../client/src/minimap.js';

const image = { id: 1, revision: 0, width: 1, height: 1, fullWidth: 1, fullHeight: 1,
  offsetX: 0, offsetY: 0, pixels: new Uint8Array([1, 2, 3, 255]) };
const frame = { generation: 2, minimap: { size: 640, images: [image] } };
const wire = {};
// Producing/superseding a scene does not consume the host's dispatched texture revisions.
const superseded = { ...frame, minimap: { ...frame.minimap, images: [{ ...image, revision: 1 }] } };
assert.equal(wire.minimapImages, undefined);
const first = minimapTexturePacket(frame, wire, 'minimapTextures', 1);
assert.notEqual(first.message.payload.images[0].pixels, image.pixels);
const delivered = structuredClone(first.message, { transfer: first.transfer });
assert.equal(first.transfer[0].byteLength, 0);
assert.equal(image.pixels.byteLength, 4);
assert.deepEqual([...delivered.payload.images[0].pixels], [1, 2, 3, 255]);
assert.equal(minimapTexturePacket(frame, wire, 'minimapTextures', 1), null);
assert.equal(minimapTexturePacket(superseded, wire, 'minimapTextures', 1).message.payload.images[0].revision, 1);
assert.ok(minimapTexturePacket({ ...frame, minimap: { ...frame.minimap, size: 800 } }, wire, 'minimapTextures', 1));
assert.throws(() => validateMinimapImage({ ...image, pixels: new Uint8Array(3) }), /Invalid minimap/);
assert.throws(() => validateMinimapImage({ ...image, width: NaN }), /Invalid minimap/);

const ctx = new MinimapGpuContext({ image: () => 9 }, () => {});
ctx.translate(10, 20); ctx.save(); ctx.rotate(Math.PI / 2); ctx.scale(2, 3);
ctx.globalAlpha = 0.5; ctx.fillStyle = '#abcdef'; ctx.fillRect(0, 1, 2, 3);
ctx.restore(); ctx.drawImage({ width: 4, height: 5 }, 6, 7);
assert.deepEqual(ctx.commands.map(c => c.type), ['shape', 'image']);
assert.equal(ctx.commands[0].alpha, 0.5);
assert.ok(Math.abs(ctx.commands[0].matrix[1] - 2) < 1e-10);
assert.deepEqual(ctx.commands[1].matrix, [1, 0, 0, 1, 10, 20]);
assert.deepEqual(ctx.commands[1].rect, [6, 7, 4, 5]);
assert.equal(ctx.commands[1].alpha, 1);
assert.throws(() => ctx.drawImage({}, 0, 0, 1), /Unsupported/);
ctx.reset(); assert.equal(ctx.commands.length, 0); assert.deepEqual(ctx.matrix, [1, 0, 0, 1, 0, 0]);

const animation = createMinimapAnimation({ simpleTank: true });
const tank = { id: 1, kind: KIND.TANK, facing: 0.3, weaponFacing: 1.2 };
assert.deepEqual(animation.parts(KIND.TANK), ['hull', 'turret']);
animation.beginFrame([tank], { weaponRecoil: () => 0 }, 100);
assert.deepEqual(animation.layers(tank).map(l => l.rotation), [0.3, 1.2]);
assert.ok(animation.layers(tank)[1].x === 0);
animation.beginFrame([tank], { weaponRecoil: () => 1 }, 101);
assert.ok(animation.layers(tank).every(l => l.x < 0 && l.y < 0));
assert.ok(Math.abs(animation.layers(tank)[1].x) > Math.abs(animation.layers(tank)[0].x));
animation.destroy();

// A cleared/resized source must dispatch new pixels even without a draw call.
const assets = new MinimapGpuAssets();
const rasterContext = Object.fromEntries(['clearRect', 'fillRect', 'strokeRect', 'drawImage', 'fill', 'stroke', 'putImageData']
  .map(name => [name, () => {}]));
rasterContext.getImageData = (x, y, width, height) => ({ width, height, data: new Uint8ClampedArray(width * height * 4) });
const raster = { width: 1, height: 1, getContext: () => rasterContext };
const rasterId = assets.image(raster);
const originalImage = assets.records.get(rasterId);
raster.width = 2;
assets.image(raster);
const resizedImage = assets.records.get(rasterId);
assert.ok(resizedImage.revision > originalImage.revision);
assert.equal(resizedImage.width, 2);

// Resize the retained texture itself, rather than clipping new pixels to its old canvas.
const priorCanvas = globalThis.OffscreenCanvas, priorImageData = globalThis.ImageData;
try {
  globalThis.OffscreenCanvas = class {
    constructor(width, height) { this.width = width; this.height = height; }
    getContext() { return { putImageData() {} }; }
  };
  globalThis.ImageData = class {};
  const textures = new MinimapGpuTextures({ Texture: { from(canvas) {
    return { canvas, source: { style: {}, update() {} }, destroy() { this.destroyed = true; } };
  } } });
  textures.update({ images: [originalImage], size: 480 });
  const originalTexture = textures.get(rasterId).texture;
  textures.update({ images: [resizedImage], size: 480 });
  assert.equal(originalTexture.destroyed, true);
  assert.equal(textures.get(rasterId).texture.canvas.width, 2);
  assert.equal(textures.get(rasterId).fullWidth, 2);
  textures.clear();
} finally {
  if (priorCanvas === undefined) delete globalThis.OffscreenCanvas; else globalThis.OffscreenCanvas = priorCanvas;
  if (priorImageData === undefined) delete globalThis.ImageData; else globalThis.ImageData = priorImageData;
}
assets.destroy();

// Only foreground buildings contribute to the cached outline. Visibility transitions and
// map-transform changes must invalidate it even if ids and world positions are unchanged.
const outlineBridge = { buildingOutlineChanged: MinimapGpuBridge.prototype.buildingOutlineChanged };
let paints = 0;
const outlineHost = {
  size: 480, _scale: 1, _offX: 0, _offY: 0, _gpuBridge: outlineBridge,
  _isForegroundPlayerMinimapEntity: Minimap.prototype._isForegroundPlayerMinimapEntity,
  _isPlayerOwnedMinimapEntity: Minimap.prototype._isPlayerOwnedMinimapEntity,
  _ensurePlayerBlipMaskLayer: () => ({ canvas: {}, ctx: { clearRect() {} } }),
  _drawEntityBlip: () => { paints++; }, _presentationScale: () => 1,
  ctx: { save() {}, restore() {}, drawImage() {} },
};
const buildings = [{ id: 1, kind: KIND.BARRACKS, owner: 1, x: 20, y: 20 },
  { id: 2, kind: KIND.BARRACKS, owner: 2, x: 40, y: 40 }];
const outline = () => Minimap.prototype._drawPlayerOwnedEntityOutline.call(outlineHost, buildings);
outline(); assert.equal(paints, 2);
outline(); assert.equal(paints, 2);
buildings[1].visionOnly = true;
outline(); assert.equal(paints, 3);
outlineHost._scale = 2;
outline(); assert.equal(paints, 4);
console.log('minimap GPU contracts passed');
