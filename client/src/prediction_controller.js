const U32_MAX = 0xffffffff;
const DEFAULT_COMMAND_TIMEOUT_MS = 15000;
const DEFAULT_UI_CONFIRMATION_SNAPSHOTS = 4;

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
  }

  reset({ enabled = this.enabled, preserveClientSeq = false } = {}) {
    const nextClientSeq = this.nextClientSeq;
    this.enabled = !!enabled;
    this.mode = this.enabled ? PREDICTION_STATE.TRACKING : PREDICTION_STATE.DISABLED;
    this.nextClientSeq = preserveClientSeq ? nextClientSeq : 1;
    this.pending = [];
    this.pendingBySeq.clear();
    this.optimisticUi = [];
    this.latestEntitiesById.clear();
    this.latestAuthoritativeTick = null;
    this.latestAckSeq = 0;
    this.latestAckTick = null;
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
  }

  issueCommand(cmd, options = {}) {
    const clientSeq = this._allocateClientSeq();
    if (!this.enabled) {
      this.issuedCount += 1;
      const sent = this.sendCommand ? this.sendCommand(cmd, clientSeq) : false;
      return { clientSeq, sent: !!sent, predicted: false };
    }
    const issuedAt = this.now();
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
    this.issuedCount += 1;
    let predicted = false;
    try {
      predicted = !!this.predictor?.enqueueCommand(clientSeq, cmd);
    } catch (err) {
      this.lastCorrection = { error: errorMessage(err), phase: "enqueue" };
    }
    if (predicted) this.enterPredicting();
    const sent = this.sendCommand ? this.sendCommand(cmd, clientSeq) : false;
    pending.sendAccepted = !!sent;
    return { clientSeq, sent: !!sent, predicted };
  }

  applyAuthoritativeSnapshot(snapshot, { allowStale = false } = {}) {
    if (!this.enabled || !snapshot || typeof snapshot !== "object") {
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
    if (!this.enabled) return this.debugSummary();
    const ackSeq = finiteU32(clientSeq);
    if (ackSeq == null || ackSeq <= this.latestAckSeq) return this.debugSummary();
    this.latestAckSeq = ackSeq;
    const ackTick = finiteU32(serverTick);
    if (ackTick != null) this.latestAckTick = ackTick;

    const kept = [];
    for (const pending of this.pending) {
      if (pending.clientSeq <= ackSeq) {
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
    if (!this.enabled) return this.debugSummary();
    const seq = finiteU32(clientSeq);
    if (seq == null) return this.debugSummary();
    const pending = this.pendingBySeq.get(seq);
    const receipt = {
      clientSeq: seq,
      receivedAt: this.now(),
      serverTick: finiteU32(detail.serverTick),
    };
    if (pending) {
      pending.receiptAt = receipt.receivedAt;
      pending.receiptTick = receipt.serverTick;
    }
    this.lastReceipt = receipt;
    this.receiptCount += 1;
    return this.debugSummary();
  }

  recordCommandRejection(clientSeq, reason = null) {
    if (!this.enabled) return this.debugSummary();
    const seq = finiteU32(clientSeq);
    if (seq == null) return this.debugSummary();
    const pending = this.pendingBySeq.get(seq);
    const rejectedAt = this.now();
    if (pending) {
      pending.rejectedAt = rejectedAt;
      pending.rejectionReason = reason;
    }
    this.lastRejected = { clientSeq: seq, reason, rejectedAt };
    this.rejectionCount += 1;
    return this.debugSummary();
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
          building: entry.building,
          unit: entry.unit,
          optimisticQueue: entry.optimisticQueue,
        });
      } else if (entry.family === "rally") {
        rally.push({
          clientSeq: entry.clientSeq,
          building: entry.building,
          plan: entry.expectedPlan.map((stage) => ({ ...stage })),
        });
      }
    }
    return { production, rally };
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
      latestAuthoritativeTick: this.latestAuthoritativeTick,
      latestAckSeq: this.latestAckSeq,
      latestAckTick: this.latestAckTick,
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
      correctionCount: this.correctionCount,
      maxCorrectionDistance: this.maxCorrectionDistance,
      snapCorrectionCount: this.snapCorrectionCount,
      lastReceipt: this.lastReceipt,
      lastRejected: this.lastRejected,
      lastCorrection: this.lastCorrection,
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

function entitiesById(entities) {
  const out = new Map();
  for (const entity of entities || []) {
    if (entity && typeof entity.id === "number") out.set(entity.id, entity);
  }
  return out;
}

function buildOptimisticUiCommand(cmd, clientSeq, issuedAt, tick, entities, optimisticUi, options) {
  if (!cmd || typeof cmd !== "object") return null;
  if (cmd.c === "train") return optimisticTrain(cmd, clientSeq, issuedAt, tick, entities, optimisticUi, options);
  if (cmd.c === "setRally") return optimisticRally(cmd, clientSeq, issuedAt, tick, entities);
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
    return queue >= entry.optimisticQueue;
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

function errorMessage(err) {
  if (err instanceof Error) return err.message;
  return String(err);
}
