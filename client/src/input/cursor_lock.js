const CURSOR_LOCK_BROWSER = "browser";

export function installedAppRuntime() {
  const standaloneDisplay = globalThis.matchMedia?.("(display-mode: standalone)")?.matches;
  const fullscreenDisplay = globalThis.matchMedia?.("(display-mode: fullscreen)")?.matches;
  return !!standaloneDisplay || !!fullscreenDisplay || globalThis.navigator?.standalone === true;
}

export function cursorLockSupported(browserPointerLockSupported) {
  return browserPointerLockSupported;
}

export function shouldRequestPointerLock({ installedAppRuntime: isInstalledApp, requireGesture }) {
  return !isInstalledApp || !!requireGesture;
}

export function automaticPointerLockDisabledForTests() {
  try {
    return new URLSearchParams(globalThis.location?.search || "").has("rtsNoAutoPointerLock");
  } catch {
    return false;
  }
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
