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
      ...snapshotReportFields({
        reportStats: stats,
        transportStats,
        snapshotProcessing: this.snapshotProcessingReport,
      }),
      frameGapMaxMs: clampU16(stats.frameGapMaxMs),
      fpsEstimate: clampU16(avgFrameMs > 0 ? 1000 / avgFrameMs : 0),
      ...clientPerfReportFields(this.frameProfiler),
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
      correctionDistancePx: report.correctionDistancePx,
      frameWorkMaxMs: report.frameWorkMaxMs,
      rendererMaxMs: report.rendererMaxMs,
      worstFramePhase: report.worstFramePhase,
    });
    this.health.resetReportStats();
    this.frameProfiler?.resetReportWindow?.();
    this.snapshotProcessingReport.reset();
  }
}

export function predictionReportFields({ prediction, predictionAdapter } = {}) {
  const controller = prediction?.debugSummary?.() || {};
  const wasm = predictionAdapter?.diagnostics?.() || {};
  const commandReport = prediction?.consumeCommandReportStats?.() || {};
  return {
    predictionMode: String(controller.mode || "disabled"),
    pendingCommandCount: clampU16(controller.commandDiagnosticPendingCount ?? controller.pendingCommandCount),
    acknowledgedCommandLatencyMs: clampU16(controller.ackLatencyMs),
    ...clampedCommandReportFields(commandReport),
    correctionDistancePx: clampU16(controller.maxCorrectionDistance),
    correctionCount: clampU32(controller.correctionCount),
    predictionDisableCount: clampU32(controller.disableCount),
    wasmTickMs: clampU16(wasm.lastTickMs),
    wasmMemoryBytes: clampU32(wasm.memoryBytes),
    predictionReplayTicks: clampU16(wasm.lastReplayTicks),
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
  };
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
