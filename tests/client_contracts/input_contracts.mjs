// tests/client_contracts/input_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { HUD } from "../../client/src/hud.js";
import { Input } from "../../client/src/input/index.js";
import { CameraNavigationInput } from "../../client/src/input/camera_navigation.js";
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
  assert(router.pointerDown({ clientX: 760, clientY: 560, button: 0, source: "locked" }), "DOM zone consumes empty HUD panel space");
  assert(router.pointerUp({ clientX: 760, clientY: 560, button: 0, source: "locked" }), "empty HUD panel click releases capture");
  assert(button.clickCount === 1, "empty HUD panel space does not click the prior button");
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
  assert(tauriBridge === nativeDesktopCursorBridge(tauriRoot), "Tauri global installs the native desktop bridge in the page world");
  assert(tauriRoot.__RTS_DESKTOP_RUNTIME.nativeCursorCapture === true, "Tauri bridge marks native cursor capture as required");
  const tauriStart = await tauriBridge.start({ x: 12, y: 34, width: 800, height: 600 });
  assert(tauriStart.active === true, "Tauri native bridge start returns the command snapshot");
  assert(tauriCalls[0].cmd === "maccursor_start", "Tauri native bridge invokes the Rust start command");
  assert(
    tauriCalls[0].payload.x === 12 && tauriCalls[0].payload.width === 800,
    "Tauri native bridge forwards cursor and viewport bounds",
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
  canvasInput.renderer = { app: { view: canvas } };
  assert(canvasInput._pointerLockTarget() === canvas, "Pointer Lock prefers the Pixi canvas target");
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
  assert(!(await rawOnlyInput._requestBrowserPointerLock()), "Pointer Lock fails closed after raw input rejection");
  assert(requests.length === 1, "Pointer Lock does not request plain fallback after raw rejection");
  assert(requests[0]?.unadjustedMovement === true, "first Pointer Lock request asks for unadjusted movement");
  assert(rawOnlyInput._lastPointerLockRequest.rawInputRequested === true, "raw rejection records the raw request");
  assert(rawOnlyInput._lastPointerLockRequest.outcome === "rejected", "raw rejection outcome is recorded");
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
    panByScreenDelta(dx, dy) {
      pans.push({ dx, dy });
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
