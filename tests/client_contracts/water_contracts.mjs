import assert from "node:assert/strict";
import { WaterLayer, shoreDistances, waterSunTint } from "../../client/src/renderer/water.js";
import { installFakePixi } from "./pixi_fakes.mjs";
import { Renderer } from "../../client/src/renderer/index.js";
import { buildStaticMap, updateStaticTerrainTiles } from "../../client/src/renderer/terrain.js";
import { MapEditorWorkerRenderer } from "../../client/src/renderer/map_editor_worker_renderer.js";
import { createMapEditorPresentation } from "../../client/src/map_editor_presentation.js";

const map = { width: 5, height: 5, tileSize: 32, terrain: Array.from({ length: 25 }, (_, i) =>
  i % 5 === 0 || i % 5 === 4 || i < 5 || i >= 20 ? 0 : 2) };
const field = shoreDistances(map);
const sample = (x, y) => field.pixels[(y * field.width + x) * 4];
assert.equal(sample(0, 0), 0, "land has zero shore distance");
assert(sample(20, 20) > sample(8, 20), "interior water is deeper than the shoreline");
assert.equal(shoreDistances({ width: 1, height: 1, tileSize: 32, terrain: [2] }).pixels[0], 255,
  "water without a land boundary saturates at the depth range");

assert.deepEqual([...waterSunTint(0xfff3e2)], [1, 1, 1],
  "Classic's review lighting preserves the selected option C colors exactly");

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

  const renderer = {
    world: new PIXI.Container(),
    layers: { terrain: new PIXI.Container(), feedback: new PIXI.Container() },
    app: { render() {} },
    _renderFrameCount: 0,
    visualNow: () => 6000,
    present: Renderer.prototype.present,
    updateStaticDoodadVisibility() {},
  };
  buildStaticMap.call(renderer, { ...map, sun: { warmth: 100 } });
  const initialWater = renderer._waterLayer;
  await initialWater.ready;
  const tint = initialWater.uniforms.uniforms.uTint.value;
  assert.deepEqual([...tint], [...new Float32Array([1, 208 / 243, 138 / 226])],
    "water follows map warmth relative to the approved Classic palette");
  const initialRequests = requests;
  updateStaticTerrainTiles.call(renderer, [{ x: 0, y: 0, code: 1 }, { x: 1, y: 1, code: 2 }]);
  assert.equal(renderer._waterLayer, initialWater, "land edits and unchanged water reuse the loaded layer");
  assert.equal(requests, initialRequests, "land edits do not reload water assets");
  renderer.present();
  assert.equal(initialWater.uniforms.uniforms.uTime, 6, "match presentations use the renderer visual clock");
  const editor = new MapEditorWorkerRenderer(renderer);
  editor.present(createMapEditorPresentation({
    frameId: 1, visualTimeMs: 2400, camera: { x: 0, y: 0, zoom: 1 },
  }));
  assert.equal(initialWater.uniforms.uniforms.uTime, 2.4, "editor presentations animate using their submitted clock");
  updateStaticTerrainTiles.call(renderer, [{ x: 1, y: 1, code: 0 }]);
  assert(initialWater.destroyed, "removing water releases the old layer");
  assert.notEqual(renderer._waterLayer, initialWater, "shoreline edits rebuild the water mesh and distance field");
  await renderer._waterLayer.ready;
  updateStaticTerrainTiles.call(renderer, [{ x: 0, y: 0, code: 2 }]);
  await renderer._waterLayer.ready;
  assert.equal(renderer._waterLayer.geometry.options.attributes.aPosition.buffer.length, 9 * 12,
    "adding water restores a tile's mesh geometry");
  renderer._waterLayer.destroy();
  renderer._terrainSprite.destroy(true);

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
