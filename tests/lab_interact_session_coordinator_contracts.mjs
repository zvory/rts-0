import assert from "node:assert/strict";

import { LabInteractService } from "../scripts/lab-interact/command_service.ts";

const calls = [];
const timeDefers = [];
let captureDeferred = null;
let fixedCaptureActive = false;
let recordWaitDeferred = null;
let statusDeferred = null;
let driverClosed = false;

const driver = {
  workspace: { root: process.cwd(), branch: "fixture", head: "a".repeat(40) },
  async status() {
    calls.push("status");
    if (statusDeferred) return statusDeferred.promise;
    return { ready: true, snapshotTick: 1, roomTime: { paused: true, speed: 0 } };
  },
  async catalog() {
    calls.push("catalog");
    return { players: [], factions: [], supportedCommandKinds: [], abilities: [] };
  },
  time(control) {
    calls.push(`time:${control.action}`);
    const pending = timeDefers.shift();
    return pending ? pending.promise : Promise.resolve({ snapshotTick: 2 });
  },
  camera() {
    calls.push("camera");
    return Promise.resolve({ camera: { version: 1 } });
  },
  recordWait() {
    calls.push("record-wait");
    return recordWaitDeferred.promise;
  },
  recordingStatus() {
    return { active: false };
  },
  recordAcceptedOperation() {},
  captureFixed() {
    calls.push("capture-fixed");
    fixedCaptureActive = true;
    return captureDeferred.promise.finally(() => { fixedCaptureActive = false; });
  },
  fixedCaptureStatus() {
    return { active: fixedCaptureActive };
  },
  cancelFixedCapture() {
    calls.push("capture-cancel");
    assert.equal(fixedCaptureActive, true, "cancellation reaches an active capture");
    return { cancelling: true };
  },
  settleRecording() {
    return null;
  },
  async close() {
    calls.push("driver-close");
    driverClosed = true;
  },
};

const service = new LabInteractService({
  workspaceRoot: process.cwd(),
  driverFactory: async () => driver,
});

try {
  const opened = await service.execute("open", {});
  const { sessionId } = opened;

  const firstTime = deferred();
  timeDefers.push(firstTime);
  const first = service.execute("time", { sessionId, control: { action: "step", ticks: 1 } });
  const second = service.execute("camera", { sessionId, camera: { action: "set", snapshot: {
    version: 1, focus: { x: 10, y: 10 }, framingScale: 1, boundsPolicy: "mapOverscroll",
  } } });
  await nextTurn();
  assert.equal(calls.filter((entry) => entry === "camera").length, 0, "serialized commands remain FIFO ordered");
  firstTime.resolve({ snapshotTick: 2 });
  await Promise.all([first, second]);
  assert.deepEqual(calls.slice(-2), ["time:step", "camera"], "the next serialized command starts after the first settles");

  const blockedTime = deferred();
  timeDefers.push(blockedTime);
  const blocked = service.execute("time", { sessionId, control: { action: "step", ticks: 1 } });
  await nextTurn();
  const observed = await service.execute("status", { sessionId });
  assert.equal(observed.status.ready, true, "observation work bypasses the semantic FIFO");
  blockedTime.resolve({ snapshotTick: 3 });
  await blocked;

  recordWaitDeferred = deferred();
  const waiting = service.execute("record-wait", { sessionId });
  await nextTurn();
  const whileWaiting = await service.execute("time", { sessionId, control: { action: "step", ticks: 1 } });
  assert.equal(whileWaiting.result.snapshotTick, 2, "record-wait does not block serialized session work");
  recordWaitDeferred.resolve({ active: false, stoppedBy: "watchdog" });
  assert.equal((await waiting).stoppedBy, "watchdog", "record-wait observes resource-local completion");

  captureDeferred = deferred();
  const capture = service.execute("capture-fixed", { sessionId, frameCount: 1 });
  await nextTurn();
  const progress = await service.execute("status", { sessionId });
  assert.equal(progress.fixedCapture.active, true, "status reports safe fixed-capture progress outside the FIFO");
  const cancelled = await service.execute("capture-cancel", { sessionId });
  assert.equal(cancelled.cancelling, true, "cancellation bypasses the serialized capture command");
  captureDeferred.resolve({ frameSummary: { count: 1 } });
  await capture;

  statusDeferred = deferred();
  const finalStatus = service.execute("status", { sessionId });
  await nextTurn();
  const closeBlocker = deferred();
  timeDefers.push(closeBlocker);
  const finalTime = service.execute("time", { sessionId, control: { action: "step", ticks: 1 } });
  await nextTurn();
  const closing = service.execute("close", { sessionId });
  await nextTurn();
  assert.equal(driverClosed, false, "close drains admitted serialized work before closing the driver");
  closeBlocker.resolve({ snapshotTick: 4 });
  await finalTime;
  await nextTurn();
  assert.equal(driverClosed, false, "close drains admitted observation work before closing the driver");
  statusDeferred.resolve({ ready: true, snapshotTick: 4, roomTime: { paused: true, speed: 0 } });
  await Promise.all([finalStatus, closing]);
  assert.equal(driverClosed, true, "close releases the driver after the semantic FIFO drains");

  console.log("lab interact session coordinator contracts passed");
} finally {
  await service.shutdown();
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

function nextTurn() {
  return new Promise((resolve) => setImmediate(resolve));
}
