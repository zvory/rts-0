import assert from "node:assert/strict";
import { _onRightClick, _issueTargetedCommand } from "../../client/src/input/commands.js";
import { KIND } from "../../client/src/protocol.js";
import { isProducerBuilding } from "../../client/src/config.js";

const grounds = {id: 10, owner: 1, kind: KIND.TOURNAMENT_GROUNDS, x: 500, y: 500};
const units = [
  {id: 1, owner: 1, kind: KIND.WARRIOR},
  {id: 2, owner: 1, kind: KIND.WARRIOR, dao: "magic"},
  {id: 3, owner: 1, kind: KIND.WORKER},
];
const commands = [];
const input = {
  state: {playerId: 1, selectedEntities: () => units, entityById: (id) => units.find((e) => e.id === id)},
  _selectedOwnUnitIds: () => units.map((e) => e.id),
  _groundAtScreen: () => ({x: 490, y: 510}),
  _resourceAtScreen: () => null,
  _entityAtScreen: () => grounds,
  _addCommandFeedback() {},
  commandInteraction: {issueCommand: (command) => commands.push(command)},
};
_onRightClick.call(input, {x: 100, y: 100});
assert.deepEqual(commands, [{c: "move", units: [1], x: 500, y: 500}],
  "right-click sends only base Disciples to the exact building center");
assert.equal(isProducerBuilding(KIND.TOURNAMENT_GROUNDS), true, "Grounds accepts rally input");
commands.length = 0;
input._selectedOwnUnitIds = () => [];
input._selectedProducerBuildingIds = () => [20];
_onRightClick.call(input, {x: 100, y: 100});
assert.equal(commands[0].c, "setRally");
assert.equal(commands[0].building, 20);
assert.equal(commands[0].x, grounds.x, "Portal rally snaps to Grounds center instead of clicked ground");
assert.equal(commands[0].y, grounds.y);
commands.length = 0;
input.clientIntent = {commandTarget: "move"};
_issueTargetedCommand.call(input, {x: 100, y: 100});
assert.equal(commands[0].c, "setRally");
assert.equal(commands[0].x, grounds.x, "command-card rally also targets Grounds center");
assert.equal(commands[0].y, grounds.y);
console.log("tournament_contracts: ok");
