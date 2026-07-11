import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  boundLogLine,
  LabInteractDriver,
  LabInteractDriverError,
  DRIVER_STATES,
  generatedRoomId,
  safeToken,
  transitionDriverState,
  validateWorkspaceRoot,
  withTimeout,
} from "../scripts/lab-interact/driver.mjs";
import {
  LAB_INTERACT_BRIDGE_KEY,
  LAB_INTERACT_BRIDGE_VERSION,
  LAB_INTERACT_LIMITS,
  LabInteractBridge,
  labInteractLaunchEnabled,
  normalizeInspectionQuery,
} from "../client/src/lab_interact_bridge.js";
import { Camera } from "../client/src/camera.js";
import { ABILITY, DEFAULT_FACTION_ID, KIND, LAB_ROLE } from "../client/src/protocol.js";
import { labSpawnUnitKindsForFaction } from "../client/src/lab_spawn_catalog.js";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const workspace = validateWorkspaceRoot(root);
assert.equal(workspace.root, fs.realpathSync(root), "lab-interact validates the selected checkout top level");
assert.match(workspace.head, /^[0-9a-f]{40}$/i, "lab-interact records a selected checkout SHA");

const junk = fs.mkdtempSync(path.join(os.tmpdir(), "rts-lab-interact-invalid-"));
try {
  assert.throws(() => validateWorkspaceRoot(junk), (error) => error?.code === "invalidWorkspace");
} finally {
  fs.rmSync(junk, { recursive: true, force: true });
}

assert.equal(safeToken("safe_room-2", "fallback"), "safe_room-2", "safe Lab Interact names are retained");
assert.equal(safeToken("../escape", "fallback"), "fallback", "unsafe Lab Interact names are rejected");
assert.match(generatedRoomId("0123456789abcdef"), /^labinteract-[A-Za-z0-9_-]+$/, "generated rooms stay protocol-safe");
const oversizedLogLine = `2026-07-11 INFO client network report ${"detail=".repeat(200)} final-marker`;
const boundedLogLine = boundLogLine(oversizedLogLine);
assert.ok(boundedLogLine.length <= 512, "driver bounds each diagnostic server-log line");
assert.ok(boundedLogLine.startsWith("2026-07-11 INFO"), "bounded diagnostic log lines retain event identity");
assert.ok(boundedLogLine.endsWith("final-marker"), "bounded diagnostic log lines retain the newest detail tail");
assert.match(boundedLogLine, /<truncated>/, "bounded diagnostic log lines disclose truncation");
assert.equal(boundLogLine(oversizedLogLine, 8).length, 8, "explicit diagnostic bounds are always honored");

const diagnosticDriver = new LabInteractDriver({ workspaceRoot: root });
diagnosticDriver.page = new EventEmitter();
diagnosticDriver.attachPageDiagnostics();
diagnosticDriver.page.emit("console", { type: () => "error", text: () => oversizedLogLine });
assert.equal(
  diagnosticDriver.pageConsoleErrors[0],
  boundedLogLine,
  "captured browser diagnostics apply the same per-entry size bound as server-log tails",
);

assert.equal(transitionDriverState(DRIVER_STATES.OPENING, "opened"), DRIVER_STATES.OPEN, "driver opens once");
assert.equal(transitionDriverState(DRIVER_STATES.OPEN, "closing"), DRIVER_STATES.CLOSING, "driver closes from open");
assert.equal(transitionDriverState(DRIVER_STATES.CLOSING, "closed"), DRIVER_STATES.CLOSED, "driver reaches closed state");
assert.throws(
  () => transitionDriverState(DRIVER_STATES.CLOSED, "opened"),
  (error) => error instanceof LabInteractDriverError && error.code === "invalidLifecycle",
  "driver rejects invalid process transitions",
);

await assert.rejects(
  withTimeout(new Promise(() => {}), 5, "contract timeout"),
  (error) => error?.code === "timeout",
  "driver normalizes timeouts",
);
assert.throws(
  () => new LabInteractDriver({ workspaceRoot: root, timeoutMs: 60_001 }),
  (error) => error?.code === "invalidTimeout",
  "driver bounds per-operation waits",
);
const pageErrorDriver = new LabInteractDriver({ workspaceRoot: root });
pageErrorDriver.state = DRIVER_STATES.OPEN;
pageErrorDriver.page = { evaluate: async () => ({ ok: true, value: { ready: true, reason: "ready" } }) };
pageErrorDriver.pageErrors.push("frame failed");
assert.deepEqual(
  await pageErrorDriver.status(),
  { ready: false, reason: "pageError" },
  "driver does not report readiness after a page error",
);

