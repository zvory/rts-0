// tests/client_contracts/weapon_feedback_contracts.mjs
// Weapon identity hints must affect visual feedback without making clients depend on them.

import { assert } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { EVENT, KIND, SETUP, STATE, WEAPON_KIND } from "../../client/src/protocol.js";
import {
  sampleWeaponRecoilCycle,
  weaponRecoilDurationMs,
} from "../../client/src/weapon_recoil_cycle.js";
import { liveUnitIconRigPoseFor } from "../../client/src/renderer/rigs/unit_icon_sources.js";

const start = {
  playerId: 1,
  tick: 0,
  map: {
    width: 4,
    height: 4,
    tileSize: 32,
    terrain: new Array(16).fill(0),
    resources: [],
  },
  players: [
    { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    { id: 2, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
  ],
};

{
  assert(
    weaponRecoilDurationMs(KIND.TANK, WEAPON_KIND.TANK_CANNON) === 650,
    "tank cannon recoil retains its authored 650 ms duration",
  );
  assert(
    weaponRecoilDurationMs(KIND.RIFLEMAN, WEAPON_KIND.ARTILLERY_GUN) === 980,
    "explicit weapon identity overrides the attacker's fallback recoil duration",
  );
  assert(
    weaponRecoilDurationMs(KIND.RIFLEMAN, "unknown_future_weapon") === 420,
    "unknown weapon identity falls back to the attacker's recoil duration",
  );
  assert(
    sampleWeaponRecoilCycle(KIND.TANK, 0, WEAPON_KIND.TANK_COAX).active === false,
    "tank coax feedback does not create a cannon recoil cycle",
  );

  const initial = sampleWeaponRecoilCycle(KIND.TANK, 0, WEAPON_KIND.TANK_CANNON);
  const kickBoundary = sampleWeaponRecoilCycle(KIND.TANK, 650 * 0.18, WEAPON_KIND.TANK_CANNON);
  const complete = sampleWeaponRecoilCycle(KIND.TANK, 650, WEAPON_KIND.TANK_CANNON);
  assert(initial.active && initial.phase === 0 && initial.progress === 1,
    "recoil cycle begins at full authored displacement");
  assert(
    kickBoundary.active && Math.abs(kickBoundary.progress - 0.88) < 1e-12 && kickBoundary.phase === 0.18,
    "recoil cycle preserves the cosine-settlement boundary after the held kick",
  );
  assert(
    complete.active && complete.phase === 1 && Math.abs(complete.progress) < 1e-12,
    "recoil cycle settles to rest at its inclusive duration boundary",
  );
  assert(
    sampleWeaponRecoilCycle(KIND.TANK, 650.001, WEAPON_KIND.TANK_CANNON).active === false,
    "recoil cycle expires immediately after its authored duration",
  );

  const tankRecoilPose = liveUnitIconRigPoseFor(KIND.TANK, { recoilProgress: 1, recoilPhase: 0 });
  assert(
    Math.abs(tankRecoilPose.parts["part.hull"].transform.x + 7.65) < 0.0001 &&
      Math.abs(tankRecoilPose.parts["part.barrel"].transform.scaleX - 0.728915663) < 0.0001 &&
      Math.abs(tankRecoilPose.parts["part.tank.flashCone"].alpha - 1) < 0.0001 &&
      Math.abs(tankRecoilPose.parts["part.tank.flashCore"].geometryScale.x - 3.4) < 0.0001 &&
      Math.abs(tankRecoilPose.parts["part.tank.flashGlow"].geometryScale.x - 3) < 0.0001,
    "animated Tank icons sample the live rig's body kick, barrel contraction, and muzzle flash bindings",
  );
}

{
  const renderClock = { now: () => 500_000 };
  const state = new GameState(start, { renderClock });
  state.applySnapshot({
    tick: 1, steel: 0, oil: 0, supplyUsed: 0, supplyCap: 10,
    entities: [
      { id: 1, owner: 1, kind: KIND.TANK, x: 100, y: 100, hp: 180, maxHp: 180, state: STATE.ATTACK },
      { id: 2, owner: 2, kind: KIND.TANK, x: 160, y: 100, hp: 180, maxHp: 180, state: STATE.IDLE },
    ],
    events: [{ e: EVENT.ATTACK, from: 1, to: 2, weaponKind: WEAPON_KIND.TANK_CANNON }],
  });
  assert(state.liveMuzzleFlashes(500_050).length === 1, "snapshot visual events use the injected render-clock timestamp");
  assert(state.weaponRecoil(1, KIND.TANK, 500_050) > 0, "recoil phase samples deterministically from injected visual time");
  assert(state.currRecvTime !== 500_000, "snapshot interpolation keeps its independent real receive clock");
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
    events: [{
      e: EVENT.ATTACK,
      from: 99,
      to: 99,
      weaponKind: WEAPON_KIND.ARTILLERY_GUN,
      reveal: {
        owner: 2,
        kind: KIND.ARTILLERY,
        x: 512,
        y: 544,
        facing: 0,
        weaponFacing: 0,
        setupState: SETUP.DEPLOYED,
      },
    }],
  });
  assert(state.entityById(99)?.shotReveal === true, "artillery weapon-hint reveal creates a fog shot reveal");
  assert(state.liveMuzzleFlashes(performance.now()).length === 0, "artillery weapon-hint self-reveal does not draw a tracer");
  assert(state.weaponRecoil(99, KIND.ARTILLERY, performance.now()) > 0, "artillery weapon-hint self-reveal recoils the gun");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 2,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 5, owner: 1, kind: KIND.TANK, x: 100, y: 100, hp: 180, maxHp: 180, state: STATE.ATTACK, weaponFacing: 0 },
      { id: 6, owner: 2, kind: KIND.TANK, x: 160, y: 100, hp: 180, maxHp: 180, state: STATE.IDLE },
    ],
    events: [{ e: EVENT.ATTACK, from: 5, to: 6, toPos: [160, 100], weaponKind: WEAPON_KIND.TANK_CANNON }],
  });
  assert(state.liveMuzzleFlashes(performance.now())[0]?.weaponKind === WEAPON_KIND.TANK_CANNON, "default weapon hint is retained on muzzle feedback");
  assert(state.weaponRecoil(5, KIND.TANK, performance.now()) > 0, "default weapon hint preserves tank recoil");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 21,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 15, owner: 1, kind: KIND.TANK, x: 100, y: 100, hp: 180, maxHp: 180, state: STATE.ATTACK, weaponFacing: 0 },
      { id: 16, owner: 2, kind: KIND.WORKER, x: 160, y: 100, hp: 40, maxHp: 40, state: STATE.IDLE },
    ],
    events: [{ e: EVENT.ATTACK, from: 15, to: 16, toPos: [160, 100], weaponKind: WEAPON_KIND.TANK_COAX }],
  });
  assert(state.liveMuzzleFlashes(performance.now())[0]?.weaponKind === WEAPON_KIND.TANK_COAX, "tank coax weapon hint is retained on muzzle feedback");
  assert(state.weaponRecoil(15, KIND.TANK, performance.now()) === 0, "tank coax attack events do not start Tank cannon recoil");
  assert(state.weaponRecoilPhase(15, KIND.TANK, performance.now()) === 0, "tank coax attack events do not start a recoil phase");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 22,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 25, owner: 1, kind: KIND.TANK, x: 100, y: 100, hp: 180, maxHp: 180, state: STATE.ATTACK, weaponFacing: 0 },
      { id: 26, owner: 2, kind: KIND.TANK, x: 160, y: 100, hp: 180, maxHp: 180, state: STATE.IDLE },
    ],
    events: [
      { e: EVENT.ATTACK, from: 25, to: 26, toPos: [160, 100], weaponKind: WEAPON_KIND.TANK_CANNON },
      { e: EVENT.ATTACK, from: 25, to: 26, toPos: [160, 100], weaponKind: WEAPON_KIND.TANK_COAX },
    ],
  });
  const flashes = state.liveMuzzleFlashes(performance.now());
  assert(flashes.length === 2, "same-tick cannon and coax events both keep muzzle feedback");
  assert(flashes[0].weaponKind === WEAPON_KIND.TANK_CANNON, "same-tick cannon feedback stays first");
  assert(flashes[1].weaponKind === WEAPON_KIND.TANK_COAX, "same-tick coax feedback stays second");
  assert(state.weaponRecoil(25, KIND.TANK, performance.now()) > 0, "same-tick cannon recoil is preserved when followed by coax feedback");
  assert(state.weaponRecoilKind(25) === WEAPON_KIND.TANK_CANNON, "recoil preserves the weapon that owns the authored animation");
}

