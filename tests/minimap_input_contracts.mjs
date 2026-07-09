// Dependency-free checks for minimap input routed through MatchInputRouter.
// These cover the pointer-lock virtual-cursor path without launching a browser.

import { MatchInputRouter } from "../client/src/input/router.js";
import { ClientIntent } from "../client/src/client_intent.js";
import { createLabControlPolicy } from "../client/src/lab_control_policy.js";
import { Minimap } from "../client/src/minimap.js";
import {
  ABILITIES,
  ARTILLERY_BLANKET_RADIUS_TILES,
  ARTILLERY_MIN_RANGE_TILES,
} from "../client/src/config.js";
import { ABILITY, cmd, KIND, LAB_ROLE, ORDER_STAGE, SETUP, TERRAIN, UPGRADE } from "../client/src/protocol.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function assertApprox(actual, expected, epsilon, msg) {
  assert(Math.abs(actual - expected) <= epsilon, `${msg}: expected ${expected}, got ${actual}`);
}

function installWindowStub() {
  const listeners = [];
  globalThis.window = {
    devicePixelRatio: 1,
    innerWidth: 800,
    innerHeight: 600,
    addEventListener(type, handler) {
      listeners.push(["add", type, handler]);
    },
    removeEventListener(type, handler) {
      listeners.push(["remove", type, handler]);
    },
  };
  return listeners;
}

function fakeCanvas(rect = { left: 100, top: 200, width: 242, height: 242 }) {
  const listeners = [];
  return {
    width: rect.width,
    height: rect.height,
    listeners,
    getContext() {
      return null;
    },
    getBoundingClientRect() {
      return {
        left: rect.left,
        top: rect.top,
        right: rect.left + rect.width,
        bottom: rect.top + rect.height,
        width: rect.width,
        height: rect.height,
      };
    },
    addEventListener(type, handler) {
      listeners.push(["add", type, handler]);
    },
    removeEventListener(type, handler) {
      listeners.push(["remove", type, handler]);
    },
  };
}

function recordingContext(label) {
  return {
    label,
    calls: [],
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    globalAlpha: 1,
    clearRect(...args) {
      this.calls.push({ op: "clearRect", args });
    },
    fillRect(...args) {
      this.calls.push({ op: "fillRect", args, fillStyle: this.fillStyle, globalAlpha: this.globalAlpha });
    },
    strokeRect(...args) {
      this.calls.push({ op: "strokeRect", args, strokeStyle: this.strokeStyle, lineWidth: this.lineWidth });
    },
    drawImage(source, ...args) {
      this.calls.push({ op: "drawImage", source: source?.label || "", args });
    },
    save() {
      this.calls.push({ op: "save" });
    },
    restore() {
      this.calls.push({ op: "restore" });
    },
    beginPath() {
      this.calls.push({ op: "beginPath" });
    },
    arc(...args) {
      this.calls.push({ op: "arc", args });
    },
    stroke() {
      this.calls.push({ op: "stroke" });
    },
    fill() {
      this.calls.push({ op: "fill", fillStyle: this.fillStyle, globalAlpha: this.globalAlpha });
    },
    translate(...args) {
      this.calls.push({ op: "translate", args });
    },
    rotate(...args) {
      this.calls.push({ op: "rotate", args });
    },
    moveTo(...args) {
      this.calls.push({ op: "moveTo", args });
    },
    lineTo(...args) {
      this.calls.push({ op: "lineTo", args });
    },
    closePath() {
      this.calls.push({ op: "closePath" });
    },
  };
}

function fakeRenderableCanvas({
  width = 16,
  height = width,
  rect = { left: 0, top: 0, width, height },
  context = recordingContext("main"),
} = {}) {
  const listeners = [];
  return {
    label: "main",
    width,
    height,
    rect,
    context,
    listeners,
    getContext() {
      return context;
    },
    getBoundingClientRect() {
      return {
        left: rect.left,
        top: rect.top,
        right: rect.left + rect.width,
        bottom: rect.top + rect.height,
        width: rect.width,
        height: rect.height,
      };
    },
    addEventListener(type, handler) {
      listeners.push(["add", type, handler]);
    },
    removeEventListener(type, handler) {
      listeners.push(["remove", type, handler]);
    },
  };
}

