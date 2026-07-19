// Dependency-free checks for viewport right-click menu suppression.

import { Input } from "../client/src/input/index.js";
import { KIND } from "../client/src/protocol.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function mouseEvent({ button = 0, shiftKey = false, ctrlKey = false, metaKey = false } = {}) {
  return {
    button,
    shiftKey,
    ctrlKey,
    metaKey,
    clientX: 100,
    clientY: 120,
    prevented: false,
    stopped: false,
    preventDefault() {
      this.prevented = true;
    },
    stopPropagation() {
      this.stopped = true;
    },
  };
}

function withNavigator(value, fn) {
  const prior = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value,
  });
  try {
    fn();
  } finally {
    if (prior) Object.defineProperty(globalThis, "navigator", prior);
    else delete globalThis.navigator;
  }
}

function inputHarness() {
  const rightClicks = [];
  const input = Object.create(Input.prototype);
  input.pointerLocked = false;
  input._suppressNextContextMenuUntil = 0;
  input._eventScreenPos = () => ({ x: 100, y: 120 });
  input._trackMouse = () => {};
  input._routeLockedPointerDown = () => false;
  input._routeLockedPointerUp = () => false;
  input.cameraNavigation = null;
  input._onRightClick = (p, ev) => rightClicks.push({ p, shiftKey: !!ev.shiftKey });
  return { input, rightClicks };
}

{
  const { input, rightClicks } = inputHarness();
  const down = mouseEvent({ button: 2, shiftKey: true });
  input._handleMouseDown(down);

  assert(down.prevented, "Shift+right mousedown suppresses native default");
  assert(down.stopped, "Shift+right mousedown stops propagation");
  assert(rightClicks.length === 0, "Shift+right mousedown waits for click-or-drag resolution");

  const up = mouseEvent({ button: 2, shiftKey: true });
  input._handleMouseUp(up);
  assert(rightClicks.length === 1, "Shift+right mouseup issues one click order when no drag was promoted");
  assert(rightClicks[0].shiftKey === true, "Shift+right mouseup preserves queued modifier");

  const menu = mouseEvent({ button: 2, shiftKey: true });
  input._handleContextMenu(menu);
  assert(menu.prevented, "follow-up contextmenu suppresses native default");
  assert(menu.stopped, "follow-up contextmenu stops propagation");
  assert(rightClicks.length === 1, "follow-up contextmenu does not duplicate the order");

  input._suppressNextContextMenuUntil = 0;
  const plainMenu = mouseEvent({ button: 2, shiftKey: false });
  input._handleContextMenu(plainMenu);
  assert(rightClicks.length === 2, "later contextmenu still issues a normal right-click order");
  assert(rightClicks[1].shiftKey === false, "later contextmenu does not inherit stale Shift state");
}

withNavigator({ platform: "MacIntel", userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)" }, () => {
  const { input, rightClicks } = inputHarness();
  const selectionClicks = [];
  input.state = {
    playerId: 1,
    placement: null,
    commandTarget: null,
  };
  input._entityAtScreen = () => ({ id: 7, owner: 1, kind: KIND.WORKER, x: 100, y: 120 });
  input._commitClickSelection = (p, additive, ctrl) => selectionClicks.push({ p, additive, ctrl });

  const menu = mouseEvent({ button: 2, ctrlKey: true });
  input._handleContextMenu(menu);

  assert(menu.prevented, "Mac Ctrl+contextmenu suppresses native default");
  assert(menu.stopped, "Mac Ctrl+contextmenu stops propagation");
  assert(selectionClicks.length === 1, "Mac Ctrl+contextmenu on an own unit commits selection");
  assert(selectionClicks[0].ctrl === true, "Mac Ctrl+contextmenu uses control-click selection semantics");
  assert(rightClicks.length === 0, "Mac Ctrl+contextmenu on an own unit does not issue a right-click order");
});

withNavigator({ platform: "Win32", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)" }, () => {
  const { input, rightClicks } = inputHarness();
  const selectionClicks = [];
  input.state = {
    playerId: 1,
    placement: null,
    commandTarget: null,
  };
  input._entityAtScreen = () => ({ id: 7, owner: 1, kind: KIND.WORKER, x: 100, y: 120 });
  input._commitClickSelection = (p, additive, ctrl) => selectionClicks.push({ p, additive, ctrl });

  const menu = mouseEvent({ button: 2, ctrlKey: true });
  input._handleContextMenu(menu);

  assert(selectionClicks.length === 0, "non-Mac Ctrl+contextmenu does not use Mac selection handling");
  assert(rightClicks.length === 1, "non-Mac Ctrl+contextmenu still issues a right-click order");
});

console.log("input_context_menu_contracts: ok");
