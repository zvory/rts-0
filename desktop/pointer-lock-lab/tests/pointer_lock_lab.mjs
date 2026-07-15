import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  MovementTelemetry,
  RAW_POINTER_LOCK_OPTIONS,
  directionFromDelta,
  readableError,
  requestLock,
  startLab,
} from "../ui/main.js";

class FakeNode extends EventTarget {
  constructor() {
    super();
    this.children = [];
    this.dataset = {};
    this.disabled = false;
    this.style = {};
    this.textContent = "";
  }

  prepend(child) {
    this.children.unshift(child);
    child.remove = () => {
      this.children = this.children.filter((candidate) => candidate !== child);
    };
  }

  get lastElementChild() {
    return this.children.at(-1) ?? null;
  }

  click() {
    if (!this.disabled) this.dispatchEvent(new Event("click"));
  }
}

function createFakeLab() {
  const ids = [
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
  ];
  const nodes = Object.fromEntries(ids.map((id) => [id, new FakeNode()]));
  const instruction = new FakeNode();
  nodes.captureZone.querySelector = (selector) =>
    selector === ".capture-instructions span" ? instruction : null;

  const requests = [];
  nodes.captureZone.requestPointerLock = (options) => {
    requests.push(options);
    return Promise.resolve();
  };

  const doc = new EventTarget();
  doc.pointerLockElement = null;
  doc.createElement = () => new FakeNode();
  doc.getElementById = (id) => nodes[id] ?? null;
  doc.exitPointerLock = () => {
    doc.pointerLockElement = null;
    doc.dispatchEvent(new Event("pointerlockchange"));
  };

  const root = new EventTarget();
  root.isSecureContext = true;
  root.location = { protocol: "tauri:" };
  root.performance = { now: () => 0 };

  return { doc, nodes, requests, root };
}

test("raw mode requests unadjusted movement without fallback", async () => {
  const calls = [];
  const element = {
    requestPointerLock(options) {
      calls.push(options);
      return Promise.resolve();
    },
  };

  await requestLock(element, "raw");
  await requestLock(element, "standard");

  assert.deepEqual(RAW_POINTER_LOCK_OPTIONS, { unadjustedMovement: true });
  assert.deepEqual(calls, [{ unadjustedMovement: true }, undefined]);
});

test("movement telemetry exposes directions and accumulated deltas", () => {
  const telemetry = new MovementTelemetry();
  assert.equal(telemetry.record(40, 3).lastDirection, "RIGHT");
  assert.equal(telemetry.record(0, 0).lastDirection, "RIGHT");
  assert.equal(telemetry.record(-8, 30).lastDirection, "DOWN");
  const snapshot = telemetry.snapshot();
  assert.equal(snapshot.eventCount, 2);
  assert.equal(snapshot.totalX, 32);
  assert.equal(snapshot.totalY, 33);

  telemetry.reset();
  assert.equal(telemetry.snapshot().eventCount, 0);
});

test("direction labels are stable for agent-readable assertions", () => {
  assert.equal(directionFromDelta(0, 0), "STILL");
  assert.equal(directionFromDelta(-20, 0), "LEFT");
  assert.equal(directionFromDelta(0, -20), "UP");
  assert.equal(directionFromDelta(20, 20), "DOWN-RIGHT");
  assert.equal(directionFromDelta(Number.NaN, 12), "DOWN");
});

test("errors preserve browser exception names", () => {
  assert.equal(
    readableError({ name: "NotSupportedError", message: "Raw input unavailable" }),
    "NotSupportedError: Raw input unavailable",
  );
});

test("a pending request cannot be relabeled by another lock request", () => {
  const { doc, nodes, requests, root } = createFakeLab();
  startLab(doc, root);

  assert.equal(nodes.exitLockButton.disabled, true);
  nodes.rawLockButton.click();
  assert.equal(root.__POINTER_LOCK_LAB__.getState().pendingMode, "raw");
  assert.equal(nodes.rawLockButton.disabled, true);
  assert.equal(nodes.standardLockButton.disabled, true);
  assert.deepEqual(requests, [{ unadjustedMovement: true }]);

  const standardShortcut = new Event("keydown");
  Object.defineProperties(standardShortcut, {
    altKey: { value: false },
    ctrlKey: { value: false },
    key: { value: "s" },
    metaKey: { value: false },
  });
  doc.dispatchEvent(standardShortcut);
  assert.deepEqual(requests, [{ unadjustedMovement: true }]);

  doc.pointerLockElement = nodes.captureZone;
  doc.dispatchEvent(new Event("pointerlockchange"));
  assert.equal(root.__POINTER_LOCK_LAB__.getState().activeMode, "raw");
  assert.equal(nodes.exitLockButton.disabled, false);
});

test("the static UI is isolated and has explicit agent controls", async () => {
  const [html, script, tauriConfig] = await Promise.all([
    readFile(new URL("../ui/index.html", import.meta.url), "utf8"),
    readFile(new URL("../ui/main.js", import.meta.url), "utf8"),
    readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
  ]);

  assert.match(html, /CLICK HERE TO LOCK RAW/);
  assert.match(html, /Lock standard \(control\)/);
  assert.match(script, /unadjustedMovement: true/);
  const parsedConfig = JSON.parse(tauriConfig);
  delete parsedConfig.$schema;
  const runtimeSurface = `${html}\n${script}\n${JSON.stringify(parsedConfig)}`;
  assert.doesNotMatch(runtimeSurface, /https?:\/\//i);
  assert.doesNotMatch(runtimeSurface, /bewegungskrieg|rtsLaunch|client\/src/i);
});
