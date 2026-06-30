// tests/client_contracts/renderer_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { FrameProfiler } from "../../client/src/frame_profiler.js";
import { KIND } from "../../client/src/protocol.js";
import { GROUND_DECAL_TEXTURE_WORLD_SCALE } from "../../client/src/renderer/decals.js";
import { Renderer } from "../../client/src/renderer/index.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
  _drawCommandFeedback,
  _drawMortarImpacts,
  _drawPlacement,
  _drawResourceMiningPreview,
} from "../../client/src/renderer/feedback.js";

import { installFakePixi } from "./pixi_fakes.mjs";

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

    renderer._drawSelectionAndHp(scaffold, new Set([scaffold.id]), { playerId: 1 });
    renderer._drawSelectionAndHp(completed, new Set(), { playerId: 1 });

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
      observerAnalysisOverlay: null,
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
