// tests/client_contracts/prediction_controller_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import {
  PredictionController,
  PREDICTION_STATE,
} from "../../client/src/prediction_controller.js";
import { cmd } from "../../client/src/protocol.js";

// PredictionController
// ---------------------------------------------------------------------------
{
  let clock = 100;
  const sent = [];
  const prediction = new PredictionController({
    now: () => clock,
    commandTimeoutMs: 50,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  assert(prediction.debugSummary().mode === PREDICTION_STATE.TRACKING, "PredictionController starts tracking");
  prediction.issueCommand(cmd.stop([1]));
  prediction.issueCommand(cmd.stop([2]));
  prediction.issueCommand(cmd.stop([3]));
  assert(sent.map((entry) => entry.clientSeq).join(",") === "1,2,3", "PredictionController allocates sequences");
  prediction.applyAuthoritativeSnapshot({
    tick: 10,
    netStatus: { lastSimConsumedClientSeq: 1, lastSimConsumedClientTick: 9 },
  });
  assert(prediction.pendingCommandCount === 2, "PredictionController drops acknowledged commands");
  assert(prediction.debugSummary().pendingClientSeqs.join(",") === "2,3", "ack 1 leaves 2 and 3 pending");
  prediction.applyAuthoritativeSnapshot({ tick: 10, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(prediction.debugSummary().duplicateSnapshotCount === 1, "duplicate snapshots are tracked");
  prediction.applyAuthoritativeSnapshot({ tick: 12, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(prediction.debugSummary().skippedSnapshotCount === 1, "skipped authoritative ticks are tolerated");
  prediction.applyAuthoritativeSnapshot({ tick: 11, netStatus: { lastSimConsumedClientSeq: 3 } });
  assert(prediction.pendingCommandCount === 2, "stale snapshots do not ack commands");
  assert(prediction.debugSummary().staleSnapshotCount === 1, "stale snapshot is counted");
  prediction.issueCommand(cmd.stop([4]));
  prediction.issueCommand(cmd.stop([5]));
  prediction.applyAuthoritativeSnapshot({ tick: 13, netStatus: { lastSimConsumedClientSeq: 3 } });
  assert(prediction.debugSummary().pendingClientSeqs.join(",") === "4,5", "ack 3 drops older commands");
  prediction.recordSocketReceipt(4, { serverTick: 13 });
  assert(prediction.pendingCommandCount === 2, "socket receipt does not reconcile command 4");
  prediction.recordCommandRejection(5, "invalid target");
  assert(prediction.pendingCommandCount === 2, "command rejection notice alone does not consume sim ack");
  clock = 200;
  assert(prediction.expireTimedOutCommands() === 2, "timed out pending commands are marked");
  prediction.applyAuthoritativeSnapshot({ tick: 14, netStatus: { lastSimConsumedClientSeq: 5 } });
  assert(prediction.pendingCommandCount === 0, "later sim ack clears timed-out/rejected pending commands");
  prediction.beginResync({ dx: 3 });
  assert(prediction.debugSummary().mode === PREDICTION_STATE.RESYNCING, "resync state is exposed");
  prediction.finishResync();
  assert(prediction.debugSummary().mode === PREDICTION_STATE.TRACKING, "resync returns to tracking");
  prediction.reset();
  assert(prediction.debugSummary().nextClientSeq === 1, "PredictionController reset restarts sequence ids");

  const disabledSent = [];
  const disabledPrediction = new PredictionController({
    enabled: false,
    sendCommand(command, clientSeq) {
      assert(Number.isInteger(clientSeq) && clientSeq > 0, "disabled PredictionController sends valid clientSeq");
      disabledSent.push({ command, clientSeq });
      return true;
    },
  });
  const disabledIssued = disabledPrediction.issueCommand(cmd.move([7], 120, 160));
  assert(disabledIssued.sent && !disabledIssued.predicted, "PredictionController disabled mode still sends commands");
  assert(disabledIssued.clientSeq === 1, "PredictionController disabled mode still emits protocol sequence ids");
  assert(disabledSent.length === 1 && disabledSent[0].clientSeq === 1, "disabled commands use sequenced protocol send shape");
  assert(disabledPrediction.pendingCommandCount === 0, "disabled commands are not tracked as prediction pending");
  assert(disabledPrediction.debugSummary().nextClientSeq === 2, "disabled commands consume sequence ids");

  const toggledSent = [];
  const toggledPrediction = new PredictionController({
    sendCommand(command, clientSeq) {
      toggledSent.push({ command, clientSeq });
      return true;
    },
  });
  toggledPrediction.issueCommand(cmd.stop([1]));
  toggledPrediction.reset({ enabled: false, preserveClientSeq: true });
  toggledPrediction.issueCommand(cmd.stop([2]));
  toggledPrediction.reset({ enabled: true, preserveClientSeq: true });
  toggledPrediction.issueCommand(cmd.stop([3]));
  assert(
    toggledSent.map((entry) => entry.clientSeq).join(",") === "1,2,3",
    "PredictionController preserves command sequence ids across prediction toggles",
  );
}

{
  let clock = 1_000;
  const prediction = new PredictionController({
    now: () => clock,
    sendCommand() {
      clock += 7;
      return true;
    },
  });
  const moveIssued = prediction.issueCommand(cmd.move([1], 10, 20));
  clock += 40;
  prediction.recordSocketReceipt(moveIssued.clientSeq, { serverTick: 11 });
  const buildIssued = prediction.issueCommand(cmd.build([2], "depot", 4, 5));
  clock += 80;
  prediction.applyAuthoritativeSnapshot({
    tick: 12,
    netStatus: { lastSimConsumedClientSeq: buildIssued.clientSeq, lastSimConsumedClientTick: 12 },
  });
  const report = prediction.peekCommandReportStats();
  assert(report.commandIssueToSocketSendAcceptedMaxMs === 7, "command report splits client issue-to-send timing");
  assert(report.commandFamilyMove === 1 && report.commandFamilyBuild === 1, "command report counts stable command families");
  assert(
    report.commandLifecycleExemplars.some((entry) => entry.clientSeq === buildIssued.clientSeq && entry.family === "build"),
    "command report keeps bounded lifecycle exemplars with family and clientSeq",
  );
  prediction.consumeCommandReportStats();
  assert(prediction.peekCommandReportStats().commandLifecycleExemplars.length === 0, "command exemplars reset with the report window");
}

// ---------------------------------------------------------------------------
