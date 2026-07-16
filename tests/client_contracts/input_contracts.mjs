// tests/client_contracts/input_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert, assertApprox } from "./assertions.mjs";
import { ClientIntent } from "../../client/src/client_intent.js";
import { HUD } from "../../client/src/hud.js";
import { Input } from "../../client/src/input/index.js";
import { CameraNavigationInput } from "../../client/src/input/camera_navigation.js";
import { KIND } from "../../client/src/protocol.js";
import { _controlGroupSaveModifierActive } from "../../client/src/input/control_groups.js";
import {
  cursorLockSupported,
  enterCursorLock,
  exitCursorLock,
  installTauriNativeCursorBridge,
  installedAppRuntime,
  nativeDesktopCursorBridge,
} from "../../client/src/input/cursor_lock.js";
import {
  DomClickInputZone,
  MatchInputRouter,
} from "../../client/src/input/router.js";

// ---------------------------------------------------------------------------
// Control groups
// ---------------------------------------------------------------------------
{
  const ev = (mods) => ({
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    ...mods,
  });

  assert(
    _controlGroupSaveModifierActive(ev({ altKey: true }), { isWindows: true, isInstalledApp: false }),
    "Windows browser control-group save uses Alt+number",
  );
  assert(
    !_controlGroupSaveModifierActive(ev({ ctrlKey: true }), { isWindows: true, isInstalledApp: false }),
    "Windows browser control-group save does not use Ctrl+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ ctrlKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save uses Ctrl+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ altKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save also uses Alt+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ metaKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save also uses Cmd/Meta+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ metaKey: true }), { isWindows: false, isInstalledApp: false }),
    "non-Windows control-group save keeps the existing modifier set",
  );
  assert(
    !_controlGroupSaveModifierActive(
      ev({ altKey: true, ctrlKey: true }),
      { isWindows: true, isInstalledApp: false },
    ),
    "Windows browser control-group save requires a clean Alt modifier",
  );
}

// ---------------------------------------------------------------------------
// Shared camera navigation gestures
// ---------------------------------------------------------------------------
{
  const pans = [];
  const zooms = [];
  const dom = {
    clientWidth: 300,
    clientHeight: 200,
    getBoundingClientRect() {
      return { left: 10, top: 20, width: 300, height: 200 };
    },
  };
  const camera = {
    panByScreenDelta(delta) {
      pans.push({ dx: delta.x, dy: delta.y });
    },
    dollyBy(factor, anchor) {
      zooms.push({ factor, x: anchor.x, y: anchor.y });
    },
  };
  const nav = new CameraNavigationInput(dom, camera);
  const touch = (identifier, clientX, clientY) => ({ identifier, clientX, clientY });
  let prevented = 0;
  const event = (touches) => ({
    touches,
    preventDefault() {
      prevented += 1;
    },
  });

  assert(nav.handleTouchStart(event([touch(1, 110, 120)])), "touchstart begins a camera pan gesture");
  nav.handleTouchMove(event([touch(1, 135, 150)]));
  assert(pans.length === 1, "one-finger touch drag pans the camera");
  assertApprox(pans[0].dx, 25, 0.001, "touch pan screen dx");
  assertApprox(pans[0].dy, 30, 0.001, "touch pan screen dy");
  assert(nav.handleTouchEnd(event([])), "touchend releases the camera gesture");
  assert(nav.touchGesture === null, "touchend clears shared touch gesture state");
  assert(prevented >= 3, "touch camera gestures suppress native page gestures");
  assert(
    nav.shouldSuppressMouseEvent({ preventDefault() { prevented += 1; } }),
    "touch camera gestures suppress follow-up synthetic mouse events",
  );
}

{
  const pans = [];
  const zooms = [];
  const dom = {
    clientWidth: 320,
    clientHeight: 240,
    getBoundingClientRect() {
      return { left: 0, top: 0, width: 320, height: 240 };
    },
  };
  const camera = {
    panByScreenDelta(delta) {
      pans.push({ dx: delta.x, dy: delta.y });
    },
    dollyBy(factor, anchor) {
      zooms.push({ factor, x: anchor.x, y: anchor.y });
    },
  };
  const nav = new CameraNavigationInput(dom, camera);
  const touch = (identifier, clientX, clientY) => ({ identifier, clientX, clientY });
  const event = (touches) => ({ touches, preventDefault() {} });

  nav.handleTouchStart(event([
    touch(1, 50, 50),
    touch(2, 150, 50),
  ]));
  nav.handleTouchMove(event([
    touch(1, 70, 70),
    touch(2, 230, 70),
  ]));

  assert(pans.length === 1, "pinch movement pans by the midpoint delta");
  assertApprox(pans[0].dx, 50, 0.001, "pinch midpoint pan dx");
  assertApprox(pans[0].dy, 20, 0.001, "pinch midpoint pan dy");
  assert(zooms.length === 1, "pinch movement dollies the camera");
  assertApprox(zooms[0].factor, 1.6, 0.001, "pinch dolly factor uses distance ratio");
  assertApprox(zooms[0].x, 150, 0.001, "pinch zoom anchors at midpoint x");
  assertApprox(zooms[0].y, 70, 0.001, "pinch zoom anchors at midpoint y");
}

{
  const pans = [];
  const zooms = [];
  const viewportTarget = { viewport: true };
  const outsideTarget = { viewport: false };
  const dom = {
    clientWidth: 320,
    clientHeight: 240,
    contains(target) {
      return !!target?.viewport;
    },
    getBoundingClientRect() {
      return { left: 0, top: 0, width: 320, height: 240 };
    },
  };
  const camera = {
    panByScreenDelta(delta) {
      pans.push({ dx: delta.x, dy: delta.y });
    },
    dollyBy(factor, anchor) {
      zooms.push({ factor, x: anchor.x, y: anchor.y });
    },
  };
  const nav = new CameraNavigationInput(dom, camera);
  const touch = (identifier, clientX, clientY, target = viewportTarget) => ({
    identifier,
    clientX,
    clientY,
    target,
  });
  const event = (touches, changedTouches = touches) => ({ touches, changedTouches, preventDefault() {} });

  const viewportStart = touch(1, 80, 80);
  nav.handleTouchStart(event([viewportStart], [viewportStart]));
  const outsideStart = touch(2, 180, 80, outsideTarget);
  nav.handleTouchStart(event([viewportStart, outsideStart], [outsideStart]));
  nav.handleTouchMove(event([
    touch(1, 110, 100),
    touch(2, 240, 100, outsideTarget),
  ], [
    touch(1, 110, 100),
    touch(2, 240, 100, outsideTarget),
  ]));

  assert(pans.length === 1, "touch gestures ignore touches that did not start on the viewport");
  assertApprox(pans[0].dx, 30, 0.001, "mixed touch pan keeps the viewport-started finger delta");
  assertApprox(pans[0].dy, 20, 0.001, "mixed touch pan keeps the viewport-started finger y delta");
  assert(zooms.length === 0, "outside touches are not folded into viewport pinch zoom");

  nav.handleTouchEnd(event([touch(2, 240, 100, outsideTarget)], [touch(1, 110, 100)]));
  assert(nav.touchGesture === null, "releasing the viewport touch ends the camera gesture");
}

