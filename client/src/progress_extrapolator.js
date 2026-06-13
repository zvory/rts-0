import { STATS, TICK_HZ, UPGRADES } from "./config.js";
import { STATE, isBuilding } from "./protocol.js";

export const PROGRESS_EXTRAPOLATION_MAX = 0.98;

export class ProgressExtrapolator {
  constructor({ playerId }) {
    this.playerId = playerId;
    this.active = new Map();
    this.correctionCount = 0;
    this.totalCorrection = 0;
    this.maxCorrection = 0;
    this.lastCorrection = 0;
  }

  updateFromSnapshot(entities, recvTime) {
    const next = new Map();
    for (const entity of entities || []) {
      const baseline = this._baselineFor(entity, recvTime);
      if (!baseline) continue;
      const prior = this.active.get(entity.id);
      if (prior && prior.identity === baseline.identity) {
        const predicted = this.progressFor(entity, recvTime);
        const correction = Math.abs(predicted - baseline.progress);
        this.lastCorrection = correction;
        this.totalCorrection += correction;
        this.maxCorrection = Math.max(this.maxCorrection, correction);
        this.correctionCount += 1;
      }
      next.set(entity.id, baseline);
    }
    this.active = next;
    return this.active.size;
  }

  progressFor(entity, now) {
    const baseline = this.active.get(entity?.id);
    if (!baseline || !this._matches(entity, baseline)) return safeProgress(entity?.prodProgress);
    const elapsedMs = Math.max(0, now - baseline.recvTime);
    const durationMs = Math.max(1, (baseline.durationTicks / TICK_HZ) * 1000);
    const progress = baseline.progress + elapsedMs / durationMs;
    return Math.min(PROGRESS_EXTRAPOLATION_MAX, Math.max(baseline.progress, progress));
  }

  apply(entity, now) {
    if (!entity || !this.active.has(entity.id)) return entity;
    const progress = this.progressFor(entity, now);
    if (!Number.isFinite(progress) || progress <= safeProgress(entity.prodProgress)) return entity;
    return { ...entity, prodProgress: progress, progressPredicted: true };
  }

  diagnostics() {
    return {
      activeBars: this.active.size,
      correctionCount: this.correctionCount,
      lastCorrection: round(this.lastCorrection),
      maxCorrection: round(this.maxCorrection),
      averageCorrection: this.correctionCount > 0
        ? round(this.totalCorrection / this.correctionCount)
        : 0,
    };
  }

  _baselineFor(entity, recvTime) {
    if (!entity || entity.owner !== this.playerId || !isBuilding(entity.kind)) return null;
    if (entity.state === STATE.CONSTRUCT || entity.buildProgress != null) return null;
    const queue = finitePositiveInt(entity.prodQueue);
    if (queue == null) return null;
    const progress = safeProgress(entity.prodProgress);
    if (!(progress >= 0 && progress < 1)) return null;
    const identity = activeIdentity(entity);
    if (!identity) return null;
    const durationTicks = durationTicksFor(identity);
    if (!(durationTicks > 0)) return null;
    return { id: entity.id, identity, queue, progress, durationTicks, recvTime };
  }

  _matches(entity, baseline) {
    return !!entity &&
      entity.owner === this.playerId &&
      isBuilding(entity.kind) &&
      entity.state !== STATE.CONSTRUCT &&
      entity.buildProgress == null &&
      finitePositiveInt(entity.prodQueue) === baseline.queue &&
      safeProgress(entity.prodProgress) < 1 &&
      activeIdentity(entity) === baseline.identity;
  }
}

function activeIdentity(entity) {
  if (typeof entity?.prodUpgrade === "string" && entity.prodUpgrade) return `upgrade:${entity.prodUpgrade}`;
  if (typeof entity?.prodKind === "string" && entity.prodKind) return `unit:${entity.prodKind}`;
  return null;
}

function durationTicksFor(identity) {
  const [type, kind] = identity.split(":");
  if (type === "upgrade") return finiteTicks(UPGRADES[kind]?.researchTicks);
  if (type === "unit") return finiteTicks(STATS[kind]?.buildTicks);
  return null;
}

function finiteTicks(value) {
  return Number.isFinite(value) && value > 0 ? value : null;
}

function finitePositiveInt(value) {
  return Number.isInteger(value) && value > 0 ? value : null;
}

function safeProgress(value) {
  return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : NaN;
}

function round(value) {
  return Number.isFinite(value) ? Math.round(value * 10000) / 10000 : value;
}
