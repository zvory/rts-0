import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { buildSelectionScene } from "../../client/src/input/selection_projection.js";
import { KIND, STATE } from "../../client/src/protocol.js";
import { GameState } from "../../client/src/state.js";
import { assert } from "./assertions.mjs";

const state = new GameState({
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
    { id: 1, teamId: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    { id: 2, teamId: 2, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
  ],
});
state.applySnapshot({
  tick: 1,
  steel: 0,
  oil: 0,
  supplyUsed: 0,
  supplyCap: 10,
  visibleTiles: new Array(16).fill(0),
  exploredTiles: new Array(16).fill(0),
  entities: [
    { id: 100, owner: 2, kind: KIND.ANTI_TANK_GUN, x: 16, y: 16, hp: 45, maxHp: 45, state: STATE.ATTACK },
    { id: 101, owner: 1, kind: KIND.RIFLEMAN, x: 48, y: 16, hp: 45, maxHp: 45, state: STATE.IDLE },
  ],
  events: [],
});

const attacker = state.entityById(100);
assert(attacker?.aboveFogReveal === true, "a projected enemy on a presentation-dark tile renders above fog");
assert(attacker?.shotReveal !== true, "an actionable firing reveal is not downgraded to a visual-only ghost");
assert(state.entityById(101)?.aboveFogReveal !== true, "owned units remain ordinary on presentation-dark tiles");
assert(
  buildSelectionScene({
    entities: [attacker],
    tileSize: 32,
    projection: createOrthographicProjectionSnapshot({
      x: 0, y: 0, zoom: 1, worldW: 128, worldH: 128, viewW: 128, viewH: 128,
    }, 128),
  }).proxies.length === 1,
  "an actionable above-fog firing reveal remains available to click targeting",
);

state.applySnapshot({
  tick: 2,
  steel: 0,
  oil: 0,
  supplyUsed: 0,
  supplyCap: 10,
  visibleTiles: new Array(16).fill(0),
  exploredTiles: new Array(16).fill(0),
  entities: [{ id: 101, owner: 1, kind: KIND.RIFLEMAN, x: 48, y: 16, hp: 45, maxHp: 45, state: STATE.IDLE }],
  rememberedAntiTankGuns: [{
    id: 100,
    owner: 2,
    x: 16,
    y: 16,
    facing: 0.5,
    observedTick: 1,
  }],
  events: [],
});
assert(
  state.rememberedAntiTankGuns[0]?.id === 100
    && state.rememberedAntiTankGuns[0]?.observedTick === 1,
  "GameState replaces AT-gun memory from the server snapshot instead of accumulating it locally",
);

console.log("firing_reveal_contracts: ok");
