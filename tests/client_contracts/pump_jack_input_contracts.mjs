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

const miningAnchorInput = Object.create(Input.prototype);
miningAnchorInput.state = {
  playerId: 1,
  isOwnOwner: (owner) => owner === 1,
  isAllyOwner: (owner) => owner === 2,
};
miningAnchorInput._selectionEntities = () => [
  { id: 40, owner: 3, kind: KIND.CITY_CENTRE, x: 1, y: 1, buildProgress: null },
  { id: 41, owner: 2, kind: KIND.CITY_CENTRE, x: 8, y: 8, buildProgress: 0.5 },
  { id: 42, owner: 2, kind: KIND.CITY_CENTRE, x: 20, y: 20, buildProgress: null },
  { id: 43, owner: 1, kind: KIND.CITY_CENTRE, x: 40, y: 40, buildProgress: null },
];
assert(
  miningAnchorInput._nearestCompletedMiningAnchor(0, 0, true)?.id === 42,
  "resource-range preview should use the nearest completed owned or allied mining anchor",
);
assert(
  miningAnchorInput._nearestCompletedMiningAnchor(0, 0)?.id === 43,
  "steel resource-range preview should continue to require an owned mining anchor",
);

const snapInput = Object.create(Input.prototype);
const nearOil = { id: 50, owner: 0, kind: KIND.OIL, x: 176, y: 176, remaining: 962 };
const farOil = { id: 51, owner: 0, kind: KIND.OIL, x: 336, y: 336, remaining: 962 };
const depletedOil = { id: 52, owner: 0, kind: KIND.OIL, x: 161, y: 161, remaining: 0 };
let placementPreview = null;
snapInput.state = { map };
snapInput.mouse = { x: 160, y: 160 };
snapInput._groundAtScreen = () => ({ x: 160, y: 160 });
snapInput._selectionEntities = () => [nearOil, farOil, depletedOil];
snapInput._footprintValid = (tileX, tileY) => tileX === 5 && tileY === 5;
snapInput.clientIntent = {
  placement: { building: KIND.PUMP_JACK, tileX: 0, tileY: 0, valid: false },
  updatePlacement(tileX, tileY, valid) {
    placementPreview = { tileX, tileY, valid };
  },
};
snapInput._refreshPlacement();
assert(
  placementPreview?.tileX === 5 && placementPreview?.tileY === 5 && placementPreview?.valid,
  "armed Pump Jack placement snaps to the closest visible live oil patch",
);

console.log("pump_jack_input_contracts: ok");
