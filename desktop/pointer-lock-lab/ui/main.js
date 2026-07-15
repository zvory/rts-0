export const RAW_POINTER_LOCK_OPTIONS = Object.freeze({ unadjustedMovement: true });

function finiteDelta(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function signed(value) {
  const number = finiteDelta(value);
  return `${number >= 0 ? "+" : ""}${Math.round(number)}`;
}

export function directionFromDelta(inputX, inputY) {
  const x = finiteDelta(inputX);
  const y = finiteDelta(inputY);
  if (x === 0 && y === 0) return "STILL";

  const horizontal = Math.abs(x) >= Math.abs(y) * 0.55;
  const vertical = Math.abs(y) >= Math.abs(x) * 0.55;
  const horizontalWord = x > 0 ? "RIGHT" : "LEFT";
  const verticalWord = y > 0 ? "DOWN" : "UP";

  if (horizontal && vertical) return `${verticalWord}-${horizontalWord}`;
  return horizontal ? horizontalWord : verticalWord;
}

export function readableError(error) {
  if (!error) return "Unknown error";
  const name = typeof error.name === "string" && error.name ? error.name : "Error";
  const message = typeof error.message === "string" && error.message ? error.message : String(error);
  return message.startsWith(name) ? message : `${name}: ${message}`;
}

export class MovementTelemetry {
  constructor() {
    this.reset();
  }

  reset() {
    this.eventCount = 0;
    this.totalX = 0;
    this.totalY = 0;
    this.lastX = 0;
    this.lastY = 0;
    this.lastDirection = "STILL";
    this.virtualX = 50;
    this.virtualY = 50;
  }

  record(inputX, inputY) {
    const x = finiteDelta(inputX);
    const y = finiteDelta(inputY);
    if (x === 0 && y === 0) return this.snapshot();

    this.lastX = x;
    this.lastY = y;
    this.lastDirection = directionFromDelta(x, y);
    this.eventCount += 1;
    this.totalX += x;
    this.totalY += y;
    this.virtualX = Math.max(3, Math.min(97, this.virtualX + x * 0.12));
    this.virtualY = Math.max(6, Math.min(88, this.virtualY + y * 0.12));
    return this.snapshot();
  }

  snapshot() {
    return {
      eventCount: this.eventCount,
      totalX: this.totalX,
      totalY: this.totalY,
      lastX: this.lastX,
      lastY: this.lastY,
      lastDirection: this.lastDirection,
      virtualX: this.virtualX,
      virtualY: this.virtualY,
    };
  }
}

export async function requestLock(element, mode) {
  if (!element || typeof element.requestPointerLock !== "function") {
    throw new Error("Pointer Lock API is unavailable");
  }
  const result =
    mode === "raw"
      ? element.requestPointerLock(RAW_POINTER_LOCK_OPTIONS)
      : element.requestPointerLock();
  await Promise.resolve(result);
}

export function startLab(doc = document, root = window) {
  const nodes = Object.fromEntries(
    [
      "captureZone",
      "rawLockButton",
      "standardLockButton",
      "exitLockButton",
      "resetButton",
      "lockHeadline",
      "requestMode",
      "movementStatus",
      "lastDelta",
      "lastDirection",
      "eventCount",
      "cumulativeDelta",
      "directionDisplay",
      "virtualCursor",
      "eventLog",
      "environment",
      "liveAnnouncement",
    ].map((id) => [id, doc.getElementById(id)]),
  );

  const telemetry = new MovementTelemetry();
  const startedAt = root.performance?.now?.() ?? Date.now();
  const state = {
    pendingMode: null,
    activeMode: null,
    lastResult: "Ready",
  };

  function elapsed() {
    const now = root.performance?.now?.() ?? Date.now();
    return `${((now - startedAt) / 1000).toFixed(2)}s`;
  }

  function log(message, kind = "info") {
    const item = doc.createElement("li");
    item.dataset.kind = kind;
    item.textContent = `${elapsed()}  ${message}`;
    nodes.eventLog.prepend(item);
    while (nodes.eventLog.children.length > 7) {
      nodes.eventLog.lastElementChild.remove();
    }
  }

  function isLocked() {
    return doc.pointerLockElement === nodes.captureZone;
  }

  function renderLockState() {
    const locked = isLocked();
    nodes.captureZone.dataset.locked = String(locked);
    if (locked) {
      const mode = state.activeMode ?? state.pendingMode ?? "unknown";
      nodes.lockHeadline.dataset.state = "locked";
      nodes.lockHeadline.textContent = `LOCKED — ${mode.toUpperCase()}`;
      nodes.requestMode.textContent = mode.toUpperCase();
      nodes.captureZone.querySelector(".capture-instructions span").textContent =
        "2. LOCKED — MOVE / JUMP / DRAG RIGHT";
      nodes.liveAnnouncement.textContent = `Pointer locked in ${mode} mode`;
      return;
    }

    nodes.captureZone.querySelector(".capture-instructions span").textContent =
      "1. CLICK HERE TO LOCK RAW";
    if (state.pendingMode) {
      nodes.lockHeadline.dataset.state = "requesting";
      nodes.lockHeadline.textContent = `REQUESTING ${state.pendingMode.toUpperCase()}…`;
      nodes.requestMode.textContent = state.pendingMode.toUpperCase();
    } else if (state.lastResult.startsWith("ERROR")) {
      nodes.lockHeadline.dataset.state = "error";
      nodes.lockHeadline.textContent = state.lastResult;
    } else {
      nodes.lockHeadline.dataset.state = "unlocked";
      nodes.lockHeadline.textContent = "UNLOCKED";
    }
  }

  function renderMovement(snapshot = telemetry.snapshot()) {
    const detected = snapshot.eventCount > 0;
    nodes.movementStatus.dataset.state = detected ? "detected" : "waiting";
    nodes.movementStatus.textContent = detected ? "DETECTED" : "WAITING";
    nodes.lastDelta.textContent = `X ${signed(snapshot.lastX)} · Y ${signed(snapshot.lastY)}`;
    nodes.lastDirection.textContent = snapshot.lastDirection;
    nodes.eventCount.textContent = String(snapshot.eventCount);
    nodes.cumulativeDelta.textContent = `X ${signed(snapshot.totalX)} · Y ${signed(snapshot.totalY)}`;
    nodes.directionDisplay.dataset.moving = String(detected);
    nodes.directionDisplay.textContent = detected
      ? `${snapshot.lastDirection}  ·  X ${signed(snapshot.lastX)}  Y ${signed(snapshot.lastY)}`
      : "WAITING FOR MOVEMENT";
    nodes.virtualCursor.style.left = `${snapshot.virtualX}%`;
    nodes.virtualCursor.style.top = `${snapshot.virtualY}%`;
  }

  async function beginLock(mode) {
    if (isLocked()) return;
    state.pendingMode = mode;
    state.lastResult = "Requesting";
    renderLockState();
    log(`requestPointerLock(${mode === "raw" ? "{ unadjustedMovement: true }" : "standard"})`);
    try {
      await requestLock(nodes.captureZone, mode);
      if (!isLocked()) {
        log(`${mode.toUpperCase()} request promise resolved; waiting for pointerlockchange`);
      }
    } catch (error) {
      const message = readableError(error);
      state.lastResult = `ERROR — ${mode.toUpperCase()} REJECTED`;
      state.pendingMode = null;
      renderLockState();
      log(`${mode.toUpperCase()} rejected: ${message}`, "error");
      nodes.liveAnnouncement.textContent = `${mode} pointer lock failed: ${message}`;
    }
  }

  function resetTelemetry() {
    telemetry.reset();
    renderMovement();
    log("Telemetry reset");
  }

  nodes.captureZone.addEventListener("click", () => beginLock("raw"));
  nodes.captureZone.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      beginLock("raw");
    }
  });
  nodes.rawLockButton.addEventListener("click", () => beginLock("raw"));
  nodes.standardLockButton.addEventListener("click", () => beginLock("standard"));
  nodes.exitLockButton.addEventListener("click", () => doc.exitPointerLock?.());
  nodes.resetButton.addEventListener("click", resetTelemetry);

  doc.addEventListener("pointerlockchange", () => {
    if (isLocked()) {
      state.activeMode = state.pendingMode ?? state.activeMode ?? "unknown";
      state.pendingMode = null;
      state.lastResult = "Locked";
      log(`LOCK ACQUIRED — ${state.activeMode.toUpperCase()}`, "success");
    } else {
      const previousMode = state.activeMode;
      state.activeMode = null;
      state.pendingMode = null;
      state.lastResult = "Unlocked";
      log(previousMode ? `LOCK RELEASED — ${previousMode.toUpperCase()}` : "Pointer lock is clear");
      nodes.liveAnnouncement.textContent = "Pointer lock released";
    }
    renderLockState();
  });

  doc.addEventListener("pointerlockerror", () => {
    const mode = state.pendingMode ?? "unknown";
    state.lastResult = `ERROR — ${mode.toUpperCase()} FAILED`;
    state.pendingMode = null;
    renderLockState();
    log(`pointerlockerror while requesting ${mode.toUpperCase()}`, "error");
  });

  doc.addEventListener("mousemove", (event) => {
    if (!isLocked()) return;
    if (event.movementX === 0 && event.movementY === 0) return;
    const snapshot = telemetry.record(event.movementX, event.movementY);
    renderMovement(snapshot);
    if (snapshot.eventCount <= 3 || snapshot.eventCount % 25 === 0) {
      log(`MOVE #${snapshot.eventCount}: X ${signed(snapshot.lastX)} Y ${signed(snapshot.lastY)}`,
        "success");
    }
  });

  doc.addEventListener("keydown", (event) => {
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    if (event.key.toLowerCase() === "r") beginLock("raw");
    if (event.key.toLowerCase() === "s") beginLock("standard");
    if (event.key.toLowerCase() === "c") resetTelemetry();
  });

  root.addEventListener("blur", () => log("Window blur"));
  root.addEventListener("focus", () => log("Window focus"));

  const apiAvailable = typeof nodes.captureZone.requestPointerLock === "function";
  nodes.environment.textContent = [
    `Pointer Lock API: ${apiAvailable ? "AVAILABLE" : "MISSING"}`,
    `secureContext: ${root.isSecureContext ? "YES" : "NO"}`,
    `protocol: ${root.location?.protocol ?? "unknown"}`,
  ].join("  ·  ");

  root.__POINTER_LOCK_LAB__ = Object.freeze({
    getState: () => ({
      locked: isLocked(),
      activeMode: state.activeMode,
      pendingMode: state.pendingMode,
      movement: telemetry.snapshot(),
    }),
  });

  renderLockState();
  renderMovement();
  log(apiAvailable ? "Lab ready; Pointer Lock API available" : "Pointer Lock API missing", apiAvailable ? "success" : "error");
}

if (typeof document !== "undefined" && typeof window !== "undefined") {
  startLab(document, window);
}
