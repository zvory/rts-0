import { assert } from "./assertions.mjs";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { Input } from "../../client/src/input/index.js";
import { buildSelectionScene } from "../../client/src/input/selection_projection.js";
import { KIND } from "../../client/src/protocol.js";

const worker = { id: 1, owner: 1, kind: KIND.WORKER, x: 64, y: 64 };
const friendlyTank = { id: 2, owner: 2, kind: KIND.TANK, x: 112, y: 112, facing: 0 };
const oil = { id: 3, owner: 0, kind: KIND.OIL, x: 112, y: 112, remaining: 1000 };
const map = { width: 64, height: 64, tileSize: 32 };
const input = Object.create(Input.prototype);
const commands = [];
input.state = {
  playerId: 1,
  map,
  entitiesInterpolated: () => [worker, friendlyTank, oil],
  selectedEntities: () => [worker],
  isAllyOwner: (owner) => owner === 2,
  addCommandFeedback() {},
};
input.commandInteraction = { issueCommand(command) { commands.push(command); } };
input._groundAtScreen = (x, y) => ({ x, y });
input.selectionScene = buildSelectionScene({
  entities: input.state.entitiesInterpolated(),
  tileSize: map.tileSize,
  projection: createOrthographicProjectionSnapshot({
    x: 0,
    y: 0,
    zoom: 1,
    worldW: map.width * map.tileSize,
    worldH: map.height * map.tileSize,
    viewW: 640,
    viewH: 480,
  }),
});

const friendlyTankHullPoint = { x: friendlyTank.x + 24, y: friendlyTank.y };
assert(
  input._resourceAtScreen(friendlyTankHullPoint) === null &&
    input._entityAtScreen(friendlyTankHullPoint)?.id === friendlyTank.id,
  "Pump Jack friendly-unit fixture must hit the tank hull outside the oil proxy",
);

input._onRightClick(friendlyTankHullPoint);
assert(
  commands.length === 1 &&
    commands[0].c === "build" &&
    commands[0].building === KIND.PUMP_JACK &&
    commands[0].tileX === 3 &&
    commands[0].tileY === 3,
  "worker right-click on a friendly unit standing over oil should build the underlying Pump Jack",
);

console.log("pump_jack_input_contracts: ok");