function staticCanvasFactory(layers) {
  return () => {
    const label = `static-${layers.length}`;
    const context = recordingContext(label);
    const canvas = {
      label,
      width: 1,
      height: 1,
      getContext() {
        return context;
      },
    };
    layers.push({ canvas, context });
    return canvas;
  };
}

function countCalls(context, op) {
  return context.calls.filter((call) => call.op === op).length;
}

function hasCallWithApproxArgs(context, op, expectedArgs, epsilon = 0.001) {
  return context.calls.some((call) => {
    if (call.op !== op || call.args.length !== expectedArgs.length) return false;
    return call.args.every((arg, index) => Math.abs(arg - expectedArgs[index]) <= epsilon);
  });
}

function minimapHarness({
  selected = [],
  commandTarget = null,
  commandsEnabled = true,
  controlPolicy = null,
  upgrades = [],
  legacySender = false,
  explicitClientIntent = true,
} = {}) {
  installWindowStub();
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const router = new MatchInputRouter(viewport);
  const canvas = fakeCanvas();
  const centers = [];
  const clientIntent = explicitClientIntent ? new ClientIntent() : null;
  const state = {
    playerId: 1,
    map: {
      width: 242,
      height: 242,
      tileSize: 1,
      terrain: new Array(242 * 242).fill(0),
      resources: [],
    },
    selectedEntities() {
      return selected;
    },
    upgrades,
    entitiesInterpolated() {
      return [];
    },
    players: [],
  };
  if (controlPolicy) state.controlPolicy = controlPolicy;
  if (commandTarget && clientIntent) clientIntent.beginCommandTarget(commandTarget);
  const camera = {
    centerOn(x, y) {
      centers.push({ x, y });
    },
  };
  const commandIssuer = legacySender
    ? {
        sent: [],
        command(command) {
          this.sent.push(command);
        },
      }
    : {
        sent: [],
        issueCommand(command) {
          this.sent.push(command);
        },
      };
  const minimap = new Minimap(canvas, state, camera, null, commandIssuer, router, {
    commandsEnabled,
    clientIntent,
  });
  return {
    router,
    canvas,
    state,
    camera,
    net: commandIssuer,
    commandIssuer,
    minimap,
    centers,
    clientIntent,
  };
}

function lockedEvent(clientX, clientY, button = 0, extra = {}) {
  return { clientX, clientY, button, source: "locked", ...extra };
}

// Left-click on minimap jumps the camera through the locked-cursor router.
{
  const h = minimapHarness();
  assert(h.router.pointerDown(lockedEvent(221, 321, 0)), "locked minimap left-click is consumed");
  assert(h.centers.length === 1, "minimap left-click centers the camera");
  assertApprox(h.centers[0].x, 121, 0.001, "minimap left-click world x");
  assertApprox(h.centers[0].y, 121, 0.001, "minimap left-click world y");
  h.minimap.destroy();
}

// Drag capture continues to pan after the cursor leaves the minimap, then releases cleanly.
{
  const h = minimapHarness();
  assert(h.router.pointerDown(lockedEvent(110, 210, 0)), "minimap drag starts on left-click");
  assert(h.router.pointerMove(lockedEvent(500, 500, 0)), "minimap drag move is captured outside bounds");
  assert(h.centers.length === 2, "minimap drag recenters on move");
  assertApprox(h.centers[1].x, 241, 0.001, "minimap drag clamps world x at map edge");
  assertApprox(h.centers[1].y, 241, 0.001, "minimap drag clamps world y at map edge");
  assert(h.router.pointerUp(lockedEvent(500, 500, 0)), "minimap drag release is consumed");
  assert(!h.router.pointerMove(lockedEvent(500, 500, 0)), "minimap drag capture releases after pointerUp");
  h.minimap.destroy();
}

