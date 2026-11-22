import assert from "node:assert/strict";
import { WaterLayer, shoreDistances } from "../../client/src/renderer/water.js";
import { installFakePixi } from "./pixi_fakes.mjs";

const map = { width: 5, height: 5, tileSize: 32, terrain: Array.from({ length: 25 }, (_, i) =>
  i % 5 === 0 || i % 5 === 4 || i < 5 || i >= 20 ? 0 : 2) };
const field = shoreDistances(map);
const sample = (x, y) => field.pixels[(y * field.width + x) * 4];
assert.equal(sample(0, 0), 0, "land has zero shore distance");
assert(sample(20, 20) > sample(8, 20), "interior water is deeper than the shoreline");
assert.equal(shoreDistances({ width: 1, height: 1, tileSize: 32, terrain: [2] }).pixels[0], 255,
  "water without a land boundary saturates at the depth range");

const restorePixi = installFakePixi();
const priorFetch = globalThis.fetch;
const priorBitmap = globalThis.createImageBitmap;
const response = { ok: true, text: async () => "shader", blob: async () => ({}) };
globalThis.createImageBitmap = async () => ({ width: 16, height: 16, close() {} });
try {
  let requests = 0;
  globalThis.fetch = async () => { requests += 1; return response; };
  const empty = new WaterLayer(PIXI, new PIXI.Container(), { ...map, terrain: new Array(25).fill(0) });
  assert.equal(await empty.ready, true);
  assert.equal(requests, 0, "maps without water do not load water assets");
  empty.destroy();

  const parent = new PIXI.Container();
  const water = new WaterLayer(PIXI, parent, map);
  water.update(1234);
  assert.equal(await water.ready, true);
  assert.equal(water.uniforms.uniforms.uTime, 1.234, "water consumes the submitted visual clock");
  assert.equal(parent.children.length, 1);
  assert.equal(water.textures.length, 7);
  water.destroy();
  assert.equal(parent.children.length, 0);
  assert(water.geometry.destroyed && water.shader.destroyed && water.texture.destroyed);
  assert(water.textures.every((texture) => texture.destroyed), "teardown releases every water texture");

  const pending = [];
  globalThis.fetch = () => new Promise((resolve) => pending.push(resolve));
  const late = new WaterLayer(PIXI, parent, map);
  late.destroy();
  for (const resolve of pending) resolve(response);
  assert.equal(await late.ready, false);
  assert.equal(parent.children.length, 0, "late assets never remount a destroyed water layer");

  globalThis.fetch = async () => ({ ...response, ok: false, status: 404 });
  const failed = new WaterLayer(PIXI, parent, map);
  await assert.rejects(failed.ready, /unavailable|failed to fetch/,
    "missing water assets reject readiness instead of silently passing capture checks");
  failed.destroy();
} finally {
  globalThis.fetch = priorFetch;
  if (priorBitmap === undefined) delete globalThis.createImageBitmap;
  else globalThis.createImageBitmap = priorBitmap;
  restorePixi();
}
console.log("Water rendering contracts passed");
