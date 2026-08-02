// The room accepts at most one repair response per connection every 500 ms. Keep the first retry
// beyond that window so a dropped reliable response can be repaired on the very next attempt.
const DEFAULT_RETRY_DELAYS_MS = Object.freeze([600, 1000, 2500, 5000]);

/**
 * Keeps the renderer's local ground-mark cache aligned with the small revision
 * advertised by snapshots. The server remains the source of truth; repeated
 * snapshots only coalesce requests and never advance the applied revision.
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
    this.awaitingPerspectiveSnapshot = false;
    this.destroyed = false;
    this.unsubscribeLabResults = labClient?.subscribeResult?.((result) => {
      if (result?.ok && (result.op === "setVision" || result.op === "importScenario")) {
        this.reset({ awaitSnapshot: true });
      }
    }) || null;
  }

  observeSnapshot(revision) {
    if (this.destroyed || !isRevision(revision)) return false;
    if (this.awaitingPerspectiveSnapshot) this.awaitingPerspectiveSnapshot = false;
    this.targetRevision = Math.max(this.targetRevision, revision);
    return this._ensureRequest();
  }

  applyResponse(message) {
    if (this.destroyed || this.awaitingPerspectiveSnapshot) return false;
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
    this._ensureRequest();
    return true;
  }

  reset({ awaitSnapshot = false, resetPresentation = true } = {}) {
    if (this.destroyed) return;
    this._cancelRetry();
    this.retryIndex = 0;
    this.targetRevision = 0;
    this.outstandingRequestId = null;
    this.awaitingPerspectiveSnapshot = awaitSnapshot;
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
    if (this.awaitingPerspectiveSnapshot || this.retryTimer != null) return false;
    const applied = this.state?.groundDecals?.authoritativeRevision || 0;
    if (applied >= this.targetRevision) return false;
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
