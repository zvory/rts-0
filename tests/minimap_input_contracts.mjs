// Dependency-free checks for minimap input routed through MatchInputRouter.
// These cover the pointer-lock virtual-cursor path without launching a browser.

import { MatchInputRouter } from "../client/src/input/router.js";
import { CommandComposer } from "../client/src/input/command_composer.js";
import { Minimap } from "../client/src/minimap.js";
import { KIND } from "../client/src/protocol.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function assertApprox(actual, expected, epsilon, msg) {
  assert(Math.abs(actual - expected) <= epsilon, `${msg}: expected ${expected}, got ${actual}`);
}

function installWindowStub() {
  const listeners = [];
  globalThis.window = {
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

function minimapHarness({ selected = [], commandTarget = null, commandsEnabled = true } = {}) {
  installWindowStub();
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const router = new MatchInputRouter(viewport);
  const canvas = fakeCanvas();
  const centers = [];
  const commands = [];
  const endedTargets = [];
  const state = {
    playerId: 1,
    commandTarget,
    commandComposer: new CommandComposer(),
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
    endCommandTarget() {
      endedTargets.push(this.commandTarget);
      this.commandComposer.cancel();
      this.commandTarget = null;
    },
    issueCommandTarget(ev = {}) {
      const issued = this.commandComposer.issue(ev);
      this.commandTarget = this.commandComposer.target;
      return issued;
    },
    addCommandFeedback(type, x, y) {
      commands.push({ feedback: type, x, y });
    },
    entitiesInterpolated() {
      return [];
    },
    players: [],
  };
  if (commandTarget) state.commandComposer.arm(commandTarget);
  const camera = {
    centerOn(x, y) {
      centers.push({ x, y });
    },
  };
  const net = {
    sent: [],
    command(command) {
      this.sent.push(command);
    },
  };
  const minimap = new Minimap(canvas, state, camera, null, net, router, { commandsEnabled });
  return { router, canvas, state, camera, net, minimap, centers, commands, endedTargets };
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

// Command-target left-click on minimap issues the command and exits target mode.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack" });
  assert(h.router.pointerDown(lockedEvent(150, 250, 0)), "attack-move minimap left-click is consumed");
  assert(h.net.sent.length === 1, "attack-move minimap click sends one command");
  assert(h.net.sent[0].c === "attackMove", "attack command-target sends attack-move");
  assert(h.net.sent[0].queued !== true, "plain minimap attack target does not queue attack-move");
  assert(h.state.commandTarget === null, "attack command-target exits after minimap click");
  assert(h.endedTargets.length === 1, "endCommandTarget is called");
  h.minimap.destroy();
}

// Shift command-target clicks on the minimap stay armed while the command composer preserves it.
{
  const selected = [{ id: 9, owner: 1, kind: KIND.RIFLEMAN }];
  const h = minimapHarness({ selected, commandTarget: "attack" });
  h.state.commandComposer.hold("attack", "KeyA", { shiftKey: true });
  assert(h.router.pointerDown(lockedEvent(150, 250, 0, { shiftKey: true })), "first held-A minimap attack click is consumed");
  assert(h.router.pointerDown(lockedEvent(160, 260, 0, { shiftKey: true })), "second held-A minimap attack click is consumed");
  assert(h.net.sent.length === 2, "held-A minimap targeting sends multiple commands");
  assert(h.net.sent.every((command) => command.c === "attackMove" && command.queued === true), "held-A minimap targeting queues attack-move commands");
  assert(h.state.commandTarget === "attack", "held-A minimap targeting stays armed after queued clicks");
  assert(h.endedTargets.length === 0, "held-A minimap targeting does not end while queueing");
  h.minimap.destroy();
}

// Destroy unregisters the zone so rematches cannot double-fire stale minimap handlers.
{
  const h = minimapHarness();
  assert(h.router.pointerDown(lockedEvent(150, 250, 0)), "minimap zone is registered before destroy");
  h.minimap.destroy();
  assert(!h.router.pointerDown(lockedEvent(150, 250, 0)), "minimap zone is unregistered after destroy");
}

console.log("minimap_input_contracts: ok");