// ---------------------------------------------------------------------------
// Match input router
// ---------------------------------------------------------------------------
{
  const viewport = {
    getBoundingClientRect() {
      return { left: 10, top: 20, right: 810, bottom: 620, width: 800, height: 600 };
    },
  };
  const router = new MatchInputRouter(viewport);
  const calls = [];
  const lowZone = {
    priority: 1,
    contains: () => true,
    pointerDown: () => {
      calls.push("lowDown");
      return true;
    },
  };
  const highZone = {
    priority: 10,
    contains: (ev) => ev.clientX >= 100 && ev.clientX <= 200,
    pointerDown: (ev) => {
      calls.push(["highDown", ev.viewportX, ev.viewportY]);
      return true;
    },
    pointerMove: (ev) => {
      calls.push(["highMove", ev.clientX, ev.clientY]);
      return true;
    },
    pointerUp: () => {
      calls.push("highUp");
      return true;
    },
  };
  router.registerZone(lowZone);
  const unregisterHigh = router.registerZone(highZone);

  assert(router.pointerDown({ clientX: 150, clientY: 70, button: 0, source: "locked" }), "router consumes highest matching zone");
  assert(calls[0][0] === "highDown", "higher priority matching zone receives pointerDown first");
  assert(calls[0][1] === 140 && calls[0][2] === 50, "router computes viewport-local coords");
  assert(!router.pointerMove({ clientX: 500, clientY: 500, source: "dom" }), "capture ignores different event source");
  assert(router.pointerMove({ clientX: 500, clientY: 500, source: "locked" }), "captured zone receives pointerMove outside bounds");
  assert(calls[1][0] === "highMove", "pointerDown capture is retained for moves");
  assert(!router.pointerUp({ clientX: 500, clientY: 500, source: "dom" }), "capture is not released by a different source");
  assert(router.pointerUp({ clientX: 500, clientY: 500, source: "locked" }), "captured zone receives pointerUp");
  assert(calls[2] === "highUp", "pointerUp releases the captured zone");

  unregisterHigh();
  assert(router.pointerDown({ clientX: 150, clientY: 70, button: 0 }), "router falls back after unregister");
  assert(calls.at(-1) === "lowDown", "unregistered zone no longer receives events");
}

{
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const router = new MatchInputRouter(viewport);
  const leaves = [];
  router.registerZone({
    previewSurface: "minimap",
    contains: (ev) => ev.clientX < 200,
    pointerDown: () => true,
    pointerMove: () => false,
    pointerLeave: (ev) => leaves.push(ev.source),
    pointerCancel: (ev) => leaves.push(`cancel:${ev.source}`),
  });

  router.pointerMove({ clientX: 100, clientY: 100, source: "locked" });
  assert(router.activePreviewSurface() === "minimap", "router exposes the hovered preview surface");
  router.pointerDown({ clientX: 100, clientY: 100, source: "locked" });
  assert(!router.releaseSource("dom"), "a different event source cannot release routed ownership");
  assert(router.releaseSource("locked"), "the ending event source releases routed ownership");
  assert(
    leaves.join(",") === "cancel:locked,locked",
    "source release cancels capture and leaves hover exactly once",
  );
  assert(!router.pointerMove({ clientX: 500, clientY: 500, source: "locked" }),
    "released source no longer retains pointer capture");
  assert(router.activePreviewSurface() === null, "source release relinquishes preview-surface ownership");

  router.pointerDown({ clientX: 100, clientY: 100, source: "locked" });
  assert(router.activePreviewSurface() === "minimap", "pointerDown alone establishes preview-surface ownership");
  router.pointerUp({ clientX: 100, clientY: 100, source: "locked" });
}

{
  const calls = [];
  const coveredInput = Object.create(Input.prototype);
  coveredInput.inputRouter = { activePreviewSurface: () => "minimap" };
  coveredInput._flushPointerLockCursor = () => calls.push("cursor");
  coveredInput._refreshAttackTargetPreview = () => calls.push("attack");
  coveredInput._refreshResourceMiningPreview = () => calls.push("resource");
  coveredInput._refreshAbilityTargetPreview = () => calls.push("ability");
  coveredInput._refreshPlacement = () => calls.push("placement");
  coveredInput._refreshLabToolPreview = () => calls.push("lab");
  coveredInput.update(0);
  assert(calls.join(",") === "cursor", "minimap ownership prevents viewport-underlay preview refreshes");
}

{
  let releasedShift = 0;
  const input = Object.create(Input.prototype);
  input._shiftKeyDown = false;
  input._shiftKeysDown = new Set();
  input.clientIntent = {
    releaseCommandTargetShift() { releasedShift += 1; },
  };
  const keyEvent = (code) => ({
    code,
    target: null,
    preventDefault() {},
  });

  input._handleKeyDown(keyEvent("ShiftLeft"));
  input._handleKeyDown(keyEvent("ShiftRight"));
  input._handleKeyUp(keyEvent("ShiftLeft"));
  assert(input.isShiftHeld(), "releasing one Shift key preserves live Shift state while the other remains held");
  assert(releasedShift === 0, "partial Shift release does not end Shift-preserved command targeting");
  input._handleKeyUp(keyEvent("ShiftRight"));
  assert(!input.isShiftHeld(), "releasing the final Shift key clears live Shift state");
  assert(releasedShift === 1, "final Shift release updates Shift-preserved command targeting once");
}

// ---------------------------------------------------------------------------
// Context-sensitive hover previews
// ---------------------------------------------------------------------------
{
  const moveUnit = { id: 40, owner: 1, kind: KIND.RIFLEMAN, x: 120, y: 120 };
  const enemyUnit = { id: 41, owner: 2, kind: KIND.RIFLEMAN, x: 180, y: 180 };
  const attackHoverInput = Object.create(Input.prototype);
  attackHoverInput.clientIntent = new ClientIntent();
  attackHoverInput.mouse = { x: 180, y: 180 };
  attackHoverInput._drag = null;
  attackHoverInput._groundAtScreen = (x, y) => ({ x, y });
  attackHoverInput._entityAtScreen = () => enemyUnit;
  attackHoverInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [moveUnit, enemyUnit],
    selectedEntities: () => [moveUnit],
  };

  attackHoverInput._refreshAttackTargetPreview();
  assert(
    attackHoverInput.clientIntent.attackTargetPreview?.targetId === enemyUnit.id &&
      attackHoverInput.clientIntent.attackTargetPreview.kind === KIND.RIFLEMAN,
    "enemy hover with own units selected previews the right-click attack target",
  );

  attackHoverInput.state.entitiesInterpolated = () => [moveUnit];
  attackHoverInput._entityAtScreen = () => null;
  attackHoverInput._refreshAttackTargetPreview();
  assert(attackHoverInput.clientIntent.attackTargetPreview === null, "attack target preview clears when right-click would move");

  const deconstructWorker = { id: 42, owner: 1, kind: KIND.WORKER, x: 150, y: 150 };
  const enemyTankTrap = { id: 43, owner: 2, kind: KIND.TANK_TRAP, x: 180, y: 180 };
  attackHoverInput.clientIntent.updateAttackTargetPreview({ targetId: enemyUnit.id, kind: enemyUnit.kind, x: enemyUnit.x, y: enemyUnit.y });
  attackHoverInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [deconstructWorker, enemyTankTrap],
    selectedEntities: () => [deconstructWorker],
  };
  attackHoverInput._entityAtScreen = () => enemyTankTrap;
  attackHoverInput._refreshAttackTargetPreview();
  assert(
    attackHoverInput.clientIntent.attackTargetPreview === null,
    "attack target preview stays hidden when worker right-click would deconstruct a Tank Trap",
  );
}

