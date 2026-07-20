import { ReportWindowAggregate } from "./report_window_aggregate.js";

const U32_MAX = 0xffffffff;
const DEFAULT_COMMAND_TIMEOUT_MS = 15000;
const DEFAULT_UI_CONFIRMATION_SNAPSHOTS = 4;
const COMMAND_LIFECYCLE_EXEMPLAR_LIMIT = 5;

export const COMMAND_PREDICTION_POLICIES = Object.freeze({
  train: Object.freeze({
    family: "train",
    uiOptimism: true,
    confirmation: "ownerProductionSnapshot",
  }),
  setRally: Object.freeze({
    family: "rally",
    uiOptimism: true,
    confirmation: "ownerRallyPlanSnapshot",
  }),
  build: Object.freeze({ family: "build", uiOptimism: false, confirmation: "authoritativeOnly" }),
  deconstruct: Object.freeze({ family: "deconstruct", uiOptimism: false, confirmation: "authoritativeOnly" }),
  research: Object.freeze({ family: "research", uiOptimism: false, confirmation: "authoritativeOnly" }),
  useAbility: Object.freeze({ family: "ability", uiOptimism: false, confirmation: "authoritativeOnly" }),
  setupAntiTankGuns: Object.freeze({ family: "setup", uiOptimism: false, confirmation: "authoritativeOnly" }),
  tearDownAntiTankGuns: Object.freeze({ family: "teardown", uiOptimism: false, confirmation: "authoritativeOnly" }),
});

export const PREDICTION_STATE = Object.freeze({
  DISABLED: "disabled",
  TRACKING: "tracking",
  PREDICTING: "predicting",
  RESYNCING: "resyncing",
});

/**
 * Browser-side command sequencing and reconciliation bookkeeping.
 *
 * Phase 2 intentionally keeps GameState authoritative. This controller tracks
 * local command envelopes and server sim-consumption acknowledgements so a
 * later WASM predictor can replay pending commands without changing command
 * sources again.
 */
export class PredictionController {
  constructor({
    sendCommand = null,
    predictor = null,
    enabled = true,
    now = () => performance.now(),
    commandTimeoutMs = DEFAULT_COMMAND_TIMEOUT_MS,
    uiConfirmationSnapshots = DEFAULT_UI_CONFIRMATION_SNAPSHOTS,
  } = {}) {
    this.sendCommand = sendCommand;
    this.predictor = predictor;
    this.enabled = !!enabled;
    this.now = now;
    this.commandTimeoutMs = commandTimeoutMs;
    this.uiConfirmationSnapshots = uiConfirmationSnapshots;
    this.mode = this.enabled ? PREDICTION_STATE.TRACKING : PREDICTION_STATE.DISABLED;
    this.nextClientSeq = 1;
    this.pending = [];
    this.pendingBySeq = new Map();
    this.optimisticUi = [];
    this.latestEntitiesById = new Map();
    this.latestAuthoritativeTick = null;
    this.latestAckSeq = 0;
    this.latestAckTick = null;
    this.latestAckSnapshotAppliedSeq = 0;
    this.issuedCount = 0;
    this.acknowledgedCount = 0;
    this.staleSnapshotCount = 0;
    this.duplicateSnapshotCount = 0;
    this.skippedSnapshotCount = 0;
    this.receiptCount = 0;
    this.rejectionCount = 0;
    this.timedOutCount = 0;
    this.correctionCount = 0;
    this.lastCorrection = null;
    this.lastReceipt = null;
    this.lastRejected = null;
    this.maxCorrectionDistance = 0;
    this.snapCorrectionCount = 0;
    this.uiConfirmedCount = 0;
    this.uiExpiredCount = 0;
    this.uiRejectedCount = 0;
    this.ackLatencyMs = null;
    this.maxAckLatencyMs = 0;
    this.commandDiagnosticsBySeq = new Map();
    this.commandMetaBySeq = new Map();
    this.commandDiagnosticPending = [];
    this.resetCommandReportWindow();
    this.disableReasons = {};
  }

