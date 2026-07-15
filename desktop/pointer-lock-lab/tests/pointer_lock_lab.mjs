import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  MovementTelemetry,
  RAW_POINTER_LOCK_OPTIONS,
  directionFromDelta,
  readableError,
  requestLock,
} from "../ui/main.js";

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
