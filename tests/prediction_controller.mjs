import fs from "node:fs";
import { PredictionController, PREDICTION_STATE } from "../client/src/prediction_controller.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function sentSeqs(sent) {
  return sent.map((entry) => entry.clientSeq).join(",");
}

{
  const sent = [];
  const controller = new PredictionController({
    now: () => 1000,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  for (const id of [1, 2, 3]) controller.issueCommand({ c: "stop", units: [id] });
  assert(sentSeqs(sent) === "1,2,3", "commands 1,2,3 are sequenced");
  controller.applyAuthoritativeSnapshot({ tick: 30, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "2,3", "ack 1 drops only command 1");
}

{
  const sent = [];
  const controller = new PredictionController({
    now: () => 2000,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  for (const id of [1, 2, 3, 4, 5]) controller.issueCommand({ c: "stop", units: [id] });
  controller.applyAuthoritativeSnapshot({ tick: 10, netStatus: { lastSimConsumedClientSeq: 3 } });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "4,5", "ack 3 leaves 4 and 5 pending");
  controller.recordSocketReceipt(4, { serverTick: 10 });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "4,5", "socket receipt is diagnostic only");
}

{
  const controller = new PredictionController({ sendCommand: () => true });
  controller.issueCommand({ c: "stop", units: [1] });
  controller.issueCommand({ c: "stop", units: [2] });
  controller.applyAuthoritativeSnapshot({ tick: 5, netStatus: { lastSimConsumedClientSeq: 0 } });
  controller.applyAuthoritativeSnapshot({ tick: 5, netStatus: { lastSimConsumedClientSeq: 0 } });
  controller.applyAuthoritativeSnapshot({ tick: 8, netStatus: { lastSimConsumedClientSeq: 1 } });
  controller.applyAuthoritativeSnapshot({ tick: 7, netStatus: { lastSimConsumedClientSeq: 2 } });
  const summary = controller.debugSummary();
  assert(summary.duplicateSnapshotCount === 1, "duplicate snapshots are counted");
  assert(summary.skippedSnapshotCount === 1, "skipped ticks are counted");
  assert(summary.staleSnapshotCount === 1, "out-of-date snapshots are ignored");
  assert(summary.pendingClientSeqs.join(",") === "2", "stale snapshot did not apply ack 2");
}

{
  let now = 0;
  const controller = new PredictionController({
    now: () => now,
    commandTimeoutMs: 10,
    sendCommand: () => true,
  });
  controller.issueCommand({ c: "stop", units: [1] });
  controller.recordCommandRejection(1, "bad command");
  assert(controller.pendingCommandCount === 1, "rejection does not imply sim consumption");
  now = 20;
  assert(controller.expireTimedOutCommands() === 1, "pending command timeout is reported");
  controller.applyAuthoritativeSnapshot({ tick: 2, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(controller.pendingCommandCount === 0, "ack clears rejected/timed-out command");
}

{
  const controller = new PredictionController({ enabled: false, sendCommand: () => true });
  assert(controller.debugSummary().mode === PREDICTION_STATE.DISABLED, "disabled mode is exposed");
  const result = controller.issueCommand({ c: "stop", units: [1] });
  assert(result === false, "disabled controller does not send gameplay commands");
  assert(controller.debugSummary().nextClientSeq === 1, "disabled controller does not allocate sequence ids");
}

{
  const files = [
    ["client/src/input/commands.js", "viewport right-click and hotkeys"],
    ["client/src/input/placement.js", "build placement"],
    ["client/src/minimap.js", "minimap right-click and rally"],
    ["client/src/hud.js", "HUD stop/train/research/cancel/ability"],
  ];
  for (const [file, label] of files) {
    const source = fs.readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
    assert(source.includes("commandIssuer.issueCommand"), `${file} routes ${label} through commandIssuer`);
    assert(!source.includes(".net.command("), `${file} does not send gameplay commands through Net`);
  }
}

console.log("prediction_controller: ok");
