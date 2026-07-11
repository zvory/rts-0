// tests/client_contracts/renderer_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { FrameProfiler } from "../../client/src/frame_profiler.js";
import { COLORS } from "../../client/src/config.js";
import { KIND } from "../../client/src/protocol.js";
import { GROUND_DECAL_TEXTURE_WORLD_SCALE } from "../../client/src/renderer/decals.js";
import { TrenchDecalLayer, _drawOccupiedTrenches, _drawTrenches } from "../../client/src/renderer/trenches.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { loadFrameStripTexture } from "../../client/src/renderer/rigs/frame_strip_routing.js";
import { loadPngRigAtlasTexture } from "../../client/src/renderer/rigs/png_routing.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
  _drawCommandFeedback,
  _drawMortarImpacts,
  _drawPlacement,
  _drawResourceMiningPreview,
} from "../../client/src/renderer/feedback.js";

import { installFakePixi, RecordingGraphics } from "./pixi_fakes.mjs";

{
  const priorDocument = globalThis.document;
  const priorImage = globalThis.Image;
  const rawLoads = [];
  const canvases = [];
  globalThis.document = {
    createElement(tag) {
      assert(tag === "canvas", "frame-strip color loader creates a canvas");
      const canvas = {
        width: 0,
        height: 0,
        getContext(type) {
          assert(type === "2d", "frame-strip color loader requests a 2D canvas context");
          return {
            imageSmoothingEnabled: true,
            clearRect() {},
            drawImage() {},
            getImageData() {
              return { data: new Uint8ClampedArray(canvas.width * canvas.height * 4) };
            },
            putImageData() {},
          };
        },
      };
      canvases.push(canvas);
      return canvas;
    },
  };
  globalThis.Image = class FakeImage {
    constructor() {
      this.naturalWidth = 0;
      this.width = 0;
      this.naturalHeight = 0;
      this.height = 0;
      this.onload = null;
      this.onerror = null;
    }

    set src(value) {
      this._src = value;
      queueMicrotask(() => this.onload?.());
    }
  };

  try {
    const texture = await loadFrameStripTexture(
      {
        Assets: {
          load: async (src) => {
            rawLoads.push(src);
            return { fallbackSrc: src };
          },
        },
        Texture: {
          from() {
            throw new Error("canvas texture unavailable");
          },
        },
      },
      {
        image: "/assets/rigs/test-strip.png?v=contract",
        frameWidth: 12,
        frameHeight: 8,
        frameCount: 4,
        bakedColorAdjustment: { brightness: 100, saturation: 100, hue: 100 },
      },
    );

    assert(
      texture?.fallbackSrc === "/assets/rigs/test-strip.png?v=contract",
      "adjusted strip falls back to raw texture load when canvas texture creation fails",
    );
    assert(rawLoads.length === 1, "adjusted strip fallback loads the raw source once");
    assert(canvases[0]?.width === 48, "frame-strip canvas fallback width covers every frame");
    assert(canvases[0]?.height === 8, "frame-strip canvas fallback height uses frame metadata");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorImage === undefined) delete globalThis.Image;
    else globalThis.Image = priorImage;
  }
}

