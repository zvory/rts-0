import { LAB_CHECKPOINT_SCENARIO, S, msg } from "./protocol.js";

const DEFAULT_TIMEOUT_MS = 5000;
const LEGACY_LAB_SCENARIO_KIND = "labScenario";

export class LabClient {
  constructor(net, { timeoutMs = DEFAULT_TIMEOUT_MS, timers = globalThis } = {}) {
    this.net = net;
    this.timeoutMs = timeoutMs;
    this.timers = timers;
    this.nextRequestId = 1;
    this.pending = new Map();
    this.state = null;
    this.lastResult = null;
    this.lastError = "";
    this.stateSubscribers = new Set();
    this.resultSubscribers = new Set();
    this.onState = this.onState.bind(this);
    this.onResult = this.onResult.bind(this);
    this.onClose = this.onClose.bind(this);
    this.net.on(S.LAB_STATE, this.onState);
    this.net.on(S.LAB_RESULT, this.onResult);
    this.net.on("close", this.onClose);
  }

  subscribeState(handler) {
    this.stateSubscribers.add(handler);
    if (this.state) handler(this.state);
    return () => this.stateSubscribers.delete(handler);
  }

  subscribeResult(handler) {
    this.resultSubscribers.add(handler);
    if (this.lastResult) handler(this.lastResult);
    return () => this.resultSubscribers.delete(handler);
  }

  setInitialState(state) {
    if (!state || typeof state !== "object") return;
    this.onState({ t: S.LAB_STATE, ...state });
  }

  setVision(vision) {
    return this.request({ op: "setVision", vision });
  }

  exportMap() {
    return this.request({ op: "exportMap" });
  }

  // Wire compatibility names for checkpoint setup import/export. Product UI should label these
  // as setup checkpoint actions; lab replay save/open must use a separate replay-artifact path.
  exportScenario(name = "") {
    const op = { op: "exportScenario" };
    if (name) op.name = name;
    return this.request(op);
  }

  importScenario(scenario) {
    const rejection = labScenarioImportRejection(scenario);
    if (rejection) return this.failLocalRequest("importScenario", rejection);
    return this.request({ op: "importScenario", scenario });
  }

  validateScenario(metadata) {
    return this.request({ op: "validateScenario", metadata });
  }

  submitScenario(metadata, options = {}) {
    return this.request({ op: "submitScenario", metadata }, options);
  }

  resetScenario() {
    return this.net.seekRoomTimeTo(0);
  }

  spawnEntity({ owner, kind, x, y, completed = true }) {
    return this.request({ op: "spawnEntity", owner, kind, x, y, completed: !!completed });
  }

  spawnEntities(spawns) {
    return this.request({ op: "spawnEntities", spawns });
  }

  deleteEntity(entityId) {
    return this.request({ op: "deleteEntity", entityId });
  }

  deleteEntities(entityIds) {
    return this.request({ op: "deleteEntities", entityIds });
  }

  moveEntity(entityId, x, y) {
    return this.request({ op: "moveEntity", entityId, x, y });
  }

  applyUpdates(updates) {
    return this.request({ op: "applyUpdates", updates });
  }

  setEntityOwner(entityId, owner) {
    return this.request({ op: "setEntityOwner", entityId, owner });
  }

  setPlayerResources(playerId, steel, oil) {
    return this.request({ op: "setPlayerResources", playerId, steel, oil });
  }

  setPlayerGodMode(playerId, enabled) {
    return this.request({ op: "setPlayerGodMode", playerId, enabled: !!enabled });
  }

  setCompletedResearch(playerId, upgrade, completed) {
    return this.request({ op: "setCompletedResearch", playerId, upgrade, completed: !!completed });
  }

  request(op, { timeoutMs = this.timeoutMs } = {}) {
    const requestId = this.allocateRequestId();
    const opName = typeof op?.op === "string" ? op.op : "unknown";
    const sent = this.net.lab(requestId, op);
    if (!sent) {
      this.onResult({
        t: S.LAB_RESULT,
        requestId,
        ok: false,
        op: opName,
        error: "Lab request could not be sent; the socket is not open.",
      });
      return Promise.resolve(this.lastResult);
    }

    return new Promise((resolve) => {
      const timeout = this.timers.setTimeout?.(() => {
        this.completeLocalRequest(requestId, opName, "Lab request timed out.");
      }, timeoutMs);
      this.pending.set(requestId, { resolve, timeout, opName });
    });
  }

