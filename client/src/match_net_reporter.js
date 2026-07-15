import {
  clientPerfReportFields,
  snapshotReportFields,
} from "./client_perf_report.js";
import { installTauriNativeCursorBridge } from "./input/cursor_lock.js";

const MATCH_PING_MS = 2000;
const NET_REPORT_MS = 10000;

export class MatchNetReporter {
  constructor({
    net,
    health,
    frameProfiler,
    snapshotProcessingReport,
    diagnostics = null,
    matchRunId = "",
    getLastSnapshotTick = () => 0,
    getPredictionReportFields = () => ({}),
  }) {
    this.net = net;
    this.health = health;
    this.frameProfiler = frameProfiler;
    this.snapshotProcessingReport = snapshotProcessingReport;
    this.diagnostics = diagnostics;
    this.matchRunId = matchRunId;
    this.getLastSnapshotTick = getLastSnapshotTick;
    this.getPredictionReportFields = getPredictionReportFields;
    this.matchPingTimer = undefined;
    this.netReportTimer = undefined;
  }

  startMatchPings() {
    this.stopMatchPings();
    this.net.ping();
    this.matchPingTimer = window.setInterval(() => this.net.ping(), MATCH_PING_MS);
  }

  stopMatchPings() {
    if (this.matchPingTimer !== undefined) {
      clearInterval(this.matchPingTimer);
      this.matchPingTimer = undefined;
    }
  }

  startNetReports() {
    this.stopNetReports();
    this.netReportTimer = window.setInterval(() => this.sendNetReport(), NET_REPORT_MS);
  }

  stopNetReports() {
    if (this.netReportTimer !== undefined) {
      clearInterval(this.netReportTimer);
      this.netReportTimer = undefined;
    }
  }

  sendNetReport() {
    const stats = this.health.reportStats;
    const metrics = this.health.metrics();
    const transportStats = this.net.consumeSnapshotReportStats?.() || {};
    const elapsedMs = performance.now() - this.health.reportStartedAt;
    const avgFrameMs = stats.frameCount > 0 ? stats.frameTotalMs / stats.frameCount : 0;
    const report = {
      schemaVersion: 1,
      matchRunId: this.matchRunId,
      elapsedMs: clampU32(elapsedMs),
      matchTick: clampU32(this.getLastSnapshotTick()),
      rttMs: clampU16(metrics.latencyMs),
      rttMaxMs: clampU16(stats.rttMaxMs),
      badRttSamples: clampU32(stats.badRttSamples),
      snapshotJitterMs: clampU16(metrics.jitterMs),
      snapshotGapMaxMs: clampU16(stats.snapshotGapMaxMs),
      jitterSamples: clampU32(stats.jitterSamples),
      snapshots: clampU32(stats.snapshots),
      snapshotLateFrameCount: clampU32(stats.snapshotLateFrameCount),
      predictedSnapshotLateFrameCount: clampU32(stats.predictedSnapshotLateFrameCount),
      predictedSnapshotLateFramePctX100: clampU16(
        stats.snapshotLateFrameCount > 0
          ? (stats.predictedSnapshotLateFrameCount * 10000) / stats.snapshotLateFrameCount
          : 0,
      ),
      predictionActiveLateFrameCount: clampU32(stats.predictionActiveLateFrameCount),
      ...snapshotReportFields({
        reportStats: stats,
        transportStats,
        snapshotProcessing: this.snapshotProcessingReport,
      }),
      frameGapMaxMs: clampU16(stats.frameGapMaxMs),
      fpsEstimate: clampU16(avgFrameMs > 0 ? 1000 / avgFrameMs : 0),
      ...clientPerfReportFields(this.frameProfiler),
      commandBurstBucketMs: clampU16(stats.commandBurstBucketMs),
      commandBurstMax: clampU16(stats.commandBurstMax),
      commandBurstFrameGapMaxMs: clampU16(stats.commandBurstFrameGapMaxMs),
      commandBurstWorstFramePhase: clampReportLabel(stats.commandBurstWorstFramePhase),
      commandBurstWorstFramePhaseMs: clampU16(stats.commandBurstWorstFramePhaseMs),
      hidden: !!document.hidden,
      focused: typeof document.hasFocus === "function" ? document.hasFocus() : true,
      ...cursorRuntimeReportFields(),
      wsBufferedBytes: clampU32(this.net.bufferedAmount),
      serverTickMs: clampU16(metrics.serverTickMs),
      serverLagMs: clampU16(metrics.serverLagMs),
      slowTickCount: clampU32(metrics.issues.slowTick.count),
      headOfLineCount: clampU32(metrics.issues.headOfLine.count),
      ...this.getPredictionReportFields(),
    };
    this.net.netReport(report);
    this.diagnostics?.count("client.send.netReport", {
      rttMs: report.rttMs,
      rttMaxMs: report.rttMaxMs,
      snapshotGapMaxMs: report.snapshotGapMaxMs,
      jitterSamples: report.jitterSamples,
      wsBufferedBytes: report.wsBufferedBytes,
      predictionMode: report.predictionMode,
      pendingCommandCount: report.pendingCommandCount,
      commandBurstMax: report.commandBurstMax,
      correctionDistancePx: report.correctionDistancePx,
      frameWorkMaxMs: report.frameWorkMaxMs,
      frameRafDispatchMaxMs: report.frameRafDispatchMaxMs,
      frameUnattributedMaxMs: report.frameUnattributedMaxMs,
      rendererMaxMs: report.rendererMaxMs,
      rendererUpdateMaxMs: report.rendererUpdateMaxMs,
      rendererPresentMaxMs: report.rendererPresentMaxMs,
      frameWorkBudgetMissCount: report.frameWorkBudgetMissCount,
      presentBudgetMissCount: report.presentBudgetMissCount,
      worstFramePhase: report.worstFramePhase,
      topRendererPhase: report.topRendererPhase,
      topRenderDiagnosticGroup: report.topRenderDiagnosticGroup,
    });
    this.health.resetReportStats();
    this.frameProfiler?.resetReportWindow?.();
    this.snapshotProcessingReport.reset();
  }
}

