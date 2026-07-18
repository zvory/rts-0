// tests/client_contracts/lab_input_ownership_contracts.mjs
// Focused lab input ownership contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { Input } from "../../client/src/input/index.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { KIND, LAB_ROLE, STATE } from "../../client/src/protocol.js";

{
  const p2Unit = { id: 211, owner: 2, kind: KIND.RIFLEMAN, x: 64, y: 96, hp: 45, maxHp: 45, state: STATE.IDLE };
  const p1Target = { id: 212, owner: 1, kind: KIND.RIFLEMAN, x: 96, y: 96, hp: 45, maxHp: 45, state: STATE.IDLE };
  const p2Target = { id: 213, owner: 2, kind: KIND.RIFLEMAN, x: 128, y: 96, hp: 45, maxHp: 45, state: STATE.IDLE };
  const commands = [];
  const input = Object.create(Input.prototype);
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    players: [
      { id: 1, teamId: 1 },
      { id: 2, teamId: 2 },
    ],
    selectedEntities() {
      return [p2Unit];
    },
    entitiesInterpolated() {
      return [p2Unit, p1Target, p2Target];
    },
  };
  input.controlPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  input._groundAtScreen = (x, y) => ({ x, y });
  input._entityAtScreen = (point) => point.x < 112 ? p1Target : p2Target;
  input._resourceAtScreen = () => null;
  input._selectedOwnUnitIds = Input.prototype._selectedOwnUnitIds;
  input._selectedWorkerIds = Input.prototype._selectedWorkerIds;
  input._selectedProducerBuildingIds = Input.prototype._selectedProducerBuildingIds;
  input.commandInteraction = { issueCommand: (command) => commands.push(command) };

  input._onRightClick({ x: p1Target.x, y: p1Target.y });
  assert(
    commands.at(-1)?.c === "attack" &&
      commands.at(-1)?.units.join(",") === String(p2Unit.id) &&
      commands.at(-1)?.target === p1Target.id,
    "lab P2 right-clicking a P1 unit classifies the target as an enemy",
  );
  input._onRightClick({ x: p2Target.x, y: p2Target.y });
  assert(
    commands.at(-1)?.c === "move" &&
      commands.at(-1)?.units.join(",") === String(p2Unit.id),
    "lab P2 right-clicking a P2 unit does not classify the selected owner as hostile",
  );

  const p2Worker = { id: 214, owner: 2, kind: KIND.WORKER, x: 64, y: 128, hp: 30, maxHp: 30, state: STATE.IDLE };
  const p2IncompleteDepot = {
    id: 215,
    owner: 2,
    kind: KIND.DEPOT,
    x: 160,
    y: 160,
    buildProgress: 0.5,
  };
  input.state.selectedEntities = () => [p2Worker];
  input._entityAtScreen = () => p2IncompleteDepot;
  input._onRightClick({ x: p2IncompleteDepot.x, y: p2IncompleteDepot.y });
  assert(
    commands.at(-1)?.c === "build" &&
      commands.at(-1)?.units.join(",") === String(p2Worker.id) &&
      commands.at(-1)?.building === KIND.DEPOT,
    "lab P2 right-clicking its unfinished building resumes construction instead of moving",
  );
}
