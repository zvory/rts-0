import {
  PRESENTATION_OUTCOME,
  immediatePresentationSubmission,
  isTerminalPresentationOutcome,
  outcomeRecord,
  presentationError,
  presentationIdentity,
} from "./submission.js";

const MAX_COMPLETED_IDENTITIES = 256;

export class PresentationCoordinator {
  constructor({
    publishSelectionScene = null,
    acknowledgeGroundDecals = null,
    recordCounter = null,
    recordFailure = null,
    recordProtocolError = null,
  } = {}) {
    this._publishSelectionScene = publishSelectionScene;
    this._acknowledgeGroundDecals = acknowledgeGroundDecals;
    this._recordCounter = recordCounter;
    this._recordFailure = recordFailure;
    this._recordProtocolError = recordProtocolError;
    this._pending = new Map();
    this._completed = new Map();
    this._activeGeneration = null;
    this._lastSubmittedFrameId = 0;
    this._latestPresented = null;
    this._displayedFrameCount = 0;
    this._destroyed = false;
    this._counts = {
      submitted: 0,
      retained: 0,
      presented: 0,
      superseded: 0,
      failed: 0,
      destroyed: 0,
      protocolErrors: 0,
    };
  }

  submit({ frame, selectionScene = null, submission }) {
    const identity = presentationIdentity(frame?.generation, frame?.frameId);
    const groundDecalRevision = nonNegativeInteger(
      frame?.groundDecalRevision ?? 0,
      "frame ground-decal revision",
    );

    if (this._destroyed) {
      return Promise.resolve(outcomeRecord(PRESENTATION_OUTCOME.DESTROYED, identity));
    }
    if (!this._acceptSubmissionOrder(identity)) {
      return Promise.resolve(outcomeRecord(PRESENTATION_OUTCOME.FAILED, identity, {
        error: presentationError(new Error("Presentation submission ordering was rejected.")),
      }));
    }

    const key = identityKey(identity);
    if (this._pending.has(key) || this._completed.has(key)) {
      this._protocolError(`duplicate submission for ${key}`);
      return Promise.resolve(outcomeRecord(PRESENTATION_OUTCOME.FAILED, identity, {
        error: presentationError(new Error(`Duplicate presentation submission ${key}.`)),
      }));
    }

    let resolveCompletion;
    const completion = new Promise((resolve) => { resolveCompletion = resolve; });
    const pending = {
      ...identity,
      selectionScene,
      groundDecalRevision,
      retainedSettled: false,
      terminalSettled: false,
      resolveCompletion,
      completion,
    };
    this._pending.set(key, pending);
    this._count(PRESENTATION_OUTCOME_SUBMITTED);

    const normalized = normalizeSubmission(submission, identity, (message) => this._protocolError(message));
    Promise.resolve(normalized.retained).then(
      (outcome) => this._settleRetainedChannel(identity, outcome),
      (error) => this._rejectRetainedChannel(identity, error),
    );
    Promise.resolve(normalized.settled).then(
      (outcome) => this.acceptTerminal(outcome),
      (error) => this._rejectTerminalChannel(identity, error),
    );
    return completion;
  }

  acceptRetained(outcome) {
    if (this._destroyed) return false;
    const pending = this._pendingForOutcome(outcome, PRESENTATION_OUTCOME.RETAINED);
    if (!pending) return false;
    if (pending.retainedSettled) {
      this._protocolError(`duplicate retained outcome for ${identityKey(outcome)}`);
      return false;
    }
    let revision;
    try {
      revision = nonNegativeInteger(outcome?.groundDecalRevision, "retained ground-decal revision");
    } catch (error) {
      pending.retainedSettled = true;
      this._protocolError(`invalid retained outcome for ${identityKey(outcome)}: ${presentationError(error).message}`);
      this._finishIfSettled(pending);
      return false;
    }
    pending.retainedSettled = true;
    if (revision === 0 || revision !== pending.groundDecalRevision) {
      this._protocolError(
        `retained revision ${revision} does not match frame revision ${pending.groundDecalRevision} for ${identityKey(outcome)}`,
      );
      this._finishIfSettled(pending);
      return false;
    }
    this._count(PRESENTATION_OUTCOME.RETAINED);
    safelyCall(this._acknowledgeGroundDecals, revision);
    this._finishIfSettled(pending);
    return true;
  }