  reset({ enabled = this.enabled, preserveClientSeq = false, reason = null } = {}) {
    const nextClientSeq = this.nextClientSeq;
    this.enabled = !!enabled;
    this.mode = this.enabled ? PREDICTION_STATE.TRACKING : PREDICTION_STATE.DISABLED;
    if (!this.enabled && reason) this.recordDisableReason(reason);
    this.nextClientSeq = preserveClientSeq ? nextClientSeq : 1;
    this.pending = [];
    this.pendingBySeq.clear();
    this.optimisticUi = [];
    this.latestEntitiesById.clear();
    this.latestAuthoritativeTick = null;
    this.latestAckSeq = 0;
    this.latestAckTick = null;
    this.latestAckSnapshotAppliedSeq = 0;
    if (!preserveClientSeq) {
      this.issuedCount = 0;
      this.acknowledgedCount = 0;
      this.commandDiagnosticsBySeq.clear();
      this.commandMetaBySeq.clear();
      this.commandDiagnosticPending = [];
      this.resetCommandReportWindow();
    }
    this.staleSnapshotCount = 0;
    this.duplicateSnapshotCount = 0;
    this.skippedSnapshotCount = 0;
    this.receiptCount = 0;
    this.rejectionCount = 0;
    this.timedOutCount = 0;
    this.correctionCount = 0;
    this.lastCorrection = null;
    this.lastReceipt = null;
    this.lastRejected = null;
    this.maxCorrectionDistance = 0;
    this.snapCorrectionCount = 0;
    this.uiConfirmedCount = 0;
    this.uiExpiredCount = 0;
    this.uiRejectedCount = 0;
    this.ackLatencyMs = null;
    this.maxAckLatencyMs = 0;
  }

  recordDisableReason(reason) {
    const key = typeof reason === "string" && reason ? reason : "unknown";
    this.disableReasons[key] = (this.disableReasons[key] || 0) + 1;
  }

  issueCommand(cmd, options = {}) {
    const clientSeq = this._allocateClientSeq();
    const issuedAt = this.now();
    this.trackCommandIssue(clientSeq, issuedAt, cmd);
    if (!this.enabled) {
      const sent = this.sendCommand ? this.sendCommand(cmd, clientSeq) : false;
      this.recordCommandSendAccepted(clientSeq, !!sent);
      return { clientSeq, sent: !!sent, predicted: false };
    }
    const pending = {
      clientSeq,
      cmd,
      issuedAt,
      latestKnownServerTick: this.latestAuthoritativeTick,
      receiptAt: null,
      receiptTick: null,
      rejectedAt: null,
      rejectionReason: null,
      timedOut: false,
      sendAccepted: null,
    };
    this.pending.push(pending);
    this.pendingBySeq.set(clientSeq, pending);
    const predictMovement = options?.predictMovement !== false;
    const optimistic = buildOptimisticUiCommand(
      cmd,
      clientSeq,
      issuedAt,
      this.latestAuthoritativeTick,
      this.latestEntitiesById,
      this.optimisticUi,
      options,
    );
    if (optimistic) this.optimisticUi.push(optimistic);
    let predicted = false;
    if (predictMovement) {
      try {
        predicted = !!this.predictor?.enqueueCommand(clientSeq, cmd);
      } catch (err) {
        this.lastCorrection = { error: errorMessage(err), phase: "enqueue" };
      }
    }
    if (predicted) this.enterPredicting();
    const sent = this.sendCommand ? this.sendCommand(cmd, clientSeq) : false;
    pending.sendAccepted = !!sent;
    this.recordCommandSendAccepted(clientSeq, !!sent);
    return { clientSeq, sent: !!sent, predicted };
  }

  applyAuthoritativeSnapshot(snapshot, { allowStale = false } = {}) {
    if (!snapshot || typeof snapshot !== "object") {
      return this.debugSummary();
    }
    if (!this.enabled) {
      const ackSeq = finiteU32(snapshot.netStatus?.lastSimConsumedClientSeq);
      if (ackSeq != null) {
        this.applySimAcknowledgement(ackSeq, finiteU32(snapshot.netStatus?.lastSimConsumedClientTick));
      }
      return this.debugSummary();
    }
    const tick = finiteU32(snapshot.tick);
    if (tick != null && this.latestAuthoritativeTick != null) {
      if (tick < this.latestAuthoritativeTick && !allowStale) {
        this.staleSnapshotCount += 1;
        return this.debugSummary();
      }
      if (tick === this.latestAuthoritativeTick) {
        this.duplicateSnapshotCount += 1;
      } else if (tick > this.latestAuthoritativeTick + 1) {
        this.skippedSnapshotCount += 1;
      }
    }
    if (tick != null) this.latestAuthoritativeTick = tick;
    this.latestEntitiesById = entitiesById(snapshot.entities);

    const netStatus = snapshot.netStatus || {};
    const ackSeq = finiteU32(netStatus.lastSimConsumedClientSeq);
    if (ackSeq != null) {
      const ackTick = finiteU32(netStatus.lastSimConsumedClientTick);
      this.applySimAcknowledgement(ackSeq, ackTick);
    }
    this.reconcilePredictor(snapshot);
    this.reconcileOptimisticUi(snapshot, tick);
    this.expireTimedOutCommands();
    return this.debugSummary();
  }

