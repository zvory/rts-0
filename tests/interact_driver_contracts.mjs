import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  boundLogLine,
  InteractDriver,
  InteractDriverError,
  DRIVER_STATES,
  generatedRoomId,
  safeToken,
  transitionDriverState,
  validateWorkspaceRoot,
  withTimeout,
} from "../scripts/interact/driver.ts";
import { waitForInteractStartup } from "../scripts/interact/bridge_startup.ts";
import {
  INTERACT_BRIDGE_KEY,
  INTERACT_BRIDGE_VERSION,
  INTERACT_LIMITS,
  InteractBridge,
  interactLaunchEnabled,
  normalizeInspectionQuery,
} from "../client/src/interact_bridge.js";
import { Camera } from "../client/src/camera.js";
import { ABILITY, DEFAULT_FACTION_ID, KIND, LAB_ROLE } from "../client/src/protocol.js";
import { labSpawnUnitKindsForFaction } from "../client/src/lab_spawn_catalog.js";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const workspace = validateWorkspaceRoot(root);
assert.equal(workspace.root, fs.realpathSync(root), "interact validates the selected checkout top level");
assert.match(workspace.head, /^[0-9a-f]{40}$/i, "interact records a selected checkout SHA");

const junk = fs.mkdtempSync(path.join(os.tmpdir(), "rts-interact-invalid-"));
try {
  assert.throws(() => validateWorkspaceRoot(junk), (error) => error?.code === "invalidWorkspace");
} finally {
  fs.rmSync(junk, { recursive: true, force: true });
}

assert.equal(safeToken("safe_room-2", "fallback"), "safe_room-2", "safe Interact names are retained");
assert.equal(safeToken("../escape", "fallback"), "fallback", "unsafe Interact names are rejected");
assert.match(generatedRoomId("0123456789abcdef"), /^interact-lab-[A-Za-z0-9_-]+$/, "generated rooms stay protocol-safe");
const oversizedLogLine = `2026-07-11 INFO client network report ${"detail=".repeat(200)} final-marker`;
const boundedLogLine = boundLogLine(oversizedLogLine);
assert.ok(boundedLogLine.length <= 512, "driver bounds each diagnostic server-log line");
assert.ok(boundedLogLine.startsWith("2026-07-11 INFO"), "bounded diagnostic log lines retain event identity");
assert.ok(boundedLogLine.endsWith("final-marker"), "bounded diagnostic log lines retain the newest detail tail");
assert.match(boundedLogLine, /<truncated>/, "bounded diagnostic log lines disclose truncation");
assert.equal(boundLogLine(oversizedLogLine, 8).length, 8, "explicit diagnostic bounds are always honored");

let startupWaitTimeout = null;
const rejectedStartupStatus = await waitForInteractStartup({
  waitForFunction: async (predicate, options) => {
    startupWaitTimeout = options?.timeout;
    const previousWindow = globalThis.window;
    globalThis.window = { __rtsInteract: { status: () => ({ ready: false, launchError: "map not found" }) } };
    try {
      assert.equal(predicate(), true, "driver readiness polling treats a launch error as terminal");
    } finally {
      if (previousWindow === undefined) delete globalThis.window;
      else globalThis.window = previousWindow;
    }
  },
  evaluate: async () => ({ ready: false, launchError: "map not found" }),
}, 1234);
assert.equal(startupWaitTimeout, 1234, "driver readiness polling preserves the startup timeout");
assert.deepEqual(
  rejectedStartupStatus,
  { ready: false, launchError: "map not found" },
  "driver readiness inspection preserves the launch failure for coded error reporting",
);

const diagnosticDriver = new InteractDriver({ workspaceRoot: root });
diagnosticDriver.page = new EventEmitter();
diagnosticDriver.attachPageDiagnostics();
diagnosticDriver.page.emit("console", { type: () => "error", text: () => oversizedLogLine });
assert.equal(
  diagnosticDriver.pageConsoleErrors[0],
  boundedLogLine,
  "captured browser diagnostics apply the same per-entry size bound as server-log tails",
);
await assert.rejects(
  diagnosticDriver.screenshot({ sessionId: "invalid" }),
  (error) => error?.code === "invalidSession" && Boolean(error?.details?.diagnostics),
  "screenshot failures retain driver diagnostics without a driver-level command queue",
);
await assert.rejects(
  diagnosticDriver.recordStop(),
  (error) => error?.code === "recordingInactive" && Boolean(error?.details?.diagnostics),
  "record-stop admission failures retain driver diagnostics without a driver-level command queue",
);

