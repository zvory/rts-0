import { assert } from "./assertions.mjs";
import { RESOURCE_AMOUNTS } from "../../client/src/config.js";
import { KIND } from "../../client/src/protocol.js";
import { GameState } from "../../client/src/state.js";

const start = {
  playerId: 1,
  tick: 0,
  map: {
    width: 4,
    height: 4,
    tileSize: 32,
    terrain: new Array(16).fill(0),
    resources: [{ id: 200, kind: KIND.STEEL, x: 64, y: 96 }],
  },
  players: [{ id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 }],
};
const state = new GameState(start);
state.applySnapshot({
  tick: 1,
  steel: 12,
  oil: 5,
  supplyUsed: 2,
  supplyCap: 10,
  entities: [{ id: 1, owner: 1, kind: KIND.WORKER, x: 15, y: 25, hp: 40, maxHp: 40, state: "idle" }],
  resourceDeltas: [{ id: 200, remaining: 0 }],
  trenches: [{ id: 300, x: 96, y: 128, radiusTiles: 0.375 }],
  events: [],
});
state.visualEffects.muzzleFlashes.push({ from: 1, to: 2, createdAt: 1 });
state.groundDecals.applySnapshotEvents(
  [{ e: "death", id: 1, x: 15, y: 25, kind: KIND.WORKER }],
  { tick: 1, players: state.players, tileSize: state.tileSize },
);

state.resetForReplaySeek();

assert(
  state.currRecvTime === null && state.prevRecvTime === null && state.tick === 0,
  "replay seek reset clears interpolation and authoritative snapshot state",
);
assert(
  state.resourceById.get(200).remaining === RESOURCE_AMOUNTS[KIND.STEEL],
  "replay seek reset restores mutable resource nodes to the replay baseline",
);
assert(
  state.trenches.length === 0 && state.visualEffects.muzzleFlashes.length === 0 &&
    state.groundDecals.pendingCount === 0,
  "replay seek reset clears timeline-derived world effects and durable decal history",
);
