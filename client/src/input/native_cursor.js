const CURSOR_LOCK_NATIVE = "native";
const CURSOR_LOCK_BROWSER = "browser";

export function desktopRuntime() {
  return !!globalThis.__TAURI_INTERNALS__ || !!globalThis.__TAURI__?.core;
}

function tauriInvoke() {
  const invoke = globalThis.__TAURI__?.core?.invoke || globalThis.__TAURI_INTERNALS__?.invoke;
  return typeof invoke === "function" ? invoke : null;
}

export function nativeCursorSupported() {
  return false;
}

export function cursorLockSupported(browserPointerLockSupported) {
  return browserPointerLockSupported;
}

export async function enterCursorLock(enterBrowserPointerLock, cursor = null) {
  void cursor;
  const browserLocked = await enterBrowserPointerLock();
  return browserLocked ? CURSOR_LOCK_BROWSER : null;
}

export async function exitCursorLock(mode, exitBrowserPointerLock) {
  const invoke = tauriInvoke();
  if (mode === CURSOR_LOCK_NATIVE && invoke) {
    const grabResult = await invoke("cursor_grab", { grab: false }).then(
      () => null,
      (err) => err,
    );
    const visibleResult = await invoke("cursor_visible", { visible: true }).then(
      () => null,
      (err) => err,
    );
    if (grabResult || visibleResult) throw grabResult || visibleResult;
    return;
  }

  exitBrowserPointerLock();
}
