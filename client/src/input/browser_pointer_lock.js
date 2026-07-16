import { nativeCursorDebugSnapshot } from "./cursor_lock.js";
import { pointerLockTraceSnapshot } from "./pointer_lock_diagnostics.js";

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
    trace: pointerLockTraceSnapshot(this),
    location: globalThis.location?.href || null,
    userAgent: navigator.userAgent,
  };
}

export function _focusPointerLockTarget() {
  const before = this._focusDebugState();
  const target = this._pointerLockTarget();
  const errors = [];
  let windowFocusCalled = false;
  if (typeof target.hasAttribute === "function" && !target.hasAttribute("tabindex")) target.tabIndex = -1;
  if (typeof globalThis.window?.focus === "function") {
    windowFocusCalled = true;
    try {
      globalThis.window.focus();
    } catch (err) {
      errors.push({ source: "window.focus", error: this._pointerLockErrorSummary(err) });
      // Some embedded webviews expose focus but reject it; the element focus below is still useful.
    }
  }
  if (typeof target.focus !== "function") {
    this._lastPointerLockFocusAttempt = {
      before,
      after: this._focusDebugState(),
      windowFocusCalled,
      elementFocusCalled: false,
      errors,
    };
    this._recordPointerLockTrace("focus", this._lastPointerLockFocusAttempt);
    return;
  }
  const elementFocusCalled = true;
  try {
    target.focus({ preventScroll: true });
  } catch (err) {
    errors.push({ source: "target.focus-options", error: this._pointerLockErrorSummary(err) });
    try {
      target.focus();
    } catch (fallbackErr) {
      errors.push({ source: "target.focus", error: this._pointerLockErrorSummary(fallbackErr) });
    }
  }
  this._lastPointerLockFocusAttempt = {
    before,
    after: this._focusDebugState(),
    windowFocusCalled,
    elementFocusCalled,
    errors,
  };
  this._recordPointerLockTrace("focus", this._lastPointerLockFocusAttempt);
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
  const supported = this._browserPointerLockSupported();
  const target = this._pointerLockTarget();
  this._recordPointerLockTrace("browser-request-start", {
    supported,
    target: this._elementDebugSummary(target),
    pointerLockElement: this._elementDebugSummary(this._browserPointerLockElement()),
    pointerLockElementMatches: this._browserPointerLockElement() === target,
    focus: this._focusDebugState(),
  });
  if (!supported) {
    this._reportPointerLockFailure(new Error("Pointer Lock API is unavailable."));
    return false;
  }
  try {
    const requestPointerLock = this._browserRequestPointerLock();
    if (!requestPointerLock) {
      this._reportPointerLockFailure(new Error("Pointer Lock API is unavailable."));
      return false;
    }
    const rawLocked = await this._requestBrowserPointerLockWithOptions(
      requestPointerLock,
      POINTER_LOCK_RAW_INPUT_OPTIONS,
      true,
    );
    const locked = rawLocked || this._browserPointerLockElement() === this._pointerLockTarget();
    this._recordPointerLockTrace("browser-request-complete", {
      locked,
      helperResult: rawLocked,
      outcome: this._lastPointerLockRequest?.outcome || null,
      pointerLockElementMatches: this._browserPointerLockElement() === this._pointerLockTarget(),
    });
    if (!locked) this._reportPointerLockFailure(pointerLockFailureFromRequest(this._lastPointerLockRequest));
    return locked;
  } catch (err) {
    this._finishPointerLockRequest("exception", err);
    this._reportPointerLockFailure(err);
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
  this._recordPointerLockTrace("browser-request-invoked", {
    rawInputRequested,
    returnedPromise: this._lastPointerLockRequest.returnedPromise,
    focus: this._lastPointerLockRequest.before,
  });
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
    let timer = window.setTimeout(() => {
      finish("timeout", this._browserPointerLockElement() === this._pointerLockTarget(), null);
    }, POINTER_LOCK_RESULT_TIMEOUT_MS);
    pointerLockPromise.then(
      async () => {
        clearTimeout(timer);
        timer = null;
        if (this._browserPointerLockElement() === this._pointerLockTarget()) {
          finish("resolved", true, null);
          return;
        }
        this._recordPointerLockTrace("browser-promise-resolved-awaiting-event", {
          focus: this._focusDebugState(),
          pointerLockElement: this._elementDebugSummary(this._browserPointerLockElement()),
        });
        const result = await waitForBrowserPointerLockEvent(this);
        finish(`resolved-${result.outcome}`, result.locked, result.error);
      },
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
  this._recordPointerLockTrace("browser-request-finish", {
    outcome,
    rawInputRequested: this._lastPointerLockRequest.rawInputRequested,
    returnedPromise: this._lastPointerLockRequest.returnedPromise,
    pointerLockElementMatches: this._lastPointerLockRequest.pointerLockElementMatches,
    focus: this._lastPointerLockRequest.after,
    error: this._lastPointerLockRequest.error,
  });
}

export function _reportPointerLockFailure(err) {
  const attempt = Number.isFinite(this._pointerLockAttempt) ? this._pointerLockAttempt : 0;
  if (this._lastPointerLockFailureAttempt === attempt) return false;
  this._lastPointerLockFailureAttempt = attempt;
  const error = normalizePointerLockError(err);
  this._recordPointerLockTrace("failure", {
    error: this._pointerLockErrorSummary(error),
    lastRequest: this._lastPointerLockRequest,
  });
  if (this.onPointerLockError) this.onPointerLockError(error);
  return true;
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

export async function _waitForBrowserPointerLockResult() {
  const result = await waitForBrowserPointerLockEvent(this);
  this._finishPointerLockRequest(result.outcome, result.error);
  return result.locked;
}

function waitForBrowserPointerLockEvent(input) {
  if (input._browserPointerLockElement() === input._pointerLockTarget()) {
    return Promise.resolve({ outcome: "already-locked", locked: true, error: null });
  }
  input._recordPointerLockTrace("browser-event-wait-start", {
    timeoutMs: 350,
    focus: input._focusDebugState(),
  });
  return new Promise((resolve) => {
    let done = false;
    const finish = (outcome, locked, err = null) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      document.removeEventListener("pointerlockchange", onChange);
      document.removeEventListener("pointerlockerror", onError);
      document.removeEventListener("webkitpointerlockchange", onChange);
      document.removeEventListener("webkitpointerlockerror", onError);
      resolve({ outcome, locked, error: err });
    };
    const onChange = (ev) => finish(
      ev?.type || "pointerlockchange",
      input._browserPointerLockElement() === input._pointerLockTarget(),
    );
    const onError = (ev) => finish(ev?.type || "pointerlockerror", false, ev);
    const timer = window.setTimeout(() => finish(
      "event-timeout",
      input._browserPointerLockElement() === input._pointerLockTarget(),
    ), 350);
    document.addEventListener("pointerlockchange", onChange);
    document.addEventListener("pointerlockerror", onError);
    document.addEventListener("webkitpointerlockchange", onChange);
    document.addEventListener("webkitpointerlockerror", onError);
  });
}

export function _exitBrowserPointerLock() {
  const locked = this._browserPointerLockElement() === this._pointerLockTarget();
  const exitPointerLock = this._browserExitPointerLockFn();
  this._recordPointerLockTrace("browser-exit", {
    locked,
    exitFunctionAvailable: typeof exitPointerLock === "function",
  });
  if (locked && exitPointerLock) exitPointerLock();
}

export function _handlePointerLockChange(ev) {
  const locked = this._browserPointerLockElement() === this._pointerLockTarget();
  this._recordPointerLockTrace("browser-event-change", {
    eventType: ev?.type || "pointerlockchange",
    locked,
    pointerLockElement: this._elementDebugSummary(this._browserPointerLockElement()),
    focus: this._focusDebugState(),
  });
  this._setCursorLockState(locked, locked ? "browser" : null);
}

export function _handlePointerLockError(ev) {
  this._recordPointerLockTrace("browser-event-error", {
    eventType: ev?.type || "pointerlockerror",
    error: this._pointerLockErrorSummary(ev),
    focus: this._focusDebugState(),
  });
  this._reportPointerLockFailure(ev);
}

function pointerLockFailureFromRequest(request) {
  const outcome = request?.outcome || "finished";
  const message = request?.error?.message || `Pointer Lock request ${outcome} without locking the target.`;
  const error = new Error(message);
  if (request?.error?.name) error.name = request.error.name;
  return error;
}

function normalizePointerLockError(err) {
  if (err instanceof Error) return err;
  const summary = err && typeof err === "object"
    ? {
        name: err.name || null,
        message: err.message || null,
        type: err.type || null,
      }
    : null;
  const error = new Error(
    summary?.message || (summary?.type ? `Pointer Lock emitted ${summary.type}.` : "Pointer Lock failed."),
  );
  if (summary?.name) error.name = summary.name;
  return error;
}
