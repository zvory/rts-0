const CURSOR_LOCK_NATIVE = "native";
const CURSOR_LOCK_BROWSER = "browser";

function isTauriRuntime() {
  return !!window.__TAURI_INTERNALS__;
}

function tauriInvoke() {
  const invoke = window.__TAURI__?.core?.invoke;
  return typeof invoke === "function" ? invoke : null;
}

export function nativeCursorSupported() {
  return isTauriRuntime() && !!tauriInvoke();
}

export function cursorLockSupported(browserPointerLockSupported) {
  return nativeCursorSupported() || browserPointerLockSupported;
}

export async function enterCursorLock(enterBrowserPointerLock) {
  const invoke = tauriInvoke();
  if (isTauriRuntime() && invoke) {
    await invoke("cursor_grab", { grab: true });
    try {
      await invoke("cursor_visible", { visible: false });
    } catch (err) {
      await invoke("cursor_grab", { grab: false }).catch(() => {});
      throw err;
    }
    return CURSOR_LOCK_NATIVE;
  }

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