{
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const button = {
    disabled: false,
    clickCount: 0,
    click() {
      this.clickCount += 1;
    },
    dispatchEvent(ev) {
      if (ev.type === "click") this.click();
      return true;
    },
    getAttribute() {
      return null;
    },
    closest() {
      return this;
    },
  };
  const root = {
    hidden: false,
    getBoundingClientRect() {
      return { left: 600, top: 420, right: 780, bottom: 580, width: 180, height: 160 };
    },
    contains(el) {
      return el === this || el === button;
    },
  };
  const doc = {
    elementFromPoint(x, y) {
      return x >= 620 && x <= 700 && y >= 440 && y <= 520 ? button : root;
    },
  };
  const router = new MatchInputRouter(viewport);
  router.registerZone(new DomClickInputZone(root, { documentRef: doc }));

  assert(router.pointerDown({ clientX: 640, clientY: 460, button: 0, source: "locked" }), "DOM zone consumes locked pointerDown over HUD button");
  assert(router.pointerUp({ clientX: 640, clientY: 460, button: 0, source: "locked" }), "DOM zone consumes locked pointerUp over HUD button");
  assert(button.clickCount === 1, "DOM zone forwards locked pointer click to the HUD button");
  assert(router.pointerDown({ clientX: 640, clientY: 460, button: 0, source: "locked" }), "DOM zone captures a second locked button press");
  assert(router.releaseSource("locked"), "ending pointer lock cancels the captured DOM press");
  assert(!router.pointerUp({ clientX: 640, clientY: 460, button: 0, source: "locked" }), "cancelled DOM capture ignores a later pointerUp");
  assert(button.clickCount === 1, "cancelling locked input cannot synthesize a stale HUD click");
  assert(router.pointerDown({ clientX: 760, clientY: 560, button: 0, source: "locked" }), "DOM zone consumes empty HUD panel space");
  assert(router.pointerUp({ clientX: 760, clientY: 560, button: 0, source: "locked" }), "empty HUD panel click releases capture");
  assert(button.clickCount === 1, "empty HUD panel space does not click the prior button");
}

{
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const canvas = {
    parentElement: viewport,
    closest() {
      return null;
    },
  };
  viewport.contains = (el) => el === viewport || el === canvas;

  const overlayEvents = [];
  const overlay = {
    parentElement: null,
    hidden: false,
    scrollHeight: 400,
    clientHeight: 100,
    scrollWidth: 100,
    clientWidth: 100,
    scrollTop: 0,
    scrollLeft: 0,
    getBoundingClientRect() {
      return { left: 500, top: 50, right: 760, bottom: 360, width: 260, height: 310 };
    },
    contains(el) {
      return el === this || el === overlayChild;
    },
    closest() {
      return null;
    },
    dispatchEvent(ev) {
      overlayEvents.push(ev.type);
      return true;
    },
  };
  const overlayChild = {
    parentElement: overlay,
    closest() {
      return null;
    },
    dispatchEvent(ev) {
      overlayEvents.push(ev.type);
      return true;
    },
  };
  const gameScreen = {
    hidden: false,
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
    contains(el) {
      return el === this || overlay.contains(el) || viewport.contains(el);
    },
  };
  const doc = {
    elementFromPoint(x, y) {
      if (x >= 500 && x <= 760 && y >= 50 && y <= 360) return overlayChild;
      return canvas;
    },
  };
  const priorGetComputedStyle = globalThis.getComputedStyle;
  globalThis.getComputedStyle = () => ({ overflowY: "auto", overflowX: "hidden" });
  const router = new MatchInputRouter(viewport);
  router.registerZone(new DomClickInputZone(gameScreen, {
    priority: 20,
    documentRef: doc,
    ignoreRoots: [viewport],
  }));

  assert(!router.pointerDown({ clientX: 100, clientY: 100, button: 0, source: "locked" }), "DOM zone ignores viewport hits so terrain input still receives clicks");
  assert(!router.pointerMove({ clientX: 520, clientY: 80, button: 0, source: "dom" }), "DOM zone ignores ordinary browser DOM events to avoid redispatch recursion");
  assert(router.pointerDown({ clientX: 520, clientY: 80, button: 0, source: "locked" }), "DOM zone consumes arbitrary overlay pointerDown");
  assert(router.pointerMove({ clientX: 530, clientY: 90, button: 0, source: "locked" }), "DOM zone forwards arbitrary overlay pointerMove");
  assert(router.pointerUp({ clientX: 530, clientY: 90, button: 0, source: "locked" }), "DOM zone consumes arbitrary overlay pointerUp");
  assert(
    ["pointerdown", "mousedown", "pointermove", "mousemove", "pointerup", "mouseup", "click"]
      .every((type) => overlayEvents.includes(type)),
    "DOM zone forwards pointer, mouse, and click events to arbitrary overlays",
  );
  assert(router.wheel({ clientX: 520, clientY: 80, deltaY: 42, deltaX: 0, source: "locked" }), "DOM zone consumes wheel over arbitrary overlay");
  assert(overlay.scrollTop === 42, "DOM zone applies wheel scrolling to scrollable overlay ancestors");
  if (priorGetComputedStyle === undefined) delete globalThis.getComputedStyle;
  else globalThis.getComputedStyle = priorGetComputedStyle;
}

{
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const canvas = {
    parentElement: viewport,
    closest() {
      return null;
    },
  };
  viewport.contains = (el) => el === viewport || el === canvas;
  const settingsButton = {
    disabled: false,
    clickCount: 0,
    parentElement: null,
    closest() {
      return this;
    },
    getAttribute() {
      return null;
    },
    dispatchEvent(ev) {
      if (ev.type === "click") this.clickCount += 1;
      return true;
    },
  };
  const gameScreen = {
    hidden: false,
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
    contains(el) {
      return el === this || viewport.contains(el);
    },
  };
  const gameMenu = {
    hidden: false,
    getBoundingClientRect() {
      return { left: 740, top: 10, right: 790, bottom: 120, width: 50, height: 110 };
    },
    contains(el) {
      return el === this || el === settingsButton;
    },
  };
  const doc = {
    elementFromPoint(x, y) {
      if (x >= 750 && x <= 780 && y >= 20 && y <= 50) return settingsButton;
      return canvas;
    },
  };
  const router = new MatchInputRouter(viewport);
  router.registerZone(new DomClickInputZone([gameScreen, gameMenu], {
    priority: 20,
    documentRef: doc,
    ignoreRoots: [viewport],
  }));

  assert(router.pointerDown({ clientX: 760, clientY: 30, button: 0, source: "locked" }), "DOM zone consumes locked pointerDown over sibling settings menu");
  assert(router.pointerUp({ clientX: 760, clientY: 30, button: 0, source: "locked" }), "DOM zone consumes locked pointerUp over sibling settings menu");
  assert(settingsButton.clickCount === 1, "DOM zone forwards locked clicks to settings chrome outside the game screen");
  assert(!router.pointerDown({ clientX: 100, clientY: 100, button: 0, source: "locked" }), "DOM zone still ignores viewport hits when multiple roots are registered");
}