  applySimAcknowledgement(clientSeq, serverTick = null) {
    const ackSeq = finiteU32(clientSeq);
    if (ackSeq == null || ackSeq <= this.latestAckSeq) return this.debugSummary();
    this.latestAckSeq = ackSeq;
    const ackTick = finiteU32(serverTick);
    if (ackTick != null) this.latestAckTick = ackTick;
    this.recordCommandSimAcknowledgement(ackSeq);

    if (!this.enabled) return this.debugSummary();
    const kept = [];
    for (const pending of this.pending) {
      if (pending.clientSeq <= ackSeq) {
        const latency = this.now() - pending.issuedAt;
        if (Number.isFinite(latency) && latency >= 0) {
          this.ackLatencyMs = latency;
          this.maxAckLatencyMs = Math.max(this.maxAckLatencyMs, latency);
        }
        this.pendingBySeq.delete(pending.clientSeq);
        this.acknowledgedCount += 1;
      } else {
        kept.push(pending);
      }
    }
    this.pending = kept;
    return this.debugSummary();
  }

  recordSocketReceipt(clientSeq, detail = {}) {
    const seq = finiteU32(clientSeq);
    if (seq == null) return this.debugSummary();
    if (detail?.accepted === false) {
      return this.recordCommandRejection(seq, detail.reason || null, detail);
    }
    const pending = this.pendingBySeq.get(seq);
    const receivedAt = this.now();
    const receipt = {
      clientSeq: seq,
      receivedAt,
      serverTick: finiteU32(detail.serverTick),
    };
    if (pending) {
      pending.receiptAt = receipt.receivedAt;
      pending.receiptTick = receipt.serverTick;
    }
    const diagnostic = this.commandDiagnosticsBySeq.get(seq);
    let firstReceipt = true;
    if (diagnostic && diagnostic.receiptAt == null) {
      diagnostic.receiptAt = receivedAt;
      diagnostic.receiptTick = receipt.serverTick;
      this.commandReport.commandServerReceived += 1;
      this.addCommandTiming("issueToServerReceipt", receivedAt - diagnostic.issuedAt, diagnostic);
    } else if (diagnostic) {
      firstReceipt = false;
    } else if (!diagnostic) {
      this.commandReport.commandServerReceived += 1;
    }
    this.lastReceipt = receipt;
    if (firstReceipt) this.receiptCount += 1;
    return this.debugSummary();
  }

  recordCommandRejection(clientSeq, reason = null, detail = {}) {
    const seq = finiteU32(clientSeq);
    if (seq == null) return this.debugSummary();
    const pending = this.pendingBySeq.get(seq);
    const rejectedAt = this.now();
    const serverTick = finiteU32(detail.serverTick);
    if (pending) {
      pending.rejectedAt = rejectedAt;
      pending.rejectionReason = reason;
      if (serverTick != null) pending.receiptTick = serverTick;
    }
    const diagnostic = this.commandDiagnosticsBySeq.get(seq);
    if (diagnostic && diagnostic.rejectedAt == null) {
      diagnostic.rejectedAt = rejectedAt;
      diagnostic.rejectionReason = reason;
      diagnostic.receiptTick = serverTick;
      this.commandReport.commandRejected += 1;
      this.addCommandTiming("issueToServerReceipt", rejectedAt - diagnostic.issuedAt, diagnostic);
    } else if (!diagnostic) {
      this.commandReport.commandRejected += 1;
    }
    this.lastRejected = { clientSeq: seq, reason, rejectedAt, serverTick };
    this.rejectionCount += 1;
    this.dropOptimisticUiForSeq(seq, "rejected");
    return this.debugSummary();
  }

