import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { ownEntityByKind } from "../diffs.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const GLUE_PATH = path.join(REPO_ROOT, "client/vendor/sim-wasm/rts_sim_wasm.js");
const WASM_PATH = path.join(REPO_ROOT, "client/vendor/sim-wasm/rts_sim_wasm_bg.wasm");

export class WasmLocalLane {
  constructor({ scenario, artifacts, gluePath = GLUE_PATH, wasmPath = WASM_PATH } = {}) {
    this.scenario = scenario || null;
    this.artifacts = artifacts || null;
    this.gluePath = gluePath;
    this.wasmPath = wasmPath;
    this.module = null;
    this.predictor = null;
    this.playerId = null;
    this.startInfo = null;
    this.selection = [];
    this.issuedCommands = [];
    this.frames = [];
    this.disabledReason = null;
    this.ready = false;
  }

  async start({ startInfo = null, playerId = null, baselineSnapshot = null } = {}) {
    this.startInfo = startInfo;
    this.playerId = playerId ?? startInfo?.playerId ?? null;
    if (!this.startInfo || this.playerId == null) {
      this.disable("startPayloadUnavailable");
      return this.record("start", { ready: false, disabledReason: this.disabledReason });
    }
    if (!fs.existsSync(this.gluePath) || !fs.existsSync(this.wasmPath)) {
      this.disable("wasmAssetsMissing");
      return this.record("start", {
        ready: false,
        disabledReason: this.disabledReason,
        detail: "run scripts/build-sim-wasm.sh",
      });
    }
    try {
      const module = await import(pathToFileURL(this.gluePath).href);
      await module.default({ module_or_path: fs.readFileSync(this.wasmPath) });
      this.module = module;
      this.predictor = module.WasmPredictor.fromStartJson(JSON.stringify(this.startInfo), this.playerId);
      this.ready = true;
      const frame = this.record("start", {
        ready: true,
        summary: this.summary(),
        diagnostics: this.diagnostics(),
      });
      if (baselineSnapshot && this.scenario?.setup?.localBaseline !== "none") {
        this.importBaseline(baselineSnapshot, "initial-authoritative");
      }
      return frame;
    } catch (err) {
      this.disable(errorMessage(err));
      return this.record("start", { ready: false, disabledReason: this.disabledReason });
    }
  }

  async applyStep(step) {
    if (step?.op === "waitForSnapshot") {
      return this.advanceTicks(step.minTickDelta ?? 1, "waitForSnapshot");
    }
    return this.record("step", { op: step?.op || null, summary: this.summary(), diagnostics: this.diagnostics() });
  }

  importBaseline(snapshot, label = "authoritative") {
    if (!this.ready || !this.predictor || !this.module || !snapshot) {
      return this.record("baseline", {
        label,
        imported: false,
        disabledReason: this.disabledReason || "predictorUnavailable",
      });
    }
    const baselineJson = this.module.WasmPredictor.baselineFromSnapshotJson(
      JSON.stringify(snapshot),
      this.playerId,
    );
    const baseline = JSON.parse(baselineJson);
    this.predictor.importBaselineJson(baselineJson);
    return this.record("baseline", {
      label,
      imported: true,
      baselineSummary: summarizeBaseline(baseline),
      summary: this.summary(),
      diagnostics: this.diagnostics(),
    });
  }

  async selectOwn(kind, index = 0) {
    const entity = ownEntityByKind(this.summary(), kind, index);
    if (!entity) {
      this.selection = [];
      return this.record("select", { kind, index, selected: null, disabledReason: this.disabledReason });
    }
    this.selection = [{ kind, index, id: entity.id }];
    return this.record("select", { kind, index, selected: this.selection[0] });
  }

  async issue(command, args = {}, issued = {}) {
    const clientSeq = issued.clientSeq;
    const cmd = issued.command || this.commandForSelection(command, args);
    if (!cmd || clientSeq == null) {
      return this.record("command.skipped", {
        command,
        reason: this.disabledReason || "commandUnavailable",
      });
    }
    this.issuedCommands.push({
      clientSeq,
      kind: cmd.c,
      issueStep: this.issuedCommands.length,
      command: cmd,
    });
    if (this.ready && this.predictor) {
      this.predictor.enqueueCommandJson(clientSeq, JSON.stringify(cmd));
    }
    return this.record("command.issued", {
      clientSeq,
      command: cmd,
      summary: this.summary(),
      diagnostics: this.diagnostics(),
    });
  }

  advanceTicks(ticks = 1, reason = "explicit") {
    const count = Math.max(0, Number(ticks) || 0);
    if (this.ready && this.predictor && count > 0) {
      this.predictor.advanceTicks(count);
    }
    return this.record("advance", {
      ticks: count,
      reason,
      summary: this.summary(),
      diagnostics: this.diagnostics(),
    });
  }

  async capture(label) {
    return this.record("capture", {
      label,
      summary: this.summary(),
      renderSnapshot: this.renderSnapshotSummary(),
      diagnostics: this.diagnostics(),
    }).summary;
  }

