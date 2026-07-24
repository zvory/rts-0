// tests/client_contracts/ground_decal_contracts.mjs
// Ground decal model and renderer contracts for client-only visible deaths and impacts.

import { assert, assertApprox } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import {
  GROUND_DECAL_CLASS,
  GroundDecalBuffer,
  groundDecalClassForKind,
  groundDecalClassForImpactEvent,
  normalizeGroundDecalEvent,
} from "../../client/src/state_ground_decals.js";
import { VisualEffectBuffers } from "../../client/src/state_visual_effects.js";
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

assert(groundDecalClassForKind(KIND.WORKER) === GROUND_DECAL_CLASS.INFANTRY, "workers leave infantry decals");
assert(groundDecalClassForKind(KIND.MACHINE_GUNNER) === GROUND_DECAL_CLASS.INFANTRY, "machine gunners leave infantry decals");
assert(groundDecalClassForKind(KIND.MORTAR_TEAM) === GROUND_DECAL_CLASS.INFANTRY, "mortar teams leave infantry decals");
assert(groundDecalClassForKind(KIND.TANK) === GROUND_DECAL_CLASS.SCORCH, "tanks leave scorch decals");
assert(groundDecalClassForKind(KIND.ANTI_TANK_GUN) === GROUND_DECAL_CLASS.SCORCH, "support guns leave scorch decals");
assert(groundDecalClassForKind(KIND.STEEL) === GROUND_DECAL_CLASS.NONE, "resources leave no decals");
assert(groundDecalClassForKind(KIND.CITY_CENTRE) === GROUND_DECAL_CLASS.BUILDING_SCORCH, "buildings leave footprint scorch decals");
assert(groundDecalClassForKind(KIND.TANK_TRAP) === GROUND_DECAL_CLASS.BUILDING_SCORCH, "small buildings leave footprint scorch decals");
assert(
  groundDecalClassForImpactEvent(EVENT.MORTAR_IMPACT) === GROUND_DECAL_CLASS.MORTAR_BLAST,
  "mortar impacts use the mortar starburst decal",
);
assert(
  groundDecalClassForImpactEvent(EVENT.ARTILLERY_IMPACT) === GROUND_DECAL_CLASS.ARTILLERY_BLAST,
  "artillery impacts use the artillery starburst decal",
);
assert(groundDecalClassForImpactEvent(EVENT.ATTACK) === GROUND_DECAL_CLASS.NONE, "ordinary attacks leave no blast decal");

{
  const prevById = new Map([
    [10, { id: 10, owner: 2, kind: KIND.TANK, x: 50, y: 60, facing: 1.25, weaponFacing: -0.5 }],
  ]);
  const decal = normalizeGroundDecalEvent(
    { e: EVENT.DEATH, id: 10, x: 64, y: 96, kind: KIND.TANK },
    { prevById, players: start.players, tick: 90 },
  );
  assert(decal.decalClass === GROUND_DECAL_CLASS.SCORCH, "normalizer classifies vehicle deaths");
  assert(decal.owner === 2, "normalizer recovers owner from the previous entity snapshot");
  assert(decal.color === "#00ff00", "normalizer resolves recovered owner color");
  assertApprox(decal.facing, 1.25, 0.00001, "normalizer prefers previous entity facing");
  assertApprox(decal.weaponFacing, -0.5, 0.00001, "normalizer preserves previous weapon facing");

  const repeat = normalizeGroundDecalEvent(
    { e: EVENT.DEATH, id: 10, x: 64, y: 96, kind: KIND.TANK },
    { prevById, players: start.players, tick: 90 },
  );
  assert(decal.seed === repeat.seed, "normalizer output seed is deterministic for stable death data");
  assert(decal.variant === repeat.variant, "normalizer output variant is deterministic for stable death data");
}

{
  const decal = normalizeGroundDecalEvent(
    { e: EVENT.DEATH, id: 12, x: 160, y: 192, kind: KIND.BARRACKS },
    { players: start.players, tick: 24, tileSize: 32 },
  );
  assert(decal.decalClass === GROUND_DECAL_CLASS.BUILDING_SCORCH, "normalizer classifies building deaths");
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
    "building scorch keeps every soot, burn, and ash mark inside the rectangular building footprint after downsampling",
  );
  assert(scorchRects.length >= 32,
    "building scorch softens its perimeter with scattered soot, edge bites, and ash fragments rather than straight fade bands");
  assert(!calls.some((call) => call[0] === "ellipse" || call[0] === "arc"),
    "building scorch decals are rectangular rather than oval");
}

