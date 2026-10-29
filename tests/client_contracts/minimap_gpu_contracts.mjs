import assert from 'node:assert/strict';
import { MinimapGpuContext } from '../../client/src/minimap_gpu_context.js';
import { minimapTexturePacket, validateMinimapImage } from '../../client/src/renderer/minimap_wire.js';
import { createMinimapAnimation } from '../../client/src/renderer/rigs/minimap_animation.js';
import { KIND } from '../../client/src/protocol.js';

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
console.log('minimap GPU contracts passed');