  summary() {
    if (!this.ready || !this.predictor) {
      return {
        lane: "local",
        ready: false,
        playerId: this.playerId,
        tick: this.startInfo?.tick ?? null,
        owned: [],
        entities: [],
        pendingCommands: 0,
        pendingClientSeqs: [],
        correctionMagnitude: null,
        unsupportedFields: [],
        disabledReasons: this.disabledReason ? [this.disabledReason] : ["predictorUnavailable"],
        unsupported: true,
      };
    }
    const raw = JSON.parse(this.predictor.localLaneSummaryJson());
    const entities = (raw.ownedEntities || []).map((entity) => ({
      id: entity.id,
      owner: raw.playerId,
      kind: entity.kind,
      x: round(entity.x),
      y: round(entity.y),
      hp: null,
      state: entity.state,
      orderPlan: summarizePlan([
        ...(entity.orderPlan || []),
        ...(entity.queuedOrderStages || []),
      ]),
      activeOrderPlan: summarizePlan(entity.orderPlan || []),
      queuedOrderStages: summarizePlan(entity.queuedOrderStages || []),
      rallyPlan: [],
    }));
    return {
      lane: "local",
      ready: true,
      tick: raw.tick ?? null,
      playerId: raw.playerId ?? this.playerId,
      steel: { unsupported: true },
      oil: { unsupported: true },
      supplyUsed: { unsupported: true },
      supplyCap: { unsupported: true },
      netStatus: { unsupported: true },
      entities,
      owned: entities,
      pendingCommands: raw.pendingCommands ?? 0,
      pendingClientSeqs: raw.pendingClientSeqs || [],
      correctionMagnitude: round(raw.correctionMagnitude),
      unsupportedFields: raw.unsupportedFields || [],
      disabledReasons: raw.disabledReasons || [],
      issuedCommands: this.issuedCommands.map(compactIssuedCommand),
    };
  }

  diagnostics() {
    if (!this.ready || !this.predictor) {
      return {
        ready: false,
        disabledReason: this.disabledReason || "predictorUnavailable",
      };
    }
    return { ready: true, ...JSON.parse(this.predictor.diagnosticsJson()) };
  }

  renderSnapshotSummary() {
    if (!this.ready || !this.predictor) return null;
    const snapshot = JSON.parse(this.predictor.renderSnapshotJson());
    return {
      tick: snapshot.tick,
      entityCount: snapshot.entities?.length || 0,
      entities: (snapshot.entities || []).map((entity) => ({
        id: entity.id,
        owner: entity.owner,
        kind: entity.kind,
        x: round(entity.x),
        y: round(entity.y),
        state: entity.state,
        orderPlan: summarizePlan(entity.orderPlan || []),
      })),
    };
  }

  commandForSelection(command, args = {}) {
    const unit = this.selectedEntity();
    if (!unit) return null;
    if (command === "stop") return { c: "stop", units: [unit.id] };
    if (command === "build") {
      return {
        c: "build",
        units: [unit.id],
        building: args.building || "depot",
        tileX: args.tileX ?? 1,
        tileY: args.tileY ?? 1,
        queued: !!args.queued,
      };
    }
    if (command === "attack") {
      return { c: "attack", units: [unit.id], target: args.target ?? 999999999, queued: !!args.queued };
    }
    if (command !== "move" && command !== "attackMove" && command !== "invalidMove") return null;
    return {
      c: command === "invalidMove" ? "move" : command,
      units: command === "invalidMove" ? [999999999] : [unit.id],
      x: args.x ?? unit.x + (args.dx ?? 0),
      y: args.y ?? unit.y + (args.dy ?? 0),
      queued: !!args.queued,
    };
  }

  selectedEntity() {
    const selected = this.selection[0];
    if (!selected) return null;
    return ownEntityByKind(this.summary(), selected.kind, selected.index);
  }

  disable(reason) {
    this.disabledReason = reason || "predictorUnavailable";
    this.ready = false;
  }

  record(event, extra = {}) {
    const frame = {
      event,
      localLane: "wasm",
      ready: this.ready,
      disabledReason: this.disabledReason,
      ...extra,
    };
    this.frames.push(frame);
    this.artifacts?.local(frame);
    return frame;
  }

  async close() {
    if (this.predictor && typeof this.predictor.free === "function") {
      this.predictor.free();
    }
    this.predictor = null;
    this.ready = false;
  }
}

function summarizeBaseline(baseline) {
  return {
    tick: baseline.tick,
    playerId: baseline.playerId,
    ownedEntityCount: baseline.ownedEntities?.length || 0,
    visibleObstacleCount: baseline.visibleObstacles?.length || 0,
    ownedEntities: (baseline.ownedEntities || []).map((entity) => ({
      id: entity.id,
      kind: entity.kind,
      x: round(entity.x),
      y: round(entity.y),
      orderPlan: summarizePlan(entity.orderPlan || []),
    })),
    visibleObstacles: (baseline.visibleObstacles || []).map((obstacle) => ({
      kind: obstacle.kind,
      x: round(obstacle.x),
      y: round(obstacle.y),
      radius: round(obstacle.radius),
    })),
  };
}

function summarizePlan(plan) {
  return (plan || []).map((stage) => ({
    kind: stage.kind,
    x: round(stage.x),
    y: round(stage.y),
  }));
}

function compactIssuedCommand(entry) {
  return {
    clientSeq: entry.clientSeq,
    kind: entry.kind,
    issueStep: entry.issueStep,
  };
}

function round(value) {
  return Number.isFinite(value) ? Math.round(value * 100) / 100 : value;
}

function errorMessage(err) {
  if (err instanceof Error) return err.message;
  return String(err);
}
