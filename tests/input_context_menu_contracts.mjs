// Dependency-free checks for viewport right-click menu suppression.

import { Input } from "../client/src/input/index.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function mouseEvent({ button = 0, shiftKey = false } = {}) {
  return {
    button,
    shiftKey,
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

function inputHarness() {
  const rightClicks = [];
  const input = Object.create(Input.prototype);
  input.pointerLocked = false;
  input._suppressNextContextMenuUntil = 0;
  input._eventScreenPos = () => ({ x: 100, y: 120 });
  input._trackMouse = () => {};
  input._routeLockedPointerDown = () => false;
  input._onRightClick = (p, ev) => rightClicks.push({ p, shiftKey: !!ev.shiftKey });
  return { input, rightClicks };
}

{
  const { input, rightClicks } = inputHarness();
  const down = mouseEvent({ button: 2, shiftKey: true });
  input._handleMouseDown(down);

  assert(down.prevented, "Shift+right mousedown suppresses native default");
  assert(down.stopped, "Shift+right mousedown stops propagation");
  assert(rightClicks.length === 1, "Shift+right mousedown issues one order immediately");
  assert(rightClicks[0].shiftKey === true, "Shift+right mousedown preserves queued modifier");

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

console.log("input_context_menu_contracts: ok");
