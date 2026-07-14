import { ProgressExtrapolator, PROGRESS_EXTRAPOLATION_MAX } from "../client/src/progress_extrapolator.js";
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
    kind: KIND.CITY_CENTRE,
    state: STATE.IDLE,
    prodKind: KIND.WORKER,
    prodQueue: 1,
    prodProgress: 0.25,
    ...extra,
  };
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  ex.updateFromSnapshot([building()], 1000);
  const out = ex.apply(building(), 1500);
  assert(out.prodProgress > 0.25, "unit production advances after authoritative baseline");
  assert(out.progressPredicted === true, "unit production is marked as extrapolated");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const entity = building();
  ex.updateFromSnapshot([entity], 1000);
  ex.setPaused(true, 1500);
  const atPause = ex.apply(entity, 1500).prodProgress;
  const whilePaused = ex.apply(entity, 30_000).prodProgress;
  approx(whilePaused, atPause, 0.0001, "another player's live pause freezes production progress");
  assert(ex.diagnostics().paused === true, "progress diagnostics report the frozen display clock");

  ex.updateFromSnapshot([building({ prodProgress: 0.3 })], 30_000);
  ex.setPaused(false, 30_000);
  const atResume = ex.apply(building({ prodProgress: 0.3 }), 30_000).prodProgress;
  const afterResume = ex.apply(building({ prodProgress: 0.3 }), 30_500).prodProgress;
  approx(atResume, 0.3, 0.0001, "post-unpause snapshot rebases progress without paused-time catch-up");
  assert(afterResume > atResume, "progress resumes after the authoritative post-unpause baseline");
  assert(ex.diagnostics().paused === false, "progress diagnostics report the resumed display clock");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const entity = building({
    kind: KIND.RESEARCH_COMPLEX,
    prodKind: undefined,
    prodUpgrade: UPGRADE.TANK_UNLOCK,
    prodProgress: 0.4,
  });
  ex.updateFromSnapshot([entity], 0);
  const out = ex.apply(entity, 1000);
  assert(out.prodProgress > 0.4, "research advances after authoritative baseline");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const entity = building({ prodProgress: 0.97 });
  ex.updateFromSnapshot([entity], 0);
  const out = ex.apply(entity, 60_000);
  approx(out.prodProgress, PROGRESS_EXTRAPOLATION_MAX, 0.0001, "progress clamps below completion");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const scaffold = building({
    kind: KIND.BARRACKS,
    state: STATE.CONSTRUCT,
    prodKind: undefined,
    prodQueue: undefined,
    prodProgress: undefined,
    buildProgress: 0.25,
    buildActive: true,
  });
  ex.updateFromSnapshot([scaffold], 0);
  const out = ex.apply(scaffold, 500);
  assert(out.buildProgress > 0.25, "active construction advances after authoritative baseline");
  assert(out.buildProgressPredicted === true, "construction is marked as extrapolated separately");
  assert(out.progressPredicted === true, "construction participates in generic progress diagnostics");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const scaffold = building({
    kind: KIND.BARRACKS,
    state: STATE.CONSTRUCT,
    prodKind: undefined,
    prodQueue: undefined,
    prodProgress: undefined,
    buildProgress: 0.25,
    buildActive: true,
  });
  ex.updateFromSnapshot([scaffold], 0);
  ex.setPaused(true, 500);
  const atPause = ex.apply(scaffold, 500).buildProgress;
  approx(
    ex.apply(scaffold, 10_000).buildProgress,
    atPause,
    0.0001,
    "live pause freezes construction progress",
  );
  ex.setPaused(false, 10_000);
  assert(
    ex.apply(scaffold, 10_500).buildProgress > atPause,
    "construction progress resumes without counting paused wall time",
  );
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const scaffold = building({
    kind: KIND.BARRACKS,
    state: STATE.CONSTRUCT,
    prodKind: undefined,
    prodQueue: undefined,
    prodProgress: undefined,
    buildProgress: 0.97,
    buildActive: true,
  });
  ex.updateFromSnapshot([scaffold], 0);
  const out = ex.apply(scaffold, 60_000);
  approx(out.buildProgress, PROGRESS_EXTRAPOLATION_MAX, 0.0001, "construction progress clamps below completion");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const active = building({
    kind: KIND.BARRACKS,
    state: STATE.CONSTRUCT,
    prodKind: undefined,
    prodQueue: undefined,
    prodProgress: undefined,
    buildProgress: 0.4,
    buildActive: true,
  });
  ex.updateFromSnapshot([active], 0);
  const paused = { ...active, buildActive: false };
  ex.updateFromSnapshot([paused], 500);
  const out = ex.apply(paused, 1000);
  assert(out.buildProgressPredicted !== true, "construction extrapolation stops without active server signal");
  assert(ex.diagnostics().constructionBars === 0, "paused construction is not active");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  ex.updateFromSnapshot([building({ prodProgress: 0.5 })], 0);
  const predicted = ex.apply(building({ prodProgress: 0.5 }), 1000).prodProgress;
  ex.updateFromSnapshot([building({ prodProgress: 0.45 })], 1000);
  const corrected = ex.apply(building({ prodProgress: 0.45 }), 1000).prodProgress;
  assert(predicted > corrected, "lower authoritative correction resets display progress");
  assert(ex.diagnostics().correctionCount === 1, "correction is counted");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  ex.updateFromSnapshot([building()], 0);
  const changed = building({ prodKind: KIND.RIFLEMAN, kind: KIND.BARRACKS, prodProgress: 0.1 });
  ex.updateFromSnapshot([changed], 1000);
  assert(ex.apply(changed, 2000).prodProgress > 0.1, "new identity starts a fresh baseline");
  assert(ex.apply(building(), 2000).progressPredicted !== true, "old identity no longer extrapolates");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  ex.updateFromSnapshot([building()], 0);
  ex.updateFromSnapshot([building({ prodQueue: 0, prodKind: undefined, prodProgress: undefined })], 1000);
  const out = ex.apply(building({ prodQueue: 0, prodKind: undefined, prodProgress: undefined }), 2000);
  assert(out.progressPredicted !== true, "cancellation clears extrapolation");
  assert(ex.diagnostics().activeBars === 0, "cancelled production is not active");
}

