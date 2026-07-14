import { STATS, TICK_HZ, UPGRADES } from "./config.js";
import { STATE, isBuilding } from "./protocol.js";

export const PROGRESS_EXTRAPOLATION_MAX = 0.98;

export class ProgressExtrapolator {
  constructor({ playerId }) {
    this.playerId = playerId;
    this.active = new Map();
    this.pausedAt = null;
    this.accumulatedPausedMs = 0;
    this.correctionCount = 0;
    this.totalCorrection = 0;
    this.maxCorrection = 0;
    this.lastCorrection = 0;
  }

  updateFromSnapshot(entities, recvTime) {
    const activeRecvTime = this._activeTime(recvTime);
    const next = new Map();
    for (const entity of entities || []) {
      const baseline = this._baselineFor(entity, activeRecvTime);
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
    if (!baseline || !this._matches(entity, baseline)) return safeProgress(currentProgress(entity, baseline));
    const elapsedMs = Math.max(0, this._activeTime(now) - baseline.recvTime);
    const durationMs = Math.max(1, (baseline.durationTicks / TICK_HZ) * 1000);
    const progress = baseline.progress + elapsedMs / durationMs;
    return Math.min(PROGRESS_EXTRAPOLATION_MAX, Math.max(baseline.progress, progress));
  }

  apply(entity, now) {
    if (!entity || !this.active.has(entity.id)) return entity;
    const baseline = this.active.get(entity.id);
    const progress = this.progressFor(entity, now);
    if (!Number.isFinite(progress) || progress <= safeProgress(currentProgress(entity, baseline))) return entity;
    if (baseline.type === "construction") {
      return { ...entity, buildProgress: progress, progressPredicted: true, buildProgressPredicted: true };
    }
    return { ...entity, prodProgress: progress, progressPredicted: true };
  }

  setPaused(paused, now) {
    const wallTime = finiteTime(now);
    if (paused) {
      if (this.pausedAt == null) this.pausedAt = wallTime;
      return;
    }
    if (this.pausedAt == null) return;
    this.accumulatedPausedMs += Math.max(0, wallTime - this.pausedAt);
    this.pausedAt = null;
  }

  diagnostics() {
    let productionBars = 0;
    let constructionBars = 0;
    for (const baseline of this.active.values()) {
      if (baseline.type === "construction") constructionBars += 1;
      else productionBars += 1;
    }
    return {
      activeBars: this.active.size,
      paused: this.pausedAt != null,
      productionBars,
      constructionBars,
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
    const construction = constructionBaseline(entity, recvTime);
    if (construction) return construction;
    if (entity.state === STATE.CONSTRUCT || entity.buildProgress != null) return null;
    if (entity.prodWaiting === true) return null;
    const queue = finitePositiveInt(entity.prodQueue);
    if (queue == null) return null;
    const progress = safeProgress(entity.prodProgress);
    if (!(progress >= 0 && progress < 1)) return null;
    const identity = activeIdentity(entity);
    if (!identity) return null;
    const durationTicks = durationTicksFor(identity);
    if (!(durationTicks > 0)) return null;
    return { id: entity.id, type: "production", identity, queue, progress, durationTicks, recvTime };
  }

  _activeTime(now) {
    const wallTime = finiteTime(now);
    const effectiveWallTime = this.pausedAt == null ? wallTime : Math.min(wallTime, this.pausedAt);
    return effectiveWallTime - this.accumulatedPausedMs;
  }

  _matches(entity, baseline) {
    if (baseline?.type === "construction") return constructionMatches(entity, baseline, this.playerId);
    return !!entity &&
      entity.owner === this.playerId &&
      isBuilding(entity.kind) &&
      entity.state !== STATE.CONSTRUCT &&
      entity.buildProgress == null &&
      entity.prodWaiting !== true &&
      finitePositiveInt(entity.prodQueue) === baseline.queue &&
      safeProgress(entity.prodProgress) < 1 &&
      activeIdentity(entity) === baseline.identity;
  }
}

function constructionBaseline(entity, recvTime) {
  if (entity.buildActive !== true) return null;
  const progress = safeProgress(entity.buildProgress);
  if (!(progress >= 0 && progress < 1)) return null;
  const durationTicks = finiteTicks(STATS[entity.kind]?.buildTicks);
  if (!(durationTicks > 0)) return null;
  return {
    id: entity.id,
    type: "construction",
    identity: `build:${entity.kind}`,
    progress,
    durationTicks,
    recvTime,
  };
}

function constructionMatches(entity, baseline, playerId) {
  return !!entity &&
    entity.owner === playerId &&
    isBuilding(entity.kind) &&
    entity.buildActive === true &&
    safeProgress(entity.buildProgress) < 1 &&
    `build:${entity.kind}` === baseline.identity;
}

function currentProgress(entity, baseline) {
  return baseline?.type === "construction" ? entity?.buildProgress : entity?.prodProgress;
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

function finiteTime(value) {
  return Number.isFinite(value) ? value : 0;
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