// Shift-right-click on minimap with a selected unit issues a queued move order through the locked path.
{
  const selected = [{ id: 7, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected });
  assert(h.router.pointerDown(lockedEvent(180, 280, 2, { shiftKey: true })), "locked minimap right-click is consumed");
  assert(h.net.sent.length === 1, "minimap right-click sends one command");
  assert(h.net.sent[0].c === "move", "minimap right-click sends move");
  assert(h.net.sent[0].queued === true, "shift minimap right-click queues move");
  assert(h.net.sent[0].units.length === 1 && h.net.sent[0].units[0] === 7, "move uses selected unit");
  assertApprox(h.net.sent[0].x, 80, 0.001, "move command minimap x");
  assertApprox(h.net.sent[0].y, 80, 0.001, "move command minimap y");
  h.minimap.destroy();
}

// Legacy one-argument senders still work for minimap right-clicks while Match uses PredictionController.
{
  const selected = [{ id: 7, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, legacySender: true });
  assert(h.router.pointerDown(lockedEvent(180, 280, 2)), "legacy minimap right-click is consumed");
  assert(h.net.sent.length === 1 && h.net.sent[0].c === "move", "legacy minimap right-click sends move");
  h.minimap.destroy();
}

// Right-click with no selected controllable units is consumed by the minimap but sends no order.
{
  const h = minimapHarness();
  assert(h.router.pointerDown(lockedEvent(180, 280, 2)), "empty-selection minimap right-click is consumed");
  assert(h.net.sent.length === 0, "empty-selection minimap right-click sends no command");
  h.minimap.destroy();
}

// Replay minimaps keep camera clicks local and never issue gameplay commands.
{
  const selected = [{ id: 7, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandsEnabled: false });
  assert(h.router.pointerDown(lockedEvent(180, 280, 2)), "replay minimap right-click is consumed");
  assert(h.net.sent.length === 0, "replay minimap right-click sends no command");
  assert(h.router.pointerDown(lockedEvent(221, 321, 0)), "replay minimap left-click still recenters camera");
  assert(h.centers.length === 1, "replay minimap keeps local camera controls");
  h.minimap.destroy();
}

// Lab operators can issue minimap commands through the lab command-surface predicate
// even though lab starts keep normal gameplay command capabilities disabled.
{
  const selected = [{ id: 17, owner: 2, kind: KIND.RIFLEMAN }];
  const labPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  const h = minimapHarness({
    selected,
    controlPolicy: labPolicy,
    commandsEnabled: false,
  });
  assert(h.router.pointerDown(lockedEvent(180, 280, 2)), "lab operator minimap right-click is consumed");
  assert(h.net.sent.length === 1, "lab operator minimap right-click sends one command through the lab command issuer");
  assert(h.net.sent[0].c === "move", "lab operator minimap command routes selected units");
  assert(h.net.sent[0].units.length === 1 && h.net.sent[0].units[0] === 17, "lab minimap command uses selected owner units");
  h.minimap.destroy();
}

// Read-only lab starts keep minimap camera controls but do not issue commands.
{
  const selected = [{ id: 18, owner: 2, kind: KIND.RIFLEMAN }];
  const readOnlyPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } });
  const h = minimapHarness({
    selected,
    controlPolicy: readOnlyPolicy,
    commandsEnabled: false,
  });
  assert(h.router.pointerDown(lockedEvent(180, 280, 2)), "read-only lab minimap right-click is consumed");
  assert(h.net.sent.length === 0, "read-only lab minimap right-click sends no command");
  assert(h.router.pointerDown(lockedEvent(221, 321, 0)), "read-only lab minimap left-click still recenters camera");
  assert(h.centers.length === 1, "read-only lab keeps minimap camera controls");
  h.minimap.destroy();
}

// Command-target left-click on minimap issues the command and exits target mode.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack" });
  assert(h.router.pointerDown(lockedEvent(150, 250, 0)), "attack-move minimap left-click is consumed");
  assert(h.net.sent.length === 1, "attack-move minimap click sends one command");
  assert(h.net.sent[0].c === "attackMove", "attack command-target sends attack-move");
  assert(h.net.sent[0].queued !== true, "plain minimap attack target does not queue attack-move");
  assert(h.clientIntent.commandTarget === null, "attack command-target exits after minimap click");
  h.minimap.destroy();
}