  recordAckSnapshotApplied(clientSeq, snapshotReceivedAt) {
    const seq = finiteU32(clientSeq);
    const receivedAt = Number(snapshotReceivedAt);
    if (seq == null || seq <= this.latestAckSnapshotAppliedSeq || !Number.isFinite(receivedAt)) {
      return this.debugSummary();
    }
    this.latestAckSnapshotAppliedSeq = seq;
    const meta = this.commandMetaBySeq.get(seq);
    this.addCommandTiming(
      "ackSnapshotReceivedToApplied",
      this.now() - receivedAt,
      this.commandDiagnosticsBySeq.get(seq) || meta || { clientSeq: seq, family: "other", issuedAt: receivedAt },
    );
    for (const key of this.commandMetaBySeq.keys()) {
      if (key <= seq) this.commandMetaBySeq.delete(key);
    }
    return this.debugSummary();
  }

  dropOptimisticUiForSeq(clientSeq, reason = "dropped") {
    const seq = finiteU32(clientSeq);
    if (seq == null || this.optimisticUi.length === 0) return 0;
    const before = this.optimisticUi.length;
    this.optimisticUi = this.optimisticUi.filter((entry) => entry.clientSeq !== seq);
    const dropped = before - this.optimisticUi.length;
    if (dropped > 0 && reason === "rejected") this.uiRejectedCount += dropped;
    return dropped;
  }

  enterPredicting() {
    if (this.enabled) this.mode = PREDICTION_STATE.PREDICTING;
  }

  beginResync(correction = null) {
    if (!this.enabled) return;
    this.mode = PREDICTION_STATE.RESYNCING;
    this.correctionCount += 1;
    this.lastCorrection = correction;
  }

  finishResync() {
    if (this.enabled) this.mode = PREDICTION_STATE.TRACKING;
  }

  reconcilePredictor(snapshot) {
    if (!this.enabled || !this.predictor) return null;
    try {
      const result = this.predictor.reconcile(snapshot, this.pending);
      if (!result) return null;
      const distance = Number(result.correctionDistance) || 0;
      this.maxCorrectionDistance = Math.max(this.maxCorrectionDistance, distance);
      if (result.snapCorrection) this.snapCorrectionCount += 1;
      if (distance > 0.01) {
        this.beginResync({
          distance,
          snap: !!result.snapCorrection,
          tick: finiteU32(snapshot?.tick),
        });
      } else if (this.mode === PREDICTION_STATE.RESYNCING) {
        this.finishResync();
      } else if (this.pending.length > 0) {
        this.enterPredicting();
      } else if (this.mode === PREDICTION_STATE.PREDICTING) {
        this.mode = PREDICTION_STATE.TRACKING;
      }
      return result;
    } catch (err) {
      this.beginResync({ error: errorMessage(err), phase: "reconcile" });
      return null;
    }
  }

  expireTimedOutCommands(now = this.now()) {
    if (!this.enabled || !(this.commandTimeoutMs > 0)) return 0;
    let count = 0;
    for (const pending of this.pending) {
      if (!pending.timedOut && now - pending.issuedAt >= this.commandTimeoutMs) {
        pending.timedOut = true;
        count += 1;
      }
    }
    this.timedOutCount += count;
    return count;
  }

  reconcileOptimisticUi(snapshot, tick = null) {
    if (!this.enabled || this.optimisticUi.length === 0) return;
    const entities = this.latestEntitiesById;
    const kept = [];
    for (const entry of this.optimisticUi) {
      if (tick != null && entry.lastSeenTick !== tick) {
        entry.snapshotsSeen += 1;
        entry.lastSeenTick = tick;
      }
      if (optimisticUiConfirmed(entry, entities)) {
        this.uiConfirmedCount += 1;
        continue;
      }
      if (entry.snapshotsSeen >= this.uiConfirmationSnapshots) {
        this.uiExpiredCount += 1;
        continue;
      }
      kept.push(entry);
    }
    this.optimisticUi = kept;
  }