  allocateRequestId() {
    const id = this.nextRequestId;
    this.nextRequestId += 1;
    if (this.nextRequestId > 0xffffffff) this.nextRequestId = 1;
    return id;
  }

  failLocalRequest(opName, error) {
    const requestId = this.allocateRequestId();
    this.onResult({
      t: S.LAB_RESULT,
      requestId,
      ok: false,
      op: opName,
      error,
    });
    return Promise.resolve(this.lastResult);
  }

  onState(message) {
    this.state = {
      room: message?.room || "",
      operatorId: Number.isFinite(message?.operatorId) ? message.operatorId : null,
      role: message?.role || "",
      vision: message?.vision || null,
      godModePlayers: normalizePlayerIds(message?.godModePlayers),
      dirty: !!message?.dirty,
      operationCount: Number.isFinite(message?.operationCount) ? message.operationCount : 0,
    };
    for (const handler of this.stateSubscribers) handler(this.state);
  }

  onResult(message) {
    const requestId = Number(message?.requestId);
    const pending = this.pending.get(requestId);
    if (pending) {
      this.pending.delete(requestId);
      if (pending.timeout != null) this.timers.clearTimeout?.(pending.timeout);
    }
    this.lastResult = {
      requestId,
      ok: !!message?.ok,
      op: message?.op || "",
      error: message?.error || "",
      failedIndex: Number.isInteger(message?.failedIndex) ? message.failedIndex : null,
      details: message?.details && typeof message.details === "object" ? message.details : null,
      outcome: message?.outcome || null,
    };
    this.lastError = this.lastResult.ok ? "" : this.lastResult.error;
    for (const handler of this.resultSubscribers) handler(this.lastResult);
    pending?.resolve(this.lastResult);
  }

  onClose() {
    for (const [requestId, pending] of Array.from(this.pending.entries())) {
      this.completeLocalRequest(
        requestId,
        pending.opName || "",
        "Lab request could not complete; the socket disconnected.",
      );
    }
  }

  completeLocalRequest(requestId, opName, error) {
    if (!this.pending.has(requestId)) return;
    this.onResult({
      t: S.LAB_RESULT,
      requestId,
      ok: false,
      op: opName,
      error,
    });
  }

  destroy() {
    this.net.off(S.LAB_STATE, this.onState);
    this.net.off(S.LAB_RESULT, this.onResult);
    this.net.off("close", this.onClose);
    for (const [requestId, pending] of this.pending) {
      if (pending.timeout != null) this.timers.clearTimeout?.(pending.timeout);
      pending.resolve({
        requestId,
        ok: false,
        op: "",
        error: "Lab client was destroyed before the request completed.",
      });
    }
    this.pending.clear();
    this.stateSubscribers.clear();
    this.resultSubscribers.clear();
  }
}

export function labVisionLabel(vision) {
  if (!vision || typeof vision !== "object") return "-";
  if (vision.mode === "all") return "Full";
  if (vision.mode === "team") return `Team ${vision.teamId}`;
  return String(vision.mode || "-");
}

export const labVision = Object.freeze({
  all: () => msg.labVisionAll(),
  team: (teamId) => msg.labVisionTeam(teamId),
});

function normalizePlayerIds(ids) {
  if (!Array.isArray(ids)) return [];
  const seen = new Set();
  const out = [];
  for (const value of ids) {
    const id = Number(value);
    if (!Number.isInteger(id) || id <= 0 || seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out.sort((a, b) => a - b);
}

function labScenarioImportRejection(scenario) {
  if (!scenario || typeof scenario !== "object") {
    return "Lab setup import expects a checkpoint-backed setup JSON object.";
  }
  if (scenario.kind === LEGACY_LAB_SCENARIO_KIND) {
    return "Legacy labScenario JSON is no longer supported; export a checkpoint-backed lab setup from a current build.";
  }
  if (typeof scenario.kind === "string" && scenario.kind !== LAB_CHECKPOINT_SCENARIO.KIND) {
    return `Lab setup import expects kind ${JSON.stringify(LAB_CHECKPOINT_SCENARIO.KIND)}.`;
  }
  return "";
}
