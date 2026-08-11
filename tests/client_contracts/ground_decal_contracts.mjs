// Authoritative ground-decal model and renderer contracts.

import { assert, assertApprox } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import {
  GROUND_DECAL_CLASS,
  GroundDecalBuffer,
  normalizeAuthoritativeGroundDecal,
  normalizeAuthoritativeTankTrail,
} from "../../client/src/state_ground_decals.js";
import { stampGroundDecal } from "../../client/src/renderer/decals.js";
import { EVENT, KIND } from "../../client/src/protocol.js";

const start = {
  playerId: 1,
  map: { width: 4, height: 4, tileSize: 32, terrain: new Array(16).fill(0), resources: [] },
  players: [
    { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    { id: 2, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
  ],
};

function authoritativeRecord(id, overrides = {}) {
  return {
    id,
    decalClass: GROUND_DECAL_CLASS.INFANTRY,
    sourceKind: KIND.RIFLEMAN,
    x: 32,
    y: 64,
    owner: 1,
    seed: id + 100,
    ...overrides,
  };
}

{
  const trail = normalizeAuthoritativeTankTrail({
    id: 9,
    poses: [[400, 800, 0], [432, 800, 4096]],
  });
  assert(trail?.decalClass === GROUND_DECAL_CLASS.TANK_TREADS && trail.poses.length === 2,
    "packed authoritative tank trails retain their checkpointed pose sequence");
  const buffer = new GroundDecalBuffer();
  const applied = buffer.applyAuthoritativeBatch({
    revision: 1,
    decals: [],
    tankTrails: [{ id: 9, poses: [[400, 800, 0], [432, 800, 4096]] }],
  });
  assert(applied.queued === 1 && buffer.consumePending()[0]?.decalClass === "tankTreads",
    "authoritative trail chunks enter the same durable presentation queue as decals");
  assert(!normalizeAuthoritativeTankTrail({ id: 10, poses: [[-1, 0, 0], [0, 0, 0]] }),
    "out-of-range packed trail poses are rejected");
}

{
  const decal = normalizeAuthoritativeGroundDecal(authoritativeRecord(10, {
    decalClass: GROUND_DECAL_CLASS.SCORCH,
    sourceKind: KIND.TANK,
    owner: 2,
    facing: 1.25,
    weaponFacing: -0.5,
  }), { players: start.players });
  assert(decal.kind === KIND.TANK && decal.decalClass === GROUND_DECAL_CLASS.SCORCH,
    "authoritative records preserve source kind and decal class");
  assert(decal.owner === 2 && decal.color === "#00ff00",
    "authoritative records resolve their projected owner color");
  assertApprox(decal.facing, 1.25, 0.00001, "authoritative records preserve unit facing");
  assertApprox(decal.weaponFacing, -0.5, 0.00001, "authoritative records preserve weapon facing");
  assert(decal.variant === (decal.seed % 4), "authoritative seed deterministically selects the atlas variant");
}

{
  const decal = normalizeAuthoritativeGroundDecal(authoritativeRecord(12, {
    decalClass: GROUND_DECAL_CLASS.BUILDING_SCORCH,
    sourceKind: KIND.BARRACKS,
    x: 160,
    y: 192,
  }), { players: start.players, tileSize: 32 });
  assert(decal.footprintWidth === 96 && decal.footprintHeight === 64,
    "building decals use the building's full rectangular footprint dimensions");

  const calls = [];
  const ctx = {
    save() { calls.push(["save"]); },
    restore() { calls.push(["restore"]); },
    fillRect(x, y, width, height) { calls.push(["fillRect", x, y, width, height]); },
    ellipse() { calls.push(["ellipse"]); },
    arc() { calls.push(["arc"]); },
  };
  assert(stampGroundDecal(ctx, decal, 4), "renderer stamps a building scorch decal");
  const scorchRects = calls.filter((call) => call[0] === "fillRect");
  assert(
    scorchRects.every((rect) => rect[1] >= 28 && rect[2] >= 40 && rect[1] + rect[3] <= 52 && rect[2] + rect[4] <= 56),
    "building scorch remains inside its rectangular footprint after downsampling",
  );
  assert(scorchRects.length >= 32, "building scorch includes softened soot and ash fragments");
  assert(!calls.some((call) => call[0] === "ellipse" || call[0] === "arc"),
    "building scorch decals remain rectangular");
}

{
  const mortar = normalizeAuthoritativeGroundDecal(authoritativeRecord(20, {
    decalClass: GROUND_DECAL_CLASS.MORTAR_BLAST,
    sourceKind: KIND.MORTAR_TEAM,
    owner: 0,
    radiusTiles: 1,
  }), { tileSize: 40 });
  const artillery = normalizeAuthoritativeGroundDecal(authoritativeRecord(21, {
    decalClass: GROUND_DECAL_CLASS.ARTILLERY_BLAST,
    sourceKind: KIND.ARTILLERY,
    owner: 0,
    radiusTiles: 3,
  }), { tileSize: 40 });
  assert(mortar.radiusWorld === 40, "mortar radius uses the map tile size");
  assert(artillery.radiusWorld === 120, "artillery radius uses the map tile size");
  const fallback = normalizeAuthoritativeGroundDecal(authoritativeRecord(22, {
    decalClass: GROUND_DECAL_CLASS.ARTILLERY_BLAST,
    sourceKind: KIND.ARTILLERY,
    owner: 0,
    radiusTiles: undefined,
  }), { tileSize: 40 });
  assert(fallback.radiusTiles === 2 && fallback.radiusWorld === 80,
    "artillery fallback mirrors the authoritative outer radius");
}

{
  const buffer = new GroundDecalBuffer();
  const first = authoritativeRecord(30);
  const applied = buffer.applyAuthoritativeBatch({ revision: 1, decals: [first, first] }, {
    players: start.players,
  });
  assert(applied.queued === 1 && buffer.pendingCount === 1,
    "authoritative batches deduplicate stable server ids");
  const reconciled = buffer.reconcileBatch();
  assert(reconciled.decals.length === 1 && reconciled.revision === 1,
    "frame reconciliation exposes one durable presentation batch");
  assert(buffer.reconcileBatch().decals === reconciled.decals,
    "a failed frame reuses its unacknowledged decal batch");
  buffer.applyAuthoritativeBatch({ revision: 2, decals: [authoritativeRecord(31)] }, {
    players: start.players,
  });
  assert(buffer.acknowledgeReconciled(reconciled.revision) === 1,
    "an exact receipt clears only its reconciled batch");
  const second = buffer.reconcileBatch();
  assert(second.revision > reconciled.revision && second.decals[0]?.id === 31,
    "records arriving while a receipt is pending advance in a later presentation batch");
  assert(buffer.acknowledgeReconciled(999) === 0,
    "a stale or future presentation receipt cannot clear pending decals");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{ id: 50, owner: 2, kind: KIND.SCOUT_CAR, x: 96, y: 96, facing: 2.2, hp: 10, maxHp: 100 }],
    events: [],
  });
  state.applySnapshot({
    tick: 2,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: [{ e: EVENT.DEATH, id: 50, x: 96, y: 96, kind: KIND.SCOUT_CAR }],
  });
  assert(state.consumePendingGroundDecals().length === 0,
    "transient death events no longer create permanent decals");
  state.applyAuthoritativeGroundDecals({
    revision: 1,
    decals: [authoritativeRecord(500, {
      decalClass: GROUND_DECAL_CLASS.SCORCH,
      sourceKind: KIND.SCOUT_CAR,
      x: 96,
      y: 96,
      owner: 2,
      facing: 2.2,
    })],
  });
  const decals = state.consumePendingGroundDecals();
  assert(decals.length === 1 && decals[0].owner === 2,
    "GameState queues fog-scoped authoritative records");
  assertApprox(decals[0].facing, 2.2, 0.00001, "GameState preserves server-authored facing");
}

