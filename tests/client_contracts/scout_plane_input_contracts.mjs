// tests/client_contracts/scout_plane_input_contracts.mjs
// Scout Plane-specific input contracts split out of the broad state/input suite.

import { assert } from "./assertions.mjs";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { Minimap } from "../../client/src/minimap.js";
import { KIND, LAB_ROLE, STATE } from "../../client/src/protocol.js";
import { GameState } from "../../client/src/state.js";

function startInfo() {
  return {
    playerId: 1,
    tick: 0,
    map: {
      width: 6,
      height: 6,
      tileSize: 32,
      terrain: new Array(36).fill(0),
      resources: [],
    },
    players: [
      { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
      { id: 2, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
    ],
  };
}

function inputForState(state) {
  const input = Object.create(Input.prototype);
  input.state = state;
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  input.dom = { clientWidth: 400, clientHeight: 300 };
  return input;
}

function commandInput(selected, entities) {
  const commands = [];
  const input = Object.create(Input.prototype);
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => entities,
    selectedEntities: () => selected,
  };
  input.commandIssuer = {
    issueCommand(command) {
      commands.push(command);
      return true;
    },
  };
  input._worldAt = (x, y) => ({ x, y });
  input._addCommandFeedback = () => {};
  return { input, commands };
}

{
  const scoutPlane = {
    id: 5500,
    owner: 1,
    kind: KIND.SCOUT_PLANE,
    x: 96,
    y: 96,
    hp: 40,
    maxHp: 40,
    state: STATE.IDLE,
  };
  const selectionState = new GameState(startInfo());
  selectionState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [scoutPlane],
    events: [],
  });
  const selectionInput = inputForState(selectionState);
  assert(
    selectionInput._worldPointHitsEntity(scoutPlane, 122, 96, 32),
    "Scout Plane still has a mirrored render hit body",
  );
  selectionInput._commitClickSelection({ x: 96, y: 96 }, false, false);
  assert(Array.from(selectionState.selection).length === 0, "Scout Plane cannot be click-selected");
  selectionInput._commitBoxSelection({ x0: 70, y0: 70, x1: 120, y1: 120 }, false);
  assert(Array.from(selectionState.selection).length === 0, "Scout Plane cannot be box-selected");
  selectionState.setSelection([scoutPlane.id]);
  assert(Array.from(selectionState.selection).length === 0, "direct selection admission rejects Scout Plane ids");
  selectionState.setControlGroup(3, [scoutPlane.id]);
  assert(selectionState.controlGroups[3].length === 0, "Scout Plane cannot be stored in a control group");
}

{
  const scoutPlane = {
    id: 5600,
    owner: 2,
    kind: KIND.SCOUT_PLANE,
    x: 96,
    y: 96,
    hp: 40,
    maxHp: 40,
    state: STATE.IDLE,
  };
  const labState = new GameState({ ...startInfo(), spectator: true });
  labState.controlPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  labState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [scoutPlane],
    events: [],
  });
  const labInput = inputForState(labState);
  labInput._commitClickSelection({ x: 96, y: 96 }, false, false);
  assert(Array.from(labState.selection).join(",") === "5600", "lab operator can still click-select a Scout Plane for inspection");
  labState.clearSelection();
  labInput._commitBoxSelection({ x0: 70, y0: 70, x1: 120, y1: 120 }, false);
  assert(Array.from(labState.selection).join(",") === "5600", "lab operator can still box-select a Scout Plane for inspection");
  labState.setSelection([scoutPlane.id]);
  assert(Array.from(labState.selection).join(",") === "5600", "lab selection admission preserves Scout Plane ids");
  labState.setControlGroup(3, [scoutPlane.id]);
  assert(labState.controlGroups[3].join(",") === "5600", "lab control groups can store inspectable Scout Planes");
}

{
  const orders = [];
  const rifleman = { id: 615, owner: 1, kind: KIND.RIFLEMAN, x: 96, y: 128 };
  const scoutPlane = { id: 616, owner: 1, kind: KIND.SCOUT_PLANE, x: 120, y: 128 };
  const minimap = Object.create(Minimap.prototype);
  minimap.state = {
    playerId: 1,
    selectedEntities: () => [rifleman, scoutPlane],
  };
  minimap.clientIntent = new ClientIntent();
  minimap.clientIntent.beginCommandTarget("attack");
  minimap._issueCommand = (command) => orders.push(command);
  minimap._addCommandFeedback = () => {};
  minimap._issueOrder(512, 544, true);
  assert(
    orders.length === 1 &&
      orders[0].c === "attackMove" &&
      orders[0].units.join(",") === "615" &&
      orders[0].queued === true,
    "minimap attack-move ignores Scout Planes and commands only land units",
  );
}

{
  const rifleman = { id: 40, owner: 1, kind: KIND.RIFLEMAN, x: 120, y: 120 };
  const enemy = { id: 41, owner: 2, kind: KIND.RIFLEMAN, x: 180, y: 180 };
  const scoutPlane = { id: 90, owner: 1, kind: KIND.SCOUT_PLANE, x: 140, y: 150 };
  let setup = commandInput([rifleman, scoutPlane], [rifleman, scoutPlane, enemy]);
  setup.input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    setup.commands.length === 1 &&
      setup.commands[0].c === "attack" &&
      setup.commands[0].units.join(",") === "40" &&
      setup.commands[0].target === enemy.id &&
      setup.commands[0].queued === true,
    "mixed land plus Scout Plane right-click attacks only with land units",
  );

  setup = commandInput([scoutPlane], [scoutPlane, enemy]);
  setup.input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(setup.commands.length === 0, "Scout Plane-only right-click issues no command");
}

{
  const commands = [];
  const rifleman = { id: 140, owner: 1, kind: KIND.RIFLEMAN, x: 128, y: 128 };
  const scoutPlane = { id: 141, owner: 1, kind: KIND.SCOUT_PLANE, x: 140, y: 140 };
  const enemy = { id: 142, owner: 2, kind: KIND.RIFLEMAN, x: 320, y: 320 };
  const input = Object.create(Input.prototype);
  input.state = {
    playerId: 1,
    selectedEntities: () => [rifleman, scoutPlane],
  };
  input.clientIntent = new ClientIntent();
  input.commandIssuer = { issueCommand: (command) => commands.push(command) };
  input._worldAt = (x, y) => ({ x, y });
  input._entityAtWorld = () => enemy;
  input._addCommandFeedback = () => {};
  input.clientIntent.beginCommandTarget("attack");
  input._issueTargetedCommand({ x: enemy.x, y: enemy.y }, { shiftKey: true });
  assert(
    commands.length === 1 &&
      commands[0].c === "attack" &&
      commands[0].units.join(",") === "140" &&
      commands[0].target === enemy.id &&
      commands[0].queued === true,
    "targeted attacks ignore Scout Planes and command only land units",
  );
}
