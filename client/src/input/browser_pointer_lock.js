import { nativeCursorDebugSnapshot } from "./cursor_lock.js";

const POINTER_LOCK_RESULT_TIMEOUT_MS = 700;
const POINTER_LOCK_RAW_INPUT_OPTIONS = Object.freeze({ unadjustedMovement: true });

export function _browserPointerLockSupported() {
  return this._browserRequestPointerLock() !== null && this._browserExitPointerLockFn() !== null;
}

export function _pointerLockTarget() {
  const view = this.renderElement;
  return view && typeof view.requestPointerLock === "function" ? view : this.dom;
}

export function _browserRequestPointerLock() {
  const target = this._pointerLockTarget();
  const fn = target.requestPointerLock || target.webkitRequestPointerLock;
  return typeof fn === "function" ? fn.bind(target) : null;
}

export function _browserExitPointerLockFn() {
  const fn = document.exitPointerLock || document.webkitExitPointerLock;
  return typeof fn === "function" ? fn.bind(document) : null;
}

export function _browserPointerLockElement() {
  return document.pointerLockElement || document.webkitPointerLockElement || null;
}

export function _elementDebugSummary(el) {
  if (!el) return null;
  return {
    tag: el.tagName,
    id: el.id || null,
    className: el.className || null,
    requestPointerLock: typeof el.requestPointerLock,
    webkitRequestPointerLock: typeof el.webkitRequestPointerLock,
  };
}

export function pointerLockDebugSnapshot() {
  const target = this._pointerLockTarget();
  const tauriGlobals = Object.keys(globalThis)
    .filter((key) => key.includes("TAURI"))
    .sort();
  return {
    installedAppRuntime: this.installedAppRuntime(),
    pointerLocked: this.pointerLocked,
    pointerLockElementMatches: this._browserPointerLockElement() === target,
    pointerLockElementIsViewport: this._browserPointerLockElement() === this.dom,
    pointerLockElementIsTarget: this._browserPointerLockElement() === target,
    viewport: this._elementDebugSummary(this.dom),
    lockTarget: this._elementDebugSummary(target),
    requestPointerLock: typeof target.requestPointerLock,
    webkitRequestPointerLock: typeof target.webkitRequestPointerLock,
    exitPointerLock: typeof document.exitPointerLock,
    webkitExitPointerLock: typeof document.webkitExitPointerLock,
    nativeCursor: nativeCursorDebugSnapshot(this.desktopCursor),
    nativeCursorBridgePresent: !!globalThis.__RTS_NATIVE_CURSOR,
    desktopRuntime: globalThis.__RTS_DESKTOP_RUNTIME || null,
    tauriGlobals,
    hasPointerLockElement: "pointerLockElement" in document,
    hasWebkitPointerLockElement: "webkitPointerLockElement" in document,
    documentHasFocus: typeof document.hasFocus === "function" ? document.hasFocus() : null,
    activeElement: document.activeElement
      ? {
          tag: document.activeElement.tagName,
          id: document.activeElement.id || null,
          className: document.activeElement.className || null,
        }
      : null,
    attempts: this._pointerLockAttempt,
    lastFocusAttempt: this._lastPointerLockFocusAttempt,
    lastRequest: this._lastPointerLockRequest,
    location: globalThis.location?.href || null,
    userAgent: navigator.userAgent,
  };
}

export function _focusPointerLockTarget() {
  const before = this._focusDebugState();
  const target = this._pointerLockTarget();
  if (typeof target.hasAttribute === "function" && !target.hasAttribute("tabindex")) target.tabIndex = -1;
  if (typeof globalThis.window?.focus === "function") {
    try {
      globalThis.window.focus();
    } catch {
      // Some embedded webviews expose focus but reject it; the element focus below is still useful.
    }
  }
  if (typeof target.focus !== "function") {
    this._lastPointerLockFocusAttempt = { before, after: this._focusDebugState(), elementFocusCalled: false };
    return;
  }
  const elementFocusCalled = true;
  try {
    target.focus({ preventScroll: true });
  } catch {
    target.focus();
  }
  this._lastPointerLockFocusAttempt = { before, after: this._focusDebugState(), elementFocusCalled };
}

export function _focusDebugState() {
  const doc = globalThis.document;
  return {
    documentHasFocus: typeof doc?.hasFocus === "function" ? doc.hasFocus() : null,
    activeElement: doc?.activeElement
      ? {
          tag: doc.activeElement.tagName,
          id: doc.activeElement.id || null,
          className: doc.activeElement.className || null,
        }
      : null,
  };
}

