import assert from "node:assert/strict";
import { _onRightClick, _issueTargetedCommand } from "../../client/src/input/commands.js";
import { KIND } from "../../client/src/protocol.js";
import { isProducerBuilding } from "../../client/src/config.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { installFakePixi } from "./pixi_fakes.mjs";

const grounds = {id: 10, owner: 1, kind: KIND.TOURNAMENT_GROUNDS, x: 500, y: 500};
const units = [
  {id: 1, owner: 1, kind: KIND.WARRIOR},
  {id: 2, owner: 1, kind: KIND.MAGIC_ADEPT, dao: "magic"},
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
const restorePixi = installFakePixi();
try {
  const parent = { clientWidth: 640, clientHeight: 480,
    appendChild(view) { view.parentNode = this; }, removeChild(view) { view.parentNode = null; } };
  const renderer = await Renderer.create(parent);
  renderer._map = { tileSize: 32 };
  const building = { ...grounds, hp: 500, maxHp: 500, state: "idle" };
  const colors = new Map([[1, 0x4878c8]]);
  const state = { playerId: 1 };
  const rects = () => {
    const calls = renderer._pools.buildingOverlays.get(building.id).calls;
    return calls.slice(calls.findLastIndex((call) => call[0] === "clear") + 1)
      .filter((call) => call[0] === "drawRect");
  };
  for (let count = 0; count < 4; count += 1) {
    renderer._drawBuilding({ ...building, tournamentEntrants: count }, colors, state);
    const calls = rects();
    assert.equal(calls.length, 5, "admission bar has background, fill, and three dividers");
    assert.equal(calls[1][3], 56 * count / 4, "one quarter fills per admitted disciple");
    assert.equal(calls[0][2], 436, "admission bar sits below the 4×4 building health bar at y=428");
  }
  // Equal fractions must still redraw when switching between segmented and continuous bars.
  renderer._drawBuilding({ ...building, tournamentEntrants: 1 }, colors, state);
  renderer._drawBuilding({ ...building, tournamentEntrants: 4, prodKind: KIND.WARRIOR,
    prodProgress: 0.25 }, colors, state);
  assert.equal(rects().length, 2, "running tournament replaces slots with ordinary production bar");
  assert.equal(rects()[0][2], 442, "running tournament uses ordinary production position");
  for (const entity of [
    { ...building, id: 11 },
    { ...building, id: 12, tournamentEntrants: 2, buildProgress: 0.5 },
  ]) {
    renderer._drawBuilding(entity, colors, state);
    assert.equal(renderer._pools.buildingOverlays.has(entity.id), false,
      "no admission slots during construction or without recipient-visible entrant count");
  }
  renderer.destroy();
} finally {
  restorePixi();
}

console.log("tournament_contracts: ok");
