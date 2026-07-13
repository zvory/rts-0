import { strict as assert } from "node:assert";

import { AUTO_SPECTATOR_MIN_ZOOM, AutoSpectatorDirector } from "../../client/src/auto_spectator.js";
import { Camera } from "../../client/src/camera.js";
import { EVENT, KIND } from "../../client/src/protocol.js";

function createHarness({ enabled = false, players = null } = {}) {
  const entities = new Map();
  const normalizedPlayers = players || [
    { id: 1, teamId: 1 },
    { id: 2, teamId: 2 },
  ];
  const state = {
    map: { width: 125, height: 94, tileSize: 32 },
    players: normalizedPlayers,
    entityById: (id) => entities.get(id),
    entitiesInterpolated: () => [...entities.values()],
    teamIdForPlayer: (id) => normalizedPlayers.find((player) => player.id === id)?.teamId ?? null,
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
    entitiesInterpolated: () => [],
  };
  const camera = new Camera(320, 240, { minZoom: AUTO_SPECTATOR_MIN_ZOOM, maxZoom: 2 });
  camera.setMapBounds(4032, 4032);
  const director = new AutoSpectatorDirector({ camera, state, enabled: true });
  director.decide(0);
  assert.equal(director.diagnostics().mode, "overview", "quiet scenes use gradual overview mode");
  assert.equal(director.diagnostics().moveKind, "zoom", "quiet scenes begin a smooth zoom");
  assert.equal(camera.snapshot().framingScale, 1, "quiet scenes never jump directly to a full-map view");
  director.update(1);
  assert(camera.snapshot().framingScale < 1, "quiet overview widens during its transition");
  assert(camera.snapshot().framingScale > 0.94, "quiet overview takes more than one second to widen");
  director.decide(30);
  director.update(3);
  assert(Math.abs(camera.snapshot().framingScale - 0.94) < 0.001,
    "frequent decisions do not compound an in-progress overview zoom");
  director.decide(60);
  director.update(4);
  assert(camera.snapshot().framingScale > 0.85,
    "successive quiet shots widen in small steps instead of revealing the full map");
}

{
  const { camera, director, entities } = createHarness();
  entities.set(1, { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 2800, y: 2100 });
  entities.set(2, { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 3000, y: 2200 });
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
  const standoff = camera.snapshot();
  assert.equal(director.diagnostics().mode, "contact", "expired combat falls back to nearby enemies");
  assert(Math.abs(standoff.focus.x - 2900) < 0.001, "camera stays on the opposing units after fire stops");
  assert(standoff.framingScale > 0.25, "a standoff never triggers a full-map zoom");
}

{
  const { camera, director, entities } = createHarness({ enabled: true });
  entities.set(1, { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 700, y: 900 });
  entities.set(2, { id: 2, owner: 2, kind: KIND.TANK, x: 1100, y: 900 });
  director.observeSnapshot({ tick: 1, events: [] });
  assert.equal(director.diagnostics().mode, "contact", "nearby opposing units form a likely contact");
  director.update(1);
  assert(Math.abs(camera.snapshot().focus.x - 900) < 0.001, "likely contact frames both sides");
  assert(camera.snapshot().framingScale > 0.25, "likely contact remains a local shot");
}

{
  const { director, entities } = createHarness({ enabled: true });
  entities.set(1, { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 400, y: 1000 });
  entities.set(2, { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 1600, y: 1000 });
  director.observeSnapshot({ tick: 0, events: [] });
  assert.equal(director.diagnostics().mode, "overview", "distant stationary enemies do not create an empty shot");
  entities.get(1).x = 500;
  entities.get(2).x = 1500;
  director.observeSnapshot({ tick: 30, events: [] });
  const contact = director.diagnostics().contact;
  assert.equal(director.diagnostics().mode, "contact", "intersecting movement vectors predict contact");
  assert(contact.predictedDistanceTiles < 1, "predicted contact uses closest future separation");
  assert(contact.etaTicks > 0 && contact.etaTicks <= 180, "predicted contact stays inside the six-second horizon");
}

{
  const players = [
    { id: 1, teamId: 1 },
    { id: 2, teamId: 1 },
    { id: 3, teamId: 2 },
  ];
  const { camera, director, entities } = createHarness({ enabled: true, players });
  entities.set(1, { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 600, y: 600 });
  entities.set(2, { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 620, y: 600 });
  entities.set(3, { id: 3, owner: 3, kind: KIND.RIFLEMAN, x: 900, y: 600 });
  director.observeSnapshot({ tick: 1, events: [] });
  director.update(1);
  assert(camera.snapshot().focus.x > 700, "same-team neighbors are ignored when choosing contact");
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

  director.moveTo([{ x: 860, y: 460 }, { x: 940, y: 540 }], 96);
  director.update(0.25);
  director.moveTo([{ x: 960, y: 460 }, { x: 1040, y: 540 }], 96);
  director.update(0.75);
  assert(!director.diagnostics().transitioning, "repeated decisions cannot extend a pan past one second");
  assert(Math.abs(camera.snapshot().focus.x - 1000) < 0.001, "a retargeted pan reaches the latest framing");

  director.moveTo([{ x: 3300, y: 2400 }, { x: 3500, y: 2600 }], 96);
  assert.equal(director.diagnostics().moveKind, "cut", "distant reframes cut immediately");
}

{
  const { camera, director } = createHarness({ enabled: true });
  director.decide(0);
  const beforeResizeScale = camera.snapshot().framingScale;
  camera.resize(600, 400);
  director.handleViewportChange();
  assert.equal(camera.snapshot().framingScale, beforeResizeScale, "viewport changes do not force an overview jump");
  assert(!director.diagnostics().transitioning, "viewport reframing does not leave a stale transition");
}

{
  const { director, entities } = createHarness({ enabled: true });
  entities.set(1, { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 600, y: 600 });
  entities.set(2, { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 700, y: 600 });
  director.observeSnapshot({ tick: 90, events: [{ e: EVENT.ATTACK, from: 1, to: 2 }] });
  director.observeSnapshot({ tick: 20, events: [] });
  const afterSeek = director.diagnostics();
  assert.equal(afterSeek.sampleCount, 0, "backward replay seeks discard future combat samples");
  assert.equal(afterSeek.latestTick, 20, "backward replay seeks adopt the rebuilt snapshot tick");
  assert.equal(afterSeek.trackedUnitCount, 2, "backward replay seeks rebuild motion tracking from the new tick");
}

console.log("  ✓ auto spectator contracts");
