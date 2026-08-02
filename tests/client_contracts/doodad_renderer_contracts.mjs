import assert from "node:assert/strict";

import { DoodadLayer } from "../../client/src/renderer/doodad_layer.js";
import {
  DOODAD_MANIFEST,
  DOODAD_TYPE_IDS,
  MAX_DOODADS,
} from "../../client/src/renderer/doodad_manifest.js";
import { LAYERS } from "../../client/src/renderer/layers.js";
import { MapEditorWorkerRenderer } from "../../client/src/renderer/map_editor_worker_renderer.js";
import { installFakePixi } from "./pixi_fakes.mjs";

const restorePixi = installFakePixi();
try {
  const understory = new PIXI.Container();
  const canopies = new PIXI.Container();
  const readiness = new Map();
  const textures = [];
  const layer = new DoodadLayer({
    pixi: PIXI,
    understoryLayer: understory,
    canopyLayer: canopies,
    trackAsset(id, promise) {
      const tracked = Promise.resolve(promise).then(
        (value) => {
          readiness.set(id, value ? "ready" : "failed");
          return value;
        },
        () => {
          readiness.set(id, "failed");
          return null;
        },
      );
      return tracked;
    },
    async loadTexture(_pixi, image) {
      const texture = PIXI.Texture.from(image);
      textures.push(texture);
      return texture;
    },
  });
  await layer.ready();
  for (const species of ["oak", "pine", "birch", "spruce", "aspen", "alder"]) {
    assert.equal(DOODAD_MANIFEST[`tree.${species}`]?.image, `/assets/doodads/tree-${species}.png`,
      `${species} uses its exact worker-safe raster asset path`);
    assert.equal(DOODAD_MANIFEST[`tree.${species}.topdown`]?.image, `/assets/doodads/tree-${species}-topdown.png`,
      `${species} top-down variant uses its exact worker-safe raster asset path`);
  }
  assert.equal(DOODAD_TYPE_IDS.filter((typeId) => typeId.startsWith("tree.")).length, 12,
    "renderer catalog exposes six tree species in side-view and top-down variants");
  assert.equal(readiness.size, DOODAD_TYPE_IDS.length, "every allowlisted doodad asset is readiness-tracked");
  assert([...readiness.values()].every((status) => status === "ready"), "doodad readiness requires every texture");

  layer.replace([
    { id: 1, typeId: "tree.oak", x: 50, y: 50 },
    { id: 2, typeId: "wildflower.single", x: 70, y: 80, color: "#D95F8D" },
    { id: 4, typeId: "tree.alder.topdown", x: 90, y: 90 },
    { id: 3, typeId: "not-a-doodad", x: 10, y: 10 },
    { id: 1, typeId: "tree.pine", x: 90, y: 90 },
  ]);
  assert.equal(layer.instances.size, 3, "replace reconciles only unique allowlisted doodads");
  assert.equal(canopies.children.length, 2, "side-view and top-down tree art share the canopy layer above unit bodies");
  assert.equal(understory.children.length, 3, "flowers and tree shadows share the private below-unit layer");
  assert.equal(layer.instances.get(2).display.tint, 0xd95f8d, "wildflower colors tint the authored white-petal asset");

  const nearCamera = { containsProjected: ({ x, y }) => x < 300 && y < 300 };
  const visible = layer.update(1000, nearCamera);
  const firstRotation = layer.instances.get(1).display.rotation;
  layer.update(1000, nearCamera);
  assert.equal(layer.instances.get(1).display.rotation, firstRotation, "sway is deterministic for stable id and visual time");
  assert.equal(visible, 3, "viewport culling retains nearby doodads");

  layer.patch({
    upserts: [{ id: 2, typeId: "wildflower.cluster", x: 1000, y: 1000, color: "#44AA66" }],
    removedIds: [1, 4],
  });
  assert.equal(layer.instances.size, 1, "patch removes and upserts without a full static reconciliation");
  assert.equal(
    layer.update(1500, nearCamera),
    0,
    "off-viewport doodads skip per-frame transform work",
  );
  assert.equal(layer.instances.get(2).display.visible, false, "culled doodad sprites are hidden");
  assert.equal(MAX_DOODADS, 4096, "renderer enforces the shared authored doodad bound");

  const unitIndex = LAYERS.indexOf("units");
  assert(
    LAYERS.indexOf("doodadUnderstory") < unitIndex
      && LAYERS.indexOf("doodadCanopies") > unitIndex
      && LAYERS.indexOf("doodadCanopies") < LAYERS.indexOf("selectionRings")
      && LAYERS.indexOf("doodadCanopies") < LAYERS.indexOf("hpBars")
      && LAYERS.indexOf("doodadCanopies") < LAYERS.indexOf("fog"),
    "doodad private layers bracket units while canopies stay beneath selection, HP, and authoritative fog",
  );

  layer.destroy();
  assert.equal(layer.instances.size, 0, "doodad teardown releases every static instance");
  assert(textures.every((texture) => texture.destroyed), "doodad teardown releases renderer-owned textures");

  const failedLayer = new DoodadLayer({
    pixi: PIXI,
    understoryLayer: new PIXI.Container(),
    canopyLayer: new PIXI.Container(),
    trackAsset(_id, promise) { return Promise.resolve(promise).catch(() => null); },
    loadTexture(_pixi, image) {
      return image.endsWith("tree-oak.png")
        ? Promise.reject(new Error("oak asset missing"))
        : Promise.resolve(PIXI.Texture.from(image));
    },
  });
  await assert.rejects(failedLayer.ready(), /tree\.oak/, "a missing allowlisted raster blocks renderer readiness");
  failedLayer.destroy();

  const editorCalls = [];
  const editorRenderer = {
    layers: { feedback: new PIXI.Container() },
    world: {
      position: { set() {} },
      scale: { set() {} },
    },
    replaceStaticDoodads: (doodads) => editorCalls.push(["replace", doodads]),
    patchStaticDoodads: (update) => editorCalls.push(["patch", update]),
    updateStaticDoodadWind: (time) => editorCalls.push(["wind", time]),
    present: () => editorCalls.push(["present"]),
    destroy: () => editorCalls.push(["destroy"]),
  };
  const editor = new MapEditorWorkerRenderer(editorRenderer);
  editor.present({
    version: 1,
    generation: 1,
    frameId: 1,
    visualTimeMs: 2400,
    camera: { x: 0, y: 0, zoom: 1 },
    terrainUpdate: null,
    doodadUpdate: { kind: "replace", revision: 1, doodads: [{ id: 8, typeId: "tree.pine", x: 4, y: 5 }] },
    overlay: {
      revision: 1,
      gridPaths: [],
      guides: [],
      guideCentre: null,
      sites: [],
      paintPreview: null,
      doodadSelection: { id: 8, x: 4, y: 5 },
      doodadBrushPreview: { x: 20, y: 20, radius: 12, mode: "spray", typeId: "wildflower.single", color: "#ffffff" },
    },
  });
  editor._applyDoodads({ kind: "patch", revision: 2, upserts: [], removedIds: [8] });
  editor._applyDoodads({ kind: "replace", revision: 1, doodads: [] });
  assert.equal(editorCalls.filter(([kind]) => kind === "replace").length, 1, "editor applies each monotonic doodad revision once");
  assert.equal(editorCalls.filter(([kind]) => kind === "patch").length, 1, "editor forwards compact doodad patches");
  assert(editorCalls.some(([kind, time]) => kind === "wind" && time === 2400), "editor sway uses its detached visual clock");
  editor.destroy();
  editor.destroy();
  assert.equal(editorCalls.filter(([kind]) => kind === "destroy").length, 1, "editor renderer teardown remains idempotent");
} finally {
  restorePixi();
}

console.log("✅ doodad_renderer_contracts.mjs: static assets, layering, wind, editor updates, and teardown passed");
