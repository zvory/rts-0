import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  AgentLabDriver,
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
import { ABILITY, DEFAULT_FACTION_ID, LAB_ROLE } from "../client/src/protocol.js";
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
assert.throws(
  () => new AgentLabDriver({ workspaceRoot: root, timeoutMs: 60_001 }),
  (error) => error?.code === "invalidTimeout",
  "driver bounds per-operation waits",
);
const pageErrorDriver = new AgentLabDriver({ workspaceRoot: root });
pageErrorDriver.state = DRIVER_STATES.OPEN;
pageErrorDriver.page = { evaluate: async () => ({ ok: true, value: { ready: true, reason: "ready" } }) };
pageErrorDriver.pageErrors.push("frame failed");
assert.deepEqual(
  await pageErrorDriver.status(),
  { ready: false, reason: "pageError" },
  "driver does not report readiness after a page error",
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
assert.equal(inspection.cameraViewport, false, "inspection viewport filtering is opt-in");
assert.equal(normalizeInspectionQuery({ cameraViewport: true }).cameraViewport, true, "inspection accepts the bounded camera viewport filter");

assert.equal(agentLabLaunchEnabled({ pathname: "/lab", search: "?agentLab=1" }), true, "explicit Agent Lab URL enables the bridge");
assert.equal(agentLabLaunchEnabled({ pathname: "/lab", search: "?agentLab=0" }), false, "normal Lab URLs do not expose the bridge");
assert.equal(agentLabLaunchEnabled({ pathname: "/", search: "?agentLab=1" }), false, "non-Lab URLs never expose the bridge");
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
assert.deepEqual(catalog.value.abilities, Object.values(ABILITY), "bridge catalog exposes mirrored ability ids for the MCP command validator");
bridge.destroy();
assert.equal(windowLike[AGENT_LAB_BRIDGE_KEY], undefined, "bridge teardown removes the launch-gated global");

const replacementMatch = {
  state: { currRecvTime: 2, tick: 3 },
  capabilities: { roomTime: { available: true } },
  roomTimeControls: { roomTimeState: { currentTick: 3, speed: 0, paused: true } },
};
const seekApp = {
  net: { ws: { readyState: 1 } },
  labClient: { state: { role: LAB_ROLE.OPERATOR, room: "contract" } },
  match: null,
};
seekApp.match = {
  state: { currRecvTime: 1, tick: 7 },
  capabilities: { roomTime: { available: true } },
  roomTimeControls: { roomTimeState: { currentTick: 7, speed: 0, paused: true } },
  net: { seekRoomTimeTo: () => { seekApp.match = replacementMatch; } },
};
const seekBridge = new AgentLabBridge({ enabled: true, app: seekApp, windowLike: {}, sleep: async () => {} });
const seek = await seekBridge.time({ action: "seek", tick: 999 });
assert.equal(seek.snapshotTick, 3, "bridge returns the server-observed tick when a seek is clamped to retained history");
seekBridge.destroy();

const viewportCamera = {
  x: 0,
  y: 0,
  zoom: 1,
  viewW: 100,
  viewH: 100,
  worldToScreen(x, y) { return { x: (x - this.x) * this.zoom, y: (y - this.y) * this.zoom }; },
  screenToWorld(x, y) { return { x: this.x + x / this.zoom, y: this.y + y / this.zoom }; },
  setZoom(zoom) { this.zoom = zoom; },
  centerOn(x, y) { this.x = x - this.viewW / (2 * this.zoom); this.y = y - this.viewH / (2 * this.zoom); },
};
const viewportEntities = [
  { id: 1, kind: "rifleman", owner: 1, x: 20, y: 20, hp: 100, maxHp: 100, state: "idle", orderPlan: [] },
  { id: 2, kind: "rifleman", owner: 2, x: 240, y: 240, hp: 100, maxHp: 100, state: "idle", orderPlan: [] },
];
const viewportBridge = new AgentLabBridge({
  enabled: true,
  windowLike: {},
  app: {
    net: { ws: { readyState: 1 } },
    labClient: { state: { role: LAB_ROLE.OPERATOR, room: "contract" } },
    match: {
      camera: viewportCamera,
      state: {
        currRecvTime: 1,
        tick: 7,
        players: [],
        map: { name: "Default", width: 64, height: 64, tileSize: 32 },
        entitiesInterpolated: () => viewportEntities,
        entityById: (id) => viewportEntities.find((entity) => entity.id === id),
      },
      capabilities: { roomTime: { available: true } },
      roomTimeControls: { roomTimeState: { currentTick: 7, speed: 0, paused: true } },
    },
  },
});
const viewportInspection = await viewportBridge.inspect({ cameraViewport: true, limit: 10 });
assert.deepEqual(viewportInspection.entities.map((entity) => entity.id), [1], "bridge inspection can filter to the current camera viewport");
assert.deepEqual(viewportInspection.camera.worldBounds, { minX: 0, minY: 0, maxX: 100, maxY: 100 }, "bridge inspection reports applied camera world bounds");
const focused = viewportBridge.camera({ action: "focus", entityIds: [1, 2], padding: 10 });
assert.ok(focused.camera.zoom > 0 && focused.camera.worldBounds.maxX > focused.camera.worldBounds.minX, "bridge focus applies bounded padding and returns camera bounds");
viewportBridge.destroy();

console.log("✅ agent_lab_driver_contracts.mjs: all contract assertions passed");
