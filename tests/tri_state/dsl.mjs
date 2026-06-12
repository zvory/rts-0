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

export function waitForSnapshot(options = {}) {
  return { op: "waitForSnapshot", ...options };
}

export function capture(label) {
  return { op: "capture", label };
}

export function assertRemoteClientOwnedPosition(options = {}) {
  return { op: "assertRemoteClientOwnedPosition", tolerancePx: 1, ...options };
}

export function assertOrderPlansMatch(options = {}) {
  return { op: "assertOrderPlansMatch", ...options };
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