{
  const state = new GameState(start);
  state.applyAuthoritativeGroundDecals({
    revision: 2,
    decals: [
      authoritativeRecord(700, {
        decalClass: GROUND_DECAL_CLASS.MORTAR_BLAST,
        sourceKind: KIND.MORTAR_TEAM,
        owner: 0,
        radiusTiles: 1,
      }),
      authoritativeRecord(701, {
        decalClass: GROUND_DECAL_CLASS.ARTILLERY_BLAST,
        sourceKind: KIND.ARTILLERY,
        owner: 0,
        radiusTiles: 2,
      }),
    ],
  });
  const decals = state.consumePendingGroundDecals();
  assert(decals[0].radiusWorld === 32 && decals[1].radiusWorld === 64,
    "GameState supplies its map tile size for authoritative impact marks");
  state.applyAuthoritativeGroundDecals({ revision: 3, decals: [authoritativeRecord(700)] });
  assert(state.consumePendingGroundDecals().length === 0,
    "overlapping authoritative deltas do not stamp a stable id twice");

  const gap = state.groundDecals.applySnapshotDelta(
    {
      afterRevision: 8,
      revision: 10,
      decals: [authoritativeRecord(702)],
    },
    { players: state.players, tileSize: state.map.tileSize },
  );
  assert(gap.accepted && !gap.complete && state.groundDecals.authoritativeRevision === 3,
    "a snapshot range gap does not advance the complete authoritative cursor");
  assert(state.consumePendingGroundDecals().length === 1,
    "fog-entitled rows from a gapped snapshot still appear immediately");
  state.applyAuthoritativeGroundDecals({
    revision: 10,
    decals: [authoritativeRecord(700), authoritativeRecord(701), authoritativeRecord(702)],
  });
  assert(state.consumePendingGroundDecals().length === 0,
    "repairing a gapped range does not repaint a fast-path row already retained by stable id");
}
