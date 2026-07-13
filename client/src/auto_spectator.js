import { EVENT } from "./protocol.js";

export const AUTO_SPECTATOR_MIN_ZOOM = 0.05;

const DECISION_INTERVAL_TICKS = 30;
const ACTIVITY_WINDOW_TICKS = 90;
const CLUSTER_RADIUS_TILES = 10;
const CURRENT_FIGHT_BONUS = 1.25;
const DEATH_WEIGHT = 4;
const IMPACT_WEIGHT = 2;
const BATTLE_PADDING_CSS_PX = 96;
const MAP_PADDING_CSS_PX = 16;
const PAN_DURATION_SECONDS = 1;
const PAN_DEAD_ZONE_CSS_PX = 40;
const ZOOM_DEAD_ZONE_RATIO = 0.05;
const CUT_DISTANCE_VIEWPORTS = 1;
const MAX_ACTIVITY_SAMPLES = 900;

const POSITIONED_IMPACTS = new Set([
  EVENT.MORTAR_IMPACT,
  EVENT.ARTILLERY_IMPACT,
  EVENT.PANZERFAUST_IMPACT,
]);

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y)
    ? { x: point.x, y: point.y }
    : null;
}

function entityPoint(state, id) {
  const entity = Number.isFinite(id) ? state?.entityById?.(id) : null;
  return finitePoint(entity);
}

function attackSample(event, state, tick) {
  const from = entityPoint(state, event.from) || finitePoint(event.reveal);
  const to = entityPoint(state, event.to) || finitePoint({
    x: event.toPos?.[0],
    y: event.toPos?.[1],
  });
  const points = [from, to].filter(Boolean);
  if (points.length === 0) return null;
  return sampleFromPoints(points, 1, tick);
}

function sampleFromPoints(points, weight, tick) {
  const x = points.reduce((sum, point) => sum + point.x, 0) / points.length;
  const y = points.reduce((sum, point) => sum + point.y, 0) / points.length;
  return { tick, x, y, weight, points };
}

function eventSample(event, state, tick) {
  if (!event || typeof event !== "object") return null;
  if (event.e === EVENT.ATTACK) return attackSample(event, state, tick);
  if (event.e === EVENT.DEATH) {
    const point = finitePoint(event);
    return point ? sampleFromPoints([point], DEATH_WEIGHT, tick) : null;
  }
  if (POSITIONED_IMPACTS.has(event.e)) {
    const point = finitePoint(event);
    return point ? sampleFromPoints([point], IMPACT_WEIGHT, tick) : null;
  }
  return null;
}

function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function addSampleToCluster(cluster, sample) {
  cluster.samples.push(sample);
  cluster.points.push(...sample.points);
  cluster.weight += sample.weight;
  const centerWeight = cluster.centerWeight + sample.weight;
  cluster.x = (cluster.x * cluster.centerWeight + sample.x * sample.weight) / centerWeight;
  cluster.y = (cluster.y * cluster.centerWeight + sample.y * sample.weight) / centerWeight;
  cluster.centerWeight = centerWeight;
}

function clusterSamples(samples, radius) {
  const clusters = [];
  for (const sample of samples) {
    let nearest = null;
    let nearestDistance = Number.POSITIVE_INFINITY;
    for (const cluster of clusters) {
      const candidateDistance = distance(sample, cluster);
      if (candidateDistance <= radius && candidateDistance < nearestDistance) {
        nearest = cluster;
        nearestDistance = candidateDistance;
      }
    }
    if (nearest) {
      addSampleToCluster(nearest, sample);
    } else {
      clusters.push({
        x: sample.x,
        y: sample.y,
        weight: sample.weight,
        centerWeight: sample.weight,
        samples: [sample],
        points: [...sample.points],
      });
    }
  }
  return clusters;
}

function selectFight(clusters, currentCenter, radius) {
  let best = null;
  let bestScore = -1;
  for (const cluster of clusters) {
    const staysOnCurrentFight = currentCenter && distance(cluster, currentCenter) <= radius * 1.5;
    const score = cluster.weight * (staysOnCurrentFight ? CURRENT_FIGHT_BONUS : 1);
    if (score > bestScore) {
      best = cluster;
      bestScore = score;
    }
  }
  return best;
}

function mapCorners(state) {
  const tileSize = Number(state?.map?.tileSize);
  const width = Number(state?.map?.width) * tileSize;
  const height = Number(state?.map?.height) * tileSize;
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return [];
  return [{ x: 0, y: 0 }, { x: width, y: height }];
}

function interpolateView(from, to, progress) {
  const eased = progress * progress * (3 - 2 * progress);
  const fromScale = from.framingScale;
  const scaleRatio = to.framingScale / fromScale;
  return {
    version: 1,
    focus: {
      x: from.focus.x + (to.focus.x - from.focus.x) * eased,
      y: from.focus.y + (to.focus.y - from.focus.y) * eased,
    },
    framingScale: fromScale * Math.pow(scaleRatio, eased),
    boundsPolicy: "mapOverscroll",
  };
}

