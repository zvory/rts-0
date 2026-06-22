// tests/client_contracts/renderer_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { FrameProfiler } from "../../client/src/frame_profiler.js";
import { KIND } from "../../client/src/protocol.js";
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

    const rig = renderer._liveRigPools.buildingRigs.get(entity.id)?.container;
    const overlay = renderer._pools.buildingOverlays.get(entity.id);
    const overlayRects = overlay?.calls.filter((call) => call[0] === "drawRect") || [];
    const buildingsIndex = renderer.world.children.indexOf(renderer.layers.buildings);
    const overlaysIndex = renderer.world.children.indexOf(renderer.layers.buildingOverlays);
    assert(rig && renderer.layers.buildings.children.includes(rig), "SVG building rig renders on the buildings layer");
    assert(overlayRects.length === 2, "under-construction building draws progress bar rectangles");
    assert(overlaysIndex > buildingsIndex, "building progress overlay layer renders above SVG building bodies");
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
