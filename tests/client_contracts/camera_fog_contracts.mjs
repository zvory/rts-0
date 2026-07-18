// tests/client_contracts/camera_fog_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertApprox,
  assertHasMethod,
} from "./assertions.mjs";
import { Camera } from "../../client/src/camera.js";
import {
  restoreInitialCameraView,
  selectInitialCameraView,
} from "../../client/src/camera_view_selection.js";
import { CAMERA } from "../../client/src/config.js";
import { Fog } from "../../client/src/fog.js";
import { KIND, TERRAIN } from "../../client/src/protocol.js";

// Camera
// ---------------------------------------------------------------------------
{
  const cam = new Camera(800, 600);
  assert(cam instanceof Camera, "Camera constructor should return an instance");
  assert(typeof cam.x === "number", "Camera.x");
  assert(typeof cam.y === "number", "Camera.y");
  assert(typeof cam.zoom === "number", "Camera.zoom");
  assertHasMethod(cam, "update", "Camera");
  assertHasMethod(cam, "worldToScreen", "Camera");
  assertHasMethod(cam, "screenToWorld", "Camera");
  assertHasMethod(cam, "centerOn", "Camera");
  assertHasMethod(cam, "setBounds", "Camera");
  assertHasMethod(cam, "setView", "Camera");

  cam.setBounds(1000, 800, 800, 600);
  cam.centerOn(500, 400);
  assert(cam.x >= 0 && cam.y >= 0, "Camera clamped after centerOn");

  // Inverse check
  const world = { x: 123, y: 456 };
  const screen = cam.worldToScreen(world.x, world.y);
  const back = cam.screenToWorld(screen.x, screen.y);
  assert(Math.abs(back.x - world.x) < 0.001, "worldToScreen / screenToWorld inverse x");
  assert(Math.abs(back.y - world.y) < 0.001, "worldToScreen / screenToWorld inverse y");

  cam.setView({ x: 120, y: 140, zoom: 1.25 });
  assertApprox(cam.x, 120, 0.001, "Camera.setView restores x");
  assertApprox(cam.y, 140, 0.001, "Camera.setView restores y");
  assertApprox(cam.zoom, 1.25, 0.001, "Camera.setView restores zoom");

  cam.setView({ centerX: 500, centerY: 400, zoom: 1 });
  assertApprox(cam.x + cam.viewW / (2 * cam.zoom), 500, 0.001, "Camera.setView centers centerX");
  assertApprox(cam.y + cam.viewH / (2 * cam.zoom), 400, 0.001, "Camera.setView centers centerY");

  cam.setZoom(99, 400, 300);
  assertApprox(cam.zoom, CAMERA.maxZoom, 0.001, "Camera default zoom caps at the live-match limit");

  const labCam = new Camera(800, 600, { maxZoom: CAMERA.labMaxZoom });
  labCam.setBounds(1000, 800, 800, 600);
  labCam.setZoom(99, 400, 300);
  assertApprox(labCam.zoom, CAMERA.labMaxZoom, 0.001, "Camera accepts the higher lab zoom limit");
  labCam.setView({ x: 0, y: 0, zoom: 99 });
  assertApprox(labCam.zoom, CAMERA.labMaxZoom, 0.001, "Camera.setView respects the higher lab zoom limit");

  const editorCam = new Camera(800, 600, { minZoom: 0.05, maxZoom: 4 });
  editorCam.setZoom(0.1, 400, 300);
  assertApprox(editorCam.zoom, 0.1, 0.001, "Camera accepts a lower Map Editor zoom limit");
  editorCam.setView({ x: 0, y: 0, zoom: 0.001 });
  assertApprox(editorCam.zoom, 0.05, 0.001, "Camera.setView respects the per-session minimum zoom");

  const invalidLimitCam = new Camera(800, 600, { maxZoom: "invalid" });
  invalidLimitCam.setZoom(99, 400, 300);
  assertApprox(invalidLimitCam.zoom, CAMERA.maxZoom, 0.001, "Camera falls back to the live-match cap for invalid limits");

  const nullOptionsCam = new Camera(800, 600, null);
  nullOptionsCam.setZoom(99, 400, 300);
  assertApprox(nullOptionsCam.zoom, CAMERA.maxZoom, 0.001, "Camera tolerates null options");

  const boundedCam = new Camera(1920, 1080, { maxVisibleWorldPx: 3200 });
  boundedCam.setZoom(0.01);
  assertApprox(boundedCam.viewW / boundedCam.zoom, 3200, 0.001,
    "wide live-player viewports stop at the configured world span");
  assert(boundedCam.viewH / boundedCam.zoom <= 3200,
    "the shorter live-player viewport axis stays within the configured world span");
  boundedCam.setMapBounds(20_000, 20_000);
  boundedCam.focusAt({ x: 10_000, y: 10_000 });
  const focusBeforeResize = boundedCam.snapshot().focus;
  boundedCam.resize(800, 4000);
  boundedCam.setZoom(0.01);
  assertApprox(boundedCam.viewH / boundedCam.zoom, 3200, 0.001,
    "portrait live-player viewports apply the same cap to their taller axis");
  assert(boundedCam.viewW / boundedCam.zoom <= 3200,
    "portrait live-player viewports keep both axes within the configured world span");
  assertApprox(boundedCam.snapshot().focus.x, focusBeforeResize.x, 0.001,
    "orthographic cap changes preserve camera focus across viewport resizes");
  assertApprox(boundedCam.snapshot().focus.y, focusBeforeResize.y, 0.001,
    "orthographic cap changes preserve camera focus across viewport resizes");
}

