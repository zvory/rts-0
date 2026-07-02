// tests/client_contracts/scout_plane_input_contracts.mjs
// Scout Plane-specific input contracts split out of the broad state/input suite.

import { assert } from "./assertions.mjs";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import { Minimap } from "../../client/src/minimap.js";
import { KIND, STATE } from "../../client/src/protocol.js";
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
  return { input, commands };
}

// Selection and control groups use the mirrored Scout Plane body dimensions.
{
  const selectableScoutPlane = {
    id: 5500,
    owner: 1,
    kind: KIND.SCOUT_PLANE,
    x: 96,
    y: 96,
    hp: 40,
    maxHp: 40,
    state: STATE.IDLE,
  };
  const scoutSelectionState = new GameState(startInfo());
  scoutSelectionState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [selectableScoutPlane],
    events: [],
  });
  const scoutSelectionInput = inputForState(scoutSelectionState);
  assert(
    scoutSelectionInput._worldPointHitsEntity(selectableScoutPlane, 122, 96, 32),
    "Scout Plane click targeting uses its mirrored 48x34 selection body",
  );
  assert(
    scoutSelectionInput._entityIntersectsRect(selectableScoutPlane, 119, 94, 124, 98),
    "Scout Plane box selection uses its mirrored 48x34 selection body",
  );
  scoutSelectionInput._commitClickSelection({ x: 96, y: 96 }, false, false);
  assert(
    Array.from(scoutSelectionState.selection).join(",") === "5500",
    "Scout Plane can be selected directly for hidden-client inspection",
  );
  scoutSelectionState.clearSelection();
  scoutSelectionInput._commitBoxSelection({ x0: 70, y0: 70, x1: 120, y1: 120 }, false);
  assert(
    Array.from(scoutSelectionState.selection).join(",") === "5500",
    "Scout Plane can be box-selected as a friendly unit",
  );
  scoutSelectionState.setControlGroup(3, scoutSelectionState.selection);
  assert(
    scoutSelectionState.controlGroups[3].join(",") === "5500",
    "Scout Plane can be stored in a control group",
  );
}

// Minimap attack-move retargets Scout Planes without issuing invalid attack orders.
{
  const minimapScoutOrders = [];
  const minimapRifleman = { id: 615, owner: 1, kind: KIND.RIFLEMAN, x: 96, y: 128 };
  const minimapScoutPlane = { id: 616, owner: 1, kind: KIND.SCOUT_PLANE, x: 120, y: 128 };
  const minimapOrders = Object.create(Minimap.prototype);
  minimapOrders.state = {
    playerId: 1,
    selectedEntities: () => [minimapRifleman, minimapScoutPlane],
  };
  minimapOrders.clientIntent = new ClientIntent();
  minimapOrders.clientIntent.beginCommandTarget("attack");
  minimapOrders._issueCommand = (command) => minimapScoutOrders.push(command);
  minimapOrders._addCommandFeedback = () => {};
  minimapOrders._issueOrder(512, 544, true);
  assert(
    minimapScoutOrders.length === 2 &&
      minimapScoutOrders[0].c === "attackMove" &&
      minimapScoutOrders[0].units.join(",") === "615" &&
      minimapScoutOrders[0].queued === true &&
      minimapScoutOrders[1].c === "move" &&
      minimapScoutOrders[1].units.join(",") === "616" &&
      minimapScoutOrders[1].x === 512 &&
      minimapScoutOrders[1].y === 544 &&
      minimapScoutOrders[1].queued === true,
    "minimap attack-move sends land units to attack while Scout Planes receive only move retargets",
  );
}

// Context right-clicks split land attacks from Scout Plane orbit retargeting.
{
  const moveUnit = { id: 40, owner: 1, kind: KIND.RIFLEMAN, x: 120, y: 120 };
  const enemyUnit = { id: 41, owner: 2, kind: KIND.RIFLEMAN, x: 180, y: 180 };
  const selectedScoutPlane = { id: 90, owner: 1, kind: KIND.SCOUT_PLANE, x: 140, y: 150 };
  let setup = commandInput(
    [moveUnit, selectedScoutPlane],
    [moveUnit, selectedScoutPlane, enemyUnit],
  );
  setup.input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    setup.commands.length === 2 &&
      setup.commands[0].c === "attack" &&
      setup.commands[0].units.join(",") === "40" &&
      setup.commands[0].target === enemyUnit.id &&
      setup.commands[0].queued === true &&
      setup.commands[1].c === "move" &&
      setup.commands[1].units.join(",") === "90" &&
      setup.commands[1].queued === true,
    "mixed land plus Scout Plane right-click attacks with land units and retargets planes",
  );

  setup = commandInput([selectedScoutPlane], [selectedScoutPlane, enemyUnit]);
  setup.input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    setup.commands.length === 1 &&
      setup.commands[0].c === "move" &&
      setup.commands[0].units.join(",") === "90" &&
      setup.commands[0].x === enemyUnit.x &&
      setup.commands[0].y === enemyUnit.y &&
      setup.commands[0].queued === true,
    "Scout Plane-only enemy right-click retargets orbit instead of issuing attack",
  );

  const enemyScoutPlane = { id: 91, owner: 2, kind: KIND.SCOUT_PLANE, x: 180, y: 180 };
  setup = commandInput([moveUnit], [moveUnit, enemyScoutPlane]);
  setup.input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    setup.commands.length === 1 &&
      setup.commands[0].c === "move" &&
      setup.commands[0].units.join(",") === "40" &&
      setup.commands[0].queued === true,
    "right-clicking a visible enemy Scout Plane moves instead of issuing an invalid attack",
  );
}

