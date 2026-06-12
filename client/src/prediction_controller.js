const U32_MAX = 0xffffffff;
const DEFAULT_COMMAND_TIMEOUT_MS = 15000;

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
    enabled = true,
    now = () => performance.now(),
    commandTimeoutMs = DEFAULT_COMMAND_TIMEOUT_MS,
  } = {}) {
    this.sendCommand = sendCommand;
    this.enabled = !!enabled;
    this.now = now;
    this.commandTimeoutMs = commandTimeoutMs;
    this.mode = this.enabled ? PREDICTION_STATE.TRACKING : PREDICTION_STATE.DISABLED;
    this.nextClientSeq = 1;
    this.pending = [];
    this.pendingBySeq = new Map();
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
  }

  reset({ enabled = this.enabled } = {}) {
    this.enabled = !!enabled;
    this.mode = this.enabled ? PREDICTION_STATE.TRACKING : PREDICTION_STATE.DISABLED;
    this.nextClientSeq = 1;
    this.pending = [];
    this.pendingBySeq.clear();
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
  }

  issueCommand(cmd) {
    if (!this.enabled) {
      return false;
    }
    const clientSeq = this._allocateClientSeq();
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
    this.issuedCount += 1;
    const sent = this.sendCommand ? this.sendCommand(cmd, clientSeq) : false;
    pending.sendAccepted = !!sent;
    return { clientSeq, sent: !!sent };
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

    const netStatus = snapshot.netStatus || {};
    const ackSeq = finiteU32(netStatus.lastSimConsumedClientSeq);
    if (ackSeq != null) {
      const ackTick = finiteU32(netStatus.lastSimConsumedClientTick);
      this.applySimAcknowledgement(ackSeq, ackTick);
    }
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
      correctionCount: this.correctionCount,
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
