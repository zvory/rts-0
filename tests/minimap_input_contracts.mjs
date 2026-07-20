// Dependency-free checks for minimap input routed through MatchInputRouter.
// These cover the pointer-lock virtual-cursor path without launching a browser.

import { MatchInputRouter } from "../client/src/input/router.js";
import { ClientIntent } from "../client/src/client_intent.js";
import { CommandInteraction } from "../client/src/command_interaction.js";
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
  const windowRef = {
    devicePixelRatio: 1,
    innerWidth: 800,
    innerHeight: 600,
    listeners,
    addEventListener(type, handler) {
      listeners.push(["add", type, handler]);
    },
    removeEventListener(type, handler) {
      listeners.push(["remove", type, handler]);
    },
  };
  globalThis.window = windowRef;
  return windowRef;
}

function fakeCanvas(rect = { left: 100, top: 200, width: 242, height: 242 }) {
  const listeners = [];
  const capturedPointers = [];
  const releasedPointers = [];
  return {
    width: rect.backingWidth ?? rect.width,
    height: rect.backingHeight ?? rect.height,
    listeners,
    capturedPointers,
    releasedPointers,
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
    setPointerCapture(pointerId) {
      capturedPointers.push(pointerId);
    },
    releasePointerCapture(pointerId) {
      releasedPointers.push(pointerId);
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
      this.calls.push({ op: "stroke", strokeStyle: this.strokeStyle, lineWidth: this.lineWidth });
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

function dynamicCanvasOwnerDocument(layers) {
  return {
    createElement() {
      const label = `dynamic-${layers.length}`;
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
    },
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
  rect = undefined,
} = {}) {
  const windowRef = installWindowStub();
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const router = new MatchInputRouter(viewport);
  const canvas = fakeCanvas(rect);
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
  if (commandTarget && clientIntent) clientIntent.beginCommandTarget(commandTarget);
  const camera = {
    focusAt(point) {
      centers.push({ x: point.x, y: point.y });
    },
    viewportGroundPolygon() {
      return [];
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
  const commandInteraction = new CommandInteraction({
    commandIssuer,
    clientIntent,
    selectedEntities: () => state.selectedEntities(),
  });
  const minimap = new Minimap(canvas, state, camera, null, commandInteraction, router, {
    commandsEnabled,
    clientIntent,
    controlPolicy,
  });
  return {
    router,
    canvas,
    window: windowRef,
    state,
    camera,
    net: commandIssuer,
    commandIssuer,
    commandInteraction,
    minimap,
    centers,
    clientIntent,
  };
}

function lockedEvent(clientX, clientY, button = 0, extra = {}) {
  return { clientX, clientY, button, source: "locked", ...extra };
}

function listenerFor(target, type, operation = "add") {
  return target.listeners.find(([entryOperation, entryType]) => entryOperation === operation && entryType === type)?.[2];
}

function pointerEvent(canvas, clientX, clientY, {
  pointerId = 1,
  pointerType = "touch",
  isPrimary = true,
  button = 0,
  shiftKey = false,
  ctrlKey = false,
  metaKey = false,
  altKey = false,
} = {}) {
  return {
    target: canvas,
    currentTarget: canvas,
    clientX,
    clientY,
    pointerId,
    pointerType,
    isPrimary,
    button,
    shiftKey,
    ctrlKey,
    metaKey,
    altKey,
    defaultPrevented: false,
    propagationStopped: false,
    preventDefault() {
      this.defaultPrevented = true;
    },
    stopPropagation() {
      this.propagationStopped = true;
    },
  };
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

// Native minimap input uses Pointer Events, preserves CSS-scaled coordinates, and captures drags.
{
  const h = minimapHarness({
    rect: { left: 100, top: 200, width: 121, height: 121, backingWidth: 242, backingHeight: 242 },
  });
  h.window.devicePixelRatio = 2;
  const down = listenerFor(h.canvas, "pointerdown");
  const move = listenerFor(h.canvas, "pointermove");
  const up = listenerFor(h.canvas, "pointerup");
  assert(down && move && up && listenerFor(h.canvas, "pointercancel"), "minimap installs the native Pointer Events lifecycle");
  assert(!listenerFor(h.canvas, "mousedown"), "minimap no longer relies on compatibility mouse down events");
  assert(!listenerFor(h.window, "mousemove"), "minimap no longer relies on window mousemove events");

  const press = pointerEvent(h.canvas, 160.5, 260.5, { pointerId: 11, pointerType: "pen" });
  down(press);
  assert(press.defaultPrevented, "primary minimap pointer press prevents browser gesture handling");
  assert(h.canvas.capturedPointers.includes(11), "primary minimap pointer press captures its pointer");
  assert(h.centers.length === 1, "primary minimap pointer press recenters the camera");
  assertApprox(h.centers[0].x, 121, 0.001, "CSS-scaled minimap pointer press resolves world x");
  assertApprox(h.centers[0].y, 121, 0.001, "CSS-scaled minimap pointer press resolves world y");

  const drag = pointerEvent(h.canvas, 500, 500, { pointerId: 11, pointerType: "pen" });
  move(drag);
  assert(drag.defaultPrevented, "captured minimap pointer moves prevent browser gesture handling");
  assert(h.centers.length === 2, "captured minimap drag recenters outside the canvas");
  assertApprox(h.centers[1].x, 241, 0.001, "captured minimap drag clamps world x at map edge");
  assertApprox(h.centers[1].y, 241, 0.001, "captured minimap drag clamps world y at map edge");
  up(pointerEvent(h.canvas, 500, 500, { pointerId: 11, pointerType: "pen" }));
  assert(h.canvas.releasedPointers.includes(11), "primary minimap pointer release drops capture");
  h.minimap.destroy();
}

// The viewport footprint is the semantic ground polygon; empty/partial views stay bounded.
{
  installWindowStub().devicePixelRatio = 2;
  const canvas = fakeRenderableCanvas({ width: 200, height: 200 });
  const polygon = [
    { x: 10, y: 5 },
    { x: 30, y: 5 },
    { x: 30, y: 20 },
    { x: 10, y: 20 },
  ];
  const camera = {
    viewportGroundPolygon() {
      return polygon;
    },
    focusAt() {},
  };
  const state = {
    map: { width: 100, height: 100, tileSize: 1 },
    players: [],
  };
  const minimap = new Minimap(canvas, state, camera, null, { issueCommand() {} });
  assert(minimap._ensureTransform(), "minimap establishes its map transform for viewport drawing");
  minimap._drawViewport();
  assert(
    hasCallWithApproxArgs(canvas.context, "moveTo", [20.5, 10.5]) &&
      hasCallWithApproxArgs(canvas.context, "lineTo", [60.5, 40.5]),
    "orthographic semantic viewport polygon matches the legacy minimap rectangle",
  );
  assert(countCalls(canvas.context, "closePath") === 1, "bounded viewport polygon closes its own footprint");

  const strokes = countCalls(canvas.context, "stroke");
  polygon.splice(0, polygon.length);
  minimap._drawViewport();
  assert(countCalls(canvas.context, "stroke") === strokes, "empty ground footprint draws no invented bounds");

  polygon.push({ x: 10, y: 5 }, { x: 30, y: 5 });
  minimap._drawViewport();
  assert(countCalls(canvas.context, "stroke") === strokes + 1, "partial two-point ground footprint draws safely");
  assert(countCalls(canvas.context, "closePath") === 1, "partial footprint is not closed into fabricated bounds");

  polygon.push({ x: Number.NaN, y: 20 });
  minimap._drawViewport();
  assert(countCalls(canvas.context, "stroke") === strokes + 1, "malformed footprint is rejected as a whole");
  minimap.destroy();
}

// An armed target only fires after a clean touch/pen tap, never after a drag, cancellation, or pinch.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack" });
  const down = listenerFor(h.canvas, "pointerdown");
  const move = listenerFor(h.canvas, "pointermove");
  const up = listenerFor(h.canvas, "pointerup");
  const cancel = listenerFor(h.canvas, "pointercancel");

  down(pointerEvent(h.canvas, 150, 250, { pointerId: 21 }));
  assert(h.net.sent.length === 0, "armed touch target waits for clean pointer release");
  up(pointerEvent(h.canvas, 150, 250, { pointerId: 21 }));
  assert(h.net.sent.length === 1 && h.net.sent[0].c === "attackMove", "clean touch target issues the existing command");
  assert(h.clientIntent.commandTarget === null, "clean touch target exits target mode");

  h.clientIntent.beginCommandTarget("attack");
  down(pointerEvent(h.canvas, 150, 250, { pointerId: 22 }));
  move(pointerEvent(h.canvas, 170, 270, { pointerId: 22 }));
  up(pointerEvent(h.canvas, 170, 270, { pointerId: 22 }));
  assert(h.net.sent.length === 1, "dragging an armed touch target does not issue a command");
  assert(h.clientIntent.commandTarget === "attack", "dragging an armed touch target keeps the target armed");

  down(pointerEvent(h.canvas, 150, 250, { pointerId: 23 }));
  cancel(pointerEvent(h.canvas, 150, 250, { pointerId: 23 }));
  assert(h.canvas.releasedPointers.includes(23), "pointer cancellation releases minimap capture");
  assert(h.net.sent.length === 1, "cancelling an armed touch target does not issue a command");

  down(pointerEvent(h.canvas, 150, 250, { pointerId: 24 }));
  down(pointerEvent(h.canvas, 160, 260, { pointerId: 25, isPrimary: false }));
  up(pointerEvent(h.canvas, 150, 250, { pointerId: 24 }));
  assert(h.net.sent.length === 1, "a pinch never resolves as an armed minimap target tap");
  h.minimap.destroy();

  const ability = minimapHarness({
    selected: [{ id: 10, owner: 1, kind: KIND.SCOUT_CAR }],
    commandTarget: { kind: "ability", ability: ABILITY.SMOKE },
  });
  const abilityDown = listenerFor(ability.canvas, "pointerdown");
  const abilityUp = listenerFor(ability.canvas, "pointerup");
  abilityDown(pointerEvent(ability.canvas, 150, 250, { pointerId: 26 }));
  abilityUp(pointerEvent(ability.canvas, 150, 250, { pointerId: 26 }));
  assert(
    ability.net.sent.length === 1 &&
      ability.net.sent[0].c === "useAbility" &&
      ability.net.sent[0].ability === ABILITY.SMOKE,
    "clean touch target issues object-valued ability commands",
  );
  assert(ability.clientIntent.commandTarget === null, "clean ability target tap exits target mode");
  ability.minimap.destroy();
}

// Desktop Pointer Events keep right-click and Shift queueing semantics without a mouse fallback.
{
  const selected = [{ id: 7, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected });
  const down = listenerFor(h.canvas, "pointerdown");
  const rightClick = pointerEvent(h.canvas, 180, 280, {
    pointerId: 31,
    pointerType: "mouse",
    button: 2,
    shiftKey: true,
  });
  down(rightClick);
  assert(rightClick.defaultPrevented && rightClick.propagationStopped, "desktop minimap right-click still suppresses the browser menu");
  assert(h.net.sent.length === 1 && h.net.sent[0].c === "move", "desktop minimap right-click still issues move");
  assert(h.net.sent[0].queued === true, "desktop Shift-right-click still queues move");
  h.minimap.destroy();

  const replay = minimapHarness({ selected, commandsEnabled: false });
  const replayDown = listenerFor(replay.canvas, "pointerdown");
  replayDown(pointerEvent(replay.canvas, 180, 280, { pointerId: 32, pointerType: "mouse", button: 2 }));
  assert(replay.net.sent.length === 0, "replay minimap Pointer Events remain read-only");
  replayDown(pointerEvent(replay.canvas, 221, 321, { pointerId: 33, pointerType: "mouse" }));
  assert(replay.centers.length === 1, "replay minimap Pointer Events retain local camera inspection");
  replay.minimap.destroy();
}

// Window blur and teardown cancel an in-flight primary gesture and release capture.
{
  const h = minimapHarness();
  const down = listenerFor(h.canvas, "pointerdown");
  down(pointerEvent(h.canvas, 150, 250, { pointerId: 41 }));
  const blur = listenerFor(h.window, "blur");
  assert(blur, "minimap installs a blur cleanup handler");
  blur();
  assert(h.canvas.releasedPointers.includes(41), "window blur releases captured minimap pointers");
  down(pointerEvent(h.canvas, 150, 250, { pointerId: 42 }));
  h.minimap.destroy();
  assert(h.canvas.releasedPointers.includes(42), "minimap teardown releases captured minimap pointers");
  assert(listenerFor(h.canvas, "pointerdown", "remove"), "minimap teardown removes its Pointer Events listeners");
  assert(listenerFor(h.window, "blur", "remove"), "minimap teardown removes its blur cleanup listener");
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

// Production rallies do not target resource nodes on the minimap.
{
  const cityCentre = { id: 12, owner: 1, kind: KIND.CITY_CENTRE };
  const h = minimapHarness({ selected: [cityCentre] });
  const steel = { id: 80, owner: 0, kind: KIND.STEEL, x: 120, y: 130, remaining: 1000 };
  const oil = { id: 81, owner: 0, kind: KIND.OIL, x: 150, y: 160, remaining: 1000 };
  h.state.map.resources = [steel, oil];

  h.minimap._issueOrder(steel.x, steel.y, false);
  assert(h.net.sent.length === 0, "minimap steel rally sends no command");
  h.minimap._issueOrder(oil.x, oil.y, false);
  assert(h.net.sent.length === 0, "minimap oil rally sends no command");
  h.minimap.destroy();
}

// Unified Artillery Fire uses the same two-click center/radius gesture on the minimap.
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
    commandTarget: { kind: "ability", ability: ABILITY.POINT_FIRE },
  });
  const center = { x: artillery.x + 5, y: artillery.y };
  assert(
    h.minimap._issuePrimaryTarget(center, { shiftKey: true }) === true &&
      h.net.sent.length === 0 &&
      h.clientIntent.commandTarget?.ability === ABILITY.POINT_FIRE,
    "minimap Artillery Fire first click stores a center and keeps targeting armed",
  );
  h.minimap._issuePrimaryTarget({ x: center.x + 6, y: center.y }, { shiftKey: true });
  assert(
    h.net.sent[0]?.c === "artilleryFire" &&
      h.net.sent[0].x === center.x &&
      h.net.sent[0].radiusTiles === 6 &&
      h.net.sent[0].queued === true &&
      h.clientIntent.commandTarget?.ability === ABILITY.POINT_FIRE &&
      h.clientIntent.artilleryFireCenter === null,
    "minimap Artillery Fire second click sends the raw center and selected radius",
  );
  assert(
    h.clientIntent.commandFeedback[0]?.kind === "artillery" &&
      h.clientIntent.commandFeedback[0].x === artillery.x + ARTILLERY_MIN_RANGE_TILES &&
      h.clientIntent.commandFeedback[0].y === artillery.y &&
      h.clientIntent.commandFeedback[0].radiusTiles === 6,
    "minimap Artillery Fire feedback uses the locked center and selected radius",
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
    { id: 33, owner: 1, kind: KIND.MORTAR_TEAM, x: 70, y: 80 },
  ];
  const h = minimapHarness({ selected, commandTarget: "setupAntiTankGuns" });
  assert(h.router.pointerMove(lockedEvent(190, 290, 0)), "setup minimap hover is consumed");
  const preview = h.clientIntent.antiTankGunSetupPreview;
  assert(preview?.source === "minimap", "setup minimap hover records the minimap as preview source");
  assertApprox(preview.mouseX, 90, 0.001, "setup minimap preview world x");
  assertApprox(preview.mouseY, 90, 0.001, "setup minimap preview world y");
  assert(preview.guns.length === 3, "setup minimap preview filters to support weapons");
  assert(
    preview.guns.some((e) => e.id === 31) &&
      preview.guns.some((e) => e.id === 32) &&
      preview.guns.some((e) => e.id === 33),
    "setup minimap preview includes anti-tank guns, artillery, and mortars",
  );
  assert(h.router.releaseSource("locked"), "pointer-lock exit releases minimap preview ownership");
  h.clientIntent.updateAntiTankGunSetupPreview({
    source: "viewport",
    mouseX: 650,
    mouseY: 450,
    guns: selected.slice(1),
  });
  h.minimap.updateCommandTargetPreview();
  assert(
    h.clientIntent.antiTankGunSetupPreview?.source === "viewport" &&
      h.clientIntent.antiTankGunSetupPreview?.mouseX === 650,
    "pointer-lock exit cannot leave a stale minimap point that overwrites native battlefield hover",
  );
  assert(h.router.pointerMove(lockedEvent(190, 290, 0)), "re-entering the minimap restores routed hover ownership");
  assert(!h.router.pointerMove(lockedEvent(500, 500, 0)), "leaving the minimap releases locked-cursor preview ownership");
  h.clientIntent.updateAntiTankGunSetupPreview({
    source: "viewport",
    mouseX: 700,
    mouseY: 500,
    guns: selected.slice(1),
  });
  h.minimap.updateCommandTargetPreview();
  assert(
    h.clientIntent.antiTankGunSetupPreview?.source === "viewport" &&
      h.clientIntent.antiTankGunSetupPreview?.mouseX === 700,
    "a stale locked-cursor minimap point does not overwrite the battlefield setup preview",
  );
  h.minimap.destroy();
}

// Native minimap drags release setup-preview ownership when the pointer returns to the battlefield.
{
  const selected = [
    { id: 41, owner: 1, kind: KIND.ANTI_TANK_GUN, x: 30, y: 40 },
    { id: 42, owner: 1, kind: KIND.ARTILLERY, x: 50, y: 60 },
  ];
  const h = minimapHarness({ selected, commandTarget: "setupAntiTankGuns" });
  const down = listenerFor(h.canvas, "pointerdown");
  const move = listenerFor(h.canvas, "pointermove");
  const leave = listenerFor(h.canvas, "pointerleave");
  const up = listenerFor(h.canvas, "pointerup");
  assert(down && move && leave && up, "minimap installs native hover ownership lifecycle listeners");

  move(pointerEvent(h.canvas, 180, 280, { pointerId: 60, pointerType: "mouse" }));
  assert(h.router.activePreviewSurface() === "minimap", "native hover gives minimap preview-surface ownership");
  assert(
    h.clientIntent.antiTankGunSetupPreview?.source === "minimap",
    "native mouse hover previews support-weapon setup with pointer-lock parity",
  );
  leave(pointerEvent(h.canvas, 350, 280, { pointerId: 60, pointerType: "mouse" }));
  assert(h.router.activePreviewSurface() === null, "native leave releases minimap preview-surface ownership");

  down(pointerEvent(h.canvas, 180, 280, { pointerId: 62, pointerType: "mouse" }));
  leave(pointerEvent(h.canvas, 350, 280, { pointerId: 62, pointerType: "mouse" }));
  assert(
    h.router.activePreviewSurface() === "minimap",
    "native pointer capture keeps minimap preview-surface ownership after a drag leaves the canvas",
  );
  up(pointerEvent(h.canvas, 350, 280, { pointerId: 62, pointerType: "mouse" }));
  assert(
    h.router.activePreviewSurface() === null,
    "a native drag released outside the canvas relinquishes minimap preview-surface ownership",
  );

  down(pointerEvent(h.canvas, 180, 280, { pointerId: 61, pointerType: "mouse" }));
  move(pointerEvent(h.canvas, 200, 300, { pointerId: 61, pointerType: "mouse" }));
  up(pointerEvent(h.canvas, 200, 300, { pointerId: 61, pointerType: "mouse" }));
  assert(
    h.clientIntent.commandTarget === "setupAntiTankGuns" &&
      h.clientIntent.antiTankGunSetupPreview?.source === "minimap",
    "a native minimap drag keeps setup targeting armed at its live minimap point",
  );

  leave(pointerEvent(h.canvas, 350, 300, { pointerId: 61, pointerType: "mouse" }));
  h.clientIntent.updateAntiTankGunSetupPreview({
    source: "viewport",
    mouseX: 700,
    mouseY: 500,
    guns: selected,
  });
  h.minimap.updateCommandTargetPreview();
  assert(
    h.clientIntent.antiTankGunSetupPreview?.source === "viewport" &&
      h.clientIntent.antiTankGunSetupPreview?.mouseX === 700,
    "a completed native minimap drag cannot pin anti-tank or artillery cones to its last point",
  );
  h.minimap.destroy();
  assert(listenerFor(h.canvas, "pointerleave", "remove"), "minimap teardown removes its native hover listener");
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
  h.minimap.updateCommandTargetPreview(false);
  const unqueuedPreviewGun = h.clientIntent.antiTankGunSetupPreview?.guns[0];
  assert(
    unqueuedPreviewGun?.x === 50 && unqueuedPreviewGun?.y === 60,
    "releasing Shift while stationary immediately restores the current setup origin",
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
  h.minimap._issueOrder(plannedOrigin.x + 6, plannedOrigin.y, true);
  assert(
    h.net.sent[0]?.c === "artilleryFire" &&
      h.net.sent[0].queued === true &&
      h.net.sent[0].x === plannedOrigin.x &&
      h.net.sent[0].y === plannedOrigin.y &&
      h.net.sent[0].radiusTiles === 6,
    "minimap queued Artillery Fire sends the same raw command semantics as world targeting",
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
    { id: 33, owner: 1, kind: KIND.MORTAR_TEAM, x: 70, y: 80 },
  ];
  const h = minimapHarness({ selected, commandTarget: "setupAntiTankGuns" });
  assert(h.router.pointerDown(lockedEvent(200, 300, 0)), "setup minimap click is consumed");
  assert(h.net.sent.length === 1, "setup minimap click remains one budgeted command");
  assert(h.net.sent[0].c === "setupAntiTankGuns", "setup minimap click sends setupAntiTankGuns");
  assert(
    h.net.sent[0].units.includes(31) &&
      h.net.sent[0].units.includes(32) &&
      h.net.sent[0].units.includes(33),
    "setup minimap click includes all support weapons in one admission unit list",
  );
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

// Foreground player minimap blips draw above resource marks and get a merged outline mask.
{
  installWindowStub();
  const staticLayers = [];
  const dynamicLayers = [];
  const canvas = fakeRenderableCanvas({ width: 16, height: 16 });
  const previousOffscreenCanvas = globalThis.OffscreenCanvas;
  globalThis.OffscreenCanvas = undefined;
  canvas.ownerDocument = dynamicCanvasOwnerDocument(dynamicLayers);
  const state = {
    playerId: 1,
    map: {
      width: 4,
      height: 4,
      tileSize: 1,
      terrain: new Array(16).fill(TERRAIN.GRASS),
      resources: [{ id: 10, kind: "steel", x: 2, y: 2, remaining: 100 }],
    },
    selectedEntities() {
      return [];
    },
    entitiesInterpolated() {
      return [{ id: 501, owner: 1, kind: KIND.RIFLEMAN, x: 2, y: 2 }];
    },
    players: [],
  };
  const minimap = new Minimap(
    canvas,
    state,
    { x: 0, y: 0, zoom: 1, viewW: 4, viewH: 4 },
    null,
    { issueCommand() {} },
    null,
    { staticCanvasFactory: staticCanvasFactory(staticLayers) },
  );
  try {
    minimap.render();
  } finally {
    minimap.destroy();
    globalThis.OffscreenCanvas = previousOffscreenCanvas;
  }

  assert(staticLayers.length === 2, "resource test creates terrain and resource static layers");
  assert(dynamicLayers.length === 1, "player blip outline uses a dynamic mask layer");
  const resourceLayer = staticLayers[1];
  const maskLayer = dynamicLayers[0];
  const resourceDrawIndex = canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === resourceLayer.canvas.label,
  );
  const playerDrawIndex = canvas.context.calls.findIndex((call) =>
    hasCallWithApproxArgs({ calls: [call] }, "fillRect", [6.72, 6.72, 2.56, 2.56]),
  );
  const outlineDrawIndexes = canvas.context.calls.flatMap((call, index) =>
    call.op === "drawImage" && call.source === maskLayer.canvas.label ? [index] : [],
  );
  assert(resourceDrawIndex >= 0, "resource layer draws before foreground player blips");
  assert(playerDrawIndex > resourceDrawIndex, "foreground player blip draws above the resource layer");
  assert(outlineDrawIndexes.length === 4, "foreground player outline draws one mask pass per cardinal offset");
  assert(
    outlineDrawIndexes.every((index) => index > resourceDrawIndex && index < playerDrawIndex),
    "foreground player outline is layered between resources and the final blip fill",
  );
  assert(
    hasCallWithApproxArgs(maskLayer.context, "fillRect", [6.72, 6.72, 2.56, 2.56]),
    "outline mask uses the supply-scaled player blip footprint",
  );
}

// Legacy vision-only intel remains below fog while keeping the scaled player marker size.
{
  installWindowStub();
  const staticLayers = [];
  const dynamicLayers = [];
  const canvas = fakeRenderableCanvas({ width: 16, height: 16 });
  const previousOffscreenCanvas = globalThis.OffscreenCanvas;
  globalThis.OffscreenCanvas = undefined;
  canvas.ownerDocument = dynamicCanvasOwnerDocument(dynamicLayers);
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
      return [{ id: 601, owner: 2, kind: KIND.RIFLEMAN, x: 2, y: 2, visionOnly: true }];
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
  const minimap = new Minimap(
    canvas,
    state,
    { x: 0, y: 0, zoom: 1, viewW: 4, viewH: 4 },
    fog,
    { issueCommand() {} },
    null,
    { staticCanvasFactory: staticCanvasFactory(staticLayers) },
  );
  try {
    minimap.render();
  } finally {
    minimap.destroy();
    globalThis.OffscreenCanvas = previousOffscreenCanvas;
  }

  const visionBlipIndex = canvas.context.calls.findIndex((call) =>
    hasCallWithApproxArgs({ calls: [call] }, "fillRect", [6.72, 6.72, 2.56, 2.56]),
  );
  const fogIndex = canvas.context.calls.findIndex((call, index) =>
    index > visionBlipIndex && call.op === "fillRect" && call.args[2] > 10,
  );
  assert(visionBlipIndex >= 0, "vision-only player intel uses the supply-scaled player blip footprint");
  assert(fogIndex > visionBlipIndex, "vision-only player intel is drawn before the fog overlay");
  assert(dynamicLayers.length === 0, "vision-only player intel does not create a foreground outline mask");
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
    hasCallWithApproxArgs(canvas.context, "moveTo", [10.16, 8]),
    "Scout Plane blip starts an aircraft path at the plane canvas position",
  );
  assert(
    hasCallWithApproxArgs(canvas.context, "lineTo", [6.56, 6.24])
      && hasCallWithApproxArgs(canvas.context, "lineTo", [7.28, 8])
      && hasCallWithApproxArgs(canvas.context, "lineTo", [6.56, 9.76]),
    "Scout Plane blip draws the expected multi-point aircraft silhouette",
  );
  assert(
    canvas.context.calls.some((call) => call.op === "stroke"),
    "Scout Plane blip includes an outline for readability",
  );
  assert(
    hasCallWithApproxArgs(canvas.context, "fillRect", [10.72, 6.72, 2.56, 2.56]),
    "ordinary ground units still draw square minimap blips at their canvas position",
  );
  assert(
    !hasCallWithApproxArgs(canvas.context, "fillRect", [6.72, 6.72, 2.56, 2.56]),
    "Scout Plane blips should not also use the ordinary square unit marker",
  );
  minimap.destroy();
}

// Player minimap blips scale by unit supply and total building resource cost.
{
  installWindowStub();
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
    players: [],
  };
  const minimap = new Minimap(
    canvas,
    state,
    { x: 0, y: 0, zoom: 1, viewW: 4, viewH: 4 },
    null,
    { issueCommand() {} },
  );
  assertApprox(minimap._entityBlipScale({ kind: KIND.WORKER }), 0.5, 0.0001, "Worker blip is half size");
  assertApprox(minimap._entityBlipScale({ kind: KIND.RIFLEMAN }), 0.5, 0.0001, "Rifleman blip is half size");
  assertApprox(minimap._entityBlipScale({ kind: KIND.TANK }), 1, 0.0001, "Tank keeps the current blip size");
  assertApprox(minimap._entityBlipScale({ kind: KIND.MACHINE_GUNNER }), 4 / 7, 0.0001, "unit blips interpolate by supply");
  assertApprox(minimap._entityBlipScale({ kind: KIND.TANK_TRAP }), 0.5, 0.0001, "Tank Trap blip is half size");
  assertApprox(minimap._entityBlipScale({ kind: KIND.CITY_CENTRE }), 1, 0.0001, "City Centre keeps the current blip size");
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

// Positional attack alerts use a slower red pulse with a crisp white inner rim.
{
  installWindowStub();
  const canvas = fakeRenderableCanvas({ width: 220, height: 220 });
  const minimap = new Minimap(
    canvas,
    { map: null },
    {},
    {},
    { issueCommand() {} },
  );
  minimap.ping(40, 60, "alert", true);
  minimap._pings[0].startedAt = 100;
  minimap._drawPings(650);

  const arcs = canvas.context.calls.filter((call) => call.op === "arc");
  const strokes = canvas.context.calls.filter((call) => call.op === "stroke");
  assert(arcs.length === 2, "attack alert draws both the red ring and its inner rim");
  assertApprox(arcs[0].args[2], 11.5, 0.001, "attack alert advances halfway through its 1.1 second pulse");
  assertApprox(arcs[1].args[2], 9.5, 0.001, "attack alert inner rim stays two pixels inside the red ring");
  assert(
    strokes[0].strokeStyle === "#ff4d4d" && strokes[0].lineWidth === 2,
    "attack alert keeps its strong red outer stroke",
  );
  assert(
    strokes[1].strokeStyle === "rgba(255,255,255,0.95)" && strokes[1].lineWidth === 1,
    "attack alert draws a crisp white inner stroke",
  );

  canvas.context.calls.length = 0;
  minimap._pings.length = 0;
  minimap.ping(40, 60, "alert");
  minimap._pings[0].startedAt = 100;
  minimap._drawPings(550);
  assert(
    canvas.context.calls.filter((call) => call.op === "arc").length === 1,
    "other positional alerts retain the generic single-ring treatment",
  );
  minimap._drawPings(1000);
  assert(minimap._pings.length === 0, "generic positional alerts retain their 900 ms lifetime");
  minimap.destroy();
}

console.log("minimap_input_contracts: ok");