export async function _requestBrowserPointerLock() {
  if (!this._browserPointerLockSupported()) {
    if (this.onPointerLockError) this.onPointerLockError(new Error("Pointer Lock API is unavailable."));
    return false;
  }
  try {
    const requestPointerLock = this._browserRequestPointerLock();
    if (!requestPointerLock) {
      if (this.onPointerLockError) this.onPointerLockError(new Error("Pointer Lock API is unavailable."));
      return false;
    }
    const rawLocked = await this._requestBrowserPointerLockWithOptions(
      requestPointerLock,
      POINTER_LOCK_RAW_INPUT_OPTIONS,
      true,
    );
    return rawLocked || this._browserPointerLockElement() === this._pointerLockTarget();
  } catch (err) {
    this._finishPointerLockRequest("exception", err);
    if (this.onPointerLockError) this.onPointerLockError(err);
    return false;
  }
}

export async function _requestBrowserPointerLockWithOptions(requestPointerLock, options, rawInputRequested) {
  let result;
  try {
    result = options === undefined ? requestPointerLock() : requestPointerLock(options);
  } catch (err) {
    this._lastPointerLockRequest = {
      attempt: this._pointerLockAttempt,
      at: new Date().toISOString(),
      rawInputRequested,
      returnedPromise: false,
      before: this._focusDebugState(),
      outcome: "pending",
    };
    this._finishPointerLockRequest("exception", err);
    return false;
  }
  this._lastPointerLockRequest = {
    attempt: this._pointerLockAttempt,
    at: new Date().toISOString(),
    rawInputRequested,
    returnedPromise: !!(result && typeof result.then === "function"),
    before: this._focusDebugState(),
    outcome: "pending",
  };
  if (result && typeof result.then === "function") {
    return await this._waitForPointerLockPromise(result);
  }
  return await this._waitForBrowserPointerLockResult();
}

export function _waitForPointerLockPromise(pointerLockPromise) {
  return new Promise((resolve) => {
    let done = false;
    const finish = (outcome, locked, err = null) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      this._finishPointerLockRequest(outcome, err);
      resolve(locked);
    };
    const timer = window.setTimeout(() => {
      finish("timeout", this._browserPointerLockElement() === this._pointerLockTarget(), null);
    }, POINTER_LOCK_RESULT_TIMEOUT_MS);
    pointerLockPromise.then(
      () => finish("resolved", this._browserPointerLockElement() === this._pointerLockTarget(), null),
      (err) => finish("rejected", false, err),
    );
  });
}

export function _finishPointerLockRequest(outcome, err = null) {
  if (!this._lastPointerLockRequest) return;
  this._lastPointerLockRequest = {
    ...this._lastPointerLockRequest,
    outcome,
    after: this._focusDebugState(),
    pointerLockElementMatches: this._browserPointerLockElement() === this._pointerLockTarget(),
    error: err ? this._pointerLockErrorSummary(err) : null,
  };
}

export function _pointerLockErrorSummary(err) {
  if (err instanceof Error) return { name: err.name, message: err.message };
  if (err && typeof err === "object") {
    return {
      type: err.type || null,
      name: err.name || null,
      message: err.message || null,
    };
  }
  return err == null ? null : { message: String(err) };
}

export function _waitForBrowserPointerLockResult() {
  if (this._browserPointerLockElement() === this._pointerLockTarget()) return Promise.resolve(true);
  return new Promise((resolve) => {
    let done = false;
    const finish = (locked) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      document.removeEventListener("pointerlockchange", onChange);
      document.removeEventListener("pointerlockerror", onError);
      document.removeEventListener("webkitpointerlockchange", onChange);
      document.removeEventListener("webkitpointerlockerror", onError);
      resolve(locked);
    };
    const onChange = () => finish(this._browserPointerLockElement() === this._pointerLockTarget());
    const onError = () => finish(false);
    const timer = window.setTimeout(() => finish(this._browserPointerLockElement() === this._pointerLockTarget()), 350);
    document.addEventListener("pointerlockchange", onChange);
    document.addEventListener("pointerlockerror", onError);
    document.addEventListener("webkitpointerlockchange", onChange);
    document.addEventListener("webkitpointerlockerror", onError);
  });
}

export function _exitBrowserPointerLock() {
  if (this._browserPointerLockElement() === this._pointerLockTarget()) {
    const exitPointerLock = this._browserExitPointerLockFn();
    if (exitPointerLock) exitPointerLock();
  }
}

export function _handlePointerLockChange() {
  const locked = this._browserPointerLockElement() === this._pointerLockTarget();
  this._setCursorLockState(locked, locked ? "browser" : null);
}

export function _handlePointerLockError(ev) {
  if (this.onPointerLockError) this.onPointerLockError(ev);
}