const babylonDriver = new InteractDriver({ workspaceRoot: root, renderer: "babylon" });
babylonDriver.server = { baseUrl: "http://127.0.0.1:8081/" };
babylonDriver.workspace = workspace;
const babylonUrl = new URL(babylonDriver.launchUrl());
assert.equal(babylonUrl.searchParams.get("rtsRenderer"), "babylon", "Interact can launch the explicit Babylon route");
assert.equal(babylonUrl.searchParams.get("map"), "1v1", "blank Interact labs use the current default 1v1 map");
const spectatorDriver = new InteractDriver({ workspaceRoot: root, mode: "game", spectate: ["ai_2_1", "ai_turtle"] });
spectatorDriver.server = { baseUrl: "http://127.0.0.1:8081/" };
spectatorDriver.workspace = workspace;
const spectatorUrl = new URL(spectatorDriver.launchUrl());
assert.equal(spectatorUrl.searchParams.get("rtsRole"), "spectator", "AI-vs-AI Interact uses the ordinary spectator launch role");
assert.deepEqual(spectatorUrl.searchParams.getAll("rtsAi"), ["1:ai_2_1", "2:ai_turtle"], "AI-vs-AI Interact fills exactly two opposing AI seats");
assert.equal(spectatorUrl.searchParams.has("rtsName"), false, "AI-vs-AI Interact does not create a player seat");
const automaticSpectatorDriver = new InteractDriver({
  workspaceRoot: root,
  mode: "game",
  spectate: ["ai_2_1", "ai_turtle"],
  autoSpectator: true,
});
assert.equal(automaticSpectatorDriver.options.autoSpectator, true, "AI-vs-AI Interact retains the fight-following launch preference");

assert.equal(transitionDriverState(DRIVER_STATES.OPENING, "opened"), DRIVER_STATES.OPEN, "driver opens once");
assert.equal(transitionDriverState(DRIVER_STATES.OPEN, "closing"), DRIVER_STATES.CLOSING, "driver closes from open");
assert.equal(transitionDriverState(DRIVER_STATES.CLOSING, "closed"), DRIVER_STATES.CLOSED, "driver reaches closed state");
assert.throws(
  () => transitionDriverState(DRIVER_STATES.CLOSED, "opened"),
  (error) => error instanceof InteractDriverError && error.code === "invalidLifecycle",
  "driver rejects invalid process transitions",
);