  acceptTerminal(outcome) {
    if (this._destroyed) return false;
    if (!isTerminalPresentationOutcome(outcome?.status)) {
      this._protocolError(`invalid terminal presentation outcome ${JSON.stringify(outcome?.status)}`);
      return false;
    }
    if (this._destroyed && outcome.status === PRESENTATION_OUTCOME.PRESENTED) {
      this._protocolError(`presented-after-destroy outcome for ${identityKey(outcome)}`);
      return false;
    }
    const pending = this._pendingForOutcome(outcome, outcome.status);
    if (!pending) return false;
    if (pending.terminalSettled) {
      this._protocolError(`duplicate terminal outcome for ${identityKey(outcome)}`);
      return false;
    }
    pending.terminalSettled = true;

    let acceptedOutcome = outcome;
    if (outcome.status === PRESENTATION_OUTCOME.PRESENTED) {
      if (this._destroyed) {
        this._protocolError(`presented-after-destroy outcome for ${identityKey(outcome)}`);
        acceptedOutcome = outcomeRecord(PRESENTATION_OUTCOME.DESTROYED, pending);
      } else if (this._latestPresented && compareIdentity(pending, this._latestPresented) <= 0) {
        this._protocolError(`stale presented outcome for ${identityKey(outcome)}`);
        acceptedOutcome = outcomeRecord(PRESENTATION_OUTCOME.FAILED, pending, {
          error: presentationError(new Error("Presented frame is older than the visible frame.")),
        });
      } else {
        this._latestPresented = Object.freeze({ generation: pending.generation, frameId: pending.frameId });
        this._displayedFrameCount += 1;
        safelyCall(this._publishSelectionScene, pending.selectionScene);
      }
    }

    this._count(acceptedOutcome.status);
    if (acceptedOutcome.status === PRESENTATION_OUTCOME.FAILED) {
      safelyCall(this._recordFailure, acceptedOutcome.error || presentationError(new Error("Renderer failed the frame.")));
    }
    pending.resolveCompletion(acceptedOutcome);
    this._finishIfSettled(pending);
    return true;
  }

  resetGeneration(generation) {
    const nextGeneration = positiveInteger(generation, "presentation generation");
    if (this._destroyed || (this._activeGeneration != null && nextGeneration <= this._activeGeneration)) {
      this._protocolError(`invalid presentation generation reset to ${nextGeneration}`);
      return false;
    }
    this._supersedeOlderGenerations(nextGeneration);
    this._activeGeneration = nextGeneration;
    this._lastSubmittedFrameId = 0;
    return true;
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    for (const pending of [...this._pending.values()]) {
      if (!pending.terminalSettled) {
        pending.terminalSettled = true;
        const outcome = outcomeRecord(PRESENTATION_OUTCOME.DESTROYED, pending);
        this._count(PRESENTATION_OUTCOME.DESTROYED);
        pending.resolveCompletion(outcome);
      }
      pending.retainedSettled = true;
      this._rememberCompleted(pending, PRESENTATION_OUTCOME.DESTROYED);
      this._pending.delete(identityKey(pending));
    }
    this._publishSelectionScene = null;
    this._acknowledgeGroundDecals = null;
    this._recordFailure = null;
  }

  diagnostics() {
    return Object.freeze({
      ...this._counts,
      pending: this._pending.size,
      displayedFrameCount: this._displayedFrameCount,
      latestPresented: this._latestPresented,
      activeGeneration: this._activeGeneration,
      destroyed: this._destroyed,
    });
  }

  get displayedFrameCount() {
    return this._displayedFrameCount;
  }

  get latestPresented() {
    return this._latestPresented;
  }

  _acceptSubmissionOrder(identity) {
    if (this._activeGeneration == null) {
      this._activeGeneration = identity.generation;
      this._lastSubmittedFrameId = 0;
    } else if (identity.generation > this._activeGeneration) {
      this._supersedeOlderGenerations(identity.generation);
      this._activeGeneration = identity.generation;
      this._lastSubmittedFrameId = 0;
    } else if (identity.generation < this._activeGeneration) {
      this._protocolError(`stale generation submission for ${identityKey(identity)}`);
      return false;
    }
    if (identity.frameId <= this._lastSubmittedFrameId) {
      this._protocolError(`non-monotonic frame submission for ${identityKey(identity)}`);
      return false;
    }
    this._lastSubmittedFrameId = identity.frameId;
    return true;
  }