// ---------------------------------------------------------------------------
// Pointer lock bridge
// ---------------------------------------------------------------------------
{
  const priorMatchMedia = globalThis.matchMedia;
  const priorNavigatorDescriptor = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  const priorDesktopRuntimeDescriptor = Object.getOwnPropertyDescriptor(globalThis, "__RTS_DESKTOP_RUNTIME");
  globalThis.matchMedia = (query) => ({ matches: query === "(display-mode: standalone)" });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { standalone: false },
  });
  assert(installedAppRuntime(), "standalone display mode marks an installed app runtime");
  globalThis.matchMedia = (query) => ({ matches: query === "(display-mode: fullscreen)" });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { standalone: false },
  });
  assert(!installedAppRuntime(), "browser fullscreen mode does not mark an installed app runtime");
  globalThis.matchMedia = () => ({ matches: false });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { standalone: false },
  });
  assert(!installedAppRuntime(), "regular browser tabs are not installed app runtimes");
  Object.defineProperty(globalThis, "__RTS_DESKTOP_RUNTIME", {
    configurable: true,
    value: { shell: "tauri", platform: "macos" },
  });
  assert(installedAppRuntime(), "Tauri desktop runtime marks an installed app runtime");
  if (priorMatchMedia === undefined) delete globalThis.matchMedia;
  else globalThis.matchMedia = priorMatchMedia;
  if (priorNavigatorDescriptor) Object.defineProperty(globalThis, "navigator", priorNavigatorDescriptor);
  else delete globalThis.navigator;
  if (priorDesktopRuntimeDescriptor) Object.defineProperty(globalThis, "__RTS_DESKTOP_RUNTIME", priorDesktopRuntimeDescriptor);
  else delete globalThis.__RTS_DESKTOP_RUNTIME;

  assert(cursorLockSupported(true), "browser pointer lock keeps cursor lock available");
  assert(!cursorLockSupported(false, null), "cursor lock remains unavailable without browser or native support");
  {
    const priorRequiredRuntimeDescriptor = Object.getOwnPropertyDescriptor(globalThis, "__RTS_DESKTOP_RUNTIME");
    Object.defineProperty(globalThis, "__RTS_DESKTOP_RUNTIME", {
      configurable: true,
      value: { nativeCursorCapture: true, pointerLockDisabled: true },
    });
    assert(!cursorLockSupported(true, null), "desktop native-cursor runtime does not use browser Pointer Lock fallback");
    let browserFallbackCalled = 0;
    let bridgeError = null;
    try {
      await enterCursorLock(async () => {
        browserFallbackCalled += 1;
        return true;
      }, null, null);
    } catch (err) {
      bridgeError = err;
    }
    assert(browserFallbackCalled === 0, "missing desktop bridge fails before browser Pointer Lock");
    assert(
      bridgeError?.message === "Native cursor bridge is unavailable in the desktop shell.",
      "missing desktop bridge reports the native bridge failure",
    );
    if (priorRequiredRuntimeDescriptor) {
      Object.defineProperty(globalThis, "__RTS_DESKTOP_RUNTIME", priorRequiredRuntimeDescriptor);
    } else {
      delete globalThis.__RTS_DESKTOP_RUNTIME;
    }
  }

  const fakeBridge = {
    startCalls: [],
    stopCalls: [],
    supported() {
      return true;
    },
    start(bounds) {
      this.startCalls.push(bounds);
      return Promise.resolve({ active: true, mode: "native-macos" });
    },
    stop(reason) {
      this.stopCalls.push(reason);
      return Promise.resolve({ active: false });
    },
    diagnostics() {
      return { supported: true, backend: "native-macos", active: false };
    },
  };
  assert(cursorLockSupported(false, fakeBridge), "native cursor bridge makes cursor lock available without browser Pointer Lock");
  assert(nativeDesktopCursorBridge({ __RTS_NATIVE_CURSOR: fakeBridge }) === fakeBridge, "native desktop bridge is discovered from the runtime global");
  const tauriCalls = [];
  const tauriRoot = {
    __RTS_DESKTOP_RUNTIME: {
      shell: "tauri",
      platform: "macos",
      nativeCursorBackend: true,
      nativeCursorCapture: true,
      pointerLockDisabled: true,
      aggressiveCursorLock: true,
    },
    __TAURI__: {
      core: {
        invoke(cmd, payload) {
          tauriCalls.push({ cmd, payload });
          return Promise.resolve({ active: true, lastReason: "capture-start" });
        },
      },
    },
  };
  const tauriBridge = installTauriNativeCursorBridge(tauriRoot);
  assert(tauriBridge === nativeDesktopCursorBridge(tauriRoot), "macOS Tauri runtime installs the native desktop bridge in the page world");
  assert(tauriRoot.__RTS_DESKTOP_RUNTIME.nativeCursorCapture === true, "macOS runtime keeps native cursor capture required");
  const tauriStart = await tauriBridge.start({ x: 12, y: 34, width: 800, height: 600 });
  assert(tauriStart.active === true, "Tauri native bridge start returns the command snapshot");
  assert(tauriCalls[0].cmd === "maccursor_start", "Tauri native bridge invokes the Rust start command");
  assert(
    tauriCalls[0].payload.x === 12 && tauriCalls[0].payload.width === 800,
    "Tauri native bridge forwards cursor and viewport bounds",
  );
  const windowsTauriRoot = {
    __RTS_DESKTOP_RUNTIME: {
      shell: "tauri",
      platform: "windows",
      nativeCursorBackend: false,
      nativeCursorCapture: false,
      pointerLockDisabled: false,
      aggressiveCursorLock: false,
    },
    __TAURI__: tauriRoot.__TAURI__,
  };
  assert(
    installTauriNativeCursorBridge(windowsTauriRoot) === null,
    "Windows Tauri runtime does not infer or install the macOS native cursor bridge",
  );
  assert(
    windowsTauriRoot.__RTS_NATIVE_CURSOR === undefined,
    "Windows Tauri runtime leaves the native cursor global absent",
  );
  const tauriOnlyRoot = { __TAURI__: tauriRoot.__TAURI__ };
  assert(
    installTauriNativeCursorBridge(tauriOnlyRoot) === null,
    "Tauri globals alone do not imply macOS native cursor mode",
  );
  let nativeBrowserFallbackCalled = 0;
  const nativeMode = await enterCursorLock(
    async () => {
      nativeBrowserFallbackCalled += 1;
      return true;
    },
    { x: 42, y: 64 },
    fakeBridge,
    { width: 800, height: 600 },
  );
  assert(nativeMode === "native-macos", "native cursor bridge is preferred when available");
  assert(nativeBrowserFallbackCalled === 0, "native cursor bridge does not invoke browser Pointer Lock fallback");
  assert(fakeBridge.startCalls[0].x === 42 && fakeBridge.startCalls[0].width === 800, "native cursor bridge receives cursor and viewport bounds");
  let nativeBrowserExitCalled = false;
  await exitCursorLock("native-macos", () => {
    nativeBrowserExitCalled = true;
  }, fakeBridge, "test-stop");
  assert(fakeBridge.stopCalls[0] === "test-stop", "native cursor exit releases native capture");
  assert(!nativeBrowserExitCalled, "native cursor exit does not call browser Pointer Lock exit");

  let browserFallbackCalled = 0;
  const mode = await enterCursorLock(
    async () => {
      browserFallbackCalled += 1;
      return true;
    },
    { x: 42, y: 64 },
  );
  assert(mode === "browser", "cursor lock uses browser Pointer Lock");
  assert(browserFallbackCalled === 1, "browser Pointer Lock is invoked once");

  let browserExitCalled = false;
  await exitCursorLock("browser", () => {
    browserExitCalled = true;
  });
  assert(browserExitCalled, "cursor lock exits through browser Pointer Lock");

  const priorDocument = globalThis.document;
  const prefixedDom = {
    webkitRequestPointerLock() {},
  };
  let webkitExitCalled = false;
  globalThis.document = {
    webkitPointerLockElement: prefixedDom,
    webkitExitPointerLock() {
      webkitExitCalled = true;
    },
  };
  const prefixedInput = Object.create(Input.prototype);
  prefixedInput.dom = prefixedDom;
  assert(prefixedInput._browserPointerLockSupported(), "WebKit-prefixed Pointer Lock is supported");
  assert(prefixedInput._browserPointerLockElement() === prefixedDom, "WebKit-prefixed lock element is detected");
  prefixedInput._exitBrowserPointerLock();
  assert(webkitExitCalled, "WebKit-prefixed Pointer Lock exit is called");
  globalThis.document = priorDocument;
}

