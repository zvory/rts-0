const CURSOR_LOCK_BROWSER = "browser";

export function installedAppRuntime() {
  const standaloneDisplay = globalThis.matchMedia?.("(display-mode: standalone)")?.matches;
  return !!standaloneDisplay || globalThis.navigator?.standalone === true;
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
  void mode;
  exitBrowserPointerLock();
}
