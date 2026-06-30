// tests/client_contracts/death_vision_targeting_contracts.mjs
// Focused input contracts for attack-targetable death vision.

import { GameState } from "../../client/src/state.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import { KIND, STATE } from "../../client/src/protocol.js";

import { assert } from "./assertions.mjs";

const start = {
  playerId: 1,
  tick: 0,
  map: {
    width: 12,
    height: 12,
    tileSize: 32,
    terrain: new Array(144).fill(0),
    resources: [],
  },
  players: [
    { id: 1, teamId: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    { id: 2, teamId: 2, name: "B", color: "#0000ff", startTileX: 3, startTileY: 3 },
  ],
};

const ownWorker = { id: 201, owner: 1, kind: KIND.WORKER, x: 32, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
const deathVisionDepot = {
  id: 204,
  owner: 2,
  kind: KIND.DEPOT,
  x: 128,
  y: 32,
  hp: 160,
  maxHp: 160,
  state: STATE.IDLE,
  visionOnly: true,
};

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 0,
    steel: 100,
    oil: 100,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [ownWorker, deathVisionDepot],
    events: [],
  });

  const commands = [];
  const input = Object.create(Input.prototype);
  input.state = state;
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  input._worldAt = Input.prototype._worldAt;
  input._entityAtWorld = Input.prototype._entityAtWorld;
  input._worldPointHitsEntity = Input.prototype._worldPointHitsEntity;
  input._resourceAtWorld = Input.prototype._resourceAtWorld;
  input._selectedOwnUnitIds = Input.prototype._selectedOwnUnitIds;
  input._selectedGathererIds = Input.prototype._selectedGathererIds;
  input._selectedWorkerIds = Input.prototype._selectedWorkerIds;
  input._selectedProducerBuildingIds = Input.prototype._selectedProducerBuildingIds;
  input._issueCommand = (command) => commands.push(command);

  state.setSelection([ownWorker.id]);
  input._onRightClick({ x: deathVisionDepot.x, y: deathVisionDepot.y }, { shiftKey: true });
  const command = commands.at(-1);
  assert(
    command.c === "attack" && command.target === deathVisionDepot.id && command.queued === true,
    "right-clicking a death-vision enemy building with own units selected sends queued attack",
  );
}

{
  const state = new GameState(start);
  state.applySnapshot({
    tick: 0,
    steel: 100,
    oil: 100,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [ownWorker, deathVisionDepot],
    events: [],
  });

  const input = Object.create(Input.prototype);
  input.state = state;
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  input._worldAt = Input.prototype._worldAt;
  input._entityAtWorld = Input.prototype._entityAtWorld;
  input._worldPointHitsEntity = Input.prototype._worldPointHitsEntity;
  input._closestIdsToPoint = Input.prototype._closestIdsToPoint;
  input._commitClickSelection = Input.prototype._commitClickSelection;

  state.setSelection([ownWorker.id]);
  input._commitClickSelection({ x: deathVisionDepot.x, y: deathVisionDepot.y }, false, false);
  assert(state.selection.size === 0, "normal selection hit-testing still ignores death-vision entities");
}

{
  const sentCommands = [];
  const input = Object.create(Input.prototype);
  const intent = new ClientIntent();
  input.state = { playerId: 1 };
  input.clientIntent = intent;
  input.renderer = { drawSelectionBox() {} };
  input.commandIssuer = { issueCommand: (command) => sentCommands.push(command) };
  input._worldAt = (x, y) => ({ x, y });
  input._selectedOwnUnitIds = () => [ownWorker.id];
  input._commitClickSelection = () => {};
  input._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  input._trackMouse = () => {};
  let lookupOptions = null;
  input._entityAtWorld = (_x, _y, _ownPreferred, options = {}) => {
    lookupOptions = options;
    return options.includeVisionOnly ? deathVisionDepot : null;
  };

  intent.beginCommandTarget("attack");
  input._onLeftDown({ x: deathVisionDepot.x, y: deathVisionDepot.y }, {});
  const command = sentCommands.at(-1);
  assert(
    lookupOptions?.includeVisionOnly === true && command.c === "attack" && command.target === deathVisionDepot.id,
    "attack targeting includes death-vision entities as direct attack targets",
  );
}