await assert.rejects(
  withTimeout(new Promise(() => {}), 5, "contract timeout"),
  (error) => error?.code === "timeout",
  "driver normalizes timeouts",
);
const openAbortController = new AbortController();
let resolveLateBrowser;
let disposedLateBrowser = null;
const abortableDriver = new InteractDriver({ workspaceRoot: root, signal: openAbortController.signal });
const heldBrowserStartup = abortableDriver.openStep(
  new Promise((resolve) => { resolveLateBrowser = resolve; }),
  "browser startup",
  async (browser) => { disposedLateBrowser = browser; },
);
openAbortController.abort();
await assert.rejects(
  heldBrowserStartup,
  (error) => error?.code === "sessionClosed",
  "driver aborts browser startup without waiting for the underlying browser operation",
);
resolveLateBrowser("late-browser");
await new Promise((resolve) => setImmediate(resolve));
assert.equal(disposedLateBrowser, "late-browser", "driver disposes a browser that finishes launching after startup cancellation");
assert.throws(
  () => new InteractDriver({ workspaceRoot: root, timeoutMs: 60_001 }),
  (error) => error?.code === "invalidTimeout",
  "driver bounds per-operation waits",
);
const pageErrorDriver = new InteractDriver({ workspaceRoot: root });
pageErrorDriver.state = DRIVER_STATES.OPEN;
pageErrorDriver.page = { evaluate: async () => ({ ok: true, value: { ready: true, reason: "ready" } }) };
pageErrorDriver.pageErrors.push("frame failed");
assert.deepEqual(
  await pageErrorDriver.status(),
  { ready: false, reason: "pageError" },
  "driver does not report readiness after a page error",
);
const dragEvents = [];
const dragDriver = new InteractDriver({ workspaceRoot: root });
dragDriver.state = DRIVER_STATES.OPEN;
dragDriver.page = {
  evaluate: async () => ({ left: 10, top: 20, width: 800, height: 600 }),
  mouse: {
    move: async (x, y) => { dragEvents.push(["move", x, y]); },
    down: async ({ button }) => { dragEvents.push(["down", button]); },
    up: async ({ button }) => { dragEvents.push(["up", button]); },
  },
  keyboard: {
    down: async (key) => { dragEvents.push(["keyDown", key]); },
    up: async (key) => { dragEvents.push(["keyUp", key]); },
  },
};
const dragged = await dragDriver.drag({
  button: "left",
  from: { x: 100, y: 120 },
  to: { x: 300, y: 320 },
  steps: 2,
  durationMs: 0,
  holdKeys: ["attack", "shift"],
});
assert.deepEqual(dragEvents, [
  ["move", 110, 140],
  ["keyDown", "a"],
  ["keyDown", "Shift"],
  ["down", "left"],
  ["move", 210, 240],
  ["move", 310, 340],
  ["up", "left"],
  ["keyUp", "Shift"],
  ["keyUp", "a"],
], "driver translates viewport-local drag coordinates and releases bounded held keys in reverse order");
assert.deepEqual(dragged.viewport, { width: 800, height: 600 }, "driver reports the viewport used for the gesture");
await assert.rejects(
  dragDriver.drag({ from: { x: 100, y: 100 }, to: { x: 800, y: 100 } }),
  (error) => error?.code === "outsideViewport" && error?.details?.viewport?.width === 800,
  "driver rejects a drag endpoint outside the current rendered viewport",
);
const concludedCaptureDriver = new InteractDriver({ workspaceRoot: root });
concludedCaptureDriver.pageErrors = [];
concludedCaptureDriver.pageConsoleErrors = [];
concludedCaptureDriver.options.timeoutMs = 50;
concludedCaptureDriver.callBridge = async () => ({
  ready: true,
  phase: "concluded",
  frame: 12,
  frameErrors: [],
  renderErrors: [],
  missingTextureSubjectIds: [],
  failedAssets: [],
});
assert.equal(
  (await concludedCaptureDriver.waitForCaptureReadiness([])).frame,
  12,
  "driver accepts the stable stopped renderer frame behind a concluded score screen",
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
assert.equal(INTERACT_LIMITS.focusEntities, 400, "bridge camera focus shares the 400-reference bound");
assert.equal(INTERACT_LIMITS.captureSubjects, 400, "bridge readiness checks share the 400-subject bound");
assert.equal(INTERACT_BRIDGE_VERSION, 6, "browser-local selection increments the Lab bridge version");
assert.equal(inspection.cameraViewport, false, "inspection viewport filtering is opt-in");
assert.equal(normalizeInspectionQuery({ cameraViewport: true }).cameraViewport, true, "inspection accepts the bounded camera viewport filter");

assert.equal(interactLaunchEnabled({ pathname: "/lab", search: "?interact=lab" }), true, "the explicit Interact Lab URL enables the bridge");
assert.equal(interactLaunchEnabled({ pathname: "/lab", search: "?interact=0" }), false, "normal Lab URLs do not expose the bridge");
assert.equal(interactLaunchEnabled({ pathname: "/", search: "?interact=lab" }), false, "non-Lab URLs never expose the bridge");
assert.equal(interactLaunchEnabled({ pathname: "/lab", search: "?interact=1" }), false, "the pre-namespace launch flag no longer exposes the bridge");
const failedLaunchBridge = new InteractBridge({
  enabled: true,
  windowLike: {},
  app: { net: { ws: { readyState: 1 } }, labClient: null, match: null },
});
failedLaunchBridge.noteLaunchError('Cannot load lab map "flat": map not found: "flat"');
assert.deepEqual(
  failedLaunchBridge.status(),
  {
    version: INTERACT_BRIDGE_VERSION,
    enabled: true,
    ready: false,
    reason: "launchError",
    launchError: 'Cannot load lab map "flat": map not found: "flat"',
    websocketConnected: true,
    startReceived: false,
    labRole: "",
    room: "",
    snapshotTick: null,
    roomTime: null,
    camera: null,
    cameraViewport: null,
    cameraWorldBounds: null,
    selection: [],
  },
  "the launch-gated bridge exposes a bounded server startup failure",
);
failedLaunchBridge.destroy();
let failedStartupPageClosed = false;
let failedStartupBrowserClosed = false;
let failedStartupServerClosed = false;
const failedStartupPage = Object.assign(new EventEmitter(), {
  goto: async () => {},
  waitForFunction: async () => {},
  evaluate: async () => ({
    ready: false,
    reason: "launchError",
    launchError: 'Cannot load lab map "flat": map not found: "flat"',
  }),
  close: async () => { failedStartupPageClosed = true; },
});
const failedStartupBrowser = {
  version: async () => "contract-browser",
  newPage: async () => failedStartupPage,
  close: async () => { failedStartupBrowserClosed = true; },
};
const failedStartupDriver = new InteractDriver({
  workspaceRoot: root,
  chromeFinder: () => "/contract/chrome",
  puppeteerLoader: async () => ({ launch: async () => failedStartupBrowser }),
  privateServerFactory: async () => ({
    baseUrl: "http://127.0.0.1:8081/",
    logPath: "",
    reused: false,
    close: async () => { failedStartupServerClosed = true; },
  }),
});
try {
  await assert.rejects(
    failedStartupDriver.open(),
    (error) => error?.code === "launchFailed" && error?.details?.status?.reason === "launchError",
    "the driver maps a terminal bridge startup error without waiting for readiness",
  );
} finally {
  await failedStartupDriver.close();
  if (failedStartupDriver.sessionDir) fs.rmSync(failedStartupDriver.sessionDir, { recursive: true, force: true });
}
assert.equal(failedStartupPageClosed, true, "failed startup closes the browser page");
assert.equal(failedStartupBrowserClosed, true, "failed startup closes the browser");
assert.equal(failedStartupServerClosed, true, "failed startup closes the private server");
const windowLike = {};
const bridge = new InteractBridge({
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
        map: { name: "Chokes", width: 64, height: 64, tileSize: 32 },
      },
      capabilities: { roomTime: { available: true } },
      roomTimeControls: { roomTimeState: { currentTick: 7, speed: 0, paused: true } },
    },
  },
});
bridge.noteLaunchError("late server error");
assert.equal(bridge.status().ready, true, "server errors after readiness do not poison an active Interact session");
assert.deepEqual(Object.keys(windowLike[INTERACT_BRIDGE_KEY]).sort(), ["call", "status", "version"], "bridge surface exposes no app internals");
const catalog = await windowLike[INTERACT_BRIDGE_KEY].call("catalog", {});
const faction = catalog.value.factions.find((entry) => entry.id === DEFAULT_FACTION_ID);
assert.deepEqual(faction.units, labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID), "bridge catalog matches the human Lab spawn palette");
assert.deepEqual(catalog.value.abilities, Object.values(ABILITY), "bridge catalog exposes mirrored ability ids for command validation");
bridge.destroy();
assert.equal(windowLike[INTERACT_BRIDGE_KEY], undefined, "bridge teardown removes the launch-gated global");

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
const seekBridge = new InteractBridge({ enabled: true, app: seekApp, windowLike: {}, sleep: async () => {} });
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
const viewportBridge = new InteractBridge({
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
        selection: new Set(),
        players: [],
        map: { name: "Chokes", width: 64, height: 64, tileSize: 32 },
        entitiesInterpolated: () => viewportEntities,
        entityById: (id) => viewportEntities.find((entity) => entity.id === id),
        setSelection(ids) { this.selection = new Set(ids); },
        selectedEntities() { return viewportEntities.filter((entity) => this.selection.has(entity.id)); },
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
const selected = await viewportBridge.select({ entityIds: [1, 2] });
assert.deepEqual(selected.selection, [1, 2], "Lab bridge selection replaces browser-local selection with bounded entity ids");
assert.deepEqual(selected.entities.map((entity) => entity.id), [1, 2], "Lab bridge selection returns its exact projected entities");
assert.deepEqual(viewportBridge.status().selection, [1, 2], "Lab status carries selection into capture provenance");
assert.deepEqual(viewportBridge.captureReadiness().selection, [1, 2], "Lab capture readiness records the rendered selection");
assert.deepEqual(viewportBridge.inspect({ limit: 10 }).selection, [1, 2], "Lab inspection reports the current browser-local selection");
assert.deepEqual((await viewportBridge.select({ entityIds: [] })).selection, [], "Lab bridge accepts an empty selection as clear");
const focused = viewportBridge.camera({ action: "focus", entityIds: [1, 2], padding: 10 });
assert.ok(focused.camera.framingScale > 0 && focused.cameraWorldBounds.maxX > focused.cameraWorldBounds.minX, "bridge focus applies bounded padding and returns semantic camera data");
const groupFocused = viewportBridge.camera({ action: "focus", entityIds: [1, 2] });
assert.equal(groupFocused.camera.framingScale, 100 / 316, "bridge preserves the 48-world-pixel default for multi-subject framing");
const buildingFocused = viewportBridge.camera({ action: "focus", entityIds: [3] });
assert.equal(buildingFocused.camera.framingScale, 100 / 96, "bridge preserves the 48-world-pixel default for single-building framing");
const closeFocused = viewportBridge.camera({ action: "focus", entityIds: [1] });
assert.equal(closeFocused.camera.framingScale, 100 / 64, "bridge focus defaults to a 32-world-pixel close framing for readable single-subject captures");
viewportBridge.destroy();

console.log("✅ interact_driver_contracts.mjs: all contract assertions passed");