{
  const state = new GameState(start);
  const now = performance.now();
  state.weaponRecoilById.set(91, now - 500);
  assert(
    state.weaponRecoil(91, KIND.RIFLEMAN, now, WEAPON_KIND.ARTILLERY_GUN) > 0,
    "explicit weapon hint can extend recoil timing through the GameState facade",
  );
  const hintedPhase = state.weaponRecoilPhase(91, KIND.RIFLEMAN, now, WEAPON_KIND.ARTILLERY_GUN);
  assert(
    hintedPhase > 0.5 && hintedPhase < 0.52,
    "explicit weapon hint also drives the linear recoil phase",
  );
  state.weaponRecoilById.set(92, now - 421);
  assert(state.weaponRecoilPhase(92, KIND.RIFLEMAN, now) === 0, "expired recoil phase returns zero");
  assert(!state.weaponRecoilById.has(92), "expired recoil phase prunes stale recoil records");
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 3,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 7, owner: 1, kind: KIND.RIFLEMAN, x: 100, y: 120, hp: 40, maxHp: 40, state: STATE.ATTACK },
      { id: 8, owner: 2, kind: KIND.WORKER, x: 140, y: 120, hp: 40, maxHp: 40, state: STATE.IDLE },
    ],
    events: [{ e: EVENT.ATTACK, from: 7, to: 8, toPos: [140, 120], weaponKind: "unknown_future_weapon" }],
  });
  assert(state.liveMuzzleFlashes(performance.now()).length === 1, "unknown weapon hint still creates normal muzzle feedback");
  assert(state.weaponRecoil(7, KIND.RIFLEMAN, performance.now()) > 0, "unknown weapon hint preserves attacker-kind recoil fallback");
}
