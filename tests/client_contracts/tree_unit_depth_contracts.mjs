import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import { DoodadLayer } from "../../client/src/renderer/doodad_layer.js";
import { createForestOutlineFilter } from "../../client/src/renderer/forest_outline_filter.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { liveRigRoutePlanFor } from "../../client/src/renderer/rigs/live_routing.js";
import { _drawTreeOccludedUnitOutlines } from "../../client/src/renderer/tree_unit_occlusion.js";
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
  const outlineCalls = [];
  const outlineRenderer = {
    _doodads: doodads,
    _drawUnit(entity, _colors, _state, pools) { outlineCalls.push({ entity, pools }); },
    _recordRenderDiagnostic() {},
    _recordRenderError(_label, error) { throw error; },
  };
  assert.equal(
    _drawTreeOccludedUnitOutlines.call(outlineRenderer, entities, {}, new Map(), { renderContexts }),
    4,
    "every already-visible friendly, allied, and enemy unit behind a canopy enters the outline surface",
  );
  assert.deepEqual(outlineCalls.map((call) => call.entity.id), [1, 2, 3, 5],
    "in-front units remain unchanged while visible enemies are treated like friendly units");
  for (const call of outlineCalls) {
    assert.equal(call.pools.alpha, undefined, "outline source retains complete merged alpha for the shader");
    assert.equal(call.pools.omitShadow, true, "forest outline does not duplicate the unit shadow");
    assert.equal(call.pools.omitEffects, true, "forest outline does not duplicate weapon effects");
    assert.equal(call.pools.liveRigUnit, "forestUnitOutlineRigs", "unit parts share the filtered outline surface");
    assert.equal(call.pools.liveRigOverlay, "forestUnitOutlineRigOverlays", "rig overlays use the filtered surface");
  }
  outlineCalls.length = 0;
  assert.equal(
    _drawTreeOccludedUnitOutlines.call(
      outlineRenderer,
      entities.filter((entity) => entity.owner !== 3),
      {},
      new Map(),
      { renderContexts },
    ),
    2,
    "the pass cannot outline an enemy omitted by authoritative visibility filtering",
  );

  const filter = createForestOutlineFilter(PIXI);
  const fragment = filter.options.glProgram.options.fragment;
  assert(fragment.includes("neighborAlpha - centerAlpha"),
    "the post-process subtracts merged center alpha and emits only the outer edge");
  assert(fragment.includes("vec4(vec3(outlineAlpha), outlineAlpha)"),
    "the shader emits premultiplied white without filling transparent filter pixels");
  assert.equal(filter.padding, 3, "the filter surface leaves room for the expanded silhouette");
  assert.equal(filter.resources.forestOutlineUniforms.uniforms.uThickness.value, 1.65,
    "the outline uses a compact screen-space sampling radius");
  filter.destroy();

  const outlinePools = {
    omitShadow: true,
    omitEffects: true,
    unit: "forestUnitOutlines",
    overlay: "forestUnitOutlines",
    liveRigUnit: "forestUnitOutlineRigs",
    liveRigOverlay: "forestUnitOutlineRigOverlays",
  };
  const firstOutlinePlan = liveRigRoutePlanFor(KIND.TANK, outlinePools);
  const secondOutlinePlan = liveRigRoutePlanFor(KIND.TANK, { ...outlinePools });
  assert.equal(firstOutlinePlan, secondOutlinePlan,
    "forest outline rig routes reuse one cached immutable plan across occluded units and frames");
  assert.deepEqual(firstOutlinePlan.poolNames, [
    "forestUnitOutlineRigs",
  ], "the cached forest route omits both shadow and weapon-effect pools");
  assert.equal(
    liveRigRoutePlanFor(KIND.TANK, { omitShadow: true }).routes.some((route) => route.parts.includes("part.shadow")),
    false,
    "omit flags cannot accidentally reuse the normal cached route profile",
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
  assert(renderer.layers.forestUnitOutlines,
    "the production renderer keeps filtered forest outlines above canopies in a dedicated layer");
  assert.deepEqual(renderer.layers.forestUnitOutlines.filters, [renderer._forestOutlineFilter],
    "one layer-level filter merges all routed rig parts before deriving the outline");
  renderer._drawMissingTexture({ id: 808, x: 10, y: 83 }, "units");
  assert.equal(renderer._pools.units.get(808).zIndex, 83,
    "Graphics fallback unit bodies use the same world-Y depth key");
  const retainedFilter = renderer._forestOutlineFilter;
  renderer.destroy();
  assert.equal(retainedFilter.destroyed, true, "renderer teardown releases the forest outline filter");
} finally {
  restorePixi();
}

function unit(id, owner, kind, x, y, widthPx = 28) {
  return { id, owner, kind, x, y, facing: 0, visualBounds: { widthPx } };
}

console.log("✅ tree_unit_depth_contracts.mjs: strict world-Y depth and GPU-merged forest outlines passed");