{
  const priorDocument = globalThis.document;
  const priorImage = globalThis.Image;
  const rawLoads = [];
  const canvases = [];
  globalThis.document = {
    createElement(tag) {
      assert(tag === "canvas", "PNG atlas color loader creates a canvas");
      const canvas = {
        width: 0,
        height: 0,
        getContext(type) {
          assert(type === "2d", "PNG atlas color loader requests a 2D canvas context");
          return {
            imageSmoothingEnabled: true,
            clearRect() {},
            drawImage() {},
            getImageData() {
              return { data: new Uint8ClampedArray(canvas.width * canvas.height * 4) };
            },
            putImageData() {},
          };
        },
      };
      canvases.push(canvas);
      return canvas;
    },
  };
  globalThis.Image = class FakeImage {
    constructor() {
      this.naturalWidth = 0;
      this.width = 0;
      this.naturalHeight = 0;
      this.height = 0;
      this.onload = null;
      this.onerror = null;
    }

    set src(value) {
      this._src = value;
      queueMicrotask(() => this.onload?.());
    }
  };

  try {
    const texture = await loadPngRigAtlasTexture(
      {
        Assets: {
          load: async (src) => {
            rawLoads.push(src);
            return { fallbackSrc: src };
          },
        },
        Texture: {
          from() {
            throw new Error("canvas texture unavailable");
          },
        },
      },
      {
        image: "/assets/rigs/test-atlas.png?v=contract",
        grid: { width: 32, height: 24 },
        runtimeColorAdjustment: { brightness: 105, saturation: 100, hue: 100 },
      },
    );

    assert(
      texture?.fallbackSrc === "/assets/rigs/test-atlas.png?v=contract",
      "adjusted atlas falls back to raw texture load when canvas texture creation fails",
    );
    assert(rawLoads.length === 1, "adjusted atlas fallback loads the raw source once");
    assert(canvases[0]?.width === 32, "PNG atlas canvas fallback width uses atlas metadata");
    assert(canvases[0]?.height === 24, "PNG atlas canvas fallback height uses atlas metadata");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorImage === undefined) delete globalThis.Image;
    else globalThis.Image = priorImage;
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    const adjustedTexture = PIXI.Texture.from("adjusted-atlas");
    adjustedTexture.rtsRendererOwnedTexture = true;
    const rawTexture = PIXI.Texture.from("raw-strip");
    renderer._livePngRigAtlasTextures.set(KIND.TANK, adjustedTexture);
    renderer._liveFrameStripTextures.set(KIND.RIFLEMAN, rawTexture);
    renderer._visualFrameStripTextures.set("rifleman:test", adjustedTexture);
    renderer._visualFrameStripTextureLoads.set("rifleman:test", Promise.resolve(adjustedTexture));

    renderer.destroy();

    assert(renderer._livePngRigAtlasTextures.size === 0, "renderer teardown clears live PNG atlas textures");
    assert(renderer._liveFrameStripTextures.size === 0, "renderer teardown clears live frame-strip textures");
    assert(renderer._visualFrameStripTextures.size === 0, "renderer teardown clears visual frame-strip textures");
    assert(renderer._visualFrameStripTextureLoads.size === 0, "renderer teardown clears pending visual strip loads");
    assert(adjustedTexture.destroyed, "renderer-owned adjusted textures are destroyed on teardown");
    assert(rawTexture.destroyed === false, "shared raw Pixi asset textures stay owned by the asset cache");

    const lateTexture = PIXI.Texture.from("late-adjusted");
    lateTexture.rtsRendererOwnedTexture = true;
    renderer._storeLoadedTexture(renderer._livePngRigAtlasTextures, KIND.TANK, lateTexture);
    assert(renderer._livePngRigAtlasTextures.size === 0, "late texture loads are not cached after teardown");
    assert(lateTexture.destroyed, "late renderer-owned texture loads are destroyed after teardown");
  } finally {
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  const priorConsoleError = console.error;
  const consoleErrors = [];
  console.error = (...args) => consoleErrors.push(args);
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    const profiler = new FrameProfiler();
    renderer._drawUnit = () => {
      throw new Error("broken worker art");
    };
    renderer._drawMortarImpacts = () => {
      throw new Error("broken mortar overlay");
    };

    let placementDraws = 0;
    const noOpOverlay = () => {};
    for (const name of [
      "_drawAbilityObjects",
      "_drawSmokes",
      "_drawFog",
      "_drawSmokeCanisters",
      "_drawCommandFeedback",
      "_drawMortarTargets",
      "_drawMortarLaunches",
      "_drawMortarShells",
      "_drawArtilleryLaunches",
      "_drawArtilleryTargets",
      "_drawArtilleryImpacts",
      "_drawSelectedUnitRanges",
      "_drawSelectedMortarRanges",
      "_drawBreakthroughAuras",
      "_drawAbilityTargetPreview",
      "_drawAntiTankGunSetupPreview",
      "_drawOrderPlan",
      "_drawDebugPathOverlay",
      "_drawRallyPoints",
      "_drawResourceMiningPreview",
      "_drawMuzzleFlashes",
    ]) {
      renderer[name] = noOpOverlay;
    }
    renderer._drawPlacement = () => {
      placementDraws += 1;
    };

    renderer.render(
      {
        playerId: 1,
        players: [{ id: 1, color: "#4878c8" }],
        selection: new Set(),
        rememberedBuildings: [],
        map: { tileSize: 32 },
        entitiesInterpolated: () => [
          { id: 101, owner: 1, kind: KIND.WORKER, x: 100, y: 120, facing: 0 },
        ],
      },
      {
        x: 0,
        y: 0,
        zoom: 1,
      },
      null,
      1,
      { profiler },
    );

    const fallback = renderer._pools.units.get(101);
    const rendererPhases = new Set(profiler.summary().phases.map((phase) => phase.label));
    assert(placementDraws === 1, "renderer continues later overlays after a render helper throws");
    assert(renderer._renderErrors.get("unit:worker")?.count === 1, "renderer records entity render errors by kind");
    assert(renderer._renderErrors.get("mortarImpacts")?.count === 1, "renderer records overlay render errors by label");
    assert(rendererPhases.has("renderer.units"), "renderer records unit sub-phase timing");
    assert(rendererPhases.has("renderer.feedbackOverlays"), "renderer records feedback overlay sub-phase timing");
    assert(profiler.summary().context.entityCount === 1, "renderer profiler context includes entity count");
    assert(fallback?.calls.some((call) => call[0] === "drawRect"), "broken entity art draws a checkerboard fallback");
    assert(
      consoleErrors.some((args) => String(args[0]).includes("[RTS_RENDER] skipped unit:worker")),
      "renderer logs recovered render errors",
    );
    assert(globalThis.__rtsRenderErrors?.latest?.label === "mortarImpacts", "renderer exposes latest render error diagnostics");
  } finally {
    console.error = priorConsoleError;
    restorePixi();
    delete globalThis.__rtsRenderErrors;
  }
}

