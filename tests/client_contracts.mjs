// tests/client_contracts.mjs
// Lightweight dependency-free checks that the client modules export the expected
// constructors and pure methods documented in DESIGN.md §4.1.
//
// This does NOT spin up a browser or a server. Modules that require DOM / Pixi
// (Renderer, Input, HUD, Minimap, Lobby) are not instantiated here.

import { Net } from "../client/src/net.js";
import { GameState } from "../client/src/state.js";
import { Camera } from "../client/src/camera.js";
import { Fog } from "../client/src/fog.js";
import { MINING_CC_RANGE_TILES, STATS } from "../client/src/config.js";
import { formatTankOilUsed, playerHasCompletedKind } from "../client/src/hud.js";
import { Audio } from "../client/src/audio.js";
import { machineGunnerHasAudibleTarget } from "../client/src/combat_audio.js";
import {
  COMPACT_SNAPSHOT_VERSION,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  NOTICE_SEVERITY,
  SETUP,
  SETUP_CODE,
  STATE,
  STATE_CODE,
  TERRAIN,
  decodeServerMessage,
} from "../client/src/protocol.js";
import { Input, footprintValidAgainstEntities } from "../client/src/input.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function assertApprox(actual, expected, epsilon, msg) {
  assert(
    Math.abs(actual - expected) <= epsilon,
    `${msg}: expected ${expected}, got ${actual}`,
  );
}

function assertThrows(fn, msg) {
  let threw = false;
  try {
    fn();
  } catch (err) {
    threw = true;
  }
  assert(threw, msg);
}

function assertHasMethod(obj, name, msgPrefix = "") {
  assert(
    typeof obj[name] === "function",
    `${msgPrefix || "Object"} missing method "${name}"`,
  );
}

function assertHasGetter(obj, name, msgPrefix = "") {
  const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(obj) || obj, name);
  assert(
    d && typeof d.get === "function",
    `${msgPrefix || "Object"} missing getter "${name}"`,
  );
}

function fakeAudioParam(value = 1) {
  return {
    value,
    cancelScheduledValues() {},
    setValueAtTime(v) { this.value = v; },
    linearRampToValueAtTime(v) { this.value = v; },
  };
}

class FakeAudioNode {
  connect() { return this; }
  disconnect() {}
}

class FakeBufferSource extends FakeAudioNode {
  constructor() {
    super();
    this.playbackRate = fakeAudioParam(1);
    this.buffer = null;
    this.onended = null;
    this.started = false;
    this.stopped = false;
  }
  start() {
    this.started = true;
  }
  stop() {
    this.stopped = true;
    if (this.onended) this.onended();
  }
}

function fakeGain() {
  const node = new FakeAudioNode();
  node.gain = fakeAudioParam(1);
  return node;
}