assert.throws(
  () => normalizeInspectionQuery({ ids: Array.from({ length: 401 }, (_, index) => index + 1) }),
  (error) => error?.code === "invalidInput",
  "oversized entity filters are rejected rather than broadening inspection",
);
assert.throws(
  () => normalizeInspectionQuery({ ids: ["not-an-id"] }),
  (error) => error?.code === "invalidInput",
  "invalid entity filters are rejected rather than broadening inspection",
);
const inspection = normalizeInspectionQuery({
  ids: [1, 1, 3],
  owners: [1, 1, 2],
  kinds: ["rifleman", "rifleman", "tank"],
  limit: 400,
});
assert.deepEqual([...inspection.ids], [1, 3], "inspection entity filters are deduplicated");
assert.deepEqual([...inspection.owners], [1, 2], "inspection owner filters are deduplicated");
assert.equal(inspection.kinds.size, 2, "inspection kind filters are bounded and deduplicated");
assert.equal(inspection.limit, 400, "inspection result limits support the large-scene operational bound");
assert.equal(LAB_INTERACT_LIMITS.focusEntities, 400, "bridge camera focus shares the 400-reference bound");
assert.equal(LAB_INTERACT_LIMITS.captureSubjects, 400, "bridge readiness checks share the 400-subject bound");
assert.equal(LAB_INTERACT_BRIDGE_VERSION, 3, "semantic camera tooling shape increments the bridge version");
assert.equal(inspection.cameraViewport, false, "inspection viewport filtering is opt-in");
assert.equal(normalizeInspectionQuery({ cameraViewport: true }).cameraViewport, true, "inspection accepts the bounded camera viewport filter");

assert.equal(labInteractLaunchEnabled({ pathname: "/lab", search: "?labInteract=1" }), true, "explicit Lab Interact URL enables the bridge");
assert.equal(labInteractLaunchEnabled({ pathname: "/lab", search: "?labInteract=0" }), false, "normal Lab URLs do not expose the bridge");
assert.equal(labInteractLaunchEnabled({ pathname: "/", search: "?labInteract=1" }), false, "non-Lab URLs never expose the bridge");
const windowLike = {};
const bridge = new LabInteractBridge({
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
assert.deepEqual(Object.keys(windowLike[LAB_INTERACT_BRIDGE_KEY]).sort(), ["call", "status", "version"], "bridge surface exposes no app internals");
const catalog = await windowLike[LAB_INTERACT_BRIDGE_KEY].call("catalog", {});
const faction = catalog.value.factions.find((entry) => entry.id === DEFAULT_FACTION_ID);
assert.deepEqual(faction.units, labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID), "bridge catalog matches the human Lab spawn palette");
assert.deepEqual(catalog.value.abilities, Object.values(ABILITY), "bridge catalog exposes mirrored ability ids for command validation");
bridge.destroy();
assert.equal(windowLike[LAB_INTERACT_BRIDGE_KEY], undefined, "bridge teardown removes the launch-gated global");

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
const seekBridge = new LabInteractBridge({ enabled: true, app: seekApp, windowLike: {}, sleep: async () => {} });
const seek = await seekBridge.time({ action: "seek", tick: 999 });
assert.equal(seek.snapshotTick, 3, "bridge returns the server-observed tick when a seek is clamped to retained history");
seekBridge.destroy();

const viewportCamera = new Camera(100, 100, { minZoom: 0.01, maxZoom: 16 });
viewportCamera.setMapBounds(1_000, 1_000);
const viewportEntities = [
  {
    id: 1, kind: "rifleman", owner: 1, x: 20, y: 20, hp: 100, maxHp: 100,
    state: "idle", targetId: 2, weaponFacing: 0.25, setupState: "deployed", orderPlan: [],
  },
  { id: 2, kind: "rifleman", owner: 2, x: 240, y: 240, hp: 100, maxHp: 100, state: "idle", orderPlan: [] },
  { id: 3, kind: KIND.CITY_CENTRE, owner: 1, x: 400, y: 400, hp: 100, maxHp: 100, state: "idle", orderPlan: [] },
];
const viewportBridge = new LabInteractBridge({
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
assert.deepEqual(
  {
    state: viewportInspection.entities[0].state,
    activity: viewportInspection.entities[0].activity,
    targetId: viewportInspection.entities[0].targetId,
    weaponFacing: viewportInspection.entities[0].weaponFacing,
    setupState: viewportInspection.entities[0].setupState,
  },
  { state: "idle", activity: "engaging", targetId: 2, weaponFacing: 0.25, setupState: "deployed" },
  "bridge inspection distinguishes autonomous combat activity from explicit order state",
);
assert.equal(viewportInspection.camera.version, 1, "bridge inspection reports CameraSnapshotV1");
assert.deepEqual(viewportInspection.cameraWorldBounds, { minX: 0, minY: 0, maxX: 100, maxY: 100 }, "bridge inspection reports semantic camera world bounds");
const focused = viewportBridge.camera({ action: "focus", entityIds: [1, 2], padding: 10 });
assert.ok(focused.camera.framingScale > 0 && focused.cameraWorldBounds.maxX > focused.cameraWorldBounds.minX, "bridge focus applies bounded padding and returns semantic camera data");
const groupFocused = viewportBridge.camera({ action: "focus", entityIds: [1, 2] });
assert.equal(groupFocused.camera.framingScale, 100 / 316, "bridge preserves the 48-world-pixel default for multi-subject framing");
const buildingFocused = viewportBridge.camera({ action: "focus", entityIds: [3] });
assert.equal(buildingFocused.camera.framingScale, 100 / 96, "bridge preserves the 48-world-pixel default for single-building framing");
const closeFocused = viewportBridge.camera({ action: "focus", entityIds: [1] });
assert.equal(closeFocused.camera.framingScale, 100 / 64, "bridge focus defaults to a 32-world-pixel close framing for readable single-subject captures");
viewportBridge.destroy();

console.log("✅ lab_interact_driver_contracts.mjs: all contract assertions passed");