// Artillery abilities issued through the minimap keep raw commands but show locked local feedback.
{
  const artillery = {
    id: 17,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
    setupFacing: 0,
  };
  const h = minimapHarness({
    selected: [artillery],
    commandTarget: { kind: "ability", ability: ABILITY.BLANKET_FIRE },
  });
  h.minimap._issueOrder(artillery.x + 5, artillery.y, true);
  assert(
    h.net.sent[0]?.c === "useAbility" &&
      h.net.sent[0].ability === ABILITY.BLANKET_FIRE &&
      h.net.sent[0].x === artillery.x + 5 &&
      h.net.sent[0].queued === true,
    "minimap Blanket Fire targeting sends the raw queued ability command",
  );
  assert(
    h.clientIntent.commandFeedback[0]?.kind === "artillery" &&
      h.clientIntent.commandFeedback[0].x === artillery.x + ARTILLERY_MIN_RANGE_TILES &&
      h.clientIntent.commandFeedback[0].y === artillery.y &&
      h.clientIntent.commandFeedback[0].radiusTiles === ARTILLERY_BLANKET_RADIUS_TILES,
    "minimap Blanket Fire feedback uses the locked artillery center and blanket radius",
  );
  h.minimap.destroy();
}

// Smoke Plus minimap targeting uses the upgraded cloud radius in local feedback.
{
  const scoutCar = { id: 21, owner: 1, kind: KIND.SCOUT_CAR, x: 100, y: 100 };
  const h = minimapHarness({
    selected: [scoutCar],
    commandTarget: { kind: "ability", ability: ABILITY.SMOKE },
    upgrades: [UPGRADE.SMOKE_PLUS],
  });
  h.minimap._issueOrder(120, 100, false);
  assert(
    h.net.sent[0]?.c === "useAbility" &&
      h.net.sent[0].ability === ABILITY.SMOKE &&
      h.net.sent[0].x === 120,
    "minimap Smoke targeting sends the ability command",
  );
  const upgradedRadiusTiles = ABILITIES[ABILITY.SMOKE].upgradedRadiusTiles;
  assert(
    h.clientIntent.commandFeedback[0]?.radiusTiles === upgradedRadiusTiles,
    "minimap Smoke feedback uses the upgraded Smoke Plus radius",
  );
  h.minimap.destroy();
}

// Setup targeting previews selected support weapons toward the hovered minimap point.
{
  const selected = [
    { id: 30, owner: 1, kind: KIND.RIFLEMAN, x: 10, y: 20 },
    { id: 31, owner: 1, kind: KIND.ANTI_TANK_GUN, x: 30, y: 40 },
    { id: 32, owner: 1, kind: KIND.ARTILLERY, x: 50, y: 60 },
  ];
  const h = minimapHarness({ selected, commandTarget: "setupAntiTankGuns" });
  assert(h.router.pointerMove(lockedEvent(190, 290, 0)), "setup minimap hover is consumed");
  const preview = h.clientIntent.antiTankGunSetupPreview;
  assert(preview?.source === "minimap", "setup minimap hover records the minimap as preview source");
  assertApprox(preview.mouseX, 90, 0.001, "setup minimap preview world x");
  assertApprox(preview.mouseY, 90, 0.001, "setup minimap preview world y");
  assert(preview.guns.length === 2, "setup minimap preview filters to support weapons");
  assert(
    preview.guns.some((e) => e.id === 31) && preview.guns.some((e) => e.id === 32),
    "setup minimap preview includes anti-tank guns and artillery",
  );
  h.minimap.destroy();
}