{
  const snapshot = (x, y, framingScale = 1) => ({
    version: 1,
    focus: { x, y },
    framingScale,
    boundsPolicy: "mapOverscroll",
  });
  const currentView = snapshot(1, 2);
  const pendingView = snapshot(3, 4);
  const visualProfileView = snapshot(5, 6, 0.9);
  const scenarioView = { centerX: 70, centerY: 80 };
  assert(
    selectInitialCameraView({
      currentView,
      pendingView,
      visualProfileView,
      scenarioView,
    }) === currentView,
    "active match camera is preserved before launch defaults",
  );
  assert(
    selectInitialCameraView({ pendingView, visualProfileView, scenarioView }) === pendingView,
    "pending branch camera is preserved before launch defaults",
  );
  assert(
    selectInitialCameraView({ visualProfileView, scenarioView }) === visualProfileView,
    "visual profile camera overrides the scenario default camera",
  );
  assert(
    selectInitialCameraView({ scenarioView }) === scenarioView,
    "scenario camera is used as the lab launch default",
  );

  const camera = new Camera(100, 80, { minZoom: 0.01, maxZoom: 16 });
  camera.setMapBounds(1_000, 1_000);
  assert(
    restoreInitialCameraView(camera, scenarioView),
    "server-owned scenario camera center normalizes through the semantic snapshot edge",
  );
  assertApprox(camera.snapshot().focus.x, 70, 0.001, "scenario center restores semantic focus x");
  assertApprox(camera.snapshot().focus.y, 80, 0.001, "scenario center restores semantic focus y");

  assert(
    restoreInitialCameraView(camera, { centerX: 400, centerY: 500, zoom: 2 }),
    "legacy centered launch views still restore through the semantic camera edge",
  );
  assertApprox(camera.snapshot().focus.x, 400, 0.001, "legacy center restores semantic focus x");
  assertApprox(camera.snapshot().focus.y, 500, 0.001, "legacy center restores semantic focus y");
  assertApprox(camera.snapshot().framingScale, 2, 0.001, "legacy center preserves its requested zoom");

  assert(
    !restoreInitialCameraView(camera, { centerX: null, centerY: null }),
    "non-numeric centers do not coerce to a valid semantic launch view",
  );
}