{
  const ex = new ProgressExtrapolator({ playerId: 1 });
  const entity = building();
  const resources = { steel: 500, oil: 200, supplyUsed: 3, supplyCap: 10, upgrades: [] };
  ex.updateFromSnapshot([entity], 0);
  ex.apply(entity, 1000);
  assert(resources.steel === 500 && resources.oil === 200, "resources are not affected");
  assert(resources.supplyUsed === 3 && resources.supplyCap === 10, "supply is not affected");
  assert(resources.upgrades.length === 0, "upgrades are not affected");
}

{
  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
  state.applySnapshot({
    tick: 1,
    steel: 500,
    oil: 200,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [building({ x: 64, y: 64, hp: 500, maxHp: 500 })],
    events: [],
  });
  state.setSelection([10]);
  for (const baseline of state.progressExtrapolator.active.values()) baseline.recvTime -= 500;
  const byId = state.entityById(10);
  const selected = state.selectedEntities()[0];
  const rendered = state.entitiesInterpolated(1).find((entity) => entity.id === 10);
  assert(byId.prodProgress > 0.25, "entityById exposes extrapolated production progress");
  approx(selected.prodProgress, byId.prodProgress, 0.02, "selectedEntities sees the same display progress");
  approx(rendered.prodProgress, byId.prodProgress, 0.02, "entitiesInterpolated sees the same display progress");
  state.setOptimisticCommandState({ production: [{ building: 10, unit: KIND.WORKER, optimisticQueue: 2 }] });
  const optimistic = state.entityById(10);
  assert(optimistic.optimisticProduction === true, "optimistic train marker remains separate");
  assert(optimistic.prodProgress > 0.25, "optimistic queue layering does not suppress active progress");
  assert(state.resources.steel === 500 && state.resources.oil === 200, "GameState resources stay authoritative");
  assert(state.resources.supplyUsed === 1 && state.resources.supplyCap === 10, "GameState supply stays authoritative");
  assert(state.upgrades.length === 0, "GameState upgrades stay authoritative");
}

{
  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
  state.applySnapshot({
    tick: 1,
    steel: 500,
    oil: 200,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [building({
      id: 20,
      kind: KIND.BARRACKS,
      state: STATE.CONSTRUCT,
      prodKind: undefined,
      prodQueue: undefined,
      prodProgress: undefined,
      buildProgress: 0.3,
      buildActive: true,
      x: 96,
      y: 96,
      hp: 300,
      maxHp: 300,
    })],
    events: [],
  });
  for (const baseline of state.progressExtrapolator.active.values()) baseline.recvTime -= 500;
  const byId = state.entityById(20);
  const rendered = state.entitiesInterpolated(1).find((entity) => entity.id === 20);
  assert(byId.buildProgress > 0.3, "entityById exposes extrapolated construction progress");
  approx(rendered.buildProgress, byId.buildProgress, 0.02, "entitiesInterpolated sees construction display progress");
  assert(byId.buildProgressPredicted === true, "construction display prediction is marked");
  assert(state.resources.supplyCap === 10, "construction extrapolation does not change supply");
}

console.log("progress_extrapolator: ok");
