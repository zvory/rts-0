import { KIND, STATE, UPGRADE } from "../client/src/protocol.js";
import { GameState } from "../client/src/state.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function approx(actual, expected, epsilon, msg) {
  assert(Math.abs(actual - expected) <= epsilon, `${msg}: expected ${expected}, got ${actual}`);
}

function building(extra = {}) {
  return {
    id: 10,
    owner: 1,
    kind: KIND.RESOURCE_DEPOT,
    state: STATE.IDLE,
    x: 64,
    y: 64,
    hp: 500,
    maxHp: 500,
    prodKind: KIND.WORKER,
    prodQueue: 1,
    prodProgress: 0.25,
    ...extra,
  };
}

function gameState({ playerId = 1, players = null } = {}) {
  return new GameState({
    playerId,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: players || [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
}

function snapshot(state, entities, tick = 1) {
  state.applySnapshot({
    tick,
    steel: 500,
    oil: 200,
    supplyUsed: 1,
    supplyCap: 10,
    upgrades: [],
    entities,
    events: [],
  });
}

function progressFrame(state, progress, { includePose = true } = {}) {
  state.applyPredictionDisplayOverlay({
    predictionFrame: { tick: state.tick, entities: [], progress },
    includePose,
  });
}

{
  const state = gameState();
  snapshot(state, [building()]);
  progressFrame(state, [{
    id: 10,
    kind: "production",
    identity: `unit:${KIND.WORKER}`,
    fraction: 0.4,
    hp: 0,
    state: STATE.CONSTRUCT,
    prodQueue: 99,
    futureSentinel: "must-not-compose",
  }]);
  const out = state.entityById(10);
  assert(out.prodProgress === 0.4 && out.progressPredicted === true, "production progress patch advances the display");
  assert(out.hp === 500 && out.state === STATE.IDLE && out.prodQueue === 1, "progress composition preserves authoritative gameplay fields");
}

{
  const state = gameState();
  snapshot(state, [building({ prodWaiting: true, prodProgress: 0 })]);
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.5 }]);
  assert(state.entityById(10).prodProgress === 0, "unpaid production rejects extrapolated progress");
  assert(state.entityById(10).progressPredicted !== true, "unpaid production is not marked predicted");
}

{
  const state = gameState();
  const research = building({
    kind: KIND.ENGINEERING_COMPLEX,
    prodKind: undefined,
    prodUpgrade: UPGRADE.TANK_UNLOCK,
    prodProgress: 0.4,
  });
  snapshot(state, [research]);
  progressFrame(state, [{ id: 10, kind: "production", identity: `upgrade:${UPGRADE.TANK_UNLOCK}`, fraction: 0.5 }]);
  assert(state.entityById(10).prodProgress === 0.5, "research identity admits its progress patch");
}

{
  const state = gameState();
  snapshot(state, [building({ prodProgress: 0.97 })]);
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.999 }]);
  approx(state.entityById(10).prodProgress, 0.98, 0.0001, "client defensively clamps predicted progress below completion");
}

{
  const state = gameState();
  const scaffold = building({
    kind: KIND.BARRACKS,
    state: STATE.CONSTRUCT,
    prodKind: undefined,
    prodQueue: undefined,
    prodProgress: undefined,
    buildProgress: 0.25,
    buildActive: true,
  });
  snapshot(state, [scaffold]);
  progressFrame(state, [{ id: 10, kind: "construction", identity: `build:${KIND.BARRACKS}`, fraction: 0.35 }]);
  const out = state.entityById(10);
  assert(out.buildProgress === 0.35, "active construction admits predicted progress");
  assert(out.buildProgressPredicted === true && out.progressPredicted === true, "construction prediction carries both diagnostic flags");
  state.setProgressPredictionPaused(true);
  assert(state.entityById(10).buildProgress === 0.35, "pausing retains the frozen progress patch");
  assert(state.progressPredictionDebug().paused === true, "progress diagnostics expose the paused display runtime");
  state.clearPredictionPose();
  assert(state.entityById(10).buildProgress === 0.35, "clearing pose does not clear frozen progress");
}