  optimisticUiState() {
    const production = [];
    const rally = [];
    for (const entry of this.optimisticUi) {
      if (entry.family === "train") {
        production.push({
          clientSeq: entry.clientSeq,
          family: entry.family,
          building: entry.building,
          unit: entry.unit,
          optimisticQueue: entry.optimisticQueue,
          predicted: true,
        });
      } else if (entry.family === "rally") {
        rally.push({
          clientSeq: entry.clientSeq,
          family: entry.family,
          building: entry.building,
          plan: entry.expectedPlan.map((stage) => ({ ...stage, predicted: true })),
          predicted: true,
        });
      }
    }
    return { production, rally };
  }

  predictionDisplayOverlay() {
    return {
      optimisticCommands: this.optimisticUiState(),
    };
  }

  get pendingCommandCount() {
    return this.pending.length;
  }

  debugSummary() {
    return {
      mode: this.mode,
      enabled: this.enabled,
      pendingCommandCount: this.pending.length,
      pendingClientSeqs: this.pending.map((entry) => entry.clientSeq),
      commandDiagnosticPendingCount: this.commandDiagnosticPending.length,
      commandDiagnosticPendingSeqs: this.commandDiagnosticPending.map((entry) => entry.clientSeq),
      latestAuthoritativeTick: this.latestAuthoritativeTick,
      latestAckSeq: this.latestAckSeq,
      latestAckTick: this.latestAckTick,
      latestAckSnapshotAppliedSeq: this.latestAckSnapshotAppliedSeq,
      nextClientSeq: this.nextClientSeq,
      issuedCount: this.issuedCount,
      acknowledgedCount: this.acknowledgedCount,
      staleSnapshotCount: this.staleSnapshotCount,
      duplicateSnapshotCount: this.duplicateSnapshotCount,
      skippedSnapshotCount: this.skippedSnapshotCount,
      receiptCount: this.receiptCount,
      rejectionCount: this.rejectionCount,
      timedOutCount: this.timedOutCount,
      optimisticUiCount: this.optimisticUi.length,
      optimisticUiClientSeqs: this.optimisticUi.map((entry) => entry.clientSeq),
      uiConfirmedCount: this.uiConfirmedCount,
      uiExpiredCount: this.uiExpiredCount,
      uiRejectedCount: this.uiRejectedCount,
      optimisticUiByFamily: optimisticUiByFamily(this.optimisticUi),
      correctionCount: this.correctionCount,
      maxCorrectionDistance: this.maxCorrectionDistance,
      snapCorrectionCount: this.snapCorrectionCount,
      ackLatencyMs: this.ackLatencyMs,
      maxAckLatencyMs: this.maxAckLatencyMs,
      commandReport: this.peekCommandReportStats(),
      disableReasons: { ...this.disableReasons },
      disableCount: Object.values(this.disableReasons).reduce((sum, count) => sum + count, 0),
      lastReceipt: this.lastReceipt,
      lastRejected: this.lastRejected,
      lastCorrection: this.lastCorrection,
    };
  }

  resetCommandReportWindow() {
    this.commandReportWindowStartedAt = this.now();
    this.commandReport = {
      commandsIssued: 0,
      commandSocketSendAccepted: 0,
      commandServerReceived: 0,
      commandSimAcknowledged: 0,
      commandRejected: 0,
      commandFamilyMove: 0,
      commandFamilyAttackMove: 0,
      commandFamilyBuild: 0,
      commandFamilyTrain: 0,
      commandFamilyOther: 0,
      maxPendingCommandCount: this.commandDiagnosticPending?.length || 0,
      predictionReplayMaxMs: 0,
      predictionReplayMaxTicks: 0,
      predictionReplayBudgetExceededCount: 0,
      commandLifecycleExemplars: [],
    };
    this.commandTimings = {
      issueToSocketSendAccepted: new ReportWindowAggregate(),
      issueToServerReceipt: new ReportWindowAggregate(),
      serverReceiptToSimAck: new ReportWindowAggregate(),
      issueToSimAck: new ReportWindowAggregate(),
      ackSnapshotReceivedToApplied: new ReportWindowAggregate(),
    };
    this.commandTimingLatest = {
      issueToSocketSendAccepted: 0,
      issueToServerReceipt: 0,
      serverReceiptToSimAck: 0,
      issueToSimAck: 0,
      ackSnapshotReceivedToApplied: 0,
    };
  }