{
  const restorePixi = installFakePixi();
  const priorDocument = globalThis.document;
  class FakeCanvasContext {
    constructor() {
      this.calls = [];
    }
    set fillStyle(value) { this.calls.push(["fillStyle", value]); }
    get fillStyle() { return ""; }
    fillRect(x, y, w, h) { this.calls.push(["fillRect", x, y, w, h]); }
    clearRect(x, y, w, h) { this.calls.push(["clearRect", x, y, w, h]); }
    beginPath() { this.calls.push(["beginPath"]); }
    moveTo(x, y) { this.calls.push(["moveTo", x, y]); }
    lineTo(x, y) { this.calls.push(["lineTo", x, y]); }
    closePath() { this.calls.push(["closePath"]); }
    fill() { this.calls.push(["fill"]); }
  }
  globalThis.document = {
    createElement(tag) {
      assert(tag === "canvas", "trench renderer only creates canvas elements");
      const ctx = new FakeCanvasContext();
      return {
        width: 0,
        height: 0,
        getContext(type) {
          assert(type === "2d", "trench renderer requests a 2d canvas context");
          return ctx;
        },
      };
    },
  };
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer.buildStaticMap({ width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0) });
    let decalLayerResets = 0;
    let trenchLayerResets = 0;
    renderer._initGroundDecalsForMap = () => { decalLayerResets += 1; };
    renderer._initTrenchesForMap = () => { trenchLayerResets += 1; };
    renderer.previewStaticTerrain({ width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(2) });
    assert(decalLayerResets === 0 && trenchLayerResets === 0,
      "local map-draft terrain preview preserves existing ground-decal and trench layers");
    const terrainIndex = renderer.world.children.indexOf(renderer.layers.terrain);
    const decalsIndex = renderer.world.children.indexOf(renderer.layers.decals);
    const trenchesIndex = renderer.world.children.indexOf(renderer.layers.trenches);
    const unitShadowsIndex = renderer.world.children.indexOf(renderer.layers.unitShadows);
    const occupantShadowsIndex = renderer.world.children.indexOf(renderer.layers.trenchOccupantShadows);
    const unitsIndex = renderer.world.children.indexOf(renderer.layers.units);
    const occupantLipsIndex = renderer.world.children.indexOf(renderer.layers.trenchOccupantLips);
    const selectionIndex = renderer.world.children.indexOf(renderer.layers.selectionRings);
    const resourcesIndex = renderer.world.children.indexOf(renderer.layers.resources);
    assert(terrainIndex < decalsIndex && decalsIndex < trenchesIndex && trenchesIndex < resourcesIndex,
      "renderer mounts trench ground above decals and below resources/units");
    assert(
      unitShadowsIndex < occupantShadowsIndex &&
        occupantShadowsIndex < occupantLipsIndex &&
        occupantLipsIndex < unitsIndex &&
        unitsIndex < selectionIndex,
      "occupied-trench berm layers sit below unit art and selection feedback",
    );
    assert(renderer.layers.trenches.children.length === 1, "renderer owns one persistent trench decal sprite");
    renderer.destroy();
    assert(renderer.layers.trenches.children.length === 0, "renderer teardown removes the trench decal sprite");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  const priorDocument = globalThis.document;
  const canvasContexts = [];
  class FakeCanvasContext {
    constructor() {
      this.calls = [];
    }
    set fillStyle(value) { this.calls.push(["fillStyle", value]); }
    get fillStyle() { return ""; }
    clearRect(x, y, w, h) { this.calls.push(["clearRect", x, y, w, h]); }
    beginPath() { this.calls.push(["beginPath"]); }
    moveTo(x, y) { this.calls.push(["moveTo", x, y]); }
    lineTo(x, y) { this.calls.push(["lineTo", x, y]); }
    closePath() { this.calls.push(["closePath"]); }
    fill() { this.calls.push(["fill"]); }
  }
  globalThis.document = {
    createElement(tag) {
      assert(tag === "canvas", "trench decal renderer only creates canvas elements");
      const ctx = new FakeCanvasContext();
      canvasContexts.push(ctx);
      return {
        width: 0,
        height: 0,
        getContext(type) {
          assert(type === "2d", "trench decal renderer requests a 2d canvas context");
          return ctx;
        },
      };
    },
  };
  try {
    const diagnostics = [];
    const trenchLayer = new TrenchDecalLayer({
      layer: new PIXI.Container(),
      pixi: PIXI,
      getDocument: () => globalThis.document,
      recordDiagnostic(label, amount = 1) {
        diagnostics.push([label, amount]);
      },
    });
    trenchLayer.resetForMap({ width: 12, height: 12, tileSize: 32 });
    const renderer = {
      _trenchDecals: trenchLayer,
      _map: { tileSize: 32 },
    };
    const state = {
      trenches: [
        { id: 2, x: 132, y: 96, radiusTiles: 0.375 },
        { id: 1, x: 96, y: 96, radiusTiles: 0.375 },
        { id: 3, x: 280, y: 96, radiusTiles: 0.375 },
      ],
    };
    const drawn = _drawTrenches.call(renderer, state);
    const trenchCtx = canvasContexts[0];
    const callsAfterDraw = trenchCtx.calls.length;
    const firstBegin = trenchCtx.calls.findIndex((call) => call[0] === "beginPath");
    const firstClose = trenchCtx.calls.findIndex((call, index) => index > firstBegin && call[0] === "closePath");
    const firstPolygon = trenchCtx.calls.slice(firstBegin, firstClose + 1)
      .filter((call) => call[0] === "moveTo" || call[0] === "lineTo");
    const xs = firstPolygon.map((call) => call[1]);
    const ys = firstPolygon.map((call) => call[2]);
    const width = Math.max(...xs) - Math.min(...xs);
    const height = Math.max(...ys) - Math.min(...ys);
    const authoritativeDiameter =
      (state.trenches[0].radiusTiles * renderer._map.tileSize * 2) / trenchLayer.downsample;

    assert(drawn === 3, "trench renderer draws all valid authoritative trench snapshots");
    assert(trenchLayer.displayObjectCount() === 1, "trench decals use one persistent display object");
    assert(trenchLayer.totalStamped === 3, "trench renderer stamps all visible foxholes into the texture");
    assert(trenchLayer.textureUpdateCount === 1, "trench renderer updates the texture once for a changed snapshot");
    assert(firstPolygon.length >= 20, "trench footprints are constructed from low-poly circular polygons");
    assert(width / height > 0.78 && width / height < 1.22, "trench footprints stay circular instead of oval");
    assert(width > authoritativeDiameter * 0.85 && width < authoritativeDiameter * 1.15,
      "trench footprints render at the authoritative trench diameter");
    assert(height > authoritativeDiameter * 0.85 && height < authoritativeDiameter * 1.15,
      "trench footprints render at the authoritative trench diameter");
    assert(trenchCtx.calls.some((call) => call[0] === "fillStyle" && call[1] === "rgb(90,56,34)"),
      "trench base decal is opaque dirt");
    assert(trenchCtx.calls.some((call) => call[0] === "fillStyle" && String(call[1]).startsWith("rgba(32,20,13")),
      "trench decals use interior dark fills for depth");
    assert(!trenchCtx.calls.some((call) => call[0] === "ellipse" || call[0] === "arc" || call[0] === "stroke"),
      "trench decals avoid smooth ellipse strokes and yellow outline rendering");
    assert(!trenchCtx.calls.some((call) => (call[0] === "moveTo" || call[0] === "lineTo") && call[1] === 33 && call[2] === 24),
      "nearby trenches are not linked by center-to-center strokes");
    assert(diagnostics.some(([label, amount]) => label === "renderer.trenches.visible" && amount === 3),
      "trench renderer records visible trench diagnostics");

    _drawTrenches.call(renderer, state);
    assert(trenchLayer.textureUpdateCount === 1, "unchanged trench snapshots do not redraw the texture");
    assert(trenchCtx.calls.length === callsAfterDraw, "normal frames do not redraw historical trench pixels");
    trenchLayer.destroy();
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  const priorDocument = globalThis.document;
  const canvasContexts = [];
  class FakeCanvasContext {
    constructor() {
      this.calls = [];
    }
    save() { this.calls.push(["save"]); }
    restore() { this.calls.push(["restore"]); }
    translate(x, y) { this.calls.push(["translate", x, y]); }
    rotate(angle) { this.calls.push(["rotate", angle]); }
    scale(x, y) { this.calls.push(["scale", x, y]); }
    clearRect(x, y, w, h) { this.calls.push(["clearRect", x, y, w, h]); }
    fillRect(x, y, w, h) { this.calls.push(["fillRect", x, y, w, h]); }
    beginPath() { this.calls.push(["beginPath"]); }
    moveTo(x, y) { this.calls.push(["moveTo", x, y]); }
    lineTo(x, y) { this.calls.push(["lineTo", x, y]); }
    closePath() { this.calls.push(["closePath"]); }
    ellipse(x, y, rx, ry, rotation) { this.calls.push(["ellipse", x, y, rx, ry, rotation]); }
    arc(x, y, radius, start, end) { this.calls.push(["arc", x, y, radius, start, end]); }
    fill() { this.calls.push(["fill"]); }
  }
  globalThis.document = {
    createElement(tag) {
      assert(tag === "canvas", "ground decal renderer only creates canvas elements");
      const ctx = new FakeCanvasContext();
      canvasContexts.push(ctx);
      return {
        width: 0,
        height: 0,
        getContext(type) {
          assert(type === "2d", "ground decal renderer requests a 2d canvas context");
          return ctx;
        },
      };
    },
  };
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer.buildStaticMap({ width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0) });
    assert(renderer.layers.decals.children.length === 1, "renderer creates exactly one permanent decal sprite");
    assert(renderer._groundDecals.downsample === GROUND_DECAL_TEXTURE_WORLD_SCALE, "decal texture uses the configured downsample");

    const pending = [];
    for (let i = 0; i < 120; i += 1) {
      pending.push({
        id: 1000 + i,
        kind: i % 2 === 0 ? KIND.WORKER : KIND.TANK,
        decalClass: i % 2 === 0 ? "infantry" : "scorch",
        x: 20 + (i % 12) * 12,
        y: 24 + Math.floor(i / 12) * 10,
        owner: 1,
        color: "#4878c8",
        facing: 0.1 * i,
        weaponFacing: 0.1 * i,
        seed: 9000 + i,
        variant: i % 4,
      });
    }
    const state = {
      consumePendingGroundDecals() {
        return pending.splice(0);
      },
    };
    renderer._drawGroundDecals(state);
    assert(renderer.layers.decals.children.length === 1, "stamping many decals does not create per-death display objects");
    assert(renderer._groundDecals.totalStamped === 120, "renderer stamps all queued decals into the permanent texture");
    assert(renderer._groundDecals.textureUpdateCount === 1, "renderer updates the decal texture once per consumed batch");
    renderer._drawGroundDecals(state);
    assert(renderer._groundDecals.textureUpdateCount === 1, "renderer does not update the decal texture when no decals are pending");
    const decalCtx = canvasContexts[1];
    assert(decalCtx.calls.some((call) => call[0] === "ellipse"), "infantry decals draw placeholder blob ellipses");
    assert(decalCtx.calls.some((call) => call[0] === "fillRect"), "vehicle decals draw placeholder paint fragments");
    renderer.destroy();
    renderer.destroy();
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer._map = { tileSize: 32 };
    const scaffold = {
      id: 503,
      owner: 1,
      kind: KIND.BARRACKS,
      x: 160,
      y: 160,
      hp: 10,
      maxHp: 100,
      state: "construct",
      buildProgress: 0.42,
    };
    const completed = {
      id: 504,
      owner: 1,
      kind: KIND.BARRACKS,
      x: 220,
      y: 160,
      hp: 42,
      maxHp: 100,
      state: "idle",
    };
    const entrenched = {
      id: 505,
      owner: 1,
      kind: KIND.RIFLEMAN,
      x: 260,
      y: 160,
      hp: 40,
      maxHp: 40,
      state: "idle",
      occupiedTrenchId: 80,
    };

    renderer._drawSelectionAndHp(scaffold, new Set([scaffold.id]), { playerId: 1 });
    renderer._drawSelectionAndHp(completed, new Set(), { playerId: 1 });
    renderer._drawSelectionAndHp(entrenched, new Set(), { playerId: 1 });

    const scaffoldHpRects = renderer._pools.hpBars.get(scaffold.id)?.calls.filter((call) => call[0] === "drawRect") || [];
    const scaffoldBarW = scaffoldHpRects[0]?.[3] - 2;
    assert(
      renderer._pools.selectionRings.has(scaffold.id),
      "selected under-construction building still draws a selection ring",
    );
    assert(
      scaffoldHpRects.length === 2,
      "under-construction building draws construction status on the HP bar layer",
    );
    assert(
      Math.abs(scaffoldHpRects[1][3] - scaffoldBarW * scaffold.buildProgress) < 0.001,
      "under-construction HP-layer status bar uses buildProgress instead of current HP",
    );
    assert(
      renderer._pools.hpBars.has(completed.id),
      "completed damaged building still draws a normal HP bar",
    );
    assert(
      !renderer._pools.selectionRings.has(entrenched.id),
      "occupied infantry no longer draw an unselected trench marker on the selection-ring layer",
    );
  } finally {
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer._map = { tileSize: 32 };
    const stripTexture = PIXI.Texture.from("scout-plane-strip-test-texture");
    renderer._liveFrameStripTextures.set(KIND.SCOUT_PLANE, stripTexture);
    const entity = {
      id: 507,
      owner: 1,
      kind: KIND.SCOUT_PLANE,
      x: 260,
      y: 160,
      facing: 0.25,
      hp: 40,
      maxHp: 40,
      state: "move",
      scoutPlane: {
        orbitCenter: [320, 320],
      },
    };

    renderer._drawUnit(entity, new Map([[1, 0x4878c8]]), {
      playerId: 1,
      selection: new Set([entity.id]),
      resources: { oil: 100 },
    });

    const unitStrip = renderer._liveRigPools.liveUnitRigs.get(entity.id);
    const shadowRig = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
    const ring = renderer._ringRadius(entity);
    assert(unitStrip?.strip?.unit === KIND.SCOUT_PLANE, "Scout Plane live rendering uses the PNG frame strip");
    assert(unitStrip?.texture === stripTexture, "Scout Plane frame-strip renderer uses the preloaded strip texture");
    assert(
      unitStrip?.frameTextures?.length === unitStrip?.strip?.frameCount,
      "Scout Plane frame strip exposes one runtime texture per declared frame",
    );
    assert(shadowRig?.parts.has("part.shadow"), "Scout Plane frame-strip rendering keeps the separate SVG shadow route");
    assert(
      ring.rx === 28 && ring.ry === 22 && ring.cy === 2,
      "Scout Plane selection ring uses the mirrored 48x34 aircraft body",
    );
    assert(
      renderer.layers.units.children.includes(unitStrip.container) &&
        renderer.layers.unitShadows.children.includes(shadowRig.container),
      "Scout Plane frame strip renders through the normal unit and shadow layers",
    );
  } finally {
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer._map = { tileSize: 32 };
    const entity = {
      id: 506,
      owner: 1,
      kind: KIND.RIFLEMAN,
      x: 260,
      y: 160,
      hp: 40,
      maxHp: 40,
      state: "idle",
      occupiedTrenchId: 80,
    };
    const colorByOwner = new Map([[1, 0x4878c8]]);
    const state = {
      playerId: 1,
      selection: new Set(),
      resources: {},
    };

    renderer._drawUnit(entity, colorByOwner, state);

    const unitRig = renderer._liveRigPools.liveUnitRigs.get(entity.id);
    const shadowRig = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
    const bodyCalls = unitRig?.parts.get("part.body")?.display.calls || [];
    const shadowCalls = shadowRig?.parts.get("part.shadow")?.display.calls || [];

    assert(unitRig?.container.scaleX === 0.85 && unitRig.container.scaleY === 0.85,
      "occupied infantry rig scales down while in a trench");
    assert(shadowRig?.container.scaleX === 0.85 && shadowRig.container.scaleY === 0.85,
      "occupied infantry shadow scales with the unit rig");
    assert(
      bodyCalls.some((call) => call[0] === "beginFill" && call[1] === 0x4878c8),
      "occupied infantry rig keeps the team-colored body fill",
    );
    assert(
      !bodyCalls.some((call) => call[0] === "beginFill" && call[1] === COLORS.trenchDirt),
      "occupied infantry rig does not draw a dirt tint overlay on visible body parts",
    );
    assert(
      !shadowCalls.some((call) => call[0] === "beginFill" && call[1] === COLORS.trenchDirt),
      "occupied infantry rig does not tint the separate shadow route",
    );

    const occupiedDrawn = _drawOccupiedTrenches.call(renderer, [entity], {
      map: { tileSize: 32 },
      trenches: [{ id: 80, x: 260, y: 160, radiusTiles: 0.375 }],
    });
    const occupantShadow = renderer._pools.trenchOccupantShadows.get(entity.id);
    const occupantLip = renderer._pools.trenchOccupantLips.get(entity.id);
    assert(occupiedDrawn === 1, "occupied infantry draws one occupied-trench overlay");
    assert(
      occupantShadow?.calls.some((call) => call[0] === "beginFill" && call[1] === COLORS.trenchShadow),
      "occupied trench overlay darkens the trench basin below the unit",
    );
    assert(
      occupantLip?.calls.some((call) => call[0] === "beginFill" && call[1] === COLORS.trenchRim),
      "occupied trench overlay draws a berm graphic for occupied infantry",
    );
    const wrapPolygon = polygonAfterLineStyle(occupantShadow?.calls || [], COLORS.trenchRim);
    const wrapXs = polygonAxisValues(wrapPolygon, 0);
    const wrapYs = polygonAxisValues(wrapPolygon, 1);
    assert(
      Math.min(...wrapXs) < -8 && Math.max(...wrapXs) > 8 &&
        Math.min(...wrapYs) < -8 && Math.max(...wrapYs) > 8,
      "occupied trench back berm wraps around all sides below the unit",
    );
    const foregroundRimPolygon = polygonAfterFill(occupantLip?.calls || [], COLORS.trenchRim);
    const foregroundRimYs = polygonAxisValues(foregroundRimPolygon, 1);
    assert(
      Math.min(...foregroundRimYs) > 0,
      "occupied trench foreground lip stays on the front half instead of covering the unit center",
    );
    const missingTrenchEntity = { ...entity, id: 4310, occupiedTrenchId: 999, x: 310, y: 210 };
    const missingTrenchDrawn = _drawOccupiedTrenches.call(renderer, [missingTrenchEntity], {
      map: { tileSize: 32 },
      trenches: [],
    });
    assert(missingTrenchDrawn === 0, "occupied trench overlay requires authoritative trench terrain");
    assert(
      !renderer._pools.trenchOccupantLips.has(missingTrenchEntity.id) &&
        !renderer._pools.trenchOccupantShadows.has(missingTrenchEntity.id),
      "missing trench ids do not create client-only trench display objects",
    );

    delete entity.occupiedTrenchId;
    renderer._drawUnit(entity, colorByOwner, state);
    const emptyDrawn = _drawOccupiedTrenches.call(renderer, [entity], {
      map: { tileSize: 32 },
      trenches: [{ id: 80, x: 260, y: 160, radiusTiles: 0.375 }],
    });

    let lastClearIndex = -1;
    for (let i = bodyCalls.length - 1; i >= 0; i -= 1) {
      if (bodyCalls[i][0] === "clear") {
        lastClearIndex = i;
        break;
      }
    }
    const latestBodyCalls = bodyCalls.slice(lastClearIndex + 1);
    assert(unitRig.container.scaleX === 1 && unitRig.container.scaleY === 1,
      "infantry rig returns to normal scale after leaving a trench");
    assert(
      !latestBodyCalls.some((call) => call[0] === "beginFill" && call[1] === COLORS.trenchDirt),
      "infantry rig clears the dirt tint overlay after leaving a trench",
    );
    assert(emptyDrawn === 0, "empty trenches do not draw the occupied berm overlay");
  } finally {
    restorePixi();
  }
}

