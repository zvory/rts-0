export function scenario(name, definition) {
  if (!/^[a-z0-9][a-z0-9_-]*$/.test(name || "")) {
    throw new Error(`invalid scenario name: ${name}`);
  }
  const setup = definition.setup || {};
  const steps = definition.steps || [];
  if (!Array.isArray(steps) || steps.length === 0) {
    throw new Error(`scenario ${name} must define at least one step`);
  }
  return {
    name,
    setup: {
      kind: setup.kind || "liveRoom",
      players: setup.players ?? 1,
      prediction: setup.prediction || "disabled",
      quickstart: setup.quickstart !== false,
      devScenario: setup.devScenario || null,
      localBaseline: setup.localBaseline || "initial",
    },
    network: definition.network || { mode: "direct" },
    steps,
  };
}

export function selectOwn(kind, index = 0) {
  return { op: "selectOwn", kind, index };
}

export function issue(command, args = {}) {
  return { op: "issue", command, args };
}

export function issueBurst(commands) {
  if (!Array.isArray(commands) || commands.length === 0) {
    throw new Error("issueBurst requires at least one command");
  }
  return { op: "issueBurst", commands };
}

export function waitForSnapshot(options = {}) {
  return { op: "waitForSnapshot", ...options };
}

export function waitMs(ms) {
  return { op: "waitMs", ms };
}

export function waitForAck(clientSeq, options = {}) {
  return { op: "waitForAck", clientSeq, ...options };
}

export function capture(label) {
  return { op: "capture", label };
}

export function importLocalBaseline(options = {}) {
  return { op: "importLocalBaseline", source: "client", ...options };
}

export function advanceLocalTicks(ticks) {
  return { op: "advanceLocalTicks", ticks };
}

export function assertRemoteClientOwnedPosition(options = {}) {
  return { op: "assertRemoteClientOwnedPosition", tolerancePx: 1, ...options };
}

export function assertClientAuthoritativeOwnedStable(options = {}) {
  return { op: "assertClientAuthoritativeOwnedStable", tolerancePx: 0.01, ...options };
}

export function assertClientRenderedOwnedAdvanced(options = {}) {
  return { op: "assertClientRenderedOwnedAdvanced", minDistancePx: 1, ...options };
}

export function assertClientRenderedOwnedStable(options = {}) {
  return { op: "assertClientRenderedOwnedStable", tolerancePx: 0.01, ...options };
}

export function assertClientRenderedConverged(options = {}) {
  return { op: "assertClientRenderedConverged", tolerancePx: 2, ...options };
}

export function assertOrderPlansMatch(options = {}) {
  return { op: "assertOrderPlansMatch", ...options };
}

export function assertLocalReady(options = {}) {
  return { op: "assertLocalReady", ...options };
}

export function assertLocalDisabledReason(reason) {
  return { op: "assertLocalDisabledReason", reason };
}

export function assertLocalUnsupportedField(field) {
  return { op: "assertLocalUnsupportedField", field };
}

export function assertLocalRenderOwnedOnly() {
  return { op: "assertLocalRenderOwnedOnly" };
}

export function assertLocalOwnedStable(options = {}) {
  return { op: "assertLocalOwnedStable", ...options };
}

export function assertLocalOwnedAdvanced(options = {}) {
  return { op: "assertLocalOwnedAdvanced", ...options };
}

export function assertLocalOrderPlan(options = {}) {
  return { op: "assertLocalOrderPlan", ...options };
}

export function assertLocalPendingClientSeqs(seqs) {
  return { op: "assertLocalPendingClientSeqs", seqs };
}

export function assertLocalCorrectionAtMost(maxPx) {
  return { op: "assertLocalCorrectionAtMost", maxPx };
}

export function assertClientCorrectionBudget(options = {}) {
  return {
    op: "assertClientCorrectionBudget",
    maxPx: options.maxPx ?? 256,
    maxSnapCorrections: options.maxSnapCorrections ?? 4,
    ...options,
  };
}

export function assertLocalBaselineOwnerSafe() {
  return { op: "assertLocalBaselineOwnerSafe" };
}

export function assertClientSeqsStrictlyIncreasing(options = {}) {
  return { op: "assertClientSeqsStrictlyIncreasing", ...options };
}

export function assertClientPrediction(options = {}) {
  return { op: "assertClientPrediction", ...options };
}

export function assertClientOptimisticUi(options = {}) {
  return { op: "assertClientOptimisticUi", ...options };
}

export function assertClientRenderedProductionProgress(options = {}) {
  return { op: "assertClientRenderedProductionProgress", ...options };
}

export function waitForClientPredictionReady(options = {}) {
  return { op: "waitForClientPredictionReady", timeoutMs: 8000, ...options };
}

export function advanceClientPredictionVisual(options = {}) {
  return { op: "advanceClientPredictionVisual", ...options };
}

export function injectClientSnapshot(kind, options = {}) {
  return { op: "injectClientSnapshot", kind, ...options };
}

export function setClientSnapshotDelivery(enabled) {
  return { op: "setClientSnapshotDelivery", enabled: !!enabled };
}

export function recordSocketReceipt(clientSeq, detail = {}) {
  return { op: "recordSocketReceipt", clientSeq, detail };
}

export function recordCommandRejection(clientSeq, reason = "test rejection diagnostic") {
  return { op: "recordCommandRejection", clientSeq, reason };
}

export function expireClientCommands(options = {}) {
  return { op: "expireClientCommands", ...options };
}

export function setReplaySpeed(speed) {
  return { op: "setReplaySpeed", speed };
}

export function stepDevTick() {
  return { op: "stepDevTick" };
}

export function assertTickAdvanced(options = {}) {
  return { op: "assertTickAdvanced", delta: 1, ...options };
}

export function forceFailure(message = "forced failure artifact check") {
  return { op: "forceFailure", message };
}

export function serializableScenario(s) {
  return {
    name: s.name,
    setup: s.setup,
    network: s.network,
    steps: s.steps,
  };
}