  _supersedeOlderGenerations(nextGeneration) {
    for (const pending of [...this._pending.values()]) {
      if (pending.generation >= nextGeneration) continue;
      if (!pending.terminalSettled) {
        pending.terminalSettled = true;
        const outcome = outcomeRecord(PRESENTATION_OUTCOME.SUPERSEDED, pending);
        this._count(PRESENTATION_OUTCOME.SUPERSEDED);
        pending.resolveCompletion(outcome);
      }
      pending.retainedSettled = true;
      this._rememberCompleted(pending, PRESENTATION_OUTCOME.SUPERSEDED);
      this._pending.delete(identityKey(pending));
    }
  }

  _settleRetainedChannel(identity, outcome) {
    if (this._destroyed) return;
    const pending = this._pending.get(identityKey(identity));
    if (!pending) {
      this._lateOutcome(identity, outcome?.status || "empty retained");
      return;
    }
    if (outcome == null) {
      pending.retainedSettled = true;
      this._finishIfSettled(pending);
      return;
    }
    this.acceptRetained(outcome);
  }

  _rejectRetainedChannel(identity, error) {
    if (this._destroyed) return;
    const pending = this._pending.get(identityKey(identity));
    this._protocolError(`retained channel rejected for ${identityKey(identity)}: ${presentationError(error).message}`);
    if (!pending) return;
    pending.retainedSettled = true;
    this._finishIfSettled(pending);
  }

  _rejectTerminalChannel(identity, error) {
    if (this._destroyed) return;
    this._protocolError(`terminal channel rejected for ${identityKey(identity)}: ${presentationError(error).message}`);
    this.acceptTerminal(outcomeRecord(PRESENTATION_OUTCOME.FAILED, identity, {
      error: presentationError(error),
    }));
  }

  _pendingForOutcome(outcome, label) {
    let identity;
    try {
      identity = presentationIdentity(outcome?.generation, outcome?.frameId);
    } catch (error) {
      this._protocolError(`${label} outcome has invalid identity: ${presentationError(error).message}`);
      return null;
    }
    const key = identityKey(identity);
    const pending = this._pending.get(key);
    if (!pending) {
      this._lateOutcome(identity, label);
      return null;
    }
    return pending;
  }

  _lateOutcome(identity, label) {
    const key = identityKey(identity);
    const previous = this._completed.get(key);
    this._protocolError(`${previous ? "duplicate" : "unknown or stale"} ${label} outcome for ${key}`);
  }

  _finishIfSettled(pending) {
    if (!pending.retainedSettled || !pending.terminalSettled) return;
    const key = identityKey(pending);
    this._pending.delete(key);
    this._rememberCompleted(pending, "settled");
  }

  _rememberCompleted(identity, status) {
    const key = identityKey(identity);
    this._completed.delete(key);
    this._completed.set(key, status);
    while (this._completed.size > MAX_COMPLETED_IDENTITIES) {
      this._completed.delete(this._completed.keys().next().value);
    }
  }

  _count(status) {
    const key = status === PRESENTATION_OUTCOME_SUBMITTED ? "submitted" : status;
    if (Object.hasOwn(this._counts, key)) this._counts[key] += 1;
    safelyCall(this._recordCounter, `presentation.frames.${key}`, 1);
  }

  _protocolError(message) {
    this._counts.protocolErrors += 1;
    safelyCall(this._recordCounter, "presentation.protocolErrors", 1);
    safelyCall(this._recordProtocolError, message);
  }
}

const PRESENTATION_OUTCOME_SUBMITTED = "submitted";

function normalizeSubmission(submission, identity, protocolError) {
  if (
    submission?.version === 1
    && submission.generation === identity.generation
    && submission.frameId === identity.frameId
    && typeof submission.retained?.then === "function"
    && typeof submission.settled?.then === "function"
  ) return submission;
  protocolError(`malformed renderer submission for ${identityKey(identity)}`);
  return immediatePresentationSubmission({
    ...identity,
    status: PRESENTATION_OUTCOME.FAILED,
    error: new Error("Renderer returned a malformed presentation submission."),
  });
}

function identityKey(identity) {
  return `${identity.generation}:${identity.frameId}`;
}

function compareIdentity(a, b) {
  return a.generation === b.generation ? a.frameId - b.frameId : a.generation - b.generation;
}

function safelyCall(callback, ...args) {
  if (typeof callback !== "function") return undefined;
  try {
    return callback(...args);
  } catch {
    return undefined;
  }
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new TypeError(`${label} must be a positive integer.`);
  return value;
}

function nonNegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) throw new TypeError(`${label} must be a non-negative integer.`);
  return value;
}
