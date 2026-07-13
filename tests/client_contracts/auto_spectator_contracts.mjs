import { strict as assert } from "node:assert";

import { AUTO_SPECTATOR_MIN_ZOOM, AutoSpectatorDirector } from "../../client/src/auto_spectator.js";
import { Camera } from "../../client/src/camera.js";
import { EVENT } from "../../client/src/protocol.js";

function createHarness({ enabled = false } = {}) {
  const entities = new Map();
  const state = {
    map: { width: 125, height: 94, tileSize: 32 },
    entityById: (id) => entities.get(id),
  };
  const camera = new Camera(1000, 700, { minZoom: AUTO_SPECTATOR_MIN_ZOOM, maxZoom: 2 });
  camera.setMapBounds(4000, 3008);
  camera.focusAt({ x: 500, y: 500 });
  camera.setZoom(1);
  const director = new AutoSpectatorDirector({ camera, state, enabled });
  return { camera, director, entities };
}

{
  const state = {
    map: { width: 126, height: 126, tileSize: 32 },
    entityById: () => null,
  };
  const camera = new Camera(320, 240, { minZoom: AUTO_SPECTATOR_MIN_ZOOM, maxZoom: 2 });
  camera.setMapBounds(4032, 4032);
  const director = new AutoSpectatorDirector({ camera, state, enabled: true });
  director.decide(0);
  assert(
    camera.snapshot().framingScale <= (240 - 32) / 4032,
    "quiet auto spectator can fit the full standard map on the minimum supported viewport",
  );
}

{
  const { camera, director, entities } = createHarness();
  entities.set(1, { id: 1, x: 2800, y: 2100 });
  entities.set(2, { id: 2, x: 3000, y: 2200 });
  director.observeSnapshot({ tick: 1, events: [{ e: EVENT.ATTACK, from: 1, to: 2 }] });
  assert.equal(director.diagnostics().sampleCount, 1, "auto spectator records combat while disabled");
  director.setEnabled(true);
  assert.equal(director.diagnostics().moveKind, "cut", "enabling cuts to a distant active battle");
  assert(camera.snapshot().focus.x > 2500, "distant battle becomes the camera focus");

  const focusBeforeInterval = camera.snapshot().focus;
  director.observeSnapshot({
    tick: 15,
    events: [{ e: EVENT.DEATH, id: 3, x: 500, y: 500, kind: "rifleman" }],
  });
  assert.deepEqual(camera.snapshot().focus, focusBeforeInterval, "director does not reframe inside one decision second");

  director.observeSnapshot({ tick: 121, events: [] });
  const wholeMap = camera.snapshot();
  assert(Math.abs(wholeMap.focus.x - 2000) < 0.001, "quiet camera centers the full map horizontally");
  assert(Math.abs(wholeMap.focus.y - 1504) < 0.001, "quiet camera centers the full map vertically");
  assert(wholeMap.framingScale < 0.25, "quiet camera zooms out far enough to frame the whole map");
}

{
  const { camera, director } = createHarness({ enabled: true });
  director.moveTo([{ x: 760, y: 460 }, { x: 840, y: 540 }], 96);
  assert.equal(director.diagnostics().moveKind, "pan", "nearby reframes use an eased pan");
  const start = camera.snapshot();
  director.update(0.5);
  const middle = camera.snapshot();
  assert(middle.focus.x > start.focus.x && middle.focus.x < 800, "pan interpolates toward its target");
  director.update(0.5);
  assert(Math.abs(camera.snapshot().focus.x - 800) < 0.001, "pan reaches its target after one second");

  director.moveTo([{ x: 3300, y: 2400 }, { x: 3500, y: 2600 }], 96);
  assert.equal(director.diagnostics().moveKind, "cut", "distant reframes cut immediately");
}

{
  const { director, entities } = createHarness({ enabled: true });
  entities.set(1, { id: 1, x: 600, y: 600 });
  entities.set(2, { id: 2, x: 700, y: 600 });
  director.observeSnapshot({ tick: 90, events: [{ e: EVENT.ATTACK, from: 1, to: 2 }] });
  director.observeSnapshot({ tick: 20, events: [] });
  const afterSeek = director.diagnostics();
  assert.equal(afterSeek.sampleCount, 0, "backward replay seeks discard future combat samples");
  assert.equal(afterSeek.latestTick, 20, "backward replay seeks adopt the rebuilt snapshot tick");
}

console.log("  ✓ auto spectator contracts");