{
  const curById = new Map([
    [11, { id: 11, owner: 1, kind: KIND.RIFLEMAN, x: 30, y: 30, weaponFacing: 0.75 }],
  ]);
  const decal = normalizeGroundDecalEvent(
    { e: EVENT.DEATH, id: 11, x: 30, y: 30, kind: KIND.RIFLEMAN },
    { curById, players: start.players, tick: 12 },
  );
  assert(decal.decalClass === GROUND_DECAL_CLASS.INFANTRY, "normalizer classifies infantry deaths");
  assert(decal.owner === 1, "normalizer falls back to current entity owner when previous is missing");
  assertApprox(decal.facing, 0.75, 0.00001, "normalizer falls back from facing to weaponFacing");
}

{
  const mortar = normalizeGroundDecalEvent(
    { e: EVENT.MORTAR_IMPACT, x: 160, y: 224, radiusTiles: 1.5 },
    { tick: 42, eventIndex: 3, tileSize: 40 },
  );
  const artillery = normalizeGroundDecalEvent(
    { e: EVENT.ARTILLERY_IMPACT, x: 288, y: 192, radiusTiles: 3 },
    { tick: 42, eventIndex: 4, tileSize: 40 },
  );
  assert(mortar.decalClass === GROUND_DECAL_CLASS.MORTAR_BLAST, "mortar impact normalizes to a mortar decal");
  assert(mortar.kind === KIND.MORTAR_TEAM, "mortar impact uses only its public event type for artwork selection");
  assert(mortar.radiusWorld === 60, "mortar impact converts its authoritative radius with the map tile size");
  assert(artillery.decalClass === GROUND_DECAL_CLASS.ARTILLERY_BLAST, "artillery impact normalizes to an artillery decal");
  assert(artillery.kind === KIND.ARTILLERY, "artillery impact uses only its public event type for artwork selection");
  assert(artillery.radiusWorld === 120, "artillery impact converts its authoritative radius with the map tile size");
  const artilleryFallback = normalizeGroundDecalEvent(
    { e: EVENT.ARTILLERY_IMPACT, x: 288, y: 192 },
    { tick: 42, eventIndex: 4, tileSize: 40 },
  );
  assert(
    artilleryFallback.radiusTiles === 2 && artilleryFallback.radiusWorld === 80,
    "artillery impact fallback mirrors the current authoritative outer radius",
  );
  const visualEffects = new VisualEffectBuffers();
  visualEffects.addArtilleryImpact({ x: 288, y: 192 }, 1000);
  assert(
    visualEffects.artilleryImpacts[0].radiusTiles === 2,
    "artillery visual-effect fallback mirrors the current authoritative outer radius",
  );
  assert(
    mortar.seed === normalizeGroundDecalEvent(
      { e: EVENT.MORTAR_IMPACT, x: 160, y: 224, radiusTiles: 1.5 },
      { tick: 42, eventIndex: 3, tileSize: 40 },
    ).seed,
    "impact seed stays deterministic for the received event",
  );
}

{
  const buffer = new GroundDecalBuffer();
  const events = [
    { e: EVENT.DEATH, id: 30, x: 80, y: 80, kind: KIND.WORKER },
    { e: EVENT.DEATH, id: 30, x: 80, y: 80, kind: KIND.WORKER },
    { e: EVENT.DEATH, id: 31, x: 96, y: 80, kind: KIND.STEEL },
  ];
  const queued = buffer.applySnapshotEvents(events, { players: start.players, tick: 1 });
  assert(queued === 1, "ground decal buffer queues only one unpainted decal for duplicate death ids");
  assert(buffer.pendingCount === 1, "ground decal buffer exposes pending queue count");
  assert(buffer.consumePending().length === 1, "ground decal buffer consumes the queued decal");
  assert(buffer.consumePending().length === 0, "ground decal buffer consume is one-shot");
  buffer.applySnapshotEvents([events[0]], { players: start.players, tick: 2 });
  assert(buffer.consumePending().length === 0, "painted death ids remain deduped after queue consumption");
}