function fakeAudioContext() {
  return {
    currentTime: 0,
    createBufferSource() { return new FakeBufferSource(); },
    createStereoPanner() {
      const node = new FakeAudioNode();
      node.pan = fakeAudioParam(0);
      return node;
    },
    createBiquadFilter() {
      const node = new FakeAudioNode();
      node.type = "";
      node.frequency = fakeAudioParam(0);
      return node;
    },
    createGain: fakeGain,
    close() {},
  };
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------
{
  const decoded = decodeServerMessage({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [42, 100, 25, 3, 10],
    e: [
      [
        1,
        1,
        KIND_CODE[KIND.WORKER],
        10,
        20,
        40,
        40,
        STATE_CODE[STATE.GATHER],
        1.5,
        1.75,
        null,
        null,
        null,
        null,
        200,
        9,
      ],
      [
        2,
        1,
        KIND_CODE[KIND.MACHINE_GUNNER],
        30,
        40,
        55,
        55,
        STATE_CODE[STATE.ATTACK],
        null,
        0.3,
        null,
        null,
        null,
        null,
        null,
        7,
        SETUP_CODE[SETUP.DEPLOYED],
      ],
      [
        3,
        1,
        KIND_CODE[KIND.CITY_CENTRE],
        100,
        120,
        450,
        500,
        STATE_CODE[STATE.TRAIN],
        null,
        null,
        KIND_CODE[KIND.WORKER],
        0.25,
        2,
        0.75,
      ],
    ],
    r: [[200, 1498]],
    ev: [
      [EVENT_CODE[EVENT.ATTACK], 1, 7],
      [EVENT_CODE[EVENT.DEATH], 200, 64, 96, KIND_CODE[KIND.STEEL]],
      [EVENT_CODE[EVENT.BUILD], 3, KIND_CODE[KIND.CITY_CENTRE]],
      [EVENT_CODE[EVENT.NOTICE], "Not enough steel"],
      [EVENT_CODE[EVENT.NOTICE], "alert:under_attack", 3, 512, 768],
    ],
  });

  assert(decoded.t === "snapshot", "compact snapshot keeps the semantic tag");
  assert(decoded.tick === 42 && decoded.steel === 100 && decoded.supplyCap === 10, "compact scalars decode");
  assert(decoded.entities.length === 3, "compact entities decode");
  assert(decoded.entities[0].kind === KIND.WORKER, "entity kind code decodes");
  assert(decoded.entities[0].state === STATE.GATHER, "entity state code decodes");
  assert(decoded.entities[0].weaponFacing === 1.75, "entity optional weaponFacing decodes");
  assert(decoded.entities[0].latchedNode === 200, "entity optional latchedNode decodes");
  assert(decoded.entities[1].setupState === SETUP.DEPLOYED, "entity setupState code decodes");
  assert(decoded.entities[2].prodKind === KIND.WORKER, "entity prodKind code decodes");
  assert(decoded.entities[2].prodProgress === 0.25, "entity prodProgress decodes");
  assert(decoded.resourceDeltas[0].remaining === 1498, "resource deltas decode");
  assert(decoded.events[0].e === EVENT.ATTACK && decoded.events[0].to === 7, "attack event decodes");
  assert(decoded.events[1].kind === KIND.STEEL, "death event kind decodes");
  assert(decoded.events[3].msg === "Not enough steel", "notice event decodes");
  assert(decoded.events[3].severity === NOTICE_SEVERITY.INFO, "legacy notice defaults to info");
  assert(decoded.events[4].severity === NOTICE_SEVERITY.ALERT, "notice severity decodes");
  assert(decoded.events[4].x === 512 && decoded.events[4].y === 768, "notice position decodes");

  assertThrows(
    () => decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION, s: [1], e: [] }),
    "compact snapshot rejects malformed scalar count",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [[1, 1, 255, 0, 0, 1, 1, STATE_CODE[STATE.IDLE]]],
      }),
    "compact snapshot rejects unknown enum codes",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: new Array(20001),
      }),
    "compact snapshot enforces entity count bounds",
  );
}