  trackCommandIssue(clientSeq, issuedAt, cmd = null) {
    const family = commandLifecycleFamily(cmd);
    const diagnostic = {
      clientSeq,
      family,
      issuedAt,
      sendAccepted: false,
      receiptAt: null,
      receiptTick: null,
      simAckAt: null,
      rejectedAt: null,
      rejectionReason: null,
    };
    this.commandDiagnosticsBySeq.set(clientSeq, diagnostic);
    this.commandMetaBySeq.set(clientSeq, {
      clientSeq,
      family,
      issuedAt,
    });
    this.commandDiagnosticPending.push(diagnostic);
    this.issuedCount += 1;
    this.commandReport.commandsIssued += 1;
    this.commandReport[commandFamilyCountField(family)] += 1;
    this.commandReport.maxPendingCommandCount = Math.max(
      this.commandReport.maxPendingCommandCount,
      this.commandDiagnosticPending.length,
    );
    return diagnostic;
  }

  recordCommandSendAccepted(clientSeq, sent) {
    const diagnostic = this.commandDiagnosticsBySeq.get(clientSeq);
    if (diagnostic) diagnostic.sendAccepted = !!sent;
    if (sent) {
      this.commandReport.commandSocketSendAccepted += 1;
      if (diagnostic) this.addCommandTiming("issueToSocketSendAccepted", this.now() - diagnostic.issuedAt, diagnostic);
    }
  }

  recordCommandSimAcknowledgement(ackSeq) {
    const now = this.now();
    const kept = [];
    for (const diagnostic of this.commandDiagnosticPending) {
      if (diagnostic.clientSeq <= ackSeq) {
        diagnostic.simAckAt = now;
        this.commandDiagnosticsBySeq.delete(diagnostic.clientSeq);
        this.commandReport.commandSimAcknowledged += 1;
        this.addCommandTiming("issueToSimAck", now - diagnostic.issuedAt, diagnostic);
        if (diagnostic.receiptAt != null) {
          this.addCommandTiming("serverReceiptToSimAck", now - diagnostic.receiptAt, diagnostic);
        }
      } else {
        kept.push(diagnostic);
      }
    }
    this.commandDiagnosticPending = kept;
  }

  addCommandTiming(kind, value, diagnostic = null) {
    const number = Number(value);
    if (!Number.isFinite(number) || number < 0 || !this.commandTimings[kind]) return;
    this.commandTimingLatest[kind] = Math.round(Math.min(number, 60_000));
    this.commandTimings[kind].add(number);
    this.recordCommandLifecycleExemplar(kind, number, diagnostic);
  }

  recordCommandLifecycleExemplar(kind, value, diagnostic = null) {
    if (!diagnostic || !Number.isInteger(diagnostic.clientSeq)) return;
    const stageMs = Math.round(Math.min(Math.max(value, 0), 60_000));
    const issuedElapsedMs = Math.round(Math.max(0, diagnostic.issuedAt - this.commandReportWindowStartedAt));
    this.commandReport.commandLifecycleExemplars.push({
      clientSeq: diagnostic.clientSeq,
      family: diagnostic.family || "other",
      issuedElapsedMs,
      stage: kind,
      stageMs,
    });
    this.commandReport.commandLifecycleExemplars.sort(
      (a, b) => b.stageMs - a.stageMs || a.clientSeq - b.clientSeq,
    );
    this.commandReport.commandLifecycleExemplars.length = Math.min(
      this.commandReport.commandLifecycleExemplars.length,
      COMMAND_LIFECYCLE_EXEMPLAR_LIMIT,
    );
  }

  recordReplayBudgetExceeded({ elapsedMs = 0, replayTicks = 0 } = {}) {
    const elapsed = Number(elapsedMs);
    if (Number.isFinite(elapsed) && elapsed >= 0) {
      this.commandReport.predictionReplayMaxMs = Math.max(
        this.commandReport.predictionReplayMaxMs,
        elapsed,
      );
    }
    const ticks = Number(replayTicks);
    if (Number.isFinite(ticks) && ticks >= 0) {
      this.commandReport.predictionReplayMaxTicks = Math.max(
        this.commandReport.predictionReplayMaxTicks,
        Math.trunc(ticks),
      );
    }
    this.commandReport.predictionReplayBudgetExceededCount += 1;
    this.recordDisableReason("replay-budget-exceeded");
  }

  consumeCommandReportStats(now = this.now()) {
    const out = this.peekCommandReportStats(now);
    this.resetCommandReportWindow();
    return out;
  }

