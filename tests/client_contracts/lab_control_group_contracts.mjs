// tests/client_contracts/lab_control_group_contracts.mjs
// Focused lab control-group contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import { Input } from "../../client/src/input/index.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { KIND, LAB_ROLE, STATE } from "../../client/src/protocol.js";

const start = {
  playerId: 1,
  spectator: true,
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
    { id: 2, teamId: 2, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
  ],
};

{
  const state = new GameState(start);
  state.controlPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  const p2Worker = { id: 301, owner: 2, kind: KIND.WORKER, x: 32, y: 96, hp: 40, maxHp: 40, state: STATE.IDLE };
  const p2Rifle = { id: 302, owner: 2, kind: KIND.RIFLEMAN, x: 64, y: 96, hp: 45, maxHp: 45, state: STATE.IDLE };
  const p1Worker = { id: 303, owner: 1, kind: KIND.WORKER, x: 96, y: 96, hp: 40, maxHp: 40, state: STATE.IDLE };
  state.applySnapshot({
    tick: 0,
    steel: 100,
    oil: 100,
    supplyUsed: 0,
    supplyCap: 20,
    entities: [p2Worker, p2Rifle, p1Worker],
    events: [],
  });
  state.setSelection([p2Worker.id, p2Rifle.id]);
  state.setControlGroup(3, state.selection);
  assert(
    state.controlGroups[3].join(",") === "301,302",
    "lab control groups save selected P2 controllables even though the start payload is spectator-shaped",
  );
  state.setSelection([p1Worker.id]);
  state.selectControlGroup(3);
  assert(
    Array.from(state.selection).join(",") === "301,302",
    "lab control-group recall switches back to the saved controlled owner",
  );
  state.addToControlGroup(3, [p1Worker.id]);
  assert(
    state.controlGroups[3].join(",") === "301,302",
    "lab control groups reject adding a second owner to an existing group",
  );
  state.setControlGroup(4, [p2Worker.id, p1Worker.id]);
  assert(state.controlGroups[4].length === 0, "lab control groups reject mixed-owner saves");
}

{
  const calls = [];
  const input = Object.create(Input.prototype);
  input.state = {
    spectator: true,
    selection: new Set([301, 302]),
    controlPolicy: {
      kind: "lab",
      canUseCommandSurface() {
        return true;
      },
    },
    setControlGroup(slot, ids) {
      calls.push({ type: "set", slot, ids: Array.from(ids) });
      return Array.from(ids);
    },
    addToControlGroup() {
      return [];
    },
    selectControlGroup() {
      return [];
    },
  };
  input.selectionScene = { proxies: [{ id: 301 }, { id: 302 }] };
  input._lastControlGroupTap = null;
  const ev = {
    code: "Digit4",
    altKey: true,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    preventDefault() { this.prevented = true; },
    stopPropagation() { this.stopped = true; },
  };
  assert(
    input._handleControlGroupHotkey(ev) === true && calls[0]?.ids.join(",") === "301,302",
    "lab operator control-group hotkeys work in spectator-shaped lab matches",
  );
}
