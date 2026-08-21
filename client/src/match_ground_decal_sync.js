// The room accepts at most one repair response per connection every 500 ms. Keep the first retry
// beyond that window so a dropped reliable response can be repaired on the very next attempt.
const DEFAULT_RETRY_DELAYS_MS = Object.freeze([600, 1000, 2500, 5000]);

/**
 * Keeps the renderer's local ground-mark cache aligned with the small revision
 * advertised by snapshots. Bounded inline deltas handle ordinary discoveries;
 * the reliable request remains the repair path for gaps and cache recovery.
 */
export class GroundDecalSync {
  constructor({
    net,
    state,
    labClient = null,
    resetPresentation = null,
    retryDelaysMs = DEFAULT_RETRY_DELAYS_MS,
    setTimer = (fn, ms) => setTimeout(fn, ms),
    clearTimer = (id) => clearTimeout(id),
  } = {}) {
    this.net = net;
    this.state = state;
    this.resetPresentation = resetPresentation;
    this.retryDelaysMs = retryDelaysMs.length > 0 ? [...retryDelaysMs] : [1000];
    this.setTimer = setTimer;
    this.clearTimer = clearTimer;
    this.targetRevision = 0;
    this.nextRequestId = 1;
    this.outstandingRequestId = null;
    this.retryIndex = 0;
    this.retryTimer = null;
    this.awaitingResetSnapshot = false;
    this.blockInlineDeltaUntilRepair = false;
    this.liveTimelineEstablished = false;
    this.destroyed = false;
    this.unsubscribeLabResults = labClient?.subscribeResult?.((result) => {
      if (result?.ok && (result.op === "setVision" || result.op === "importScenario")) {
        this.reset();
      }
    }) || null;
  }

  observeSnapshot(revision, delta = null) {
    if (this.destroyed || !isRevision(revision)) return false;
    if (this.awaitingResetSnapshot) {
      // Inbound snapshots may already be queued from before a seek or perspective reset. Start a
      // correlated repair after the first one, but keep every inline tail blocked until that repair
      // establishes the replacement authority.
      this.awaitingResetSnapshot = false;
      this.targetRevision = Math.max(this.targetRevision, revision);
      return this._ensureRequest();
    }
    if (!this.blockInlineDeltaUntilRepair && delta && typeof delta === "object") {
      this.state?.groundDecals?.applySnapshotDelta?.(
        {
          revision,
          afterRevision: delta.afterRevision,
          decals: delta.decals,
          tankTrails: delta.tankTrails,
        },
        {
          players: this.state?.players,
          tileSize: this.state?.map?.tileSize,
        },
        { animateInfantryDeath: this.liveTimelineEstablished },
      );
    }
    this.liveTimelineEstablished = true;
    this.targetRevision = Math.max(this.targetRevision, revision);
    if (this.blockInlineDeltaUntilRepair) return this._ensureRequest();
    const applied = this.state?.groundDecals?.authoritativeRevision || 0;
    if (applied >= this.targetRevision) {
      this.outstandingRequestId = null;
      this.retryIndex = 0;
      this._cancelRetry();
      return false;
    }
    return this._ensureRequest();
  }

  applyResponse(message) {
    if (this.destroyed || this.awaitingResetSnapshot) return false;
    if (message?.requestId !== this.outstandingRequestId) return false;
    const result = this.state?.applyAuthoritativeGroundDecals?.(message);
    if (!result?.accepted) return false;
    this.outstandingRequestId = null;
    this.retryIndex = 0;
    this._cancelRetry();
    // A correlated response was projected after this request reached the room actor, so its
    // revision supersedes the snapshot that prompted the request. This matters across observer
    // perspective changes: an old-perspective snapshot may already be in the browser's inbound
    // queue when the local cache resets, while the replacement perspective can legitimately have
    // a lower (including zero) discovery revision.
    this.targetRevision = message.revision;
    this.blockInlineDeltaUntilRepair = false;
    this.liveTimelineEstablished = true;
    if (result.queued === 0) this.resetPresentation?.("complete");
    this._ensureRequest();
    return true;
  }

  // A cleared cache always re-establishes authority through a correlated response. Snapshot tails
  // received before that response may belong to the old perspective or replay time.
  reset({ resetPresentation = true } = {}) {
    if (this.destroyed) return;
    this._cancelRetry();
    this.retryIndex = 0;
    this.targetRevision = 0;
    this.outstandingRequestId = null;
    this.awaitingResetSnapshot = true;
    this.blockInlineDeltaUntilRepair = true;
    this.liveTimelineEstablished = false;
    this.state?.resetAuthoritativeGroundDecals?.();
    if (resetPresentation) this.resetPresentation?.();
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this._cancelRetry();
    this.unsubscribeLabResults?.();
    this.unsubscribeLabResults = null;
  }

  _ensureRequest() {
    if (this.awaitingResetSnapshot || this.retryTimer != null) return false;
    const applied = this.state?.groundDecals?.authoritativeRevision || 0;
    if (!this.blockInlineDeltaUntilRepair && applied >= this.targetRevision) return false;
    if (this.outstandingRequestId == null) {
      this.outstandingRequestId = this.nextRequestId;
      this.nextRequestId = this.nextRequestId === 0xffffffff ? 1 : this.nextRequestId + 1;
    }
    const sent = this.net?.requestGroundDecals?.(this.outstandingRequestId, applied) === true;
    this._scheduleRetry();
    return sent;
  }

  _scheduleRetry() {
    if (this.retryTimer != null || this.destroyed) return;
    const delay = this.retryDelaysMs[Math.min(this.retryIndex, this.retryDelaysMs.length - 1)];
    this.retryIndex = Math.min(this.retryIndex + 1, this.retryDelaysMs.length - 1);
    this.retryTimer = this.setTimer(() => {
      this.retryTimer = null;
      this._ensureRequest();
    }, delay);
  }

  _cancelRetry() {
    if (this.retryTimer == null) return;
    this.clearTimer(this.retryTimer);
    this.retryTimer = null;
  }
}

function isRevision(value) {
  return Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
}
