// tests/client_contracts/camera_fog_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertApprox,
  assertHasMethod,
} from "./assertions.mjs";
import { Camera } from "../../client/src/camera.js";
import { Fog } from "../../client/src/fog.js";
import { TERRAIN } from "../../client/src/protocol.js";

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

  const terrain = new Array(8 * 8).fill(TERRAIN.GRASS);
  terrain[2 * 8 + 3] = TERRAIN.ROCK;
  const blockedFog = new Fog(8, 8, terrain);
  blockedFog.update(
    [{ kind: "worker", x: 48, y: 80 }], // center of tile (1,2)
    32,
  );
  assert(blockedFog.isVisible(3, 2) === true, "stone tile itself should be visible");
  assert(blockedFog.isVisible(4, 2) === false, "stone should block fog behind it");
}

// ---------------------------------------------------------------------------