{
  const viewport = { requestPointerLock() {} };
  const canvas = { requestPointerLock() {} };
  const canvasInput = Object.create(Input.prototype);
  canvasInput.dom = viewport;
  canvasInput.renderElement = canvas;
  assert(canvasInput._pointerLockTarget() === canvas, "Pointer Lock prefers the injected render canvas target");
}

{
  let focused = false;
  let windowFocused = false;
  const priorWindow = globalThis.window;
  globalThis.window = {
    focus() {
      windowFocused = true;
    },
  };
  const focusInput = Object.create(Input.prototype);
  focusInput.dom = {
    clientWidth: 100,
    clientHeight: 80,
    focus(opts) {
      focused = !!opts?.preventScroll;
    },
  };
  focusInput.mouse = null;
  focusInput._setPointerLockCursor = () => {};
  focusInput._prepareCursorLock();
  assert(windowFocused, "Pointer Lock preparation asks the window to focus before requesting lock");
  assert(focused, "Pointer Lock preparation focuses the viewport before requesting lock");
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  let timeoutCallback = null;
  globalThis.window = {
    setTimeout(fn) {
      timeoutCallback = fn;
      return 1;
    },
  };
  globalThis.document = {
    hasFocus() { return true; },
    activeElement: { tagName: "DIV", id: "viewport", className: "" },
  };
  const pendingInput = Object.create(Input.prototype);
  pendingInput.dom = {};
  pendingInput._pointerLockAttempt = 3;
  pendingInput._lastPointerLockRequest = { attempt: 3, outcome: "pending" };
  pendingInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  pendingInput._browserPointerLockElement = () => null;
  const pending = pendingInput._waitForPointerLockPromise(new Promise(() => {}));
  assert(typeof timeoutCallback === "function", "promise Pointer Lock requests install a timeout");
  timeoutCallback();
  assert((await pending) === false, "pending Pointer Lock promise resolves false on timeout");
  assert(pendingInput._lastPointerLockRequest.outcome === "timeout", "pending Pointer Lock timeout is recorded");
  assert(
    pendingInput._pointerLockTrace.some((entry) => entry.phase === "browser-request-finish" && entry.details.outcome === "timeout"),
    "pending Pointer Lock timeout is retained in the lifecycle trace",
  );
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const doc = new EventTarget();
  const target = {};
  doc.pointerLockElement = null;
  globalThis.window = { setTimeout: globalThis.setTimeout };
  globalThis.document = doc;
  const eventConfirmedInput = Object.create(Input.prototype);
  eventConfirmedInput.dom = target;
  eventConfirmedInput._pointerLockAttempt = 4;
  eventConfirmedInput._lastPointerLockRequest = {
    attempt: 4,
    rawInputRequested: true,
    returnedPromise: true,
    outcome: "pending",
  };
  eventConfirmedInput._browserPointerLockElement = () => doc.pointerLockElement;
  eventConfirmedInput._pointerLockTarget = () => target;
  eventConfirmedInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  const waiting = eventConfirmedInput._waitForPointerLockPromise(Promise.resolve());
  await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
  assert(
    eventConfirmedInput._pointerLockTrace.some((entry) => entry.phase === "browser-promise-resolved-awaiting-event"),
    "resolved Pointer Lock promises wait for the authoritative browser event",
  );
  doc.pointerLockElement = target;
  doc.dispatchEvent(new Event("pointerlockchange"));
  assert(await waiting, "Pointer Lock succeeds when pointerlockchange follows an early promise resolution");
  assert(
    eventConfirmedInput._lastPointerLockRequest.outcome === "resolved-pointerlockchange",
    "event-confirmed Pointer Lock records both promise and browser-event completion",
  );
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}

{
  let locked = false;
  const requests = [];
  const target = {};
  const rawOnlyInput = Object.create(Input.prototype);
  rawOnlyInput._pointerLockAttempt = 4;
  rawOnlyInput._browserPointerLockSupported = () => true;
  rawOnlyInput._browserPointerLockElement = () => locked ? target : null;
  rawOnlyInput._pointerLockTarget = () => target;
  rawOnlyInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  rawOnlyInput._browserRequestPointerLock = () => (options) => {
    requests.push(options);
    if (options?.unadjustedMovement) return Promise.reject(new Error("raw input unavailable"));
    locked = true;
    return Promise.resolve();
  };
  rawOnlyInput._waitForPointerLockPromise = async (promise) => {
    try {
      await promise;
      rawOnlyInput._finishPointerLockRequest("resolved");
      return rawOnlyInput._browserPointerLockElement() === target;
    } catch (err) {
      rawOnlyInput._finishPointerLockRequest("rejected", err);
      return false;
    }
  };
  let rawFailure = null;
  rawOnlyInput.onPointerLockError = (err) => { rawFailure = err; };
  assert(!(await rawOnlyInput._requestBrowserPointerLock()), "Pointer Lock fails closed after raw input rejection");
  assert(requests.length === 1, "Pointer Lock does not request plain fallback after raw rejection");
  assert(requests[0]?.unadjustedMovement === true, "first Pointer Lock request asks for unadjusted movement");
  assert(rawOnlyInput._lastPointerLockRequest.rawInputRequested === true, "raw rejection records the raw request");
  assert(rawOnlyInput._lastPointerLockRequest.outcome === "rejected", "raw rejection outcome is recorded");
  assert(rawFailure?.message === "raw input unavailable", "raw rejection reaches installed-app diagnostics instead of failing silently");
  assert(
    rawOnlyInput._pointerLockTrace.some((entry) => entry.phase === "failure"),
    "raw rejection retains an explicit failure in the lifecycle trace",
  );
}