function polygonAfterFill(calls, fillColor) {
  for (let i = 0; i < calls.length - 1; i += 1) {
    if (calls[i][0] === "beginFill" && calls[i][1] === fillColor && calls[i + 1][0] === "drawPolygon") {
      return Array.isArray(calls[i + 1][1]) ? calls[i + 1][1] : [];
    }
  }
  return [];
}

function polygonAfterLineStyle(calls, lineColor) {
  for (let i = 0; i < calls.length - 1; i += 1) {
    if (calls[i][0] === "lineStyle" && calls[i][2] === lineColor && calls[i + 1][0] === "drawPolygon") {
      return Array.isArray(calls[i + 1][1]) ? calls[i + 1][1] : [];
    }
  }
  return [];
}

function polygonAxisValues(points, offset) {
  const out = [];
  for (let i = offset; i < points.length; i += 2) out.push(points[i]);
  return out;
}

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer._map = { tileSize: 32 };
    const entity = {
      id: 501,
      owner: 2,
      kind: KIND.BARRACKS,
      x: 160,
      y: 160,
      hp: 100,
      maxHp: 400,
      state: "idle",
      buildProgress: 0.42,
    };

    renderer._drawBuilding(entity, new Map([[2, 0xc85050]]), {
      playerId: 99,
      players: [{ id: 2, color: "#c85050" }],
      spectator: true,
    });
    renderer._drawSelectionAndHp(entity, new Set(), { playerId: 99 });

    const rig = renderer._liveRigPools.buildingRigs.get(entity.id)?.container;
    const hpRects = renderer._pools.hpBars.get(entity.id)?.calls.filter((call) => call[0] === "drawRect") || [];
    const buildingsIndex = renderer.world.children.indexOf(renderer.layers.buildings);
    const hpIndex = renderer.world.children.indexOf(renderer.layers.hpBars);
    assert(rig && renderer.layers.buildings.children.includes(rig), "SVG building rig renders on the buildings layer");
    assert(
      !renderer._pools.buildingOverlays.has(entity.id),
      "under-construction building does not draw a separate building overlay progress bar",
    );
    assert(hpRects.length === 2, "under-construction building draws construction status on the HP bar layer");
    assert(hpIndex > buildingsIndex, "HP-layer construction status renders above SVG building bodies");
  } finally {
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    renderer._map = { tileSize: 32 };
    const entity = {
      id: 502,
      owner: 2,
      kind: KIND.TANK_TRAP,
      x: 160,
      y: 160,
      hp: 120,
      maxHp: 120,
      state: "idle",
      deconstructProgress: 0.35,
    };

    renderer._drawBuilding(entity, new Map([[2, 0xc85050]]), {
      playerId: 99,
      players: [{ id: 2, color: "#c85050" }],
      spectator: true,
    });
    renderer._drawSelectionAndHp(entity, new Set(), { playerId: 99 });

    const hpRects = renderer._pools.hpBars.get(entity.id)?.calls.filter((call) => call[0] === "drawRect") || [];
    const hpBarW = hpRects[0]?.[3] - 2;
    assert(
      !renderer._pools.buildingOverlays.has(entity.id),
      "deconstructing Tank Trap does not draw a separate building overlay progress bar",
    );
    assert(hpRects.length === 2, "deconstructing Tank Trap draws reverse status on the HP bar layer");
    assert(
      Math.abs(hpRects[1][3] - hpBarW * entity.deconstructProgress) < 0.001,
      "deconstructing Tank Trap HP-layer status drains according to deconstructProgress",
    );
  } finally {
    restorePixi();
  }
}

