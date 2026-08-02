import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import { DoodadLayer } from "../../client/src/renderer/doodad_layer.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { _sweep } from "../../client/src/renderer/layers.js";
import { frameStripDrawPlanFor, reconcileActiveLiveRigPools } from "../../client/src/renderer/rigs/draw_plans.js";
import { liveRigRoutePlanFor } from "../../client/src/renderer/rigs/live_routing.js";
import { _drawTreeOccludedAllies } from "../../client/src/renderer/tree_unit_occlusion.js";
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
    unit(1, 1, 100, 70),
    unit(2, 2, 100, 74),
    unit(3, 3, 100, 72),
    unit(4, 1, 100, 110),
  ];
  const renderContexts = new Map(entities.map((entity) => [entity.id, { now: 1200, facing: 0.3 }]));
  const drawCalls = [];
  const revealRenderer = {
    _doodads: doodads,
    _drawUnit(entity, colorByOwner, state, options) { drawCalls.push({ entity, colorByOwner, state, options }); },
    _recordRenderDiagnostic() {},
    _recordRenderError(_label, error) { throw error; },
  };
  const state = {
    isOwnOwner(owner) { return owner === 1; },
    isAllyOwner(owner) { return owner === 2; },
  };
  const colorByOwner = new Map([[1, 0x4477aa], [2, 0x55aa77], [3, 0xaa5544]]);
  assert.equal(_drawTreeOccludedAllies.call(revealRenderer, entities, state, colorByOwner, { renderContexts }), 2,
    "only own and allied units behind overlapping canopies receive a reveal body");
  assert.deepEqual(drawCalls.map(({ entity }) => entity.id), [1, 2],
    "enemy and in-front units keep their ordinary presentation unchanged");
  for (const { options } of drawCalls) {
    assert.equal(options.alpha, 0.28, "canopy reveal redraws the actual unit body at low alpha");
    assert.equal(options.omitShadow, true, "canopy reveal does not duplicate the unit shadow");
    assert.equal(options.omitEffects, true, "canopy reveal does not duplicate transient weapon effects");
    assert.equal(options.liveRigUnit, "alliedTreeRevealRigs");
    assert.equal(options.liveRigOverlay, "alliedTreeRevealRigOverlays");
  }

  const revealPools = {
    omitShadow: true,
    omitEffects: true,
    unit: "alliedTreeReveals",
    overlay: "alliedTreeReveals",
    liveRigUnit: "alliedTreeRevealRigs",
    liveRigOverlay: "alliedTreeRevealRigOverlays",
  };
  const revealPlan = liveRigRoutePlanFor(KIND.RIFLEMAN, revealPools);
  const stripPlan = frameStripDrawPlanFor(revealPlan);
  assert.equal(revealPlan.shadowRoute, null, "the reveal route has no shadow route");
  assert.equal(stripPlan.unitRoute.poolName, "alliedTreeRevealRigs",
    "frame-strip reveals route their actual body into the isolated reveal pool");

  let staleOverlayDestroyed = 0;
  const normalInstance = instance();
  const revealInstance = instance();
  const rigRenderer = rigSweepHarness(7, normalInstance, revealInstance);
  rigRenderer._liveRigPools.alliedTreeRevealRigOverlays.set(7, {
    container: { visible: true },
    destroy() { staleOverlayDestroyed += 1; },
  });
  reconcileActiveLiveRigPools(rigRenderer, 7, ["alliedTreeRevealRigs"]);
  assert.equal(rigRenderer._liveRigPools.liveUnitRigs.get(7), normalInstance,
    "reveal reconciliation cannot destroy the normal unit body with the same id");
  assert.equal(staleOverlayDestroyed, 1, "reveal reconciliation still removes an inactive reveal overlay");
  reconcileActiveLiveRigPools(rigRenderer, 7, ["liveUnitRigs"]);
  assert.equal(rigRenderer._liveRigPools.alliedTreeRevealRigs.get(7), revealInstance,
    "normal reconciliation cannot churn a retained canopy reveal");

  rigRenderer._seen.liveUnitRigs.add(7);
  _sweep.call(rigRenderer);
  assert.equal(revealInstance.container.visible, false,
    "an unoccluded reveal is hidden while the live normal unit keeps the shared id retained");
  assert.equal(revealInstance.destroyed, false, "temporary unocclusion does not churn the reveal GPU instance");

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
  renderer._drawMissingTexture({ id: 808, x: 10, y: 83 }, "units");
  assert.equal(renderer._pools.units.get(808).zIndex, 83,
    "Graphics fallback unit bodies use the same world-Y depth key");
  let revealDestroyed = false;
  renderer._liveRigPools.alliedTreeRevealRigs.set(999, {
    destroy() { revealDestroyed = true; },
  });
  renderer.destroy();
  assert.equal(revealDestroyed, true, "renderer teardown releases retained allied canopy reveal rigs");
} finally {
  restorePixi();
}

function unit(id, owner, x, y) {
  return { id, owner, kind: KIND.RIFLEMAN, x, y, visualBounds: { widthPx: 28 } };
}

function instance() {
  return {
    container: { visible: true },
    destroyed: false,
    destroy() { this.destroyed = true; },
  };
}

function rigSweepHarness(id, normalInstance, revealInstance) {
  const liveRigPools = {
    liveUnitRigShadows: new Map(),
    liveUnitRigs: new Map([[id, normalInstance]]),
    liveUnitRigOverlays: new Map(),
    liveUnitRigEffects: new Map(),
    liveShotRevealRigShadows: new Map(),
    liveShotRevealRigs: new Map(),
    liveShotRevealRigOverlays: new Map(),
    liveShotRevealRigEffects: new Map(),
    alliedTreeRevealRigs: new Map([[id, revealInstance]]),
    alliedTreeRevealRigOverlays: new Map(),
  };
  const seen = Object.fromEntries(Object.keys(liveRigPools).map((name) => [name, new Set()]));
  return {
    _pools: {},
    _seen: seen,
    _unseen: new Map(),
    _liveRigPools: liveRigPools,
    _liveRigRoutes: Object.fromEntries(Object.keys(liveRigPools).map((poolName) => [poolName, {
      poolName,
      layerName: poolName.startsWith("allied") ? "alliedTreeReveals" : "units",
    }])),
    layers: {
      units: { removeChild() {} },
      alliedTreeReveals: { removeChild() {} },
    },
    _recordRenderDiagnostic() {},
  };
}

console.log("✅ tree_unit_depth_contracts.mjs: strict world-Y depth, friendly canopy reveals, and isolated pools passed");
