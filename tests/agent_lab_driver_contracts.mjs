import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  AgentLabDriverError,
  DRIVER_STATES,
  generatedRoomId,
  safeToken,
  transitionDriverState,
  validateWorkspaceRoot,
  withTimeout,
} from "../scripts/agent-lab/driver.mjs";
import {
  AGENT_LAB_BRIDGE_KEY,
  AgentLabBridge,
  agentLabLaunchEnabled,
  normalizeInspectionQuery,
} from "../client/src/agent_lab_bridge.js";
import { DEFAULT_FACTION_ID, LAB_ROLE } from "../client/src/protocol.js";
import { labSpawnUnitKindsForFaction } from "../client/src/lab_spawn_catalog.js";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const workspace = validateWorkspaceRoot(root);
assert.equal(workspace.root, fs.realpathSync(root), "agent-lab validates the selected checkout top level");
assert.match(workspace.head, /^[0-9a-f]{40}$/i, "agent-lab records a selected checkout SHA");

const junk = fs.mkdtempSync(path.join(os.tmpdir(), "rts-agent-lab-invalid-"));
try {
  assert.throws(() => validateWorkspaceRoot(junk), (error) => error?.code === "invalidWorkspace");
} finally {
  fs.rmSync(junk, { recursive: true, force: true });
}

assert.equal(safeToken("safe_room-2", "fallback"), "safe_room-2", "safe Agent Lab names are retained");
assert.equal(safeToken("../escape", "fallback"), "fallback", "unsafe Agent Lab names are rejected");
assert.match(generatedRoomId("0123456789abcdef"), /^agentlab-[A-Za-z0-9_-]+$/, "generated rooms stay protocol-safe");

assert.equal(transitionDriverState(DRIVER_STATES.OPENING, "opened"), DRIVER_STATES.OPEN, "driver opens once");
assert.equal(transitionDriverState(DRIVER_STATES.OPEN, "closing"), DRIVER_STATES.CLOSING, "driver closes from open");
assert.equal(transitionDriverState(DRIVER_STATES.CLOSING, "closed"), DRIVER_STATES.CLOSED, "driver reaches closed state");
assert.throws(
  () => transitionDriverState(DRIVER_STATES.CLOSED, "opened"),
  (error) => error instanceof AgentLabDriverError && error.code === "invalidLifecycle",
  "driver rejects invalid process transitions",
);

await assert.rejects(
  withTimeout(new Promise(() => {}), 5, "contract timeout"),
  (error) => error?.code === "timeout",
  "driver normalizes timeouts",
);

const inspection = normalizeInspectionQuery({
  ids: Array.from({ length: 150 }, (_, index) => index + 1),
  owners: [1, 1, 2],
  kinds: ["rifleman", "rifleman", "tank"],
  limit: 100,
});
assert.equal(inspection.ids.size, 0, "oversized entity filters are dropped rather than expanding inspection");
assert.deepEqual([...inspection.owners], [1, 2], "inspection owner filters are deduplicated");
assert.equal(inspection.kinds.size, 2, "inspection kind filters are bounded and deduplicated");
assert.equal(inspection.limit, 100, "inspection result limits remain bounded");

assert.equal(agentLabLaunchEnabled({ search: "?agentLab=1" }), true, "explicit Agent Lab URL enables the bridge");
assert.equal(agentLabLaunchEnabled({ search: "?agentLab=0" }), false, "normal Lab URLs do not expose the bridge");
const windowLike = {};
const bridge = new AgentLabBridge({
  enabled: true,
  windowLike,
  app: {
    net: { ws: { readyState: 1 } },
    labClient: { state: { role: LAB_ROLE.OPERATOR, room: "contract" } },
    match: {
      state: {
        currRecvTime: 1,
        tick: 7,
        players: [{ id: 1, teamId: 1, factionId: DEFAULT_FACTION_ID, name: "Lab", color: "#fff" }],
        map: { name: "Default", width: 64, height: 64, tileSize: 32 },
      },
      capabilities: { roomTime: { available: true } },
      roomTimeControls: { roomTimeState: { currentTick: 7, speed: 0, paused: true } },
    },
  },
});
assert.deepEqual(Object.keys(windowLike[AGENT_LAB_BRIDGE_KEY]).sort(), ["call", "status", "version"], "bridge surface exposes no app internals");
const catalog = await windowLike[AGENT_LAB_BRIDGE_KEY].call("catalog", {});
const faction = catalog.value.factions.find((entry) => entry.id === DEFAULT_FACTION_ID);
assert.deepEqual(faction.units, labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID), "bridge catalog matches the human Lab spawn palette");
bridge.destroy();
assert.equal(windowLike[AGENT_LAB_BRIDGE_KEY], undefined, "bridge teardown removes the launch-gated global");

console.log("✅ agent_lab_driver_contracts.mjs: all contract assertions passed");