export function predictionReportFields({ prediction, predictionAdapter } = {}) {
  const controller = prediction?.debugSummary?.() || {};
  const wasm = predictionAdapter?.diagnostics?.() || {};
  const wasmReport = predictionAdapter?.consumeReportStats?.() || {};
  const commandReport = prediction?.consumeCommandReportStats?.() || {};
  const disableCounts = stableDisableReasonCounts(controller.disableReasons || {}, wasm);
  return {
    predictionMode: String(controller.mode || "disabled"),
    pendingCommandCount: clampU16(controller.commandDiagnosticPendingCount ?? controller.pendingCommandCount),
    acknowledgedCommandLatencyMs: clampU16(controller.ackLatencyMs),
    ...clampedCommandReportFields(commandReport),
    correctionDistancePx: clampU16(controller.maxCorrectionDistance),
    correctionCount: clampU32(controller.correctionCount),
    predictionDisableCount: clampU32(controller.disableCount),
    ...disableCounts,
    wasmTickMs: clampU16(wasm.lastTickMs),
    wasmMemoryBytes: clampU32(wasm.memoryBytes),
    predictionReplayTicks: clampU16(wasm.lastReplayTicks),
    predictionReplayMaxMs: clampU16(Math.max(
      Number(wasmReport.predictionReplayMaxMs) || 0,
      Number(commandReport.predictionReplayMaxMs) || 0,
    )),
    predictionReplayMaxTicks: clampU16(Math.max(
      Number(wasmReport.predictionReplayMaxTicks) || 0,
      Number(commandReport.predictionReplayMaxTicks) || 0,
    )),
    predictionReplayBudgetExceededCount: clampU32(
      (Number(wasmReport.predictionReplayBudgetExceededCount) || 0) +
      (Number(commandReport.predictionReplayBudgetExceededCount) || 0),
    ),
  };
}

export function cursorRuntimeReportFields(root = globalThis) {
  installTauriNativeCursorBridge(root);
  const nativeCursor = root?.__RTS_NATIVE_CURSOR || null;
  const nativeDiagnostics = safeNativeCursorDiagnostics(nativeCursor);
  const tauriGlobals = Object.keys(root || {})
    .filter((key) => key.includes("TAURI"))
    .sort()
    .join(",");
  return {
    desktopRuntimePresent: !!root?.__RTS_DESKTOP_RUNTIME,
    nativeCursorBridgePresent: !!nativeCursor,
    nativeCursorSupported: nativeCursorSupported(nativeCursor),
    nativeCursorActive: !!nativeDiagnostics.active,
    nativeCursorLastReason: clampString(nativeDiagnostics.lastReason),
    nativeCursorLastError: clampString(nativeDiagnostics.lastError),
    tauriInternalsPresent: !!root?.__TAURI_INTERNALS__,
    tauriGlobalPresent: !!root?.__TAURI__,
    tauriGlobals: clampString(tauriGlobals),
  };
}

function safeNativeCursorDiagnostics(nativeCursor) {
  if (!nativeCursor || typeof nativeCursor.diagnostics !== "function") return {};
  try {
    return nativeCursor.diagnostics() || {};
  } catch {
    return {};
  }
}

function nativeCursorSupported(nativeCursor) {
  if (!nativeCursor) return false;
  if (typeof nativeCursor.supported !== "function") return nativeCursor.supported === true;
  try {
    return !!nativeCursor.supported();
  } catch {
    return false;
  }
}

function clampString(value, maxLength = 160) {
  if (value == null) return "";
  return String(value).replace(/\s+/g, " ").slice(0, maxLength);
}

