const CURSOR_LOCK_BROWSER = "browser";

export function desktopRuntime() {
  return (
    !!globalThis.__TAURI_INTERNALS__ ||
    !!globalThis.__TAURI__?.core ||
    installedWebAppRuntime()
  );
}

export function installedWebAppRuntime() {
  const standaloneDisplay = globalThis.matchMedia?.("(display-mode: standalone)")?.matches;
  const fullscreenDisplay = globalThis.matchMedia?.("(display-mode: fullscreen)")?.matches;
  return !!standaloneDisplay || !!fullscreenDisplay || globalThis.navigator?.standalone === true;
}

export function cursorLockSupported(browserPointerLockSupported) {
  return browserPointerLockSupported;
}

export function shouldRequestPointerLock({ desktopRuntime: isDesktop, requireGesture }) {
  return !isDesktop || !!requireGesture;
}

export async function enterCursorLock(enterBrowserPointerLock, cursor = null) {
  void cursor;
  const browserLocked = await enterBrowserPointerLock();
  return browserLocked ? CURSOR_LOCK_BROWSER : null;
}

export async function exitCursorLock(mode, exitBrowserPointerLock) {
  void mode;
  exitBrowserPointerLock();
}