{
  const rawSuccessRequests = [];
  const target = {};
  const rawSuccessInput = Object.create(Input.prototype);
  rawSuccessInput._pointerLockAttempt = 5;
  rawSuccessInput._browserPointerLockSupported = () => true;
  rawSuccessInput._browserPointerLockElement = () => target;
  rawSuccessInput._pointerLockTarget = () => target;
  rawSuccessInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  rawSuccessInput._browserRequestPointerLock = () => (options) => {
    rawSuccessRequests.push(options);
    return Promise.resolve();
  };
  rawSuccessInput._waitForPointerLockPromise = async (promise) => {
    await promise;
    rawSuccessInput._finishPointerLockRequest("resolved");
    return true;
  };
  assert(await rawSuccessInput._requestBrowserPointerLock(), "Pointer Lock succeeds with raw input");
  assert(rawSuccessRequests.length === 1, "raw Pointer Lock success does not make a fallback request");
  assert(rawSuccessInput._lastPointerLockRequest.rawInputRequested === true, "raw request is recorded for diagnostics");
  assert(
    rawSuccessInput._pointerLockTrace.some((entry) => entry.phase === "browser-request-complete" && entry.details.locked === true),
    "raw Pointer Lock success retains the completed lifecycle result",
  );
}

{
  const priorRuntimeDescriptor = Object.getOwnPropertyDescriptor(globalThis, "__RTS_DESKTOP_RUNTIME");
  const priorTauriDescriptor = Object.getOwnPropertyDescriptor(globalThis, "__TAURI__");
  const priorTraceDescriptor = Object.getOwnPropertyDescriptor(globalThis, "__rtsPointerLockTrace");
  const calls = [];
  Object.defineProperty(globalThis, "__RTS_DESKTOP_RUNTIME", {
    configurable: true,
    value: { shell: "tauri", platform: "windows" },
  });
  Object.defineProperty(globalThis, "__TAURI__", {
    configurable: true,
    value: {
      core: {
        invoke(command, payload) {
          calls.push({ command, payload });
          return Promise.resolve();
        },
      },
    },
  });
  const loggedInput = Object.create(Input.prototype);
  loggedInput._pointerLockAttempt = 9;
  loggedInput._recordPointerLockTrace("attempt-start", {
    browserSupported: true,
    focus: { documentHasFocus: true, activeElement: { tag: "CANVAS", id: "game" } },
  });
  await Promise.resolve();
  assert(calls.length === 1, "installed-app Pointer Lock trace writes through the existing Tauri shell command");
  assert(calls[0].command === "desktop_log_client_event", "Pointer Lock trace uses the bounded client-event logger");
  assert(calls[0].payload.event === "pointer_lock_attempt-start", "shell log records the lifecycle phase in its source field");
  assert(calls[0].payload.message.length <= 560, "Pointer Lock shell log payload stays within the installed shell bound");
  assert(
    globalThis.__rtsPointerLockTrace.records.at(-1)?.phase === "attempt-start",
    "latest Pointer Lock lifecycle trace is published for live inspection",
  );
  assert(loggedInput._pointerLockShellLog.succeeded === 1, "successful shell persistence is visible in the debug snapshot");
  if (priorRuntimeDescriptor) Object.defineProperty(globalThis, "__RTS_DESKTOP_RUNTIME", priorRuntimeDescriptor);
  else delete globalThis.__RTS_DESKTOP_RUNTIME;
  if (priorTauriDescriptor) Object.defineProperty(globalThis, "__TAURI__", priorTauriDescriptor);
  else delete globalThis.__TAURI__;
  if (priorTraceDescriptor) Object.defineProperty(globalThis, "__rtsPointerLockTrace", priorTraceDescriptor);
  else delete globalThis.__rtsPointerLockTrace;
}

{
  const tracedInput = Object.create(Input.prototype);
  tracedInput._pointerLockAttempt = 10;
  for (let i = 0; i < 90; i += 1) tracedInput._recordPointerLockTrace("bounded", { index: i });
  assert(tracedInput._pointerLockTrace.length === 80, "Pointer Lock lifecycle trace keeps a bounded in-memory ring");
  assert(tracedInput._pointerLockTrace[0].details.index === 10, "Pointer Lock trace discards only the oldest records");
}

{
  const eventInput = Object.create(Input.prototype);
  eventInput._pointerLockAttempt = 11;
  eventInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  eventInput._pointerLockErrorSummary = Input.prototype._pointerLockErrorSummary;
  let reported = null;
  eventInput.onPointerLockError = (err) => { reported = err; };
  eventInput._handlePointerLockError({ type: "pointerlockerror" });
  assert(reported?.message === "Pointer Lock emitted pointerlockerror.", "Pointer Lock error events gain an actionable synthetic message");
  assert(
    eventInput._pointerLockTrace.some((entry) => entry.phase === "browser-event-error"),
    "Pointer Lock error events are retained separately from the final failure",
  );
  assert(
    eventInput._pointerLockTrace.some((entry) => entry.phase === "failure"),
    "Pointer Lock error events reach the common failure diagnostic path",
  );
}

{
  const guardedInput = Object.create(Input.prototype);
  let browserRequests = 0;
  let resolveBrowserRequest;
  const target = {};
  guardedInput.pointerLocked = false;
  guardedInput._pointerLockAttempt = 0;
  guardedInput.desktopCursor = null;
  guardedInput._browserPointerLockSupported = () => true;
  guardedInput._prepareCursorLock = () => {};
  guardedInput._nativeCursorBounds = () => ({ width: 100, height: 80 });
  guardedInput._browserPointerLockElement = () => null;
  guardedInput._pointerLockTarget = () => target;
  guardedInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  guardedInput._requestBrowserPointerLock = () => {
    browserRequests += 1;
    return new Promise((resolve) => { resolveBrowserRequest = resolve; });
  };
  const firstRequest = guardedInput.requestPointerLock();
  const overlappingRequest = guardedInput.requestPointerLock();
  assert(firstRequest === overlappingRequest, "overlapping Pointer Lock requests share the pending request");
  assert(browserRequests === 1, "overlapping Pointer Lock requests do not issue a second browser call");
  assert(guardedInput._pointerLockAttempt === 1, "overlapping Pointer Lock requests keep one attempt identity");
  resolveBrowserRequest(true);
  assert(await firstRequest, "shared pending Pointer Lock request resolves for both callers");
  assert(guardedInput._pointerLockRequestInFlight === null, "Pointer Lock pending state clears after settlement");
}

