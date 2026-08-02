import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import { DoodadLayer } from "../../client/src/renderer/doodad_layer.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { _drawTreeOccludedUnitReveals } from "../../client/src/renderer/tree_unit_occlusion.js";
import { installFakePixi } from "./pixi_fakes.mjs";

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
    unit(3, 3, KIND.RIFLEMAN, 100, 72),
    unit(4, 1, KIND.RIFLEMAN, 100, 110),
    unit(5, 3, KIND.TANK, 101, 71, 36),
  ];
  const renderContexts = new Map(entities.map((entity) => [entity.id, { facing: entity.id * 0.1 }]));
  const hatchGraphics = new Map();
  const revealCalls = [];
  const revealRenderer = {
    _doodads: doodads,
    _slot(_pool, id) {
      if (!hatchGraphics.has(id)) hatchGraphics.set(id, new PIXI.Graphics());
      return hatchGraphics.get(id);
    },
    _drawUnit(entity, _colors, _state, pools) { revealCalls.push({ entity, pools }); },
    _recordRenderDiagnostic() {},
    _recordRenderError(_label, error) { throw error; },
  };
  assert.equal(
    _drawTreeOccludedUnitReveals.call(revealRenderer, entities, {}, new Map(), { renderContexts }),
    4,
    "every already-visible friendly, allied, and enemy unit behind a canopy receives a reveal",
  );
  assert.deepEqual([...hatchGraphics.keys()], [1, 2, 3, 5],
    "in-front units remain unchanged while visible enemies are treated like friendly units");
  for (const call of revealCalls) {
    assert.equal(call.pools.alpha, 0.58, "forest unit art remains clearly visible but translucent");
    assert.equal(call.pools.omitShadow, true, "forest reveal does not duplicate the unit shadow");
    assert.equal(call.pools.omitEffects, true, "forest reveal does not duplicate weapon effects");
  }
  for (const [id, graphics] of hatchGraphics) {
    assert(graphics.calls.some((call) => call[0] === "moveTo"), "crosshatch begins bounded line paths");
    assert(graphics.calls.some((call) => call[0] === "lineTo"), "crosshatch draws clipped diagonal segments");
    assert(graphics.calls.some((call) => call[0] === "lineStyle" && call[2] === 0xffffff && call[3] === 0.18),
      "crosshatch uses a translucent white stroke");
    assert.equal(graphics.rotation, id * 0.1, "crosshatch follows the rendered unit facing");
  }
  revealCalls.length = 0;
  assert.equal(
    _drawTreeOccludedUnitReveals.call(
      revealRenderer,
      entities.filter((entity) => entity.owner !== 3),
      {},
      new Map(),
      { renderContexts },
    ),
    2,
    "the pass cannot reveal an enemy omitted by authoritative visibility filtering",
  );

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
  assert(renderer.layers.forestUnitReveals && renderer.layers.forestUnitHatches,
    "the production renderer keeps forest reveals and hatches above canopies in dedicated layers");
  renderer._drawMissingTexture({ id: 808, x: 10, y: 83 }, "units");
  assert.equal(renderer._pools.units.get(808).zIndex, 83,
    "Graphics fallback unit bodies use the same world-Y depth key");
  const retainedReveal = renderer._slot("forestUnitReveals", 999);
  const retainedHatch = renderer._slot("forestUnitHatches", 999);
  renderer.destroy();
  assert.equal(retainedReveal.destroyed, true, "renderer teardown releases retained forest reveal graphics");
  assert.equal(retainedHatch.destroyed, true, "renderer teardown releases retained forest hatch graphics");
} finally {
  restorePixi();
}

function unit(id, owner, kind, x, y, widthPx = 28) {
  return { id, owner, kind, x, y, facing: 0, visualBounds: { widthPx } };
}

console.log("✅ tree_unit_depth_contracts.mjs: strict world-Y depth and visibility-gated forest reveals passed");