// ---------------------------------------------------------------------------
// Fog
// ---------------------------------------------------------------------------
{
  const fog = new Fog(8, 8);
  assert(fog instanceof Fog, "Fog constructor should return an instance");
  assert(fog.width === 8 && fog.height === 8, "Fog dimensions");
  assert(fog.visibleGrid instanceof Uint8Array, "Fog.visibleGrid is Uint8Array");
  assert(fog.exploredGrid instanceof Uint8Array, "Fog.exploredGrid is Uint8Array");
  assert(fog.revision === 0, "Fog starts with a stable cache revision");
  assertHasMethod(fog, "update", "Fog");
  assertHasMethod(fog, "isVisible", "Fog");
  assertHasMethod(fog, "isExplored", "Fog");

  // Out of bounds returns false
  assert(fog.isVisible(-1, 0) === false, "isVisible out-of-bounds left");
  assert(fog.isVisible(0, -1) === false, "isVisible out-of-bounds top");
  assert(fog.isVisible(8, 0) === false, "isVisible out-of-bounds right");
  assert(fog.isVisible(0, 8) === false, "isVisible out-of-bounds bottom");
  assert(fog.isExplored(-1, 0) === false, "isExplored out-of-bounds");

  // Visibility accumulation
  fog.update(
    [{ kind: "worker", x: 64, y: 64 }], // center of tile (2,2) at ts=32
    32,
  );
  const revisionAfterReveal = fog.revision;
  assert(revisionAfterReveal > 0, "Fog revision increments when visibility changes");
  assert(fog.isVisible(2, 2) === true, "tile under entity should be visible");
  assert(fog.isExplored(2, 2) === true, "tile under entity should be explored");
  fog.update(
    [{ kind: "worker", x: 64, y: 64 }],
    32,
  );
  assert(fog.revision === revisionAfterReveal, "Fog revision stays stable for identical visibility");

  // After clearing visible, explored should persist
  fog.update([], 32);
  assert(fog.revision > revisionAfterReveal, "Fog revision increments when current visibility clears");
  assert(fog.isVisible(2, 2) === false, "tile should no longer be visible");
  assert(fog.isExplored(2, 2) === true, "tile should still be explored");

  const serverFog = new Fog(2, 1);
  serverFog.update([], 32, new Uint8Array([1, 0]));
  const serverRevision = serverFog.revision;
  serverFog.update([], 32, new Uint8Array([1, 0]));
  assert(serverFog.revision === serverRevision, "server fog revisions are stable for repeated grids");
  serverFog.update([], 32, new Uint8Array([0, 1]));
  assert(serverFog.revision > serverRevision, "server fog revisions change for new grids");

  const perspectiveFog = new Fog(2, 1);
  perspectiveFog.update(
    [],
    32,
    new Uint8Array([1, 1]),
    new Uint8Array([1, 1]),
  );
  perspectiveFog.update(
    [],
    32,
    new Uint8Array([1, 0]),
    new Uint8Array([1, 0]),
  );
  assert(
    perspectiveFog.isExplored(1, 0) === false,
    "authoritative exploration replaces omniscient history when perspective narrows",
  );

  const terrain = new Array(8 * 8).fill(TERRAIN.GRASS);
  terrain[2 * 8 + 3] = TERRAIN.ROCK;
  const blockedFog = new Fog(8, 8, terrain);
  blockedFog.update(
    [{ kind: "worker", x: 48, y: 80 }], // center of tile (1,2)
    32,
  );
  assert(blockedFog.isVisible(3, 2) === true, "stone tile itself should be visible");
  assert(blockedFog.isVisible(4, 2) === false, "stone should block fog behind it");

  const barracksFog = new Fog(8, 8);
  barracksFog.update(
    [{ kind: KIND.BARRACKS, x: 112, y: 112 }], // center of tile (3,3) at ts=32
    32,
  );
  for (let ty = 2; ty <= 3; ty++) {
    for (let tx = 2; tx <= 4; tx++) {
      assert(
        barracksFog.isVisible(tx, ty) === true,
        `barracks footprint tile (${tx},${ty})`,
      );
    }
  }
  for (let ty = 1; ty <= 4; ty++) {
    for (let tx = 1; tx <= 5; tx++) {
      assert(
        barracksFog.isVisible(tx, ty) === true,
        `barracks perimeter tile (${tx},${ty})`,
      );
    }
  }
  assert(
    barracksFog.isVisible(0, 1) === false,
    "barracks sight should stop beyond west perimeter",
  );
  assert(
    barracksFog.isVisible(6, 4) === false,
    "barracks sight should stop beyond east perimeter",
  );

  const cachedFog = new Fog(8, 8, terrain);
  let cachedRayCalls = 0;
  const cachedRayClear = cachedFog._rayClear.bind(cachedFog);
  cachedFog._rayClear = (...args) => {
    cachedRayCalls += 1;
    return cachedRayClear(...args);
  };
  const stationarySource = [{ id: 7, kind: "worker", x: 48, y: 80 }];
  cachedFog.update(stationarySource, 32);
  const firstRayCalls = cachedRayCalls;
  cachedFog.update(stationarySource, 32);
  const secondRayCalls = cachedRayCalls;
  cachedFog.update(stationarySource, 32);
  assert(firstRayCalls > 0, "local fog computes the first stationary visibility stamp");
  assert(secondRayCalls > firstRayCalls, "local fog verifies an unchanged source before caching");
  assert(cachedRayCalls === secondRayCalls, "an exact stationary visibility stamp reuses no stale rays");

  const referenceFog = new Fog(24, 24, new Array(24 * 24).fill(TERRAIN.GRASS));
  const memoizedFog = new Fog(24, 24, new Array(24 * 24).fill(TERRAIN.GRASS));
  for (let frame = 0; frame < 60; frame++) {
    if (frame === 25) {
      const nextTerrain = new Array(24 * 24).fill(TERRAIN.GRASS);
      for (let y = 3; y < 22; y += 4) nextTerrain[y * 24 + (y * 7) % 23] = TERRAIN.ROCK;
      referenceFog.updateTerrain(nextTerrain);
      memoizedFog.updateTerrain(nextTerrain.slice());
    }
    const tileSize = frame < 40 ? 32 : 16;
    const sources = [
      { id: 1, kind: "worker", x: 176, y: 208 },
      {
        id: 2,
        kind: "rifleman",
        x: 80 + (frame % 9) * 5.25,
        y: 144 + (frame % 7) * 3.75,
      },
      {
        id: 3,
        kind: frame < 35 ? KIND.BARRACKS : "worker",
        x: 336,
        y: 304,
      },
    ];
    memoizedFog.update(sources, tileSize);
    referenceFog.update(sources.map(({ id: _id, ...source }) => source), tileSize);
    assert(
      memoizedFog.visibleGrid.every((value, index) => value === referenceFog.visibleGrid[index]),
      `memoized fog preserves current visibility on frame ${frame}`,
    );
    assert(
      memoizedFog.exploredGrid.every((value, index) => value === referenceFog.exploredGrid[index]),
      `memoized fog preserves explored visibility on frame ${frame}`,
    );
    assert(
      memoizedFog.revision === referenceFog.revision
        && memoizedFog.visibleRevision === referenceFog.visibleRevision
        && memoizedFog.exploredRevision === referenceFog.exploredRevision,
      `memoized fog preserves revision semantics on frame ${frame}`,
    );
  }
}

// ---------------------------------------------------------------------------
