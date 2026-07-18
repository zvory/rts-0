// tests/client_contracts/scout_plane_input_contracts.mjs
// Scout Plane-specific input contracts split out of the broad state/input suite.

import { assert } from "./assertions.mjs";
import {
  ABILITIES,
  SCOUT_PLANE_SPEED_PX_PER_TICK,
} from "../../client/src/config.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { Minimap } from "../../client/src/minimap.js";
import { ABILITY, KIND, LAB_ROLE, STATE } from "../../client/src/protocol.js";
import { _drawAbilityTargetPreview } from "../../client/src/renderer/feedback.js";
import { GameState } from "../../client/src/state.js";
import { buildSelectionScene } from "../../client/src/input/selection_projection.js";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { pointHitsOrientedVehicle } from "../../client/src/input/placement.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

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

function inputForState(state, controlPolicy = null) {
  const input = Object.create(Input.prototype);
  input.state = state;
  input.controlPolicy = controlPolicy;
  input.dom = { clientWidth: 400, clientHeight: 300 };
  input.selectionScene = buildSelectionScene({
    entities: state.entitiesInterpolated(1),
    tileSize: state.map.tileSize,
    projection: createOrthographicProjectionSnapshot({
      x: 0, y: 0, zoom: 1, worldW: 192, worldH: 192, viewW: 400, viewH: 300,
    }, 400),
  });
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
  input._groundAtScreen = (x, y) => ({ x, y });
  input._entityAtScreen = (point) => entities.find((entity) => Math.hypot(entity.x - point.x, entity.y - point.y) <= 24) || null;
  input._resourceAtScreen = () => null;
  input._addCommandFeedback = () => {};
  return { input, commands };
}

{
  const commandCar = { id: 5700, owner: 1, kind: KIND.COMMAND_CAR, x: 128, y: 160 };
  const input = Object.create(Input.prototype);
  input.mouse = { x: 480, y: 320 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    selectedEntities: () => [commandCar],
  };
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.SCOUT_PLANE });
  input.camera = {
    projectionSnapshot: () => ({
      groundAtScreen: ({ x, y }) => ({ x, y }),
    }),
  };

  input._refreshAbilityTargetPreview();
  const preview = input.clientIntent.abilityTargetPreview;
  const travelRangePx = SCOUT_PLANE_SPEED_PX_PER_TICK
    * ABILITIES[ABILITY.SCOUT_PLANE].durationTicks;
  assert(preview?.rangePx === travelRangePx, "Scout Plane targeting uses its total lifetime travel budget as the advisory range");
  assert(
    preview?.pathOrigins?.length === 1 && preview.pathOrigins[0].id === commandCar.id,
    "Scout Plane targeting connects the launching Command Car to the cursor",
  );

  const gfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: gfx, _map: input.state.map },
    { abilityTargetPreview: preview },
  );
  assert(
    gfx.calls.some((call) =>
      call[0] === "moveTo" &&
      call[1] === commandCar.x + travelRangePx &&
      call[2] === commandCar.y),
    "armed Scout Plane targeting draws the maximum travel range around the Command Car",
  );
  assert(
    gfx.calls.some((call) => call[0] === "moveTo" && call[1] === commandCar.x && call[2] === commandCar.y) &&
      gfx.calls.some((call) => call[0] === "lineTo"),
    "armed Scout Plane targeting draws a path line from the Command Car toward the cursor",
  );
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
    pointHitsOrientedVehicle(scoutPlane, 122, 96, 3),
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
  const controlPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  labState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [scoutPlane],
    events: [],
  }, labState.visualNow(), { controlPolicy });
  const labInput = inputForState(labState, controlPolicy);
  labInput._commitClickSelection({ x: 96, y: 96 }, false, false);
  assert(Array.from(labState.selection).join(",") === "5600", "lab operator can still click-select a Scout Plane for inspection");
  labState.clearSelection();
  labInput._commitBoxSelection({ x0: 70, y0: 70, x1: 120, y1: 120 }, false);
  assert(Array.from(labState.selection).join(",") === "5600", "lab operator can still box-select a Scout Plane for inspection");
  labState.setSelection([scoutPlane.id], { controlPolicy });
  assert(Array.from(labState.selection).join(",") === "5600", "lab selection admission preserves Scout Plane ids");
  labState.setControlGroup(3, [scoutPlane.id], { controlPolicy });
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
  minimap.commandInteraction = { issueCommand: (command) => orders.push(command) };
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
  input._groundAtScreen = (x, y) => ({ x, y });
  input._entityAtScreen = () => enemy;
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