// Queued minimap setup previews aim from the accepted movement endpoint, matching world-view setup.
{
  const selected = [
    {
      id: 32,
      owner: 1,
      kind: KIND.ARTILLERY,
      x: 50,
      y: 60,
      orderPlan: [
        { kind: ORDER_STAGE.ATTACK_MOVE, x: 150, y: 160 },
      ],
    },
  ];
  const h = minimapHarness({ selected, commandTarget: "setupAntiTankGuns" });
  assert(h.router.pointerMove(lockedEvent(190, 290, 0, { shiftKey: true })), "queued setup minimap hover is consumed");
  const previewGun = h.clientIntent.antiTankGunSetupPreview?.guns[0];
  assert(
    previewGun?.x === 150 && previewGun?.y === 160 && selected[0].x === 50 && selected[0].y === 60,
    "queued minimap setup preview uses movement endpoints without mutating selection",
  );
  h.minimap.destroy();
}

// Queued minimap artillery fire uses the same local planned origin and setup facing as world targeting.
{
  const artillery = {
    id: 33,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 40,
    y: 50,
    setupState: SETUP.PACKED,
    facing: 0,
    orderPlan: [],
  };
  const plannedOrigin = { x: 140, y: 150 };
  const h = minimapHarness({
    selected: [artillery],
    commandTarget: { kind: "ability", ability: ABILITY.POINT_FIRE },
  });
  h.clientIntent.recordPlannedCommand(
    cmd.move([artillery.id], plannedOrigin.x, plannedOrigin.y, false),
    [artillery],
    { sent: true, clientSeq: 7 },
  );
  h.clientIntent.recordPlannedCommand(
    cmd.setupAntiTankGuns([artillery.id], plannedOrigin.x, plannedOrigin.y + 40, true),
    [artillery],
    { sent: true, clientSeq: 8 },
  );
  h.minimap._issueOrder(plannedOrigin.x, plannedOrigin.y, true);
  assert(
    h.net.sent[0]?.ability === ABILITY.POINT_FIRE &&
      h.net.sent[0].queued === true &&
      h.net.sent[0].x === plannedOrigin.x &&
      h.net.sent[0].y === plannedOrigin.y,
    "minimap queued Point Fire sends the same raw command semantics as world targeting",
  );
  assertApprox(
    h.clientIntent.commandFeedback[0]?.x,
    plannedOrigin.x,
    0.001,
    "minimap queued Point Fire feedback keeps the locally planned origin x",
  );
  assertApprox(
    h.clientIntent.commandFeedback[0]?.y,
    plannedOrigin.y + ARTILLERY_MIN_RANGE_TILES,
    0.001,
    "minimap queued Point Fire feedback locks from the frozen setup facing",
  );
  h.minimap.destroy();
}

// Setup targeting left-click on minimap issues setupAntiTankGuns for artillery, not a move order.
{
  const selected = [
    { id: 31, owner: 1, kind: KIND.ANTI_TANK_GUN, x: 30, y: 40 },
    { id: 32, owner: 1, kind: KIND.ARTILLERY, x: 50, y: 60 },
  ];
  const h = minimapHarness({ selected, commandTarget: "setupAntiTankGuns" });
  assert(h.router.pointerDown(lockedEvent(200, 300, 0)), "setup minimap click is consumed");
  assert(h.net.sent.length === 1, "setup minimap click sends one command");
  assert(h.net.sent[0].c === "setupAntiTankGuns", "setup minimap click sends setupAntiTankGuns");
  assert(h.net.sent[0].units.includes(31) && h.net.sent[0].units.includes(32), "setup minimap click includes support weapons");
  assertApprox(h.net.sent[0].x, 100, 0.001, "setup minimap command x");
  assertApprox(h.net.sent[0].y, 100, 0.001, "setup minimap command y");
  assert(h.clientIntent.commandTarget === null, "setup minimap click exits target mode");
  h.minimap.destroy();
}

// Injected ClientIntent owns minimap target issuing without touching GameState shims.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack", explicitClientIntent: true });
  assert(h.router.pointerDown(lockedEvent(150, 250, 0)), "facade attack-move minimap click is consumed");
  assert(h.net.sent.length === 1 && h.net.sent[0].c === "attackMove", "facade minimap targeting sends attack-move");
  assert(h.clientIntent.commandTarget === null, "facade minimap targeting exits through ClientIntent");
  h.minimap.destroy();
}

