export const PRESENTATION_OUTCOME = Object.freeze({
  RETAINED: "retained",
  PRESENTED: "presented",
  SUPERSEDED: "superseded",
  FAILED: "failed",
  DESTROYED: "destroyed",
});

const TERMINAL_OUTCOMES = new Set([
  PRESENTATION_OUTCOME.PRESENTED,
  PRESENTATION_OUTCOME.SUPERSEDED,
  PRESENTATION_OUTCOME.FAILED,
  PRESENTATION_OUTCOME.DESTROYED,
]);

export function createPresentationSubmission({ generation, frameId, retained, settled }) {
  const identity = presentationIdentity(generation, frameId);
  if (!isPromiseLike(retained) || !isPromiseLike(settled)) {
    throw new TypeError("Presentation submissions require retained and settled promises.");
  }
  return Object.freeze({
    version: 1,
    ...identity,
    retained,
    settled,
  });
}

export function immediatePresentationSubmission({
  generation,
  frameId,
  retainedRevision = 0,
  status,
  error = null,
}) {
  const identity = presentationIdentity(generation, frameId);
  if (!TERMINAL_OUTCOMES.has(status)) {
    throw new TypeError(`Invalid terminal presentation outcome ${JSON.stringify(status)}.`);
  }
  const revision = nonNegativeInteger(retainedRevision, "retained ground-decal revision");
  const retained = revision > 0
    ? outcomeRecord(PRESENTATION_OUTCOME.RETAINED, identity, { groundDecalRevision: revision })
    : null;
  const settled = outcomeRecord(status, identity, error ? { error: presentationError(error) } : {});
  return createPresentationSubmission({
    ...identity,
    retained: Promise.resolve(retained),
    settled: Promise.resolve(settled),
  });
}

export function outcomeRecord(status, identity, fields = {}) {
  if (status !== PRESENTATION_OUTCOME.RETAINED && !TERMINAL_OUTCOMES.has(status)) {
    throw new TypeError(`Invalid presentation outcome ${JSON.stringify(status)}.`);
  }
  return Object.freeze({
    status,
    ...presentationIdentity(identity?.generation, identity?.frameId),
    ...fields,
  });
}

export function presentationError(error) {
  const name = typeof error?.name === "string" && error.name ? error.name : "Error";
  const message = typeof error?.message === "string" && error.message
    ? error.message
    : String(error || "Unknown presentation failure");
  return Object.freeze({ name, message: message.slice(0, 1000) });
}

export function isTerminalPresentationOutcome(status) {
  return TERMINAL_OUTCOMES.has(status);
}

export function presentationIdentity(generation, frameId) {
  return Object.freeze({
    generation: positiveInteger(generation, "presentation generation"),
    frameId: positiveInteger(frameId, "presentation frame id"),
  });
}

function isPromiseLike(value) {
  return value != null && typeof value.then === "function";
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new TypeError(`${label} must be a positive integer.`);
  return value;
}

function nonNegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) throw new TypeError(`${label} must be a non-negative integer.`);
  return value;
}
