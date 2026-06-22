const CURSOR_LOCK_BROWSER = "browser";
const CURSOR_LOCK_NATIVE_MACOS = "native-macos";

export function installedAppRuntime() {
  const standaloneDisplay = globalThis.matchMedia?.("(display-mode: standalone)")?.matches;
  return !!standaloneDisplay ||
    globalThis.navigator?.standalone === true ||
    !!globalThis.__RTS_DESKTOP_RUNTIME;
}

export function nativeDesktopCursorBridge(root = globalThis) {
  const bridge = root?.__RTS_NATIVE_CURSOR;
  return nativeBridgeSupported(bridge) ? bridge : null;
}

export function nativeBridgeSupported(bridge) {
  if (!bridge) return false;
  if (typeof bridge.supported === "function") return !!bridge.supported();
  return bridge.supported === true;
}

export function cursorLockSupported(browserPointerLockSupported, nativeBridge = nativeDesktopCursorBridge()) {
  return nativeBridgeSupported(nativeBridge) || browserPointerLockSupported;
}

export async function enterCursorLock(
  enterBrowserPointerLock,
  cursor = null,
  nativeBridge = nativeDesktopCursorBridge(),
  bounds = null,
) {
  if (nativeBridgeSupported(nativeBridge) && typeof nativeBridge.start === "function") {
    const startBounds = {
      x: Number.isFinite(cursor?.x) ? cursor.x : (Number.isFinite(bounds?.width) ? bounds.width / 2 : 0),
      y: Number.isFinite(cursor?.y) ? cursor.y : (Number.isFinite(bounds?.height) ? bounds.height / 2 : 0),
      width: Number.isFinite(bounds?.width) ? bounds.width : 0,
      height: Number.isFinite(bounds?.height) ? bounds.height : 0,
    };
    const started = await nativeBridge.start(startBounds);
    if (started?.active !== false) return started?.mode || CURSOR_LOCK_NATIVE_MACOS;
  }
  const browserLocked = await enterBrowserPointerLock();
  return browserLocked ? CURSOR_LOCK_BROWSER : null;
}

export async function exitCursorLock(
  mode,
  exitBrowserPointerLock,
  nativeBridge = nativeDesktopCursorBridge(),
  reason = "js-stop",
) {
  if (mode === CURSOR_LOCK_NATIVE_MACOS && nativeBridgeSupported(nativeBridge) && typeof nativeBridge.stop === "function") {
    await nativeBridge.stop(reason);
    return;
  }
  exitBrowserPointerLock();
}

export function nativeCursorDebugSnapshot(nativeBridge = nativeDesktopCursorBridge()) {
  if (!nativeBridgeSupported(nativeBridge)) {
    return {
      supported: false,
      backend: null,
      active: false,
    };
  }
  if (typeof nativeBridge.diagnostics === "function") {
    return nativeBridge.diagnostics();
  }
  return {
    supported: true,
    backend: nativeBridge.backend || CURSOR_LOCK_NATIVE_MACOS,
    active: false,
  };
}