export class AutoSpectatorDirector {
  constructor({ camera, state, enabled = false, onEnabledChange = null } = {}) {
    this.camera = camera;
    this.state = state;
    this.enabled = !!enabled;
    this.onEnabledChange = onEnabledChange;
    this.samples = [];
    this.latestTick = null;
    this.lastDecisionTick = null;
    this.currentFightCenter = null;
    this.transition = null;
    this.lastMoveKind = null;
  }

  setEnabled(enabled) {
    const next = !!enabled;
    if (next === this.enabled) return;
    this.enabled = next;
    this.transition = null;
    this.onEnabledChange?.(next);
    if (!next) return;
    this.lastDecisionTick = this.latestTick;
    this.decide(this.latestTick);
  }

  observeSnapshot(snapshot) {
    const tick = Number(snapshot?.tick);
    if (!Number.isFinite(tick)) return;
    if (this.latestTick != null && tick < this.latestTick) this.resetForSeek();
    this.latestTick = tick;
    for (const event of snapshot?.events || []) {
      const sample = eventSample(event, this.state, tick);
      if (sample) this.samples.push(sample);
    }
    this.pruneSamples(tick);
    if (!this.enabled) return;
    if (this.lastDecisionTick == null || tick - this.lastDecisionTick >= DECISION_INTERVAL_TICKS) {
      this.lastDecisionTick = tick;
      this.decide(tick);
    }
  }

  update(dt) {
    if (!this.enabled || !this.transition) return;
    const elapsed = Number(dt);
    if (!Number.isFinite(elapsed) || elapsed < 0) return;
    this.transition.elapsed += elapsed;
    const progress = Math.min(1, this.transition.elapsed / this.transition.duration);
    this.camera.restore(interpolateView(this.transition.from, this.transition.to, progress));
    if (progress >= 1) this.transition = null;
  }

  decide(tick = this.latestTick, { immediate = false } = {}) {
    if (!this.enabled) return;
    if (Number.isFinite(tick)) this.pruneSamples(tick);
    const radius = Math.max(1, Number(this.state?.map?.tileSize) || 32) * CLUSTER_RADIUS_TILES;
    const fight = selectFight(clusterSamples(this.samples, radius), this.currentFightCenter, radius);
    if (fight) {
      this.currentFightCenter = { x: fight.x, y: fight.y };
      this.moveTo(fight.points, BATTLE_PADDING_CSS_PX, { immediate });
      return;
    }
    this.currentFightCenter = null;
    this.moveTo(mapCorners(this.state), MAP_PADDING_CSS_PX, { immediate });
  }

  moveTo(points, paddingCssPx, { immediate = false } = {}) {
    const from = this.camera?.snapshot?.();
    const to = this.camera?.framingForWorldPoints?.(points, { paddingCssPx });
    const projection = this.camera?.projectionSnapshot?.();
    if (!from || !to || !projection?.viewport) return;

    if (immediate) {
      this.camera.restore(to);
      this.transition = null;
      this.lastMoveKind = "cut";
      return;
    }

    const distanceCss = distance(from.focus, to.focus) * from.framingScale;
    const scaleRatio = to.framingScale / from.framingScale;
    const zoomChanged = Math.abs(Math.log(scaleRatio)) > ZOOM_DEAD_ZONE_RATIO;
    if (distanceCss <= PAN_DEAD_ZONE_CSS_PX && !zoomChanged) {
      this.lastMoveKind = "hold";
      this.transition = null;
      return;
    }

    const viewportSpan = Math.max(
      projection.viewport.widthCssPx,
      projection.viewport.heightCssPx,
      1,
    );
    if (distanceCss > viewportSpan * CUT_DISTANCE_VIEWPORTS) {
      this.camera.restore(to);
      this.transition = null;
      this.lastMoveKind = "cut";
      return;
    }

    const remainingDuration = this.transition
      ? Math.max(0, this.transition.duration - this.transition.elapsed)
      : PAN_DURATION_SECONDS;
    this.transition = {
      from,
      to,
      elapsed: 0,
      duration: remainingDuration,
    };
    this.lastMoveKind = "pan";
  }

  handleViewportChange() {
    if (!this.enabled) return;
    this.decide(this.latestTick, { immediate: true });
  }

  pruneSamples(tick) {
    const oldestTick = tick - ACTIVITY_WINDOW_TICKS;
    this.samples = this.samples.filter((sample) => sample.tick >= oldestTick);
    if (this.samples.length > MAX_ACTIVITY_SAMPLES) {
      this.samples.splice(0, this.samples.length - MAX_ACTIVITY_SAMPLES);
    }
  }

  resetForSeek() {
    this.samples = [];
    this.lastDecisionTick = null;
    this.currentFightCenter = null;
    this.transition = null;
  }

  diagnostics() {
    return {
      enabled: this.enabled,
      sampleCount: this.samples.length,
      latestTick: this.latestTick,
      lastDecisionTick: this.lastDecisionTick,
      currentFightCenter: this.currentFightCenter ? { ...this.currentFightCenter } : null,
      moveKind: this.lastMoveKind,
      transitioning: !!this.transition,
    };
  }

  destroy() {
    this.enabled = false;
    this.samples = [];
    this.transition = null;
    this.currentFightCenter = null;
    this.onEnabledChange = null;
  }
}