// Explicit command targeting splits Scout Plane retargeting from land-unit attacks.
{
  const planeTargetedCommands = [];
  const targetedRifleman = { id: 140, owner: 1, kind: KIND.RIFLEMAN, x: 128, y: 128 };
  const targetedScoutPlane = { id: 141, owner: 1, kind: KIND.SCOUT_PLANE, x: 140, y: 140 };
  const targetedEnemy = { id: 142, owner: 2, kind: KIND.RIFLEMAN, x: 320, y: 320 };
  const planeTargetedInput = Object.create(Input.prototype);
  planeTargetedInput.state = {
    playerId: 1,
    selectedEntities: () => [targetedRifleman, targetedScoutPlane],
  };
  planeTargetedInput.clientIntent = new ClientIntent();
  planeTargetedInput.commandIssuer = { issueCommand: (command) => planeTargetedCommands.push(command) };
  planeTargetedInput._worldAt = (x, y) => ({ x, y });
  planeTargetedInput._entityAtWorld = () => targetedEnemy;
  planeTargetedInput._addCommandFeedback = () => {};
  planeTargetedInput.clientIntent.beginCommandTarget("attack");
  planeTargetedInput._issueTargetedCommand({ x: targetedEnemy.x, y: targetedEnemy.y }, { shiftKey: true });
  assert(
    planeTargetedCommands.length === 2 &&
      planeTargetedCommands[0].c === "attack" &&
      planeTargetedCommands[0].units.join(",") === "140" &&
      planeTargetedCommands[0].target === targetedEnemy.id &&
      planeTargetedCommands[0].queued === true &&
      planeTargetedCommands[1].c === "move" &&
      planeTargetedCommands[1].units.join(",") === "141" &&
      planeTargetedCommands[1].x === targetedEnemy.x &&
      planeTargetedCommands[1].y === targetedEnemy.y &&
      planeTargetedCommands[1].queued === true,
    "mixed targeted attack sends attack to land units and retargets Scout Planes",
  );

  const targetedEnemyScoutPlane = { id: 143, owner: 2, kind: KIND.SCOUT_PLANE, x: 336, y: 336 };
  planeTargetedCommands.length = 0;
  planeTargetedInput._entityAtWorld = () => targetedEnemyScoutPlane;
  planeTargetedInput.clientIntent.beginCommandTarget("attack");
  planeTargetedInput._issueTargetedCommand({ x: targetedEnemyScoutPlane.x, y: targetedEnemyScoutPlane.y }, { shiftKey: true });
  assert(
    planeTargetedCommands.length === 2 &&
      planeTargetedCommands[0].c === "attackMove" &&
      planeTargetedCommands[0].units.join(",") === "140" &&
      planeTargetedCommands[0].target === undefined &&
      planeTargetedCommands[0].queued === true &&
      planeTargetedCommands[1].c === "move" &&
      planeTargetedCommands[1].units.join(",") === "141" &&
      planeTargetedCommands[1].x === targetedEnemyScoutPlane.x &&
      planeTargetedCommands[1].y === targetedEnemyScoutPlane.y &&
      planeTargetedCommands[1].queued === true,
    "attack targeting a visible enemy Scout Plane does not issue invalid target attacks",
  );

  planeTargetedCommands.length = 0;
  planeTargetedInput._entityAtWorld = () => null;
  planeTargetedInput.clientIntent.beginCommandTarget("attack");
  planeTargetedInput._issueTargetedCommand({ x: 352, y: 384 }, { shiftKey: true });
  assert(
    planeTargetedCommands.length === 2 &&
      planeTargetedCommands[0].c === "attackMove" &&
      planeTargetedCommands[0].units.join(",") === "140" &&
      planeTargetedCommands[1].c === "move" &&
      planeTargetedCommands[1].units.join(",") === "141" &&
      planeTargetedCommands[1].x === 352 &&
      planeTargetedCommands[1].y === 384,
    "mixed targeted attack-move sends only a move retarget to Scout Planes",
  );

  planeTargetedCommands.length = 0;
  planeTargetedInput.clientIntent.beginCommandTarget("move");
  planeTargetedInput._issueTargetedCommand({ x: 400, y: 416 }, { shiftKey: true });
  assert(
    planeTargetedCommands.length === 1 &&
      planeTargetedCommands[0].c === "move" &&
      planeTargetedCommands[0].units.join(",") === "140,141" &&
      planeTargetedCommands[0].queued === true,
    "mixed targeted move keeps Scout Plane retargeting in the normal move command",
  );
}
