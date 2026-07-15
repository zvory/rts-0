const CURSOR_LOCK_BROWSER = "browser";
const CURSOR_LOCK_NATIVE_MACOS = "native-macos";

export function installedAppRuntime() {
  const standaloneDisplay = globalThis.matchMedia?.("(display-mode: standalone)")?.matches;
  return !!standaloneDisplay ||
    globalThis.navigator?.standalone === true ||
    !!globalThis.__RTS_DESKTOP_RUNTIME;
}

export function nativeDesktopCursorBridge(root = globalThis) {
  installTauriNativeCursorBridge(root);
  const bridge = root?.__RTS_NATIVE_CURSOR;
  return nativeBridgeSupported(bridge) ? bridge : null;
}

export function nativeBridgeSupported(bridge) {
  if (!bridge) return false;
  if (typeof bridge.supported === "function") return !!bridge.supported();
  return bridge.supported === true;
}

export function installTauriNativeCursorBridge(root = globalThis) {
  if (!root) return null;
  const existingBridge = root.__RTS_NATIVE_CURSOR;
  if (nativeBridgeSupported(existingBridge)) return existingBridge;
  if (!macosNativeCursorRuntime(root.__RTS_DESKTOP_RUNTIME)) return null;
  if (typeof tauriInvokeFn(root) !== "function") return null;

  const listeners = new Set();
  const diagnostics = {
    supported: true,
    backend: CURSOR_LOCK_NATIVE_MACOS,
    active: false,
    visual: "dom-event-time",
    movementBatched: false,
    nativeEventsReceived: 0,
    jsEventsProcessed: 0,
    droppedEvents: 0,
    backloggedEvents: 0,
    lastSequence: 0,
    lastDeliveryLatencyMs: null,
    lastReason: "ready",
    lastError: null,
  };
  const invoke = (cmd, payload = {}) => {
    const tauriInvoke = tauriInvokeFn(root);
    if (typeof tauriInvoke !== "function") {
      diagnostics.lastError = "Tauri invoke bridge is unavailable.";
      diagnostics.lastReason = "invoke-unavailable";
      return Promise.reject(new Error(diagnostics.lastError));
    }
    try {
      return Promise.resolve(tauriInvoke(cmd, payload));
    } catch (err) {
      return Promise.reject(err);
    }
  };
  const mergeDiagnostics = (snapshot) => {
    if (!snapshot || typeof snapshot !== "object") return snapshot;
    diagnostics.active = !!snapshot.active;
    diagnostics.nativeEventsReceived = toFiniteNumber(snapshot.nativeEventsReceived, diagnostics.nativeEventsReceived);
    diagnostics.droppedEvents = toFiniteNumber(snapshot.droppedEvents, diagnostics.droppedEvents);
    diagnostics.lastReason = snapshot.lastReason || diagnostics.lastReason;
    diagnostics.lastError = snapshot.lastError || null;
    return snapshot;
  };
  const dispatchNativeEvent = (detail) => {
    if (!detail || typeof detail !== "object") return;
    diagnostics.nativeEventsReceived = toFiniteNumber(detail.nativeEventsReceived, diagnostics.nativeEventsReceived + 1);
    diagnostics.jsEventsProcessed += 1;
    if (Number.isFinite(detail.sequence)) {
      if (diagnostics.lastSequence && detail.sequence > diagnostics.lastSequence + 1) {
        diagnostics.droppedEvents += detail.sequence - diagnostics.lastSequence - 1;
      }
      diagnostics.lastSequence = detail.sequence;
    }
    if (Number.isFinite(detail.sentAtMs)) {
      diagnostics.lastDeliveryLatencyMs = Math.max(0, Date.now() - detail.sentAtMs);
    }
    if (detail.type === "capture") diagnostics.active = false;
    for (const listener of Array.from(listeners)) {
      try {
        listener(detail);
      } catch (err) {
        diagnostics.lastError = err?.message || String(err);
      }
    }
  };
  const bridge = Object.freeze({
    supported: () => true,
    backend: CURSOR_LOCK_NATIVE_MACOS,
    visual: "dom-event-time",
    start: (bounds = {}) => invoke("maccursor_start", {
      x: toFiniteNumber(bounds.x, 0),
      y: toFiniteNumber(bounds.y, 0),
      width: toFiniteNumber(bounds.width, 0),
      height: toFiniteNumber(bounds.height, 0),
    }).then((snapshot) => {
      mergeDiagnostics(snapshot);
      diagnostics.active = !!snapshot?.active;
      return snapshot;
    }).catch((err) => {
      diagnostics.active = false;
      diagnostics.lastError = err?.message || String(err);
      diagnostics.lastReason = "capture-start-failed";
      throw err;
    }),
    configure: (bounds = {}) => invoke("maccursor_configure", {
      width: toFiniteNumber(bounds.width, 0),
      height: toFiniteNumber(bounds.height, 0),
    }).then(mergeDiagnostics),
    stop: (reason = "js-stop") => invoke("maccursor_stop", { reason }).then((snapshot) => {
      mergeDiagnostics(snapshot);
      diagnostics.active = false;
      return snapshot;
    }),
    diagnostics: () => Object.freeze({ ...diagnostics }),
    nativeDiagnostics: () => invoke("maccursor_diagnostics").then(mergeDiagnostics),
    onEvent: (listener) => {
      if (typeof listener !== "function") return () => {};
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    __dispatchNativeEvent: dispatchNativeEvent,
  });
  defineRuntimeGlobal(root, "__RTS_NATIVE_CURSOR", bridge);
  disableBrowserPointerLockForNativeShell(root);
  return root.__RTS_NATIVE_CURSOR === bridge ? bridge : nativeBridgeSupported(root.__RTS_NATIVE_CURSOR) ? root.__RTS_NATIVE_CURSOR : bridge;
}

function macosNativeCursorRuntime(runtime) {
  return runtime?.shell === "tauri" &&
    runtime?.platform === "macos" &&
    runtime?.nativeCursorBackend === true &&
    runtime?.nativeCursorCapture === true &&
    runtime?.pointerLockDisabled === true;
}

export function cursorLockSupported(browserPointerLockSupported, nativeBridge = nativeDesktopCursorBridge()) {
  if (nativeCursorRequired()) return nativeBridgeSupported(nativeBridge);
  return nativeBridgeSupported(nativeBridge) || browserPointerLockSupported;
}

export async function enterCursorLock(
  enterBrowserPointerLock,
  cursor = null,
  nativeBridge = nativeDesktopCursorBridge(),
  bounds = null,
) {
  const nativeRequired = nativeCursorRequired();
  if (nativeRequired && !nativeBridgeSupported(nativeBridge)) {
    throw new Error("Native cursor bridge is unavailable in the desktop shell.");
  }
  if (nativeBridgeSupported(nativeBridge) && typeof nativeBridge.start === "function") {
    const startBounds = {
      x: Number.isFinite(cursor?.x) ? cursor.x : (Number.isFinite(bounds?.width) ? bounds.width / 2 : 0),
      y: Number.isFinite(cursor?.y) ? cursor.y : (Number.isFinite(bounds?.height) ? bounds.height / 2 : 0),
      width: Number.isFinite(bounds?.width) ? bounds.width : 0,
      height: Number.isFinite(bounds?.height) ? bounds.height : 0,
    };
    const started = await nativeBridge.start(startBounds);
    if (started?.active !== false) return started?.mode || CURSOR_LOCK_NATIVE_MACOS;
    if (nativeRequired) throw new Error("Native cursor capture did not activate.");
  }
  const browserLocked = await enterBrowserPointerLock();
  return browserLocked ? CURSOR_LOCK_BROWSER : null;
}

function nativeCursorRequired(root = globalThis) {
  installTauriNativeCursorBridge(root);
  const runtime = root?.__RTS_DESKTOP_RUNTIME;
  return !!(runtime?.nativeCursorCapture || runtime?.pointerLockDisabled);
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

function tauriInvokeFn(root) {
  const candidates = [
    root.__TAURI_INTERNALS__?.invoke,
    root.__TAURI__?.core?.invoke,
    root.__TAURI__?.tauri?.invoke,
    root.__TAURI__?.invoke,
  ];
  return candidates.find((candidate) => typeof candidate === "function") || null;
}

function defineRuntimeGlobal(root, name, value) {
  try {
    Object.defineProperty(root, name, {
      value,
      configurable: true,
      writable: false,
    });
  } catch {
    try {
      root[name] = value;
    } catch {}
  }
}

function disableBrowserPointerLockForNativeShell(root) {
  const denied = () => Promise.reject(makePointerLockDeniedError(root));
  const replace = (target, name) => {
    if (!target || typeof target[name] !== "function") return;
    try {
      Object.defineProperty(target, name, {
        value: denied,
        configurable: true,
        writable: false,
      });
    } catch {}
  };
  replace(root.Element?.prototype, "requestPointerLock");
  replace(root.Element?.prototype, "webkitRequestPointerLock");
  replace(root.HTMLElement?.prototype, "requestPointerLock");
  replace(root.HTMLElement?.prototype, "webkitRequestPointerLock");
}

function makePointerLockDeniedError(root) {
  if (typeof root.DOMException === "function") {
    return new root.DOMException(
      "Pointer Lock is disabled in the macOS native-cursor shell.",
      "NotAllowedError",
    );
  }
  const err = new Error("Pointer Lock is disabled in the macOS native-cursor shell.");
  err.name = "NotAllowedError";
  return err;
}

function toFiniteNumber(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}