// ---------------------------------------------------------------------------
// Net
// ---------------------------------------------------------------------------
{
  const net = new Net("ws://example.test/ws");
  assert(net instanceof Net, "Net constructor should return an instance");
  assertHasMethod(net, "connect", "Net");
  assertHasMethod(net, "on", "Net");
  assertHasMethod(net, "off", "Net");
  assertHasMethod(net, "join", "Net");
  assertHasMethod(net, "ready", "Net");
  assertHasMethod(net, "start", "Net");
  assertHasMethod(net, "giveUp", "Net");
  assertHasMethod(net, "command", "Net");
  assertHasMethod(net, "ping", "Net");
  assertHasGetter(net, "playerId", "Net");
  assert(net.playerId === null, "Net.playerId should be null before welcome");
  assertHasMethod(net, "addAi", "Net");
  assertHasMethod(net, "removeAi", "Net");
  assertHasMethod(net, "setQuickstart", "Net");
  assertHasMethod(net, "setReplaySpeed", "Net");
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
{
  assert(MINING_CC_RANGE_TILES === 7, "client mirrors the server mining City Centre range");
  assert(STATS[KIND.CITY_CENTRE].cost.steel === 200, "City Centre cost mirrors server");
  assert(
    Array.isArray(STATS[KIND.FACTORY].requires),
    "Factory should expose all server-side build prerequisites",
  );
  assert(
    Array.isArray(STATS[KIND.TRAINING_CENTRE].requires),
    "Training Centre should expose all server-side build prerequisites",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].requires.includes(KIND.CITY_CENTRE),
    "Training Centre should require a City Centre in the command card",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].requires.includes(KIND.BARRACKS),
    "Training Centre should require a Barracks in the command card",
  );
  assert(
    STATS[KIND.FACTORY].requires.includes(KIND.CITY_CENTRE),
    "Factory should require a City Centre in the command card",
  );
  assert(
    STATS[KIND.FACTORY].requires.includes(KIND.TRAINING_CENTRE),
    "Factory should require a Training Centre in the command card",
  );
  assert(
    STATS[KIND.FACTORY].trains[0] === KIND.SCOUT_CAR,
    "Factory should put Scout Car in the leftmost train slot",
  );
  assert(STATS[KIND.SCOUT_CAR].cost.steel === 125, "Scout Car steel cost mirrors server");
  assert(STATS[KIND.SCOUT_CAR].cost.oil === 75, "Scout Car oil cost mirrors server");
  assert(STATS[KIND.SCOUT_CAR].sight === 10, "Scout Car has the largest mobile sight radius");
  assert(STATS[KIND.SCOUT_CAR].body.length === 34, "Scout Car client body length mirrors server");
  assert(STATS[KIND.SCOUT_CAR].body.width === 18, "Scout Car client body width mirrors server");
  const playerId = 1;
  const underConstructionTrainingCentre = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: 0.5 },
  ];
  assert(
    !playerHasCompletedKind(underConstructionTrainingCentre, playerId, KIND.TRAINING_CENTRE),
    "Factory should not unlock while the Training Centre is still under construction",
  );
  const underConstructionBarracks = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.BARRACKS, buildProgress: 0.5 },
  ];
  assert(
    !playerHasCompletedKind(underConstructionBarracks, playerId, KIND.BARRACKS),
    "Training Centre should not unlock while the Barracks is still under construction",
  );
  const completedTrainingCentre = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: null },
  ];
  assert(
    playerHasCompletedKind(completedTrainingCentre, playerId, KIND.TRAINING_CENTRE),
    "Factory should unlock once the Training Centre is complete",
  );
  assert(formatTankOilUsed(0.04) === "0.0", "tank oil panel rounds tiny values to tenths");
  assert(formatTankOilUsed(9.94) === "9.9", "tank oil panel keeps tenths below ten oil");
  assert(formatTankOilUsed(10.4) === "10", "tank oil panel rounds whole values above ten oil");
  assert(formatTankOilUsed(-2) === "0.0", "tank oil panel clamps negative values");
  assert(formatTankOilUsed(Number.NaN) === "0.0", "tank oil panel tolerates missing oilUsed");
}

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------
{
  const start = {
    playerId: 1,
    tick: 0,
    map: {
      width: 4,
      height: 4,
      tileSize: 32,
      terrain: new Array(16).fill(0),
      resources: [
        { id: 200, kind: KIND.STEEL, x: 64, y: 96 },
        { id: 201, kind: KIND.OIL, x: 96, y: 96 },
      ],
    },
    players: [
      { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    ],
  };
  const state = new GameState(start);
  assert(state instanceof GameState, "GameState constructor should return an instance");
  assert(state.playerId === 1, "GameState.playerId");
  assert(state.startInfo === start, "GameState.startInfo");
  assert(state.map.width === 4, "GameState.map");
  assert(state.map.resources.length === 2, "GameState keeps start payload resources");
  assert(state.resourceById.get(200).kind === KIND.STEEL, "GameState indexes resources by id");
  assert(state.resourceById.get(200).remaining === 1500, "steel defaults to full known amount");
  assert(state.resourceById.get(201).remaining === 5000, "oil defaults to full known amount");
  assert(Array.isArray(state.players), "GameState.players");
  assertHasMethod(state, "applySnapshot", "GameState");
  assertHasMethod(state, "entitiesInterpolated", "GameState");
  assertHasGetter(state, "prevRecvTime", "GameState");
  assertHasGetter(state, "currRecvTime", "GameState");
  assert(state.prevRecvTime === null, "prevRecvTime null before snapshots");
  assert(state.currRecvTime === null, "currRecvTime null before snapshots");
  assert(state.resources !== undefined, "GameState.resources");
  assert(Array.isArray(state.events), "GameState.events");
  assert(state.resourceMiningPreview === null, "GameState.resourceMiningPreview initially null");
  assertHasMethod(state, "updateResourceMiningPreview", "GameState");
  assert(state.selection instanceof Set, "GameState.selection");
  assertHasMethod(state, "setSelection", "GameState");
  assertHasMethod(state, "addToSelection", "GameState");
  assertHasMethod(state, "clearSelection", "GameState");
  assertHasMethod(state, "selectedEntities", "GameState");
  assertHasMethod(state, "entityById", "GameState");
  assert(state.placement === null, "GameState.placement initially null");
  assertHasMethod(state, "beginPlacement", "GameState");
  assertHasMethod(state, "updatePlacement", "GameState");
  assertHasMethod(state, "endPlacement", "GameState");

  // Snapshot buffering
  const t0 = performance.now();
  state.applySnapshot({
    tick: 0,
    steel: 10,
    oil: 5,
    supplyUsed: 2,
    supplyCap: 10,
    entities: [{ id: 1, owner: 1, kind: "worker", x: 10, y: 20, hp: 40, maxHp: 40, state: "idle" }],
    resourceDeltas: [{ id: 200, remaining: 1498 }],
    events: [],
  });
  assert(state.currRecvTime !== null, "currRecvTime set after first snapshot");
  assert(state.prevRecvTime === null, "prevRecvTime still null after one snapshot");
  assert(state.resources.steel === 10, "resources updated");
  assert(state.entityById(200).kind === KIND.STEEL, "static resources are available as local entities");
  assert(state.entityById(200).remaining === 1498, "resourceDeltas update known resource state");

  state.applySnapshot({
    tick: 1,
    steel: 12,
    oil: 5,
    supplyUsed: 2,
    supplyCap: 10,
    entities: [{ id: 1, owner: 1, kind: "worker", x: 15, y: 25, hp: 40, maxHp: 40, state: "idle" }],
    events: [{ e: "death", id: 200, x: 64, y: 96, kind: KIND.STEEL }],
  });
  assert(state.prevRecvTime !== null, "prevRecvTime set after two snapshots");
  assert(state.entityById(200).remaining === 0, "visible resource death tombstones known resource");
  assert(state.entityById(201).remaining === 5000, "untouched resources keep their last-known amount");
  state.updateResourceMiningPreview({
    resourceId: 200,
    resourceX: 64,
    resourceY: 96,
    ccId: 3,
    ccX: 48,
    ccY: 48,
    inRange: true,
  });
  assert(state.resourceMiningPreview?.resourceId === 200, "resource mining preview stores hover link");
  state.updateResourceMiningPreview(null);
  assert(state.resourceMiningPreview === null, "resource mining preview can be cleared");

  // Interpolation clamps alpha to [0,1]
  const entsNeg = state.entitiesInterpolated(-0.5);
  const entsOver = state.entitiesInterpolated(1.5);
  const entsMid = state.entitiesInterpolated(0.5);
  const midWorker = entsMid.find((e) => e.id === 1);
  assert(entsMid.length === 3 && midWorker, "entitiesInterpolated returns units and known resources");
  assert(midWorker.x >= 10 && midWorker.x <= 15, "interpolation works for moving units");
  assert(!("facing" in midWorker), "entitiesInterpolated does not add missing facing");

  const angleState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  angleState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "move", facing: 0 },
      {
        id: 11,
        owner: 1,
        kind: "tank",
        x: 0,
        y: 0,
        hp: 100,
        maxHp: 100,
        state: "move",
        facing: (170 * Math.PI) / 180,
        weaponFacing: (170 * Math.PI) / 180,
      },
      { id: 13, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle", facing: 0.5 },
      { id: 14, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle" },
    ],
    events: [],
  });
  angleState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 10, y: 20, hp: 40, maxHp: 40, state: "move", facing: Math.PI / 2 },
      {
        id: 11,
        owner: 1,
        kind: "tank",
        x: 0,
        y: 0,
        hp: 100,
        maxHp: 100,
        state: "move",
        facing: (-170 * Math.PI) / 180,
        weaponFacing: (-170 * Math.PI) / 180,
      },
      { id: 12, owner: 1, kind: "worker", x: 5, y: 5, hp: 40, maxHp: 40, state: "idle", facing: 1.25 },
      { id: 13, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle" },
      { id: 14, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle", facing: 0.75 },
    ],
    events: [],
  });
  const angleEnts = angleState.entitiesInterpolated(0.5);
  const quarterTurn = angleEnts.find((e) => e.id === 10);
  const wrapTurn = angleEnts.find((e) => e.id === 11);
  const newFacing = angleEnts.find((e) => e.id === 12);
  const missingCurrentFacing = angleEnts.find((e) => e.id === 13);
  const missingPriorFacing = angleEnts.find((e) => e.id === 14);
  assertApprox(quarterTurn.x, 5, 0.001, "x interpolation still works");
  assertApprox(quarterTurn.y, 10, 0.001, "y interpolation still works");
  assertApprox(quarterTurn.facing, Math.PI / 4, 0.001, "facing interpolates between snapshots");
  assertApprox(
    Math.abs(wrapTurn.facing),
    Math.PI,
    0.001,
    "facing interpolation uses the short path across angle wrap",
  );
  assertApprox(
    Math.abs(wrapTurn.weaponFacing),
    Math.PI,
    0.001,
    "weaponFacing interpolation uses the short path across angle wrap",
  );
  assertApprox(newFacing.facing, 1.25, 0.001, "missing prior entity keeps current facing");
  assert(!("facing" in missingCurrentFacing), "missing current facing does not add a field");
  assertApprox(missingPriorFacing.facing, 0.75, 0.001, "missing prior facing keeps current facing");

  // Selection resolves against current snapshot
  state.setSelection([1, 999]);
  const sel = state.selectedEntities();
  assert(sel.length === 1 && sel[0].id === 1, "selectedEntities drops stale ids");

  // Placement is local-only
  state.beginPlacement("barracks");
  assert(state.placement !== null, "placement started");
  state.updatePlacement(2, 3, true);
  assert(state.placement.tileX === 2, "updatePlacement sets tileX");
  assert(state.placement.tileY === 3, "updatePlacement sets tileY");
  assert(state.placement.valid === true, "updatePlacement sets valid");
  state.endPlacement();
  assert(state.placement === null, "endPlacement clears placement");

  const map = { width: 6, height: 6, tileSize: 32, terrain: new Array(36).fill(0) };
  const worker = { id: 7, owner: 1, kind: "worker", x: 80, y: 80 };
  const other = { id: 8, owner: 1, kind: "worker", x: 80, y: 80 };
  assert(
    footprintValidAgainstEntities([worker], new Set([7]), 1, 1, 2, 2, map) === true,
    "client_preview_allows_chosen_worker_body_inside_footprint",
  );
  assert(
    footprintValidAgainstEntities([other], new Set([7]), 1, 1, 2, 2, map) === false,
    "client_preview_rejects_other_unit_body_inside_footprint",
  );
  const tank = { id: 9, owner: 1, kind: KIND.TANK, x: 116, y: 64 };
  assert(
    footprintValidAgainstEntities([tank], new Set(), 1, 1, 2, 2, map) === false,
    "client preview should reject a tank body touching a footprint edge",
  );
  assert(STATS[KIND.TANK].body.length === 42, "tank client body length mirrors server");
  assert(STATS[KIND.TANK].body.width === 24, "tank client body width mirrors server");

  const input = Object.create(Input.prototype);
  input.state = {
    entitiesInterpolated: () => [worker, other],
  };
  input._selectedWorkerIds = () => [7, 8];
  assert(
    input._footprintValid(1, 1, 2, 2, map) === false,
    "preview should not ignore every selected worker",
  );
  input.state.entitiesInterpolated = () => [worker];
  assert(
    input._footprintValid(1, 1, 2, 2, map) === true,
    "preview should ignore the same first selected worker used for cmd.build",
  );

  const clickableTank = { id: 10, owner: 1, kind: KIND.TANK, x: 0, y: 0, facing: 0 };
  assert(
    input._worldPointHitsEntity(clickableTank, 21, 0, 32) === true,
    "tank hit testing should reach the long hull axis",
  );
  assert(
    input._worldPointHitsEntity(clickableTank, 0, 18, 32) === false,
    "tank hit testing should not use a stale circular side radius",
  );
}