  peekCommandReportStats(now = this.now()) {
    const issueToReceipt = this.commandTimings.issueToServerReceipt.summary();
    const issueToSend = this.commandTimings.issueToSocketSendAccepted.summary();
    const receiptToAck = this.commandTimings.serverReceiptToSimAck.summary();
    const issueToAck = this.commandTimings.issueToSimAck.summary();
    const ackApply = this.commandTimings.ackSnapshotReceivedToApplied.summary();
    return {
      commandsIssued: this.commandReport.commandsIssued,
      commandSocketSendAccepted: this.commandReport.commandSocketSendAccepted,
      commandServerReceived: this.commandReport.commandServerReceived,
      commandSimAcknowledged: this.commandReport.commandSimAcknowledged,
      commandRejected: this.commandReport.commandRejected,
      commandIssueToSocketSendAcceptedLatestMs: this.commandTimingLatest.issueToSocketSendAccepted,
      commandIssueToSocketSendAcceptedMaxMs: issueToSend.max,
      commandIssueToSocketSendAcceptedP95Ms: issueToSend.p95,
      commandIssueToServerReceiptLatestMs: this.commandTimingLatest.issueToServerReceipt,
      commandIssueToServerReceiptMaxMs: issueToReceipt.max,
      commandIssueToServerReceiptP95Ms: issueToReceipt.p95,
      commandServerReceiptToSimAckLatestMs: this.commandTimingLatest.serverReceiptToSimAck,
      commandServerReceiptToSimAckMaxMs: receiptToAck.max,
      commandServerReceiptToSimAckP95Ms: receiptToAck.p95,
      commandIssueToSimAckLatestMs: this.commandTimingLatest.issueToSimAck,
      commandIssueToSimAckMaxMs: issueToAck.max,
      commandIssueToSimAckP95Ms: issueToAck.p95,
      commandAckSnapshotReceivedToAppliedLatestMs: this.commandTimingLatest.ackSnapshotReceivedToApplied,
      commandAckSnapshotReceivedToAppliedMaxMs: ackApply.max,
      commandAckSnapshotReceivedToAppliedP95Ms: ackApply.p95,
      oldestPendingCommandAgeMs: oldestPendingAge(this.commandDiagnosticPending, now),
      maxPendingCommandCount: this.commandReport.maxPendingCommandCount,
      commandFamilyMove: this.commandReport.commandFamilyMove,
      commandFamilyAttackMove: this.commandReport.commandFamilyAttackMove,
      commandFamilyBuild: this.commandReport.commandFamilyBuild,
      commandFamilyTrain: this.commandReport.commandFamilyTrain,
      commandFamilyOther: this.commandReport.commandFamilyOther,
      commandLifecycleExemplars: this.commandReport.commandLifecycleExemplars.map((entry) => ({ ...entry })),
      predictionReplayMaxMs: this.commandReport.predictionReplayMaxMs,
      predictionReplayMaxTicks: this.commandReport.predictionReplayMaxTicks,
      predictionReplayBudgetExceededCount: this.commandReport.predictionReplayBudgetExceededCount,
    };
  }

  _allocateClientSeq() {
    if (this.nextClientSeq > U32_MAX) {
      throw new Error("client command sequence exhausted for this match");
    }
    return this.nextClientSeq++;
  }
}

function finiteU32(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n < 0) return null;
  return Math.min(U32_MAX, Math.trunc(n));
}

function commandLifecycleFamily(command) {
  switch (command?.c) {
    case "move":
      return "move";
    case "formationMove":
      return command.attackMove ? "attackMove" : "move";
    case "attackMove":
      return "attackMove";
    case "build":
      return "build";
    case "train":
      return "train";
    default:
      return "other";
  }
}

function commandFamilyCountField(family) {
  switch (family) {
    case "move":
      return "commandFamilyMove";
    case "attackMove":
      return "commandFamilyAttackMove";
    case "build":
      return "commandFamilyBuild";
    case "train":
      return "commandFamilyTrain";
    default:
      return "commandFamilyOther";
  }
}

function entitiesById(entities) {
  const out = new Map();
  for (const entity of entities || []) {
    if (entity && typeof entity.id === "number") out.set(entity.id, entity);
  }
  return out;
}

