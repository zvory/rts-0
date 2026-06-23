import { S, msg } from "./protocol.js";

const DEFAULT_TIMEOUT_MS = 5000;

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
    this.net.on(S.LAB_STATE, this.onState);
    this.net.on(S.LAB_RESULT, this.onResult);
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

  exportScenario(name = "") {
    const op = { op: "exportScenario" };
    if (name) op.name = name;
    return this.request(op);
  }

  importScenario(scenario) {
    return this.request({ op: "importScenario", scenario });
  }

  spawnEntity({ owner, kind, x, y, completed = true }) {
    return this.request({ op: "spawnEntity", owner, kind, x, y, completed: !!completed });
  }

  deleteEntity(entityId) {
    return this.request({ op: "deleteEntity", entityId });
  }

  moveEntity(entityId, x, y) {
    return this.request({ op: "moveEntity", entityId, x, y });
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
      const result = {
        requestId,
        ok: false,
        op: opName,
        error: "Lab request could not be sent; the socket is not open.",
      };
      this.onResult({ t: S.LAB_RESULT, ...result });
      return Promise.resolve(result);
    }

    return new Promise((resolve) => {
      const timeout = this.timers.setTimeout?.(() => {
        if (!this.pending.has(requestId)) return;
        this.pending.delete(requestId);
        const result = {
          requestId,
          ok: false,
          op: opName,
          error: "Lab request timed out.",
        };
        this.onResult({ t: S.LAB_RESULT, ...result });
        resolve(result);
      }, timeoutMs);
      this.pending.set(requestId, { resolve, timeout });
    });
  }

  allocateRequestId() {
    const id = this.nextRequestId;
    this.nextRequestId += 1;
    if (this.nextRequestId > 0xffffffff) this.nextRequestId = 1;
    return id;
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
      outcome: message?.outcome || null,
    };
    this.lastError = this.lastResult.ok ? "" : this.lastResult.error;
    for (const handler of this.resultSubscribers) handler(this.lastResult);
    pending?.resolve(this.lastResult);
  }

  destroy() {
    this.net.off(S.LAB_STATE, this.onState);
    this.net.off(S.LAB_RESULT, this.onResult);
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
  if (vision.mode === "fullWorld") return "Full world";
  if (vision.mode === "team") return `Team ${vision.teamId}`;
  if (vision.mode === "teams") return `Teams ${(vision.teamIds || []).join(", ")}`;
  return String(vision.mode || "-");
}

export const labVision = Object.freeze({
  fullWorld: () => msg.labVisionFullWorld(),
  team: (teamId) => msg.labVisionTeam(teamId),
  teams: (teamIds) => msg.labVisionTeams(teamIds),
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