{
  const state = gameState();
  snapshot(state, [building()]);
  state.applyPredictionDisplayOverlay({
    predictionFrame: {
      tick: 1,
      entities: [],
      progress: [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.3 }],
    },
    diagnostics: {
      progressCorrectionCount: 2,
      progressLastCorrection: 0.04,
      progressMaxCorrection: 0.07,
      progressAverageCorrection: 0.05,
    },
  });
  const debug = state.progressPredictionDebug();
  assert(debug.activeBars === 1 && debug.productionBars === 1, "progress diagnostics count admitted sparse patches");
  assert(debug.correctionCount === 2 && debug.maxCorrection === 0.07, "progress diagnostics project WASM correction history");
}

{
  const state = gameState();
  const scaffold = building({
    kind: KIND.BARRACKS,
    state: STATE.CONSTRUCT,
    prodKind: undefined,
    prodQueue: undefined,
    prodProgress: undefined,
    buildProgress: 0.4,
    buildActive: false,
  });
  snapshot(state, [scaffold]);
  progressFrame(state, [{ id: 10, kind: "construction", identity: `build:${KIND.BARRACKS}`, fraction: 0.6 }]);
  assert(state.entityById(10).buildProgress === 0.4, "inactive construction rejects predicted progress");
}

{
  const state = gameState();
  snapshot(state, [building({ prodProgress: 0.5 })]);
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.6 }]);
  snapshot(state, [building({ prodProgress: 0.45 })], 2);
  assert(state.entityById(10).prodProgress === 0.45, "a new authoritative snapshot clears the old progress patch before lookups");
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.5 }]);
  assert(state.entityById(10).prodProgress === 0.5, "a reconciled patch applies to the new lower baseline");
}

{
  const state = gameState();
  snapshot(state, [building()]);
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.RIFLEMAN}`, fraction: 0.8 }]);
  assert(state.entityById(10).prodProgress === 0.25, "a mismatched production identity is rejected");
  snapshot(state, [building({ prodQueue: 0, prodKind: undefined, prodProgress: undefined })], 2);
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.8 }]);
  assert(state.entityById(10).progressPredicted !== true, "cancelled production cannot retain a stale patch");
}

{
  const state = gameState({
    players: [
      { id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 },
      { id: 2, name: "B", color: "#0f0", startTileX: 2, startTileY: 2 },
    ],
  });
  snapshot(state, [building(), building({ id: 11, owner: 2, x: 96 })]);
  progressFrame(state, [
    { id: 11, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.8 },
    { id: 999, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.8 },
  ]);
  assert(state.entityById(11).prodProgress === 0.25, "another player's building rejects local progress patches");
  assert(state.entityById(999) === undefined, "progress patches cannot create missing entities");
}

{
  const state = gameState();
  snapshot(state, [building()]);
  state.setSelection([10]);
  progressFrame(state, [{ id: 10, kind: "production", identity: `unit:${KIND.WORKER}`, fraction: 0.4 }]);
  const byId = state.entityById(10);
  const selected = state.selectedEntities()[0];
  const rendered = state.entitiesInterpolated(1).find((entity) => entity.id === 10);
  approx(selected.prodProgress, byId.prodProgress, 0.0001, "selectedEntities sees the same predicted progress");
  approx(rendered.prodProgress, byId.prodProgress, 0.0001, "rendered entities see the same predicted progress");
  state.setOptimisticCommandState({ production: [{ building: 10, unit: KIND.WORKER, optimisticQueue: 2 }] });
  const optimistic = state.entityById(10);
  assert(optimistic.optimisticProduction === true && optimistic.prodQueue === 2, "optimistic queue layers after progress");
  assert(optimistic.prodProgress === 0.4, "optimistic layering preserves active-item progress");
  assert(state.resources.steel === 500 && state.resources.oil === 200, "display progress cannot affect resources");
  assert(state.resources.supplyUsed === 1 && state.resources.supplyCap === 10, "display progress cannot affect supply");
}

console.log("progress_extrapolator: ok");