{
  const lockedEscapeInput = Object.create(Input.prototype);
  let exits = 0;
  let selectionCleared = 0;
  lockedEscapeInput.pointerLocked = true;
  lockedEscapeInput.state = {
    clearSelection() {
      selectionCleared += 1;
    },
  };
  lockedEscapeInput.clientIntent = new ClientIntent();
  lockedEscapeInput.clientIntent.beginCommandTarget("move");
  lockedEscapeInput.exitPointerLock = () => {
    exits += 1;
    return Promise.resolve(true);
  };
  const ev = {
    code: "Escape",
    preventDefault() {
      this.prevented = true;
    },
  };
  lockedEscapeInput._handleKeyDown(ev);
  assert(exits === 0, "Esc does not unlock cursor lock");
  assert(lockedEscapeInput.clientIntent.commandTarget === null, "Esc still cancels command targeting while cursor-locked");
  assert(selectionCleared === 0, "cursor-locked targeting cancel does not fall through to selection clear");
  assert(ev.prevented, "cursor-locked Esc still prevents browser handling after gameplay cancel");
}

{
  const quietMoveInput = Object.create(Input.prototype);
  let routedMoves = 0;
  let previewRefreshes = 0;
  quietMoveInput.pointerLocked = true;
  quietMoveInput._panDrag = null;
  quietMoveInput._drag = null;
  quietMoveInput._lockedMovementDelta = () => ({ x: 0, y: 0 });
  quietMoveInput._routeLockedPointerMove = () => {
    routedMoves += 1;
    return false;
  };
  quietMoveInput._refreshResourceMiningPreview = () => {
    previewRefreshes += 1;
  };
  quietMoveInput._handleMouseMove({});
  assert(routedMoves === 0 && previewRefreshes === 0, "zero-delta locked mousemove does no hover work");
}

{
  let previewRefreshes = 0;
  const painted = { style: {} };
  const lockedMoveInput = Object.create(Input.prototype);
  lockedMoveInput.pointerLocked = true;
  lockedMoveInput.mouse = { x: 10, y: 20 };
  lockedMoveInput.dom = { clientWidth: 100, clientHeight: 100 };
  lockedMoveInput._panDrag = null;
  lockedMoveInput._drag = null;
  lockedMoveInput._pointerLockCursor = painted;
  lockedMoveInput._pendingPointerLockCursor = null;
  lockedMoveInput._routeLockedPointerMove = () => false;
  lockedMoveInput._refreshResourceMiningPreview = () => {
    previewRefreshes += 1;
  };
  lockedMoveInput._handleMouseMove({ movementX: 3, movementY: -4 });
  assert(lockedMoveInput.mouse.x === 13 && lockedMoveInput.mouse.y === 16, "locked mousemove updates virtual cursor state");
  assert(previewRefreshes === 0, "nonzero locked mousemove defers hover work to frame update");
  assert(painted.style.transform === undefined, "locked mousemove defers virtual cursor paint");
  lockedMoveInput._flushPointerLockCursor();
  assert(painted.style.transform === "translate(13px, 16px)", "virtual cursor paint flushes once per frame");
}

{
  const painted = { style: {} };
  const nativeMoveInput = Object.create(Input.prototype);
  nativeMoveInput.pointerLocked = true;
  nativeMoveInput._cursorLockMode = "native-macos";
  nativeMoveInput.mouse = { x: 10, y: 20 };
  nativeMoveInput.dom = {
    clientWidth: 100,
    clientHeight: 100,
    getBoundingClientRect() {
      return { left: 5, top: 7, width: 100, height: 100 };
    },
  };
  nativeMoveInput.cameraNavigation = null;
  nativeMoveInput.inputRouter = null;
  nativeMoveInput._panDrag = null;
  nativeMoveInput._drag = null;
  nativeMoveInput._pointerLockCursor = painted;
  nativeMoveInput._pendingPointerLockCursor = null;
  nativeMoveInput._handleNativeCursorEvent({ type: "move", x: 14, y: 15, dx: 4, dy: -5 });
  assert(nativeMoveInput.mouse.x === 14 && nativeMoveInput.mouse.y === 15, "native move updates virtual cursor coordinates from native event coordinates");
  assert(painted.style.transform === "translate(14px, 15px)", "native move paints the DOM cursor during the native event handler");
  assert(nativeMoveInput._pendingPointerLockCursor === null, "native move does not wait for Input.update to flush the cursor visual");
}

{
  const releasedSources = [];
  const unlockedInput = Object.create(Input.prototype);
  unlockedInput.pointerLocked = true;
  unlockedInput._cursorLockMode = "browser";
  unlockedInput.mouse = { x: 25, y: 30 };
  unlockedInput.dom = { classList: { toggle() {} } };
  unlockedInput.inputRouter = { releaseSource: (source) => releasedSources.push(source) };
  unlockedInput._pointerLockCursor = { hidden: false };
  unlockedInput._nativeButtonsMask = 1;
  unlockedInput._panDrag = null;
  unlockedInput._drag = null;
  unlockedInput._placementDrag = null;

  unlockedInput._setCursorLockState(false, null);
  assert(releasedSources.length === 1 && releasedSources[0] === "locked",
    "leaving pointer lock releases hover owned by the locked input source");
}

{
  let routed = null;
  const nativeRouteInput = Object.create(Input.prototype);
  nativeRouteInput.pointerLocked = true;
  nativeRouteInput._cursorLockMode = "native-macos";
  nativeRouteInput.mouse = { x: 0, y: 0 };
  nativeRouteInput.dom = {
    clientWidth: 120,
    clientHeight: 80,
    getBoundingClientRect() {
      return { left: 10, top: 20, width: 120, height: 80 };
    },
  };
  nativeRouteInput.cameraNavigation = null;
  nativeRouteInput.inputRouter = {
    pointerDown(ev) {
      routed = ev;
      return true;
    },
  };
  nativeRouteInput._pointerLockCursor = { style: {} };
  nativeRouteInput._pendingPointerLockCursor = null;
  nativeRouteInput._panDrag = null;
  nativeRouteInput._drag = null;
  nativeRouteInput._handleNativeCursorEvent({ type: "down", button: 0, x: 33, y: 44 });
  assert(routed.viewportX === 33 && routed.viewportY === 44, "native pointerDown routes viewport coords from the native cursor");
  assert(routed.clientX === 43 && routed.clientY === 64, "native pointerDown routes client coords matching the native cursor");
}

