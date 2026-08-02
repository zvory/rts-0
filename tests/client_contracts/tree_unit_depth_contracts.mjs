import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import { DoodadLayer } from "../../client/src/renderer/doodad_layer.js";
import { _drawAboveFogHp } from "../../client/src/renderer/entities.js";
import { Renderer } from "../../client/src/renderer/index.js";
import {
  _drawStealthUnitOutlines,
  _drawTreeOccludedUnitOutlines,
} from "../../client/src/renderer/tree_unit_occlusion.js";
import { installFakePixi, RecordingGraphics } from "./pixi_fakes.mjs";

const restorePixi = installFakePixi();
try {
  const understory = new PIXI.Container();
  const sharedUnitCanopies = new PIXI.Container();
  const doodads = new DoodadLayer({
    pixi: PIXI,
    understoryLayer: understory,
    canopyLayer: sharedUnitCanopies,
    trackAsset(_id, promise) { return promise; },
    loadTexture(_pixi, image) { return Promise.resolve(PIXI.Texture.from(image)); },
  });
  await doodads.ready();
  doodads.replace([{ id: 90, typeId: "tree.oak", x: 100, y: 100 }]);

  const oak = doodads.instances.get(90).display;
  assert.equal(sharedUnitCanopies.sortableChildren, true, "the shared tree/unit body layer enables Pixi z sorting");
  assert.equal(oak.zIndex, 100, "tree canopy depth uses its world-Y ground contact");
  assert.equal(doodads.occludesUnit({ x: 100, y: 70 }, 7), true,
    "a southern canopy overlaps a unit whose ground contact is behind it");
  assert.equal(doodads.occludesUnit({ x: 100, y: 110 }, 7), false,
    "a northern canopy never occludes a unit whose ground contact is in front");
  assert.equal(doodads.occludesUnit({ x: 260, y: 70 }, 7), false,
    "the canopy spatial index rejects distant units without a full doodad scan");

  const entities = [
    unit(1, 1, KIND.RIFLEMAN, 100, 70),
    unit(2, 2, KIND.RIFLEMAN, 100, 74),
    { ...unit(3, 3, KIND.RIFLEMAN, 100, 72), visionOnly: true, hp: 20, maxHp: 45 },
    unit(4, 1, KIND.RIFLEMAN, 100, 110),
    unit(5, 3, KIND.TANK, 101, 71, 36),
  ];
  const outlineCalls = [];
  const outlineRenderer = {
    _doodads: doodads,
    _slot(pool, id) {
      const graphics = new RecordingGraphics();
      outlineCalls.push({ pool, id, graphics });
      return graphics;
    },
    _recordRenderDiagnostic() {},
    _recordRenderError(_label, error) { throw error; },
  };
  assert.equal(
    _drawTreeOccludedUnitOutlines.call(outlineRenderer, entities),
    3,
    "ordinary friendly, allied, and visible enemy units behind a canopy receive stable outlines",
  );
  assert.deepEqual(outlineCalls.map((call) => call.id), [1, 2, 5],
    "in-front units remain unchanged and reveal-only units use their dedicated pass");
  for (const call of outlineCalls) {
    assert.equal(call.pool, "forestUnitOutlines");
    assert(call.graphics.calls.some(([kind, width, color]) =>
      kind === "lineStyle" && width === 2.25 && color === 0xffffff),
    "forest readability is a white outline with no filtered duplicate art");
  }
  outlineCalls.length = 0;
  assert.equal(
    _drawTreeOccludedUnitOutlines.call(outlineRenderer, entities.filter((entity) => entity.owner !== 3)),
    2,
    "the pass cannot outline an enemy omitted by authoritative visibility filtering",
  );
  outlineCalls.length = 0;
  assert.equal(_drawStealthUnitOutlines.call(outlineRenderer, entities), 1,
    "a reveal-only stealth unit always receives an outline even without canopy geometry");
  assert.deepEqual(outlineCalls.map(({ pool, id }) => [pool, id]), [["stealthUnitOutlines", 3]],
    "stealth reveals use the above-fog outline layer instead of a full unit rig");

  const hpCalls = [];
  _drawAboveFogHp.call({
    _hpBarSlot(id, pool) { hpCalls.push(["slot", id, pool]); return {}; },
    _hpBar(_graphics, entity) { hpCalls.push(["bar", entity.id]); },
  }, entities[2]);
  assert.deepEqual(hpCalls, [["slot", 3, "aboveFogHpBars"], ["bar", 3]],
    "damaged stealth reveals keep their HP bar above fog");

  doodads.destroy();

  const parent = {
    clientWidth: 640,
    clientHeight: 480,
    appendChild(view) { view.parentNode = this; },
    removeChild(view) { view.parentNode = null; },
  };
  const renderer = await Renderer.create(parent);
  assert.equal(renderer.layers.units.sortableChildren, true,
    "the production renderer enables strict sorting on the shared unit/canopy layer");
  assert.equal(renderer._doodads.canopyLayer, renderer.layers.units,
    "the production renderer sends tree canopies into the unit body depth layer");
  assert(renderer.layers.forestUnitOutlines,
    "the production renderer keeps forest outlines above canopies in a dedicated layer");
  assert(renderer.layers.stealthUnitOutlines,
    "the production renderer keeps reveal-only outlines above fog in a dedicated layer");
  assert.equal(renderer.layers.forestUnitOutlines.filters, undefined,
    "outline rendering does not allocate a rectangular GPU filter surface");
  renderer._drawMissingTexture({ id: 808, x: 10, y: 83 }, "units");
  assert.equal(renderer._pools.units.get(808).zIndex, 83,
    "Graphics fallback unit bodies use the same world-Y depth key");
  renderer.destroy();
} finally {
  restorePixi();
}

function unit(id, owner, kind, x, y, widthPx = 28) {
  return { id, owner, kind, x, y, facing: 0, visualBounds: { widthPx } };
}

console.log("✅ tree_unit_depth_contracts.mjs: strict depth, stealth outlines, and reveal HP passed");