{
  const restorePixi = installFakePixi();
  const parent = {
    clientWidth: 640,
    clientHeight: 480,
    appendChild(view) { view.parentNode = this; },
    removeChild(view) { view.parentNode = null; },
  };
  try {
    const renderer = new Renderer(parent);
    renderer._map = { width: 4, height: 4, tileSize: 32, terrain: new Array(16).fill(0) };
    const diagnostics = [];
    renderer._profiler = {
      recordDiagnosticCounter(label, amount = 1) { diagnostics.push([label, amount]); },
    };

    const resource = { id: 601, kind: KIND.OIL, x: 48, y: 48, remaining: 1000 };
    renderer._miningNodes = new Set();
    renderer._drawResource(resource, { isVisible: () => false });
    const resourceGfx = renderer._pools.resources.get(resource.id);
    const resourceCalls = resourceGfx.calls.length;
    const resourceChildren = renderer.layers.resources.children.length;
    diagnostics.length = 0;

    resource.x = 80;
    renderer._drawResource(resource, { isVisible: () => true });
    assert(resourceGfx.calls.length === resourceCalls, "unchanged resource geometry is not cleared or redrawn");
    assert(resourceGfx.x === 80 && resourceGfx.alpha === 1, "cached resource position and fog alpha remain live");
    assert(renderer.layers.resources.children.length === resourceChildren, "cached resource render creates no display-object churn");
    assert(
      !diagnostics.some(([label]) => label === "renderer.graphics.clear.resources" || label === "renderer.pixi.displayObject.created.resources"),
      "cached resource render records neither a clear nor object creation",
    );

    resource.remaining = 500;
    renderer._drawResource(resource, { isVisible: () => true });
    assert(resourceGfx.calls.length > resourceCalls, "resource remaining amount invalidates cached geometry");
    const afterRemaining = resourceGfx.calls.length;
    renderer._miningNodes.add(resource.id);
    renderer._drawResource(resource, { isVisible: () => true });
    assert(resourceGfx.calls.length > afterRemaining, "resource mining marker invalidates cached geometry");

    const retryResource = { id: 603, kind: KIND.OIL, x: 112, y: 48, remaining: 800 };
    renderer._drawResource(retryResource, { isVisible: () => true });
    const retryGfx = renderer._pools.resources.get(retryResource.id);
    const warmResourceKey = retryGfx.rtsStaticRenderKey;
    const warmResourceCalls = retryGfx.calls.length;
    retryResource.remaining = 400;
    const retryDrawRect = retryGfx.drawRect.bind(retryGfx);
    let throwResourceOnce = true;
    retryGfx.drawRect = (...args) => {
      if (throwResourceOnce) {
        throwResourceOnce = false;
        throw new Error("transient resource draw failure");
      }
      return retryDrawRect(...args);
    };
    const priorConsoleError = console.error;
    console.error = () => {};
    try {
      const changedDraw = renderer._drawEntitySafely("resource", retryResource, "resources", () => {
        renderer._drawResource(retryResource, { isVisible: () => true });
      });
      assert(changedDraw === false, "resource safe draw catches a transient warm-cache redraw failure");
    } finally {
      console.error = priorConsoleError;
    }
    assert(
      retryGfx.rtsStaticRenderKey === undefined,
      "failed warm-cache redraw discards the old resource key before fallback geometry",
    );
    const fallbackCalls = retryGfx.calls.length;
    const retryDraw = renderer._drawEntitySafely("resource", retryResource, "resources", () => {
      renderer._drawResource(retryResource, { isVisible: () => true });
    });
    assert(retryDraw === true, "resource geometry retries successfully on the next frame");
    assert(
      retryGfx.rtsStaticRenderKey !== undefined && retryGfx.calls.length > fallbackCalls,
      "successful resource retry replaces fallback geometry and commits its key",
    );
    const successfulRetryCalls = retryGfx.calls.length;
    renderer._drawResource(retryResource, { isVisible: () => true });
    assert(retryGfx.calls.length === successfulRetryCalls, "successful resource retry restores the normal cache fast path");
    retryResource.remaining = 800;
    renderer._drawResource(retryResource, { isVisible: () => true });
    assert(
      retryGfx.rtsStaticRenderKey === warmResourceKey
        && retryGfx.calls.length > successfulRetryCalls
        && successfulRetryCalls > warmResourceCalls,
      "resource state reversion redraws and recommits the original warm-cache key",
    );

    const building = {
      id: 602,
      owner: 2,
      kind: KIND.TANK_TRAP,
      x: 160,
      y: 160,
      hp: 120,
      maxHp: 120,
      state: "idle",
      prodProgress: 0.25,
      prodQueue: 0,
    };
    const buildingState = { playerId: 99, players: [{ id: 2, color: "#c85050" }], spectator: true };
    renderer._drawBuilding(building, new Map([[2, 0xc85050]]), buildingState);
    const buildingGfx = renderer._pools.buildings.get(building.id);
    const shadowGfx = renderer._pools.buildingShadows.get(building.id);
    const overlayGfx = renderer._pools.buildingOverlays.get(building.id);
    const buildingCalls = buildingGfx.calls.length;
    const shadowCalls = shadowGfx.calls.length;
    const overlayCalls = overlayGfx.calls.length;
    const buildingChildren = renderer.layers.buildings.children.length;
    diagnostics.length = 0;

    renderer._drawBuilding(building, new Map([[2, 0xc85050]]), buildingState);
    assert(buildingGfx.calls.length === buildingCalls, "unchanged building body is not cleared or redrawn");
    assert(shadowGfx.calls.length === shadowCalls, "unchanged building shadow is not cleared or redrawn");
    assert(overlayGfx.calls.length === overlayCalls, "unchanged building progress is not cleared or redrawn");
    assert(renderer.layers.buildings.children.length === buildingChildren, "cached building render creates no display-object churn");
    assert(
      !diagnostics.some(([label]) => label.startsWith("renderer.graphics.clear.building") || label.startsWith("renderer.pixi.displayObject.created.building")),
      "cached building render records neither target clears nor object creation",
    );

    building.x += 32;
    building.prodProgress = 0.5;
    renderer._drawBuilding(building, new Map([[2, 0xc85050]]), buildingState);
    assert(buildingGfx.calls.length > buildingCalls, "building position invalidates absolute body geometry");
    assert(shadowGfx.calls.length > shadowCalls, "building position invalidates shadow geometry");
    assert(overlayGfx.calls.length > overlayCalls, "building progress invalidates progress geometry");

    const rigBuilding = {
      id: 604,
      owner: 2,
      kind: KIND.BARRACKS,
      x: 256,
      y: 160,
      hp: 400,
      maxHp: 400,
      state: "idle",
      prodQueue: 0,
    };
    renderer._drawBuilding(rigBuilding, new Map([[2, 0xc85050]]), buildingState);
    const rigWrapper = renderer._pools.buildings.get(rigBuilding.id);
    assert(rigWrapper.rtsStaticRenderKey !== undefined, "building rig starts from a warm static wrapper cache");
    const rigInstance = renderer._liveRigPools.buildingRigs.get(rigBuilding.id);
    const rigUpdate = rigInstance.update.bind(rigInstance);
    rigInstance.update = () => { throw new Error("transient building rig failure"); };
    const priorRigConsoleError = console.error;
    console.error = () => {};
    try {
      const failedRigDraw = renderer._drawEntitySafely("building", rigBuilding, "buildings", () => {
        renderer._drawBuilding(rigBuilding, new Map([[2, 0xc85050]]), buildingState);
      });
      assert(failedRigDraw === false, "building safe draw catches a cached rig failure");
    } finally {
      console.error = priorRigConsoleError;
      rigInstance.update = rigUpdate;
    }
    assert(
      rigWrapper.rtsStaticRenderKey === undefined,
      "building fallback invalidates an existing static wrapper key",
    );
    const rigRetry = renderer._drawEntitySafely("building", rigBuilding, "buildings", () => {
      renderer._drawBuilding(rigBuilding, new Map([[2, 0xc85050]]), buildingState);
    });
    assert(
      rigRetry === true && rigWrapper.rtsStaticRenderKey !== undefined,
      "building rig retry clears fallback geometry and recommits its static wrapper key",
    );

    const visible = [1, 0, 0, 0];
    const explored = [1, 1, 0, 0];
    const fog = {
      width: 2,
      height: 2,
      revision: 1,
      visibleRevision: 1,
      exploredRevision: 1,
      revealAll: false,
      isVisible: (tx, ty) => visible[ty * 2 + tx] === 1,
      isExplored: (tx, ty) => explored[ty * 2 + tx] === 1,
    };
    renderer._drawFog(fog);
    const fogCalls = renderer._fogGfx.calls.length;
    diagnostics.length = 0;
    renderer._drawFog(fog);
    assert(renderer._fogGfx.calls.length === fogCalls, "unchanged fog revision does not clear or retessellate geometry");
    assert(
      !diagnostics.some(([label]) => label === "renderer.graphics.clear.fog"),
      "unchanged fog revision records no Graphics clear",
    );
    visible[1] = 1;
    fog.revision += 1;
    fog.visibleRevision += 1;
    renderer._drawFog(fog);
    assert(renderer._fogGfx.calls.length > fogCalls, "fog visibility revision invalidates cached geometry");
    delete fog.revision;
    const fallbackFogCalls = renderer._fogGfx.calls.length;
    renderer._drawFog(fog);
    const firstFallbackFogCalls = renderer._fogGfx.calls.length;
    renderer._drawFog(fog);
    assert(
      renderer._fogGfx.calls.length === firstFallbackFogCalls && firstFallbackFogCalls > fallbackFogCalls,
      "revisionless fog uses a stable content key",
    );
    explored[3] = 1;
    renderer._drawFog(fog);
    assert(renderer._fogGfx.calls.length > firstFallbackFogCalls, "revisionless fog content changes invalidate geometry");

    const retryFog = {
      ...fog,
      revision: 99,
      visibleRevision: 99,
      exploredRevision: 99,
    };
    const priorFogKey = renderer._fogRenderKey;
    const priorFogMap = renderer._fogRenderMap;
    const retryFogDrawRect = renderer._fogGfx.drawRect.bind(renderer._fogGfx);
    let throwFogOnce = true;
    renderer._fogGfx.drawRect = (...args) => {
      if (throwFogOnce) {
        throwFogOnce = false;
        throw new Error("transient fog draw failure");
      }
      return retryFogDrawRect(...args);
    };
    let fogFailed = false;
    try {
      renderer._drawFog(retryFog);
    } catch {
      fogFailed = true;
    }
    assert(fogFailed, "fog test exercises a transient tessellation failure");
    assert(
      renderer._fogRenderKey === priorFogKey && renderer._fogRenderMap === priorFogMap,
      "failed fog tessellation does not commit its map or render key",
    );
    const failedFogCalls = renderer._fogGfx.calls.length;
    renderer._drawFog(retryFog);
    assert(
      renderer._fogRenderKey !== priorFogKey && renderer._fogGfx.calls.length > failedFogCalls,
      "fog tessellation retries and commits only after success",
    );
    const successfulFogRetryCalls = renderer._fogGfx.calls.length;
    renderer._drawFog(retryFog);
    assert(renderer._fogGfx.calls.length === successfulFogRetryCalls, "successful fog retry restores the cache fast path");

    renderer._seen.resources.clear();
    for (let frame = 0; frame < 120; frame += 1) renderer._sweep();
    assert(!renderer._pools.resources.has(resource.id), "resource cache state is evicted with its Graphics slot");
    renderer.destroy();
    assert(
      renderer._fogRenderKey === null,
      "renderer destroy clears static Graphics and fog render keys",
    );
  } finally {
    restorePixi();
  }
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorRequestAnimationFrame = globalThis.requestAnimationFrame;
  const priorConsoleError = console.error;
  const consoleErrors = [];
  const tickFn = () => {};
  console.error = (...args) => consoleErrors.push(args);
  globalThis.window = {
    ...(priorWindow || {}),
    location: { protocol: "http:", host: "localhost", search: "" },
    localStorage: { getItem() { return null; } },
  };
  globalThis.document = {
    hidden: false,
    getElementById: () => null,
  };
  globalThis.requestAnimationFrame = (fn) => {
    assert(fn === tickFn, "frame recovery schedules the match tick callback");
    return 77;
  };
  try {
    const { Match } = await import("../../client/src/match.js");
    const match = Object.create(Match.prototype);
    Object.assign(match, {
      running: true,
      lastFrame: 1000,
      tickFn,
      frameErrors: { count: 0, lastLogAt: -Infinity },
      health: {
        noteFrameGap() {},
        refreshLatency() {},
        publish() {},
      },
      computeAlpha: () => 1,
      camera: {
        update() {
          throw new Error("camera update failed");
        },
      },
      input: { update() {} },
      advancePredictionVisual() {},
      fog: { update() {} },
      ownEntities: () => [],
      state: { map: { tileSize: 32 }, visibleTiles: null },
      renderer: { render() {} },
      hud: { update() {} },
      minimap: { render() {} },
      observerDiagnostics: null,
    });

    match.frame(1016);

    assert(match.rafId === 77, "match frame schedules the next frame after a client error");
    assert(match.frameErrors.count === 1, "match frame records recovered client errors");
    assert(
      consoleErrors.some((args) => String(args[0]).includes("[RTS_FRAME] recovered")),
      "match frame logs recovered client errors",
    );
    assert(globalThis.__rtsFrameErrors?.count === 1, "match frame exposes recovered frame diagnostics");
  } finally {
    if (priorRequestAnimationFrame === undefined) delete globalThis.requestAnimationFrame;
    else globalThis.requestAnimationFrame = priorRequestAnimationFrame;
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
    console.error = priorConsoleError;
    delete globalThis.__rtsFrameErrors;
  }
}