{
  const routed = [];
  const nativeButtonInput = Object.create(Input.prototype);
  nativeButtonInput.pointerLocked = true;
  nativeButtonInput._cursorLockMode = "native-macos";
  nativeButtonInput.mouse = { x: 0, y: 0 };
  nativeButtonInput.dom = {
    clientWidth: 120,
    clientHeight: 80,
    getBoundingClientRect() {
      return { left: 10, top: 20, width: 120, height: 80 };
    },
  };
  nativeButtonInput.cameraNavigation = null;
  nativeButtonInput.inputRouter = {
    pointerDown(ev) {
      routed.push(["down", ev.buttons]);
      return true;
    },
    pointerMove(ev) {
      routed.push(["move", ev.buttons]);
      return true;
    },
    pointerUp(ev) {
      routed.push(["up", ev.buttons]);
      return true;
    },
  };
  nativeButtonInput._pointerLockCursor = { style: {} };
  nativeButtonInput._pendingPointerLockCursor = null;
  nativeButtonInput._nativeButtonsMask = 0;
  nativeButtonInput._panDrag = null;
  nativeButtonInput._drag = null;
  nativeButtonInput._finishTankTrapPlacementDrag = () => false;

  nativeButtonInput._handleNativeCursorEvent({ type: "down", button: 0, x: 20, y: 20 });
  nativeButtonInput._handleNativeCursorEvent({ type: "move", x: 30, y: 30, dx: 10, dy: 10 });
  nativeButtonInput._handleNativeCursorEvent({ type: "up", button: 0, x: 30, y: 30 });

  assert(routed.map((entry) => entry.join(":")).join(",") === "down:1,move:1,up:0",
    "native cursor events preserve the pressed left-button mask across drag moves");
}

{
  const boxes = [];
  let committedDrag = null;
  const nativeDragInput = Object.create(Input.prototype);
  nativeDragInput.pointerLocked = true;
  nativeDragInput._cursorLockMode = "native-macos";
  nativeDragInput.mouse = { x: 10, y: 10 };
  nativeDragInput.dom = {
    clientWidth: 200,
    clientHeight: 160,
    getBoundingClientRect() {
      return { left: 0, top: 0, width: 200, height: 160 };
    },
  };
  nativeDragInput.cameraNavigation = null;
  nativeDragInput.inputRouter = null;
  nativeDragInput._pointerLockCursor = { style: {} };
  nativeDragInput._pendingPointerLockCursor = null;
  nativeDragInput._nativeButtonsMask = 0;
  nativeDragInput._panDrag = null;
  nativeDragInput._drag = null;
  nativeDragInput._postQuickCastSelectionGuard = null;
  nativeDragInput.screenOverlay = {
    setMarquee(box) {
      boxes.push(box);
    },
    clearMarquee() { boxes.push(null); },
  };
  nativeDragInput._placement = () => null;
  nativeDragInput._commandTarget = () => null;
  nativeDragInput._labTool = () => null;
  nativeDragInput._intent = () => null;
  nativeDragInput._cancelLabToolForBoxSelect = () => {};
  nativeDragInput._finishTankTrapPlacementDrag = () => false;
  nativeDragInput._commitBoxSelection = (drag) => {
    committedDrag = drag;
  };
  nativeDragInput._commitClickSelection = () => {
    throw new Error("native left-drag should not finish as a click");
  };

  nativeDragInput._handleNativeCursorEvent({ type: "down", button: 0, x: 10, y: 10 });
  nativeDragInput._handleNativeCursorEvent({ type: "move", x: 40, y: 44, dx: 30, dy: 34 });
  nativeDragInput._handleNativeCursorEvent({ type: "up", button: 0, x: 40, y: 44 });

  assert(boxes.some((box) => box?.w === 30 && box?.h === 34), "native left-drag draws the gameplay selection box");
  assert(committedDrag?.x0 === 10 && committedDrag?.x1 === 40, "native left-drag commits box selection on release");
  assert(boxes.at(-1) === null, "native left-drag clears the selection box after release");
}

{
  const pans = [];
  const nativeMiddleInput = Object.create(Input.prototype);
  nativeMiddleInput.pointerLocked = true;
  nativeMiddleInput._cursorLockMode = "native-macos";
  nativeMiddleInput.mouse = { x: 40, y: 50 };
  nativeMiddleInput.dom = {
    clientWidth: 200,
    clientHeight: 160,
    getBoundingClientRect() {
      return { left: 0, top: 0, width: 200, height: 160 };
    },
  };
  nativeMiddleInput.cameraNavigation = new CameraNavigationInput(nativeMiddleInput.dom, {
    panByScreenDelta(delta) {
      pans.push({ dx: delta.x, dy: delta.y });
    },
  });
  nativeMiddleInput.inputRouter = null;
  nativeMiddleInput._pointerLockCursor = { style: {} };
  nativeMiddleInput._pendingPointerLockCursor = null;
  nativeMiddleInput._panDrag = null;
  nativeMiddleInput._drag = null;
  nativeMiddleInput._routeLockedPointerDown = () => false;
  nativeMiddleInput._routeLockedPointerMove = () => false;
  nativeMiddleInput._routeLockedPointerUp = () => false;
  nativeMiddleInput._refreshResourceMiningPreview = () => {};
  nativeMiddleInput._placement = () => null;
  nativeMiddleInput._commandTarget = () => null;
  nativeMiddleInput._labTool = () => null;
  nativeMiddleInput._intent = () => null;

  nativeMiddleInput._handleNativeCursorEvent({ type: "down", button: 1, x: 40, y: 50 });
  nativeMiddleInput._handleNativeCursorEvent({ type: "move", x: 64, y: 68, dx: 24, dy: 18 });
  nativeMiddleInput._handleNativeCursorEvent({ type: "up", button: 1, x: 64, y: 68 });

  assert(pans.length === 1, "native middle-drag pans through shared camera navigation");
  assert(pans[0].dx === 24 && pans[0].dy === 18, "native middle-drag uses native cursor screen delta");
  assert(nativeMiddleInput.cameraNavigation.panDrag === null, "native middle-drag releases the shared pan state");
}

{
  let exits = 0;
  const nativeBlurInput = Object.create(Input.prototype);
  nativeBlurInput.pointerLocked = true;
  nativeBlurInput._shiftKeyDown = true;
  nativeBlurInput.cameraNavigation = { release() {} };
  nativeBlurInput._drag = null;
  nativeBlurInput._placement = () => null;
  nativeBlurInput._intent = () => null;
  nativeBlurInput.exitPointerLock = () => {
    exits += 1;
    return Promise.resolve(true);
  };
  nativeBlurInput._handleBlur();
  assert(exits === 1, "blur releases native cursor capture through the cursor-lock seam");
}

{
  let exits = 0;
  let removedNativeListener = 0;
  let cameraDestroyed = 0;
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  globalThis.window = {
    removeEventListener() {},
  };
  globalThis.document = {
    removeEventListener() {},
  };
  const nativeDestroyInput = Object.create(Input.prototype);
  nativeDestroyInput.exitPointerLock = () => {
    exits += 1;
    return Promise.resolve(true);
  };
  nativeDestroyInput.dom = {
    removeEventListener() {},
  };
  nativeDestroyInput.cameraNavigation = {
    destroy() {
      cameraDestroyed += 1;
    },
  };
  nativeDestroyInput._removeNativeCursorListener = () => {
    removedNativeListener += 1;
  };
  nativeDestroyInput._pointerLockCursor = {
    remove() {},
  };
  nativeDestroyInput.destroy();
  assert(exits === 1, "destroy releases native cursor capture through the cursor-lock seam");
  assert(removedNativeListener === 1, "destroy removes the native cursor event listener");
  assert(cameraDestroyed === 1, "destroy keeps existing camera navigation teardown");
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}