// Legacy one-argument senders still work for minimap attack-move target clicks.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack", legacySender: true });
  assert(h.router.pointerDown(lockedEvent(150, 250, 0)), "legacy attack-move minimap click is consumed");
  assert(h.net.sent.length === 1 && h.net.sent[0].c === "attackMove", "legacy minimap attack target sends attack-move");
  h.minimap.destroy();
}

// Shift command-target clicks on the minimap stay armed while the command composer preserves it.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack" });
  h.clientIntent.holdCommandTarget("attack", "KeyA", true);
  assert(h.router.pointerDown(lockedEvent(150, 250, 0, { shiftKey: true })), "first held-A minimap attack click is consumed");
  assert(h.router.pointerDown(lockedEvent(160, 260, 0, { shiftKey: true })), "second held-A minimap attack click is consumed");
  assert(h.net.sent.length === 2, "held-A minimap targeting sends multiple commands");
  assert(h.net.sent.every((command) => command.c === "attackMove" && command.queued === true), "held-A minimap targeting queues attack-move commands");
  assert(h.clientIntent.commandTarget === "attack", "held-A minimap targeting stays armed after queued clicks");
  h.minimap.destroy();
}

// Static terrain and resource marks are cached instead of repainted every render.
{
  installWindowStub();
  const layers = [];
  const rect = { left: 0, top: 0, width: 16, height: 16 };
  const canvas = fakeRenderableCanvas({ width: 16, height: 16, rect });
  const state = {
    playerId: 1,
    map: {
      width: 2,
      height: 2,
      tileSize: 1,
      terrain: [0, 1, 2, 0],
      resources: [
        { id: 10, kind: "steel", x: 0.5, y: 0.5, remaining: 100 },
        { id: 11, kind: "oil", x: 1.5, y: 1.5, remaining: 100 },
      ],
    },
    selectedEntities() {
      return [];
    },
    entitiesInterpolated() {
      return [];
    },
    players: [],
  };
  const fog = {
    isVisible() {
      return false;
    },
    isExplored() {
      return false;
    },
  };
  const camera = { x: 0, y: 0, zoom: 1, viewW: 2, viewH: 2, centerOn() {} };
  const minimap = new Minimap(canvas, state, camera, fog, { issueCommand() {} }, null, {
    staticCanvasFactory: staticCanvasFactory(layers),
  });

  minimap.render();
  assert(layers.length === 2, "minimap creates terrain and resource static layers");
  const terrainLayer = layers[0];
  const resourceLayer = layers[1];
  const terrainDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === terrainLayer.canvas.label,
  );
  const fogIndex = canvas.context.calls.findIndex((call, index) =>
    index > terrainDrawIndex && call.op === "fillRect",
  );
  const resourceDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === resourceLayer.canvas.label,
  );
  assert(terrainDrawIndex >= 0, "terrain static layer draws into the minimap");
  assert(fogIndex > terrainDrawIndex, "fog still draws above cached terrain");
  assert(resourceDrawIndex > fogIndex, "cached resources still draw above fog");
  const terrainFillsAfterFirst = countCalls(terrainLayer.context, "fillRect");
  const resourceFillsAfterFirst = countCalls(resourceLayer.context, "fillRect");

  minimap.render();
  assert(
    countCalls(terrainLayer.context, "fillRect") === terrainFillsAfterFirst,
    "second render reuses cached terrain layer",
  );
  assert(
    countCalls(resourceLayer.context, "fillRect") === resourceFillsAfterFirst,
    "second render reuses cached resource layer",
  );

  state.map.resources[0].remaining = 0;
  minimap.render();
  assert(
    countCalls(terrainLayer.context, "fillRect") === terrainFillsAfterFirst,
    "resource depletion does not rebuild terrain cache",
  );
  assert(
    countCalls(resourceLayer.context, "clearRect") >= 2,
    "resource depletion invalidates resource cache",
  );

  rect.width = 20;
  rect.height = 20;
  minimap.render();
  assert(
    countCalls(terrainLayer.context, "fillRect") > terrainFillsAfterFirst,
    "canvas presentation changes invalidate terrain cache",
  );
  minimap.destroy();
}