// ---------------------------------------------------------------------------
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

  cam.setBounds(1000, 800, 800, 600);
  cam.centerOn(500, 400);
  assert(cam.x >= 0 && cam.y >= 0, "Camera clamped after centerOn");

  // Inverse check
  const world = { x: 123, y: 456 };
  const screen = cam.worldToScreen(world.x, world.y);
  const back = cam.screenToWorld(screen.x, screen.y);
  assert(Math.abs(back.x - world.x) < 0.001, "worldToScreen / screenToWorld inverse x");
  assert(Math.abs(back.y - world.y) < 0.001, "worldToScreen / screenToWorld inverse y");
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
  assert(fog.isVisible(2, 2) === true, "tile under entity should be visible");
  assert(fog.isExplored(2, 2) === true, "tile under entity should be explored");

  // After clearing visible, explored should persist
  fog.update([], 32);
  assert(fog.isVisible(2, 2) === false, "tile should no longer be visible");
  assert(fog.isExplored(2, 2) === true, "tile should still be explored");

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
// Audio
// ---------------------------------------------------------------------------
{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorLocalStorage = globalThis.localStorage;
  globalThis.window = {
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.document = {
    hidden: false,
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.localStorage = {
    getItem() { return null; },
    setItem() {},
  };

  const audio = new Audio();
  assertHasMethod(audio, "play", "Audio");
  assertHasMethod(audio, "playUI", "Audio");
  assertHasMethod(audio, "stopByKey", "Audio");
  assertHasMethod(audio, "preload", "Audio");
  assertHasMethod(audio, "setListener", "Audio");
  assertHasMethod(audio, "pickVariant", "Audio");
  audio.setListener(100, 100, 2, 800);
  assertApprox(audio.listener.refDist, 400, 0.001, "Audio listener refDist derives from zoom");

  const near = audio._computeSpatial(300, 100);
  assert(near !== null, "Audio spatial near emitter should play");
  assertApprox(near.gain, 1, 0.001, "Audio spatial gain is flat inside refDist");
  assertApprox(near.pan, 0.5, 0.001, "Audio spatial pan uses dx/refDist");

  const far = audio._computeSpatial(1300, 100);
  assert(far !== null, "Audio spatial max-distance edge should play");
  assertApprox(far.gain, 1 / 3, 0.001, "Audio spatial gain attenuates at maxDist");
  assertApprox(far.lpHz, 1200, 0.001, "Audio spatial lowpass reaches far cutoff");
  assert(audio._computeSpatial(1301, 100) === null, "Audio drops sounds beyond maxDist");

  const priorPerformance = globalThis.performance;
  let now = 0;
  globalThis.performance = { now: () => now };

  let stopped = 0;
  let disconnected = 0;
  const keyedVoice = (key) => ({
    key,
    node: {
      onended: () => {},
      stop() { stopped += 1; },
    },
    trail: [{ disconnect() { disconnected += 1; } }],
  });
  audio.voices = [keyedVoice("mg:1"), keyedVoice("other"), keyedVoice("mg:1")];
  assert(audio.stopByKey("mg:1") === 2, "Audio.stopByKey reports stopped voices");
  assert(stopped === 2, "Audio.stopByKey stops matching voices");
  assert(disconnected === 2, "Audio.stopByKey disconnects matching voice nodes");
  assert(
    audio.voices.length === 1 && audio.voices[0].key === "other",
    "Audio.stopByKey keeps unrelated voices active",
  );
  audio.voices = [];

  audio.ctx = fakeAudioContext();
  audio.master = fakeGain();
  audio.gains = {
    ui: fakeGain(),
    alert: fakeGain(),
    combat_self: fakeGain(),
    combat_other: fakeGain(),
    unit_voice: fakeGain(),
    ambient: fakeGain(),
  };
  for (const [cat, gain] of Object.entries(audio.gains)) {
    gain.gain.value = audio.getCategoryVolume(cat);
  }

  for (let i = 0; i < 200; i++) audio.buffers.set(`pool_${i}`, { duration: 0.1 });
  for (let i = 0; i < 120; i++) {
    assert(audio.play(`pool_${i}`, { category: "ambient" }), "ambient voice should enqueue");
    now += 1;
  }
  for (let i = 120; i < 200; i++) {
    assert(audio.play(`pool_${i}`, { category: "alert" }), "alert voice should enqueue or evict");
    now += 1;
  }
  assert(audio.voices.length <= 48, "Audio voice pool stays capped");
  assert(audio.voices.every((v) => v.category === "alert"), "Audio priority eviction keeps highest-priority voices");

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("notice_generic", { duration: 0.5 });
  now = 10_000;
  assert(
    audio.play("notice_generic", {
      category: "alert",
      alertId: "under_attack",
      alertX: 100,
      alertY: 100,
    }),
    "first under-attack alert plays",
  );
  assert(
    !audio.play("notice_generic", {
      category: "alert",
      alertId: "under_attack",
      alertX: 120,
      alertY: 140,
    }),
    "under-attack alert dedups within the same spatial bucket",
  );
  assert(
    audio.play("notice_generic", {
      category: "alert",
      alertId: "under_attack",
      alertX: 2000,
      alertY: 100,
    }),
    "under-attack alert plays in a different spatial bucket",
  );

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("notice_supply", { duration: 2.3 });
  now = 30_000;
  assert(audio.play("notice_supply", { category: "alert" }), "first spoken alert plays");
  now += 1500;
  assert(!audio.play("notice_supply", { category: "alert" }), "spoken alert cooldown honors buffer duration");
  now += 801;
  assert(audio.play("notice_supply", { category: "alert" }), "spoken alert plays after buffer-duration cooldown");

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("duck_alert", { duration: 0.1 });
  now = 40_000;
  const ambientBefore = audio.gains.ambient.gain.value;
  const combatBefore = audio.gains.combat_self.gain.value;
  assert(audio.play("duck_alert", { category: "alert" }), "ducking alert plays");
  assert(audio.gains.ambient.gain.value < ambientBefore, "alert ducks ambient bus");
  assert(audio.gains.combat_self.gain.value < combatBefore, "alert ducks combat bus");
  audio.voices.slice().forEach((v) => v.node.stop());
  assertApprox(audio.gains.ambient.gain.value, audio.getCategoryVolume("ambient"), 0.0001, "ambient bus restores");
  assertApprox(audio.gains.combat_self.gain.value, audio.getCategoryVolume("combat_self"), 0.0001, "combat bus restores");

  audio.destroy();
  globalThis.window = priorWindow;
  globalThis.document = priorDocument;
  globalThis.localStorage = priorLocalStorage;
  globalThis.performance = priorPerformance;
}

// ---------------------------------------------------------------------------
// Combat audio
// ---------------------------------------------------------------------------
{
  assert(
    machineGunnerHasAudibleTarget({
      kind: KIND.MACHINE_GUNNER,
      state: STATE.MOVE,
      setupState: SETUP.TEARING_DOWN,
      targetId: 7,
    }),
    "MG combat loop stays active while the machine gunner still has a target",
  );
  assert(
    !machineGunnerHasAudibleTarget({
      kind: KIND.MACHINE_GUNNER,
      state: STATE.ATTACK,
      setupState: SETUP.DEPLOYED,
    }),
    "MG combat loop stops once the machine gunner has no target",
  );
  assert(
    !machineGunnerHasAudibleTarget({
      kind: KIND.RIFLEMAN,
      targetId: 7,
    }),
    "non-MG targets do not hold the MG combat loop",
  );
}

console.log("✅ client_contracts.mjs: all contract assertions passed");