function clampedCommandReportFields(report = {}) {
  return {
    commandsIssued: clampU32(report.commandsIssued),
    commandSocketSendAccepted: clampU32(report.commandSocketSendAccepted),
    commandServerReceived: clampU32(report.commandServerReceived),
    commandSimAcknowledged: clampU32(report.commandSimAcknowledged),
    commandRejected: clampU32(report.commandRejected),
    commandIssueToSocketSendAcceptedLatestMs: clampU16(report.commandIssueToSocketSendAcceptedLatestMs),
    commandIssueToSocketSendAcceptedMaxMs: clampU16(report.commandIssueToSocketSendAcceptedMaxMs),
    commandIssueToSocketSendAcceptedP95Ms: clampU16(report.commandIssueToSocketSendAcceptedP95Ms),
    commandIssueToServerReceiptLatestMs: clampU16(report.commandIssueToServerReceiptLatestMs),
    commandIssueToServerReceiptMaxMs: clampU16(report.commandIssueToServerReceiptMaxMs),
    commandIssueToServerReceiptP95Ms: clampU16(report.commandIssueToServerReceiptP95Ms),
    commandServerReceiptToSimAckLatestMs: clampU16(report.commandServerReceiptToSimAckLatestMs),
    commandServerReceiptToSimAckMaxMs: clampU16(report.commandServerReceiptToSimAckMaxMs),
    commandServerReceiptToSimAckP95Ms: clampU16(report.commandServerReceiptToSimAckP95Ms),
    commandIssueToSimAckLatestMs: clampU16(report.commandIssueToSimAckLatestMs),
    commandIssueToSimAckMaxMs: clampU16(report.commandIssueToSimAckMaxMs),
    commandIssueToSimAckP95Ms: clampU16(report.commandIssueToSimAckP95Ms),
    commandAckSnapshotReceivedToAppliedLatestMs: clampU16(report.commandAckSnapshotReceivedToAppliedLatestMs),
    commandAckSnapshotReceivedToAppliedMaxMs: clampU16(report.commandAckSnapshotReceivedToAppliedMaxMs),
    commandAckSnapshotReceivedToAppliedP95Ms: clampU16(report.commandAckSnapshotReceivedToAppliedP95Ms),
    oldestPendingCommandAgeMs: clampU16(report.oldestPendingCommandAgeMs),
    maxPendingCommandCount: clampU16(report.maxPendingCommandCount),
    commandFamilyMove: clampU32(report.commandFamilyMove),
    commandFamilyAttackMove: clampU32(report.commandFamilyAttackMove),
    commandFamilyBuild: clampU32(report.commandFamilyBuild),
    commandFamilyTrain: clampU32(report.commandFamilyTrain),
    commandFamilyOther: clampU32(report.commandFamilyOther),
    commandLifecycleExemplars: clampCommandLifecycleExemplars(report.commandLifecycleExemplars),
  };
}

function clampCommandLifecycleExemplars(exemplars = []) {
  if (!Array.isArray(exemplars)) return [];
  return exemplars.slice(0, 5).map((entry) => ({
    clientSeq: clampU32(entry?.clientSeq),
    family: clampCommandFamily(entry?.family),
    issuedElapsedMs: clampU32(entry?.issuedElapsedMs),
    stage: clampCommandStage(entry?.stage),
    stageMs: clampU16(entry?.stageMs),
  }));
}

function clampCommandFamily(value) {
  const text = String(value || "other");
  return ["move", "attackMove", "build", "train", "other"].includes(text) ? text : "other";
}

function clampCommandStage(value) {
  const text = String(value || "unknown");
  return [
    "issueToSocketSendAccepted",
    "issueToServerReceipt",
    "serverReceiptToSimAck",
    "issueToSimAck",
    "ackSnapshotReceivedToApplied",
  ].includes(text)
    ? text
    : "unknown";
}

function stableDisableReasonCounts(reasons = {}, wasm = {}) {
  const counts = {
    predictionDisableUserCount: 0,
    predictionDisableReplayCount: 0,
    predictionDisableSpectatorCount: 0,
    predictionDisableCompatibilityCount: 0,
    predictionDisableWasmCount: 0,
    predictionDisableOtherCount: 0,
  };
  for (const [reason, count] of Object.entries(reasons || {})) {
    const bucket = stableDisableReasonBucket(reason);
    counts[bucket] += clampU32(count);
  }
  if (wasm?.disabledReason && counts.predictionDisableWasmCount === 0) {
    counts.predictionDisableWasmCount += 1;
  }
  return counts;
}

function stableDisableReasonBucket(reason) {
  switch (reason) {
    case "user-disabled":
      return "predictionDisableUserCount";
    case "replay-viewer":
    case "replay-budget-exceeded":
      return "predictionDisableReplayCount";
    case "spectator":
      return "predictionDisableSpectatorCount";
    case "unsupported-local-faction":
    case "prediction-version-mismatch":
    case "prediction-unavailable":
    case "prediction-build-mismatch":
    case "compatibility-mismatch":
      return "predictionDisableCompatibilityCount";
    case "wasm-unavailable":
      return "predictionDisableWasmCount";
    default:
      return "predictionDisableOtherCount";
  }
}

function clampReportLabel(value) {
  return String(value || "").replace(/[^A-Za-z0-9_.:-]/g, "_").slice(0, 64);
}

function clampU16(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(65535, Math.round(n));
}

function clampU32(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(4294967295, Math.round(n));
}