// Scout Planes draw as aircraft-shaped minimap blips instead of square ground-unit dots.
{
  installWindowStub();
  const layers = [];
  const canvas = fakeRenderableCanvas({ width: 16, height: 16 });
  const state = {
    playerId: 1,
    map: {
      width: 4,
      height: 4,
      tileSize: 1,
      terrain: new Array(16).fill(TERRAIN.GRASS),
      resources: [],
    },
    selectedEntities() {
      return [];
    },
    entitiesInterpolated() {
      return [
        { id: 500, owner: 1, kind: KIND.SCOUT_PLANE, x: 2, y: 2 },
        { id: 501, owner: 1, kind: KIND.RIFLEMAN, x: 3, y: 2 },
      ];
    },
    players: [{ id: 1, color: "#4878c8" }],
  };
  const minimap = new Minimap(
    canvas,
    state,
    { x: 0, y: 0, zoom: 1, viewW: 4, viewH: 4 },
    null,
    { issueCommand() {} },
    null,
    { staticCanvasFactory: staticCanvasFactory(layers) },
  );
  minimap.render();
  assert(
    hasCallWithApproxArgs(canvas.context, "moveTo", [12.32, 8]),
    "Scout Plane blip starts an aircraft path at the plane canvas position",
  );
  assert(
    hasCallWithApproxArgs(canvas.context, "lineTo", [5.12, 4.48])
      && hasCallWithApproxArgs(canvas.context, "lineTo", [6.56, 8])
      && hasCallWithApproxArgs(canvas.context, "lineTo", [5.12, 11.52]),
    "Scout Plane blip draws the expected multi-point aircraft silhouette",
  );
  assert(
    canvas.context.calls.some((call) => call.op === "stroke"),
    "Scout Plane blip includes an outline for readability",
  );
  assert(
    hasCallWithApproxArgs(canvas.context, "fillRect", [9.44, 5.44, 5.12, 5.12]),
    "ordinary ground units still draw square minimap blips at their canvas position",
  );
  assert(
    !hasCallWithApproxArgs(canvas.context, "fillRect", [5.44, 5.44, 5.12, 5.12]),
    "Scout Plane blips should not also use the ordinary square unit marker",
  );
  minimap.destroy();
}

// Cacheable fog grids repaint the fog layer only when fog revisions change.
{
  installWindowStub();
  const layers = [];
  const rect = { left: 0, top: 0, width: 16, height: 16 };
  const canvas = fakeRenderableCanvas({ width: 16, height: 16, rect });
  const state = {
    playerId: 1,
    map: {
      width: 4,
      height: 2,
      tileSize: 1,
      terrain: [
        TERRAIN.GRASS,
        TERRAIN.ROCK,
        TERRAIN.GRASS,
        TERRAIN.WATER,
        TERRAIN.GRASS,
        TERRAIN.GRASS,
        TERRAIN.GRASS,
        TERRAIN.GRASS,
      ],
      resources: [],
    },
    selectedEntities() {
      return [];
    },
    entitiesInterpolated() {
      return [];
    },
    players: [],
  };
  const visibleGrid = new Uint8Array(8);
  const exploredGrid = new Uint8Array(8);
  visibleGrid[0] = 1;
  exploredGrid[0] = 1;
  exploredGrid[1] = 1;
  const fog = {
    width: 4,
    height: 2,
    visibleGrid,
    exploredGrid,
    revision: 1,
    visibleRevision: 1,
    exploredRevision: 1,
    revealAll: false,
    isVisible(tx, ty) {
      return this.visibleGrid[ty * this.width + tx] === 1;
    },
    isExplored(tx, ty) {
      return this.exploredGrid[ty * this.width + tx] === 1;
    },
  };
  const camera = { x: 0, y: 0, zoom: 1, viewW: 4, viewH: 2, centerOn() {} };
  const minimap = new Minimap(canvas, state, camera, fog, { issueCommand() {} }, null, {
    staticCanvasFactory: staticCanvasFactory(layers),
  });

  minimap.render();
  assert(layers.length === 2, "minimap creates terrain and fog static layers");
  const terrainLayer = layers[0];
  const fogLayer = layers[1];
  const terrainDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === terrainLayer.canvas.label,
  );
  const fogDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === fogLayer.canvas.label,
  );
  assert(fogDrawIndex > terrainDrawIndex, "cached fog still draws above cached terrain");
  const fogFillsAfterFirst = countCalls(fogLayer.context, "fillRect");
  assert(fogFillsAfterFirst > 0, "fog cache paints hidden minimap runs");

  minimap.render();
  assert(
    countCalls(fogLayer.context, "fillRect") === fogFillsAfterFirst,
    "second render reuses cached fog layer",
  );

  visibleGrid[1] = 1;
  fog.revision += 1;
  fog.visibleRevision += 1;
  minimap.render();
  const fogFillsAfterVisibleChange = countCalls(fogLayer.context, "fillRect");
  assert(
    fogFillsAfterVisibleChange > fogFillsAfterFirst,
    "visibility revision invalidates cached fog layer",
  );

  fog.revealAll = true;
  visibleGrid.fill(1);
  exploredGrid.fill(1);
  fog.revision += 1;
  fog.visibleRevision += 1;
  fog.exploredRevision += 1;
  minimap.render();
  assert(
    countCalls(fogLayer.context, "fillRect") === fogFillsAfterVisibleChange,
    "reveal-all fog cache clears without repainting hidden fog",
  );
  minimap.destroy();
}