{
  const buffer = new GroundDecalBuffer();
  buffer.applySnapshotEvents([
    { e: EVENT.DEATH, id: 32, x: 112, y: 80, kind: KIND.WORKER },
  ], { players: start.players, tick: 1 });
  const reconciled = buffer.reconcileBatch();
  assert(reconciled.decals.length === 1 && reconciled.revision === 1, "reconciliation exposes one revisioned decal batch to frame assembly");
  assert(buffer.reconcileBatch().decals === reconciled.decals, "a failed frame reuses its unacknowledged decal batch");
  assert(buffer.pendingCount === 1, "an unacknowledged batch remains accounted for as pending");
  assert(buffer.acknowledgeReconciled(2) === 0, "a stale or future receipt cannot clear the reconciled batch");
  assert(buffer.acknowledgeReconciled(reconciled.revision) === 1, "the exact durable receipt acknowledges the reconciled batch");
  assert(buffer.pendingCount === 0, "acknowledgement releases the reconciled decal batch");
}

{
  const buffer = new GroundDecalBuffer();
  buffer.applySnapshotEvents([
    { e: EVENT.DEATH, id: 33, x: 120, y: 80, kind: KIND.WORKER },
  ], { players: start.players, tick: 1 });
  const first = buffer.reconcileBatch();
  buffer.applySnapshotEvents([
    { e: EVENT.DEATH, id: 34, x: 128, y: 80, kind: KIND.WORKER },
  ], { players: start.players, tick: 2 });
  assert(buffer.acknowledgeReconciled(first.revision) === 1, "an exact receipt clears only its reconciled durable batch");
  const second = buffer.reconcileBatch();
  assert(second.revision > first.revision && second.decals[0]?.id === 34,
    "decals arriving while a receipt is pending advance in a later monotonic revision");
}

{
  const buffer = new GroundDecalBuffer();
  const events = [
    { e: EVENT.MORTAR_IMPACT, x: 128, y: 96, radiusTiles: 1.5 },
    { e: EVENT.ARTILLERY_IMPACT, x: 256, y: 192, radiusTiles: 3 },
  ];
  const queued = buffer.applySnapshotEvents(events, { tick: 77, tileSize: 32 });
  assert(queued === 2, "ground decal buffer queues both received blast impacts");
  const decals = buffer.consumePending();
  assert(decals[0].decalClass === GROUND_DECAL_CLASS.MORTAR_BLAST, "mortar impact preserves its decal type");
  assert(decals[1].decalClass === GROUND_DECAL_CLASS.ARTILLERY_BLAST, "artillery impact preserves its decal type");
  buffer.applySnapshotEvents(events, { tick: 77, tileSize: 32 });
  assert(buffer.consumePending().length === 0, "replayed snapshot impact events do not stamp twice");
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
  const decals = state.consumePendingGroundDecals();
  assert(decals.length === 1, "GameState.applySnapshot queues received death decals");
  assert(decals[0].owner === 2, "GameState decal queue recovers owner from the prior current snapshot");
  assertApprox(decals[0].facing, 2.2, 0.00001, "GameState decal queue recovers facing from the prior current snapshot");
  state.applySnapshot({
    tick: 3,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: [{ e: EVENT.DEATH, id: 50, x: 96, y: 96, kind: KIND.SCOUT_CAR }],
  });
  assert(state.consumePendingGroundDecals().length === 0, "GameState dedupes repeated death events by entity id");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{ id: 60, owner: 2, kind: KIND.BARRACKS, x: 80, y: 80, hp: 10, maxHp: 100 }],
    events: [],
  });
  state.applySnapshot({
    tick: 2,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: [{ e: EVENT.DEATH, id: 60, x: 80, y: 80, kind: KIND.BARRACKS }],
  });
  const decals = state.consumePendingGroundDecals();
  assert(decals.length === 1, "GameState queues received building death decals");
  assert(decals[0].footprintWidth === 96 && decals[0].footprintHeight === 64,
    "GameState supplies the map tile size for building-sized scorch decals");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: [],
  });
  const impacts = [
    { e: EVENT.MORTAR_IMPACT, x: 64, y: 96, radiusTiles: 1.5 },
    { e: EVENT.ARTILLERY_IMPACT, x: 128, y: 128, radiusTiles: 2 },
  ];
  state.applySnapshot({
    tick: 2,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: impacts,
  });
  const decals = state.consumePendingGroundDecals();
  assert(decals.length === 2, "GameState queues only the fog-filtered impact events it received");
  assert(decals[0].radiusWorld === 48, "GameState supplies its map tile size to mortar decal normalization");
  assert(decals[1].radiusWorld === 64, "GameState supplies its map tile size to artillery decal normalization");
  state.applySnapshot({
    tick: 2,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: impacts,
  });
  assert(state.consumePendingGroundDecals().length === 0, "GameState does not stamp duplicate impact snapshot events twice");
}
