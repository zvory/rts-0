// tests/client_contracts/weapon_feedback_contracts.mjs
// Weapon identity hints must affect visual feedback without making clients depend on them.

import { assert } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { EVENT, KIND, SETUP, STATE, WEAPON_KIND } from "../../client/src/protocol.js";

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
  state.weaponRecoilById.set(91, performance.now() - 500);
  assert(
    state.weaponRecoil(91, KIND.RIFLEMAN, performance.now(), WEAPON_KIND.ARTILLERY_GUN) > 0,
    "explicit weapon hint can extend recoil timing through the GameState facade",
  );
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