// Global artillery firing markers draw over fog using the supplied artillery icon image.
{
  installWindowStub();
  const layers = [];
  const canvas = fakeRenderableCanvas({ width: 16, height: 16 });
  const state = {
    playerId: 1,
    map: {
      width: 4,
      height: 4,
      tileSize: 1,
      terrain: new Array(16).fill(TERRAIN.GRASS),
      resources: [],
    },
    selectedEntities() {
      return [];
    },
    entitiesInterpolated() {
      return [];
    },
    players: [{ id: 2, color: "#d55e00" }],
  };
  const fog = {
    width: 4,
    height: 4,
    visibleGrid: new Uint8Array(16),
    exploredGrid: new Uint8Array(16),
    revision: 1,
    visibleRevision: 1,
    exploredRevision: 1,
    revealAll: false,
    isVisible() {
      return false;
    },
    isExplored() {
      return false;
    },
  };
  const minimap = new Minimap(canvas, state, { x: 0, y: 0, zoom: 1, viewW: 4, viewH: 4 }, fog, { issueCommand() {} }, null, {
    artilleryIconImage: { label: "artillery-icon" },
    staticCanvasFactory: staticCanvasFactory(layers),
  });
  minimap.markArtilleryFiring({ owner: 2, x: 2, y: 2, facing: 0.5 });
  minimap.render();
  const fogDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === "static-1",
  );
  const iconDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === "artillery-icon",
  );
  assert(fogDrawIndex >= 0, "minimap draws cached fog before artillery markers");
  assert(iconDrawIndex > fogDrawIndex, "artillery firing icon draws over fog for every player");
  assert(
    canvas.context.calls[iconDrawIndex].args[2] === 30 &&
      canvas.context.calls[iconDrawIndex].args[3] === 24,
    "artillery firing icon uses the doubled minimap dimensions",
  );
  assert(
    !canvas.context.calls.some((call) => call.op === "arc"),
    "artillery firing icon image does not draw an extra surrounding circle",
  );
  assert(
    canvas.context.calls.some((call) => call.op === "rotate" && Math.abs(call.args[0] - 0.5) < 0.001),
    "artillery firing icon uses the event facing",
  );
  minimap.destroy();
}

// Destroy unregisters the zone so rematches cannot double-fire stale minimap handlers.
{
  const h = minimapHarness();
  assert(h.router.pointerDown(lockedEvent(150, 250, 0)), "minimap zone is registered before destroy");
  h.minimap.destroy();
  assert(!h.router.pointerDown(lockedEvent(150, 250, 0)), "minimap zone is unregistered after destroy");
}

console.log("minimap_input_contracts: ok");