function buildOptimisticUiCommand(cmd, clientSeq, issuedAt, tick, entities, optimisticUi, options) {
  if (!cmd || typeof cmd !== "object") return null;
  const policy = COMMAND_PREDICTION_POLICIES[cmd.c];
  if (!policy?.uiOptimism) return null;
  if (policy.family === "train") return optimisticTrain(cmd, clientSeq, issuedAt, tick, entities, optimisticUi, options);
  if (policy.family === "rally") return optimisticRally(cmd, clientSeq, issuedAt, tick, entities);
  return null;
}

function optimisticTrain(cmd, clientSeq, issuedAt, tick, entities, optimisticUi, options) {
  const building = finiteU32(cmd.building);
  if (building == null || typeof cmd.unit !== "string") return null;
  const entity = entities.get(building) || {};
  const baselineQueue = finiteU32(entity.prodQueue) ?? 0;
  const pendingQueue = (optimisticUi || [])
    .filter((entry) => entry.family === "train" && entry.building === building)
    .reduce((max, entry) => Math.max(max, entry.optimisticQueue || 0), baselineQueue);
  return {
    family: "train",
    clientSeq,
    issuedAt,
    issuedTick: tick,
    lastSeenTick: tick,
    snapshotsSeen: 0,
    building,
    unit: cmd.unit,
    baselineQueue,
    baselineProdKind: typeof entity.prodKind === "string" ? entity.prodKind : null,
    optimisticQueue: Math.max(pendingQueue + 1, 1),
    metadata: options?.optimism || null,
  };
}

function optimisticRally(cmd, clientSeq, issuedAt, tick, entities) {
  const building = finiteU32(cmd.building);
  const x = finiteNumber(cmd.x);
  const y = finiteNumber(cmd.y);
  if (building == null || x == null || y == null) return null;
  const entity = entities.get(building) || {};
  const currentPlan = rallyPlanOf(entity);
  const stage = {
    kind: cmd.kind === "attackMove" ? "attackMove" : "move",
    x,
    y,
  };
  const expectedPlan = cmd.queued ? currentPlan.concat(stage).slice(0, 4) : [stage];
  return {
    family: "rally",
    clientSeq,
    issuedAt,
    issuedTick: tick,
    lastSeenTick: tick,
    snapshotsSeen: 0,
    building,
    expectedPlan,
  };
}

function optimisticUiConfirmed(entry, entities) {
  const entity = entities.get(entry.building);
  if (!entity) return false;
  if (entry.family === "train") {
    const queue = finiteU32(entity.prodQueue) ?? 0;
    if (queue < entry.optimisticQueue) return false;
    return typeof entity.prodKind !== "string" || entity.prodKind === entry.unit;
  }
  if (entry.family === "rally") {
    return rallyPlansEqual(rallyPlanOf(entity), entry.expectedPlan);
  }
  return false;
}

function rallyPlanOf(entity) {
  if (Array.isArray(entity?.rallyPlan) && entity.rallyPlan.length > 0) {
    return entity.rallyPlan.map(normalizeRallyStage).filter(Boolean);
  }
  if (Array.isArray(entity?.rally) && entity.rally.length === 2) {
    const x = finiteNumber(entity.rally[0]);
    const y = finiteNumber(entity.rally[1]);
    if (x != null && y != null) return [{ kind: "move", x, y }];
  }
  return [];
}

function normalizeRallyStage(stage) {
  const x = finiteNumber(stage?.x);
  const y = finiteNumber(stage?.y);
  if (x == null || y == null) return null;
  return { kind: stage.kind === "attackMove" ? "attackMove" : "move", x, y };
}

function rallyPlansEqual(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (a[i].kind !== b[i].kind) return false;
    if (Math.abs(a[i].x - b[i].x) > 0.5 || Math.abs(a[i].y - b[i].y) > 0.5) return false;
  }
  return true;
}

function finiteNumber(value) {
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

function oldestPendingAge(entries, now) {
  let oldest = 0;
  for (const entry of entries || []) {
    const age = now - entry.issuedAt;
    if (Number.isFinite(age) && age > oldest) oldest = age;
  }
  return Math.round(Math.min(oldest, 60_000));
}

function optimisticUiByFamily(entries) {
  const out = {};
  for (const entry of entries || []) {
    out[entry.family] = (out[entry.family] || 0) + 1;
  }
  return out;
}

function errorMessage(err) {
  if (err instanceof Error) return err.message;
  return String(err);
}
