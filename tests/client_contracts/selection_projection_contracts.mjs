import { assert, assertApprox } from "./assertions.mjs";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { Input } from "../../client/src/input/index.js";
import { ScreenOverlay } from "../../client/src/input/screen_overlay.js";
import {
  buildSelectionScene,
  groundCoverageForScreenRect,
  pickSelectionProxy,
  projectSelectionProxy,
  proxyIntersectsViewport,
  selectionProxiesInScreenRect,
} from "../../client/src/input/selection_projection.js";
import { KIND } from "../../client/src/protocol.js";
import { runMatchCaptureFrame } from "../../client/src/frame_recovery.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { admitSelectionIds } from "../../client/src/command_budget.js";

function orthographic({ x = 0, y = 0, zoom = 1, width = 400, height = 300 } = {}) {
  return createOrthographicProjectionSnapshot({
    x,
    y,
    zoom,
    worldW: 1024,
    worldH: 1024,
    viewW: width,
    viewH: height,
  }, width / zoom);
}

function fakePerspective({ groundAtScreen = true } = {}) {
  const viewport = Object.freeze({ widthCssPx: 400, heightCssPx: 300 });
  return Object.freeze({
    version: 1,
    viewport,
    project(point) {
      const depth = 200 + point.y;
      const scale = 200 / Math.max(1, depth);
      const x = 200 + (point.x - 100) * scale;
      const y = 40 + point.y * 0.62 - point.heightPx * scale;
      let clip = "inside";
      if (depth <= 0) clip = "behindCamera";
      else if (depth < 20 || depth > 1600) clip = "outsideDepth";
      else if (x < 0 || x > viewport.widthCssPx || y < 0 || y > viewport.heightCssPx) clip = "outsideViewport";
      return Object.freeze({ x, y, depth, clip, visible: clip === "inside" });
    },
    groundAtScreen(screen) {
      if (!groundAtScreen || screen.y < 40) return null;
      const y = (screen.y - 40) / 0.62;
      const scale = 200 / (200 + y);
      return { x: 100 + (screen.x - 200) / scale, y };
    },
  });
}

function scene(entities, projection = orthographic(), options = {}) {
  return buildSelectionScene({
    entities,
    projection,
    tileSize: 32,
    generation: options.generation ?? 1,
    frameId: options.frameId ?? 1,
  });
}

{
  const source = {
    id: 11,
    owner: 1,
    kind: KIND.RIFLEMAN,
    x: 100,
    y: 90,
    orderPlan: [{ kind: "move", x: 120, y: 100 }],
  };
  const built = scene([source]);
  const proxy = built.proxies[0];
  assert(proxy.selectClass === "unit", "unit proxies expose a renderer-neutral select class");
  assert(proxy.footprint.kind === "circle", "ordinary unit proxies use mirrored circular footprints");
  assert(proxy.anchor.heightPx === 9, "unit proxy anchor height uses the mirrored selection size");
  source.x = 300;
  source.orderPlan[0].x = 999;
  assert(proxy.anchor.x === 100, "SelectionScene proxy anchors detach from later entity movement");
  assert(proxy.interaction.orderPlan[0].x === 120, "SelectionScene interaction metadata is deeply detached");
  assert(Object.isFrozen(proxy) && Object.isFrozen(proxy.interaction.orderPlan), "selection proxies are immutable plain data");

  const building = scene([{ id: 12, owner: 1, kind: KIND.BARRACKS, x: 160, y: 160, facing: Math.PI / 2 }]).proxies[0];
  assert(building.footprint.kind === "polygon" && building.footprint.points.length === 4, "buildings use rotated tile-footprint polygons");
  assert(building.anchor.heightPx === 48, "building proxy anchor height uses half the largest mirrored footprint");
}

