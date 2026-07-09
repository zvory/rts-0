// tests/client_contracts/miss_toast_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { EVENT, KIND, STATE } from "../../client/src/protocol.js";

const start = {
  playerId: 1,
  tick: 0,
  map: { width: 4, height: 4, tileSize: 32, terrain: new Array(16).fill(0), resources: [] },
  players: [
    { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    { id: 2, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
  ],
};

const missEventState = new GameState(start);
missEventState.applySnapshot({
  tick: 13,
  steel: 0,
  oil: 0,
  supplyUsed: 0,
  supplyCap: 10,
  entities: [{ id: 23, owner: 2, kind: KIND.RIFLEMAN, x: 180, y: 116, hp: 35, maxHp: 40, state: STATE.IDLE }],
  events: [{ e: EVENT.MISS, to: 23 }],
});

assert(missEventState.liveMissToasts(performance.now())[0]?.to === 23, "miss event creates a live miss toast");
assert(missEventState.liveMuzzleFlashes(performance.now()).length === 0, "miss event does not draw a tracer");
assert(missEventState.weaponRecoil(23, KIND.RIFLEMAN, performance.now()) === 0, "miss event does not trigger target recoil");
