import {
  cursorLockSupported,
  enterCursorLock,
  exitCursorLock,
  installedAppRuntime as detectInstalledAppRuntime,
} from "./cursor_lock.js";

export function pointerLockSupported() {
  return cursorLockSupported(this._browserPointerLockSupported(), this.desktopCursor);
}

export function installedAppRuntime() {
  return detectInstalledAppRuntime();
}

export function _prepareCursorLock() {
  this._focusPointerLockTarget();
  const p = this.mouse || this._viewportCenter();
  this.mouse = this._clampViewportPoint(p);
  this._setPointerLockCursor(this.mouse);
}

export function requestPointerLock() {
  if (this.pointerLocked) {
    this._recordPointerLockTrace("attempt-skipped", { reason: "already-locked", mode: this._cursorLockMode });
    return Promise.resolve(true);
  }
  if (this._pointerLockRequestInFlight) {
    this._recordPointerLockTrace("attempt-skipped", { reason: "request-pending" });
    return this._pointerLockRequestInFlight;
  }
  this._pointerLockAttempt += 1;
  this._lastPointerLockFailureAttempt = null;
  const browserSupported = this._browserPointerLockSupported();
  const supported = cursorLockSupported(browserSupported, this.desktopCursor);
  this._recordPointerLockTrace("attempt-start", {
    supported,
    browserSupported,
    nativeBridgePresent: !!this.desktopCursor,
    runtime: globalThis.__RTS_DESKTOP_RUNTIME || null,
    focus: this._focusDebugState(),
  });
  if (!supported) {
    this._reportPointerLockFailure(new Error("Pointer Lock API is unavailable."));
    return Promise.resolve(false);
  }
  this._prepareCursorLock();
  this._recordPointerLockTrace("attempt-prepared", {
    cursor: this.mouse,
    bounds: this._nativeCursorBounds(),
    focus: this._focusDebugState(),
  });
  const request = enterCursorLock(
    () => this._requestBrowserPointerLock(),
    this.mouse,
    this.desktopCursor,
    this._nativeCursorBounds(),
  ).then((mode) => {
    this._recordPointerLockTrace("attempt-result", {
      mode: mode || null,
      pointerLocked: this.pointerLocked,
      browserElementMatches: this._browserPointerLockElement() === this._pointerLockTarget(),
    });
    if (!mode) this._reportPointerLockFailure(new Error("Pointer Lock request finished without locking the viewport."));
    if (mode && mode !== "browser") this._setCursorLockState(true, mode);
    return !!mode;
  }).catch((err) => {
    this._recordPointerLockTrace("attempt-exception", { error: this._pointerLockErrorSummary(err) });
    this._reportPointerLockFailure(err);
    return false;
  });
  const trackedRequest = request.finally(() => {
    if (this._pointerLockRequestInFlight === trackedRequest) this._pointerLockRequestInFlight = null;
    this._recordPointerLockTrace("attempt-settled", { pointerLocked: this.pointerLocked });
  });
  this._pointerLockRequestInFlight = trackedRequest;
  return trackedRequest;
}

export function exitPointerLock() {
  const mode = this._cursorLockMode;
  this._recordPointerLockTrace("exit-start", { mode, pointerLocked: this.pointerLocked });
  return exitCursorLock(mode, () => this._exitBrowserPointerLock(), this.desktopCursor, "input-exit").then(() => {
    if (mode && mode !== "browser") this._setCursorLockState(false, null);
    this._recordPointerLockTrace("exit-complete", { mode, pointerLocked: this.pointerLocked });
    return true;
  }).catch((err) => {
    this._recordPointerLockTrace("exit-failure", { mode, error: this._pointerLockErrorSummary(err) });
    if (this.onPointerLockError) this.onPointerLockError(err);
    return false;
  });
}

export function togglePointerLock() {
  return this.pointerLocked ? (this.exitPointerLock(), Promise.resolve(false)) : this.requestPointerLock();
}

export function _setCursorLockState(locked, mode) {
  const prior = { locked: this.pointerLocked, mode: this._cursorLockMode };
  if (!locked && this.pointerLocked) this.inputRouter?.releaseSource?.("locked");
  this.pointerLocked = locked;
  this._cursorLockMode = locked ? mode : null;
  this.dom.classList.toggle("pointer-locked", locked);
  if (this._pointerLockCursor) this._pointerLockCursor.hidden = !locked;
  if (locked) {
    this.mouse = this._clampViewportPoint(this.mouse || this._viewportCenter());
    this._setPointerLockCursor(this.mouse);
  } else {
    this.mouse = null;
    this._nativeButtonsMask = 0;
    this._panDrag = null;
    if (this._drag) {
      this._drag = null;
      this._dragging = false;
      this.screenOverlay?.clearMarquee?.();
    }
    this._placementDrag = null;
  }
  this._recordPointerLockTrace("state-change", {
    prior,
    next: { locked: this.pointerLocked, mode: this._cursorLockMode },
    focus: this._focusDebugState(),
  });
  if (this.onPointerLockChange) this.onPointerLockChange(locked);
}