{
  const overlap = scene([
    { id: 20, owner: 2, kind: KIND.RIFLEMAN, x: 100, y: 100 },
    { id: 21, owner: 1, kind: KIND.RIFLEMAN, x: 100, y: 100 },
  ]);
  const own = pickSelectionProxy(overlap, { x: 100, y: 100 }, {
    preference: (proxy) => proxy.owner === 1 ? 1 : 0,
  });
  assert(own.id === 21, "click picking applies ownership eligibility preference before distance");
  const stable = pickSelectionProxy(overlap, { x: 100, y: 100 });
  assert(stable.id === 20, "equal click candidates use stable numeric id ordering");

  const depthProjection = Object.freeze({
    version: 1,
    viewport: Object.freeze({ widthCssPx: 200, heightCssPx: 200 }),
    project(point) {
      const depth = point.y + 100;
      const clip = depth <= 0 ? "behindCamera" : "inside";
      return { x: 100, y: 100 - point.heightPx * 0.1, depth, clip, visible: clip === "inside" };
    },
    groundAtScreen: () => null,
  });
  const depthScene = scene([
    { id: 31, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 40 },
    { id: 32, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 },
    { id: 30, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: -220 },
  ], depthProjection);
  assert(pickSelectionProxy(depthScene, { x: 100, y: 99.1 }).id === 32, "overlapping clicks choose nearest positive visible depth before id");
  assert(projectSelectionProxy(depthScene.proxies.find((proxy) => proxy.id === 30), depthProjection) === null, "behind-camera proxies are rejected");

  const partiallyDepthClipped = scene([
    { id: 33, owner: 1, kind: KIND.RIFLEMAN, x: 100, y: -175 },
  ], fakePerspective());
  assert(
    projectSelectionProxy(partiallyDepthClipped.proxies[0], partiallyDepthClipped.projection) === null,
    "a depth-clipped footprint is rejected instead of joining its surviving vertices into a false hit polygon",
  );
}

{
  const skewed = fakePerspective({ groundAtScreen: false });
  const building = { id: 40, owner: 1, kind: KIND.BARRACKS, x: 100, y: 160, facing: Math.PI / 4 };
  const skewedScene = scene([building], skewed);
  const projected = projectSelectionProxy(skewedScene.proxies[0], skewed);
  const rect = {
    x0: projected.anchor.x - 4,
    y0: projected.anchor.y - 4,
    x1: projected.anchor.x + 4,
    y1: projected.anchor.y + 4,
  };
  assert(selectionProxiesInScreenRect(skewedScene, rect).map((proxy) => proxy.id).join(",") === "40", "skewed perspective marquee selects by the real screen rectangle without ground hits");
  assert(pickSelectionProxy(skewedScene, projected.anchor)?.id === 40, "an elevated semantic anchor remains clickable when the ground ray misses");
  assert(groundCoverageForScreenRect(skewedScene, rect).groundBounds === null, "a missed ground quadrilateral is diagnostics only and never selection authority");

  const partialRect = { x0: projected.screenPolygon[0].x - 1, y0: projected.screenPolygon[0].y - 1, x1: projected.screenPolygon[0].x + 1, y1: projected.screenPolygon[0].y + 1 };
  assert(selectionProxiesInScreenRect(skewedScene, partialRect).length === 1, "marquee intersects partially covered large oriented proxies");
}

{
  const ordered = scene([
    { id: 53, owner: 1, kind: KIND.RIFLEMAN, x: 80, y: 80 },
    { id: 51, owner: 1, kind: KIND.RIFLEMAN, x: 30, y: 30 },
    { id: 52, owner: 1, kind: KIND.RIFLEMAN, x: 30, y: 30 },
  ]);
  const ids = selectionProxiesInScreenRect(ordered, { x0: 0, y0: 0, x1: 120, y1: 120 }, { anchor: { x: 0, y: 0 } })
    .map((proxy) => proxy.id);
  assert(ids.join(",") === "51,52,53", "marquee ids order by projected distance from drag start then stable id");
  assert(proxyIntersectsViewport(ordered, ordered.proxies[0]), "viewport admission uses projected proxy containment");
}

{
  const moving = { id: 60, owner: 1, kind: KIND.RIFLEMAN, x: 60, y: 60 };
  const oldScene = scene([moving], orthographic(), { frameId: 7 });
  const input = Object.create(Input.prototype);
  input.state = { map: { width: 32, height: 32, tileSize: 32 }, playerId: 1 };
  assert(input.publishSelectionScene(oldScene), "Input accepts SelectionSceneV1 publication");
  moving.x = 180;
  moving.y = 120;
  const futureScene = scene([moving], orthographic({ x: 100, y: 0 }), { frameId: 8 });
  assert(input._entityAtScreen({ x: 60, y: 60 })?.id === 60, "moving entity picking stays on the last presented proxy pose");
  assert(input._entityAtScreen({ x: 80, y: 120 }) === null, "an unpresented camera/entity pose cannot be targeted early");
  assert(!input.publishSelectionScene({ version: 2 }), "invalid scene publication is rejected");
  assert(input.selectionScene === oldScene, "failed publication preserves the prior presented scene");
  input.publishSelectionScene(futureScene);
  assert(input._entityAtScreen({ x: 80, y: 120 })?.id === 60, "new proxy/camera data becomes targetable only after publication");

  const admitted = admitSelectionIds({
    entityById: () => null,
    isOwnOwner: (owner) => owner === 1,
    spectator: false,
  }, [moving.id], { entityById: (id) => id === moving.id ? futureScene.proxies[0].interaction : null });
  assert(admitted.ids.join(",") === String(moving.id), "selection budget admission resolves candidates from the presented scene rather than fresh mutable state");
}

