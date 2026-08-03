import assert from "node:assert/strict";

import { KIND } from "../../client/src/protocol.js";
import { DoodadLayer } from "../../client/src/renderer/doodad_layer.js";
import { _drawAboveFogHp } from "../../client/src/renderer/entities.js";
import { Renderer } from "../../client/src/renderer/index.js";
import {
  _drawStealthUnitOutlines,
  _drawTreeOccludedUnitOutlines,
} from "../../client/src/renderer/tree_unit_occlusion.js";
import {
  createUnitOutlineFilter,
  FOREST_UNIT_FILL_ALPHA,
} from "../../client/src/renderer/unit_outline_filter.js";
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
    { ...unit(3, 3, KIND.RIFLEMAN, 100, 72), visionOnly: true, hp: 20, maxHp: 45 },
    unit(4, 1, KIND.RIFLEMAN, 100, 110),
    unit(5, 3, KIND.TANK, 101, 71, 36),
  ];
  const outlineCalls = [];
  const teamFillCalls = [];
  const state = { playerId: 1 };
  const colorByOwner = new Map([[1, 0x0072b2], [2, 0xd55e00], [3, 0xcc79a7]]);
  const renderContexts = new Map(entities.map((entity) => [entity.id, {
    now: 1000 + entity.id,
    teamColor: colorByOwner.get(entity.owner),
  }]));
  const rifleFrameStripOverride = { strip: { id: "real-rifle-strip" }, texture: { id: "real-rifle-texture" } };
  const visualFrameStripOverrides = new Map([[KIND.RIFLEMAN, rifleFrameStripOverride]]);
  const outlineRenderer = {
    _doodads: doodads,
    _drawUnit(entity, colors, renderState, pools) {
      outlineCalls.push({ entity, colors, renderState, pools });
    },
    _attachForestUnitOutline(entity, colors) {
      teamFillCalls.push([entity.id, colors.get(entity.owner)]);
    },
    _recordRenderDiagnostic() {},
    _recordRenderError(_label, error) { throw error; },
  };
  assert.equal(
    _drawTreeOccludedUnitOutlines.call(outlineRenderer, entities, state, colorByOwner, {
      renderContexts,
      visualFrameStripOverrides,
    }),
    3,
    "ordinary friendly, allied, and visible enemy units behind a canopy use filtered real-rig outlines",
  );
  assert.deepEqual(outlineCalls.map((call) => call.entity.id), [1, 2, 5],
    "in-front units remain unchanged and reveal-only units use their dedicated pass");
  assert.deepEqual(teamFillCalls, [[1, 0x0072b2], [2, 0xd55e00], [5, 0xcc79a7]],
    "forest silhouettes receive each visible unit's actual owner color");
  for (const call of outlineCalls) {
    assert.equal(call.colors, colorByOwner);
    assert.equal(call.renderState, state);
    assert.equal(call.pools.unit, "forestUnitOutlines");
    assert.equal(call.pools.liveRigUnit, "forestUnitOutlineRigs");
    assert.equal(call.pools.liveRigOverlay, "forestUnitOutlineRigOverlays");
    assert.equal(call.pools.omitShadow, true);
    assert.equal(call.pools.omitEffects, true);
    assert.notEqual(call.pools.renderContext, renderContexts.get(call.entity.id),
      "the outline pass clones rather than mutates the live unit render context");
    assert.deepEqual(call.pools.renderContext, renderContexts.get(call.entity.id));
    if (call.entity.kind === KIND.RIFLEMAN) {
      assert.equal(call.pools.visualFrameStrip, rifleFrameStripOverride,
        "the outline pass reuses the same production frame-strip override as the visible unit");
    }
  }
  outlineCalls.length = 0;
  teamFillCalls.length = 0;
  assert.equal(
    _drawTreeOccludedUnitOutlines.call(
      outlineRenderer,
      entities.filter((entity) => entity.owner !== 3),
      state,
      colorByOwner,
      { renderContexts, visualFrameStripOverrides },
    ),
    2,
    "the pass cannot outline an enemy omitted by authoritative visibility filtering",
  );
  outlineCalls.length = 0;
  assert.equal(_drawStealthUnitOutlines.call(
    outlineRenderer,
    entities,
    state,
    colorByOwner,
    { visualFrameStripOverrides },
  ), 1,
    "a reveal-only stealth unit always receives an outline even without canopy geometry");
  assert.equal(outlineCalls[0].entity.id, 3);
  assert.equal(outlineCalls[0].pools.unit, "stealthUnitOutlines");
  assert.equal(outlineCalls[0].pools.liveRigUnit, "stealthUnitOutlineRigs");
  assert.equal(outlineCalls[0].pools.liveRigOverlay, "stealthUnitOutlineRigOverlays");
  assert.equal(outlineCalls[0].pools.visualFrameStrip, rifleFrameStripOverride,
    "stealth reveals reuse the real animated rifleman frame rather than proxy geometry");
  assert.equal("renderContext" in outlineCalls[0].pools, false,
    "a reveal-only unit builds its normal production render context in the outline pass");
  assert.deepEqual(teamFillCalls, [[1, 0x0072b2], [2, 0xd55e00]],
    "stealth reveals do not add a team fill after the preceding forest pass");

  const filter = createUnitOutlineFilter(PIXI, {
    fillColor: 0x0072b2,
    fillAlpha: FOREST_UNIT_FILL_ALPHA,
  });
  const fragment = filter.options.glProgram.options.fragment;
  assert.match(fragment, /texture\(uTexture, vTextureCoord\)\.a/,
    "the filter reads the rendered unit's real alpha");
  assert.match(fragment, /neighborAlpha - centerAlpha/,
    "the filter emits only the silhouette's outer edge");
  assert.doesNotMatch(fragment, /texture\([^;]+\)\.rgb/,
    "the filter never copies faction-colored pixels from the duplicated rig");
  assert.match(fragment, /uFillColor \* fillAlpha/,
    "the fill is a flat owner color masked by the production rig alpha");
  assert.match(fragment, /out vec4 finalColor;/,
    "the WebGL2 shader declares an explicit fragment output");
  assert.doesNotMatch(fragment, /\b(?:texture2D|gl_FragColor)\b/,
    "the Pixi v8 WebGL2 shader does not use legacy GLSL output or sampling syntax");
  assert.equal(filter.padding, 3, "the filter surface leaves room for the expanded silhouette");
  assert.equal(filter.resources.outlineUniforms.uniforms.uThickness.value, 1.65,
    "the outline uses the configured compact sampling radius through a Pixi uniform group");
  assert.equal(filter.resources.outlineUniforms.uniforms.uFillAlpha.value, 0.85,
    "forest silhouettes use the selected 85% fill opacity");
  const fillChannels = Array.from(filter.resources.outlineUniforms.uniforms.uFillColor.value);
  assert(fillChannels.every((value, index) => (
    Math.abs(value - [0, 114 / 255, 178 / 255][index]) < 1e-6
  )), "the shader receives normalized owner-color channels");
  filter.destroy();
  const whiteOnlyFilter = createUnitOutlineFilter(PIXI);
  assert.equal(whiteOnlyFilter.resources.outlineUniforms.uniforms.uFillAlpha.value, 0,
    "the default filter remains white-edge-only for authoritative stealth reveals");
  whiteOnlyFilter.destroy();

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
  assert.equal(renderer.layers.forestUnitOutlines.filters ?? null, null,
    "forest outlines avoid a shared filter because owner colors vary per unit");
  assert.equal(renderer.layers.stealthUnitOutlines.filters.length, 1,
    "stealth reveals filter the actual production rig alpha above fog");
  assert(renderer._liveRigPools.forestUnitOutlineRigs instanceof Map);
  assert(renderer._liveRigPools.stealthUnitOutlineRigs instanceof Map);
  renderer._drawMissingTexture({ id: 808, x: 10, y: 83 }, "units");
  assert.equal(renderer._pools.units.get(808).zIndex, 83,
    "Graphics fallback unit bodies use the same world-Y depth key");
  const forestBody = new PIXI.Container();
  const forestOverlay = new PIXI.Container();
  renderer.layers.forestUnitOutlines.addChild(forestBody);
  renderer.layers.forestUnitOutlines.addChild(forestOverlay);
  renderer._liveRigPools.forestUnitOutlineRigs.set(501, {
    container: forestBody,
    destroy() { this.container.parent?.removeChild?.(this.container); },
  });
  renderer._liveRigPools.forestUnitOutlineRigOverlays.set(501, {
    container: forestOverlay,
    destroy() { this.container.parent?.removeChild?.(this.container); },
  });
  const forestEntry = renderer._attachForestUnitOutline(
    unit(501, 2, KIND.RIFLEMAN, 100, 74),
    colorByOwner,
  );
  assert.deepEqual(forestEntry.group.children, [forestBody, forestOverlay],
    "body and overlay are filtered together so internal rig seams do not gain white edges");
  assert.equal(forestEntry.group.zIndex, 74,
    "per-unit outline groups retain world-Y ordering");
  assert.equal(
    forestEntry.filter.resources.outlineUniforms.uniforms.uFillAlpha.value,
    FOREST_UNIT_FILL_ALPHA,
    "the production forest group uses the selected fill opacity",
  );
  assert.equal(
    renderer._drawTreeOccludedUnitOutlines([], {}, colorByOwner),
    0,
    "a frame without occluded units draws no forest silhouettes",
  );
  assert.equal(forestEntry.group.visible, false,
    "retained per-unit filter groups are inactive when their unit is no longer occluded");
  renderer._attachForestUnitOutline(unit(501, 2, KIND.RIFLEMAN, 100, 74), colorByOwner);
  assert.equal(forestEntry.group.visible, true,
    "drawing a forest silhouette reactivates its retained filter group");
  const forestFilter = forestEntry.filter;
  const stealthFilter = renderer._stealthUnitOutlineFilter;
  renderer.destroy();
  assert.equal(forestFilter.destroyed, true, "per-unit forest outline filters are released on teardown");
  assert.equal(stealthFilter.destroyed, true, "stealth outline filter is released on teardown");
} finally {
  restorePixi();
}

function unit(id, owner, kind, x, y, widthPx = 28) {
  return { id, owner, kind, x, y, facing: 0, visualBounds: { widthPx } };
}

console.log("✅ tree_unit_depth_contracts.mjs: strict depth, stealth outlines, and reveal HP passed");
