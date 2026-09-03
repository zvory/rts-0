import { commandWithinBudget } from "../../client/src/command_budget.js";
import { GameState } from "../../client/src/state.js";
import { EVENT, KIND, STATE, cmd } from "../../client/src/protocol.js";
import { assert } from "./assertions.mjs";

const state = new GameState({
  playerId: 1,
  map: { width: 4, height: 4, tileSize: 32, terrain: new Array(16).fill(0), resources: [] },
  players: [{ id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 }],
});
const tanks = Array.from({ length: 5 }, (_, index) => ({
  id: index + 1,
  owner: 1,
  kind: KIND.TANK,
  x: index * 12,
  y: 20,
  hp: 100,
  maxHp: 100,
  state: STATE.IDLE,
}));
const commandCar = {
  id: 99,
  owner: 1,
  kind: KIND.COMMAND_CAR,
  x: 80,
  y: 20,
  hp: 80,
  maxHp: 80,
  state: STATE.IDLE,
};

state.applySnapshot({
  tick: 1,
  steel: 0,
  oil: 0,
  supplyUsed: 0,
  supplyCap: 80,
  entities: tanks.concat(commandCar),
  events: [],
});
state.setSelection(tanks.map((entity) => entity.id).concat(commandCar.id));
state.applySnapshot({
  tick: 2,
  steel: 0,
  oil: 0,
  supplyUsed: 0,
  supplyCap: 80,
  entities: tanks,
  events: [{ e: EVENT.DEATH, id: commandCar.id }],
});

assert(
  Array.from(state.selection).join(",") === "1,2,3",
  "losing a selected Command Car trims surviving units back to the base command budget",
);
assert(
  commandWithinBudget(state, cmd.move(Array.from(state.selection), 10, 20)).ok,
  "selection remains commandable after its selected Command Car dies",
);