{
  const projection = fakePerspective();
  const input = Object.create(Input.prototype);
  input.state = { map: { width: 32, height: 32, tileSize: 32 } };
  input.selectionScene = scene([], projection);
  assert(input._groundAtScreen(200, 20) === null, "ground interaction is nullable above the fake perspective horizon");
  const hit = input._groundAtScreen(200, 102);
  assertApprox(hit.y, 100, 1e-6, "valid ground interaction returns finite authoritative world pixels");
  assert(input._groundAtScreen(200, 20) === null, "a later ground miss never reuses the prior hit");
}

{
  const unit = { id: 65, owner: 1, kind: KIND.RIFLEMAN, x: 100, y: 100 };
  const commands = [];
  const input = Object.create(Input.prototype);
  input.state = {
    playerId: 1,
    map: { width: 32, height: 32, tileSize: 32, terrain: new Array(1024).fill(0) },
    selection: new Set([unit.id]),
    selectedEntities: () => [unit],
    isOwnOwner: (owner) => owner === 1,
  };
  input.selectionScene = scene([unit], fakePerspective({ groundAtScreen: false }));
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginCommandTarget("move");
  input.commandIssuer = { issueCommand(command) { commands.push(command); } };
  input._addCommandFeedback = () => {};
  assert(input._issueTargetedCommand({ x: 200, y: 20 }) === false, "armed point command remains unissued on a ground miss");
  assert(commands.length === 0, "ground misses cannot emit commands with invalid or cached coordinates");
  input.clientIntent.endCommandTarget();
  input._onRightClick({ x: 200, y: 20 });
  assert(commands.length === 0, "ordinary ground right-click is a no-op when the presented projection misses");
  input.clientIntent.beginPlacement(KIND.BARRACKS);
  input.clientIntent.updatePlacement(5, 5, true);
  input.mouse = { x: 200, y: 20 };
  input._refreshPlacement();
  assert(input.clientIntent.placement.valid === false, "placement invalidates its prior cached tile after a ground miss");
}

{
  const calls = [];
  const overlay = new ScreenOverlay((rect) => calls.push(rect));
  overlay.setMarquee({ x: 2, y: 3, w: 10, h: 12 });
  overlay.clearMarquee();
  overlay.destroy();
  assert(calls[0]?.w === 10 && calls.at(-1) === null, "backend-neutral marquee surface forwards draw and teardown lifecycle");
}

{
  let entities = [{ id: 70, owner: 1, kind: KIND.RIFLEMAN, x: 70, y: 70 }];
  let projection = orthographic();
  let failRender = false;
  const input = Object.create(Input.prototype);
  input.state = { map: { width: 32, height: 32, tileSize: 32 }, playerId: 1 };
  const match = {
    running: true,
    lastFrame: 0,
    frameProfiler: null,
    camera: {
      update() {},
      audioListener: () => null,
      projectionSnapshot: () => projection,
    },
    input,
    audio: null,
    minimap: { updateCommandTargetPreview() {}, render() {} },
    observerDiagnostics: null,
    advancePredictionVisual() {},
    state: {
      map: { width: 32, height: 32, tileSize: 32 },
      visibleTiles: [],
      spectator: false,
      playerId: 1,
      entitiesInterpolated: () => entities,
      selectedEntities: () => [],
    },
    fog: { update() {} },
    renderer: { render() { if (failRender) throw new Error("synthetic render failure"); } },
    clientIntent: null,
    visualProfile: null,
    hud: { update() {} },
  };
  runMatchCaptureFrame(match, 16);
  const presented = input.selectionScene;
  assert(presented?.frameId === 1, "successful frame publishes its detached SelectionScene after rendering");
  entities = [{ id: 70, owner: 1, kind: KIND.RIFLEMAN, x: 170, y: 70 }];
  projection = orthographic({ x: 100 });
  failRender = true;
  let threw = false;
  try {
    runMatchCaptureFrame(match, 32);
  } catch {
    threw = true;
  }
  assert(threw, "synthetic renderer failure reaches the capture caller");
  assert(input.selectionScene === presented, "render failure preserves the prior successfully presented SelectionScene");
}

console.log("selection_projection_contracts: ok");
