import { EVENT, KIND, isBuilding, isUnit } from "./protocol.js";
import { STATS } from "./config.js";

export const AUTO_SPECTATOR_MIN_ZOOM = 0.05;

const DECISION_INTERVAL_TICKS = 90;
const QUIET_DECISION_INTERVAL_TICKS = 30;
const ACTIVITY_WINDOW_TICKS = 90;
const COMMAND_ACTIVITY_WINDOW_TICKS = 90;
const CLUSTER_RADIUS_TILES = 10;
const CURRENT_FIGHT_BONUS = 1.25;
const DEATH_WEIGHT = 4;
const IMPACT_WEIGHT = 2;
const BATTLE_PADDING_CSS_PX = 144;
const CONTACT_PADDING_CSS_PX = 168;
const PAN_DURATION_SECONDS = 1;
const OVERVIEW_DURATION_SECONDS = 1;
const PAN_DEAD_ZONE_CSS_PX = 40;
const ZOOM_DEAD_ZONE_RATIO = 0.05;
const CUT_DISTANCE_VIEWPORTS = 1;
const MAX_PADDING_VIEWPORT_FRACTION = 0.4;
const MAX_ACTIVITY_SAMPLES = 900;
const MAX_COMMAND_ACTIVITY_SAMPLES = 900;
const CONTACT_DISTANCE_TILES = 28;
const CONTACT_CLUSTER_RADIUS_TILES = 7;
const INTERCEPT_DISTANCE_TILES = 8;
const INTERCEPT_MIN_CLOSING_TILES = 3;
const INTERCEPT_HORIZON_TICKS = 180;
const CONTACT_STICKINESS_TILES = 8;
const WORKER_SCORE_PENALTY_TILES = 5;
const OVERVIEW_ZOOM_STEP = 0.94;
const OVERVIEW_MIN_SCALE = 0.55;
const OVERVIEW_MAX_MAP_FRACTION = 0.7;
const VELOCITY_SMOOTHING = 0.35;
const ACTIVE_SHOT_WORLD_SPAN_MULTIPLIER = 1.5;

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

function distanceToBuildingFootprint(point, entity, tileSize) {
  const stats = STATS[entity?.kind];
  const halfWidth = Number(stats?.footW) * tileSize * 0.5;
  const halfHeight = Number(stats?.footH) * tileSize * 0.5;
  if (!Number.isFinite(halfWidth) || halfWidth <= 0
    || !Number.isFinite(halfHeight) || halfHeight <= 0) {
    return distance(point, entity);
  }
  const dx = Math.max(0, Math.abs(point.x - entity.x) - halfWidth);
  const dy = Math.max(0, Math.abs(point.y - entity.y) - halfHeight);
  return Math.hypot(dx, dy);
}

function artilleryImpactHasNearbyEntity(event, state, entityViews) {
  const point = finitePoint(event);
  const tileSize = Number(state?.map?.tileSize);
  const radiusTiles = Number(event?.radiusTiles);
  if (!point || !Number.isFinite(tileSize) || tileSize <= 0
    || !Number.isFinite(radiusTiles) || radiusTiles < 0) return false;
  const radius = radiusTiles * tileSize;
  return entityViews.some((entity) => {
    if (isUnit(entity?.kind)) return distance(point, entity) <= radius;
    return isBuilding(entity?.kind)
      && distanceToBuildingFootprint(point, entity, tileSize) <= radius;
  });
}

function eventSample(event, state, tick, entityViews) {
  if (!event || typeof event !== "object") return null;
  if (event.e === EVENT.ATTACK) return attackSample(event, state, tick);
  if (event.e === EVENT.DEATH) {
    const point = finitePoint(event);
    return point ? sampleFromPoints([point], DEATH_WEIGHT, tick) : null;
  }
  if (event.e === EVENT.ARTILLERY_IMPACT
    && !artilleryImpactHasNearbyEntity(event, state, entityViews)) {
    return null;
  }
  if (POSITIONED_IMPACTS.has(event.e)) {
    const point = finitePoint(event);
    return point ? sampleFromPoints([point], IMPACT_WEIGHT, tick) : null;
  }
  return null;
}

function commandSignature(entity) {
  if (!isUnit(entity?.kind) && !isBuilding(entity?.kind)) return null;
  const stages = (value) => Array.isArray(value)
    ? value.map((stage) => (
      stage?.kind === "attack"
        ? [stage.kind]
        : [stage?.kind, stage?.x, stage?.y]
    ))
    : null;
  return JSON.stringify([
    stages(entity.orderPlan),
    stages(entity.rallyPlan),
    entity.prodKind,
    entity.prodQueue,
    entity.prodUpgrade,
    Array.isArray(entity.prodUpgradeQueue) ? entity.prodUpgradeQueue : null,
    Array.isArray(entity.prodRepeatKinds) ? entity.prodRepeatKinds : null,
  ]);
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

function teamIdForOwner(state, owner) {
  const ownerId = Number(owner);
  if (!Number.isInteger(ownerId) || ownerId <= 0) return null;
  const direct = state?.teamIdForPlayer?.(ownerId);
  if (Number.isInteger(direct) && direct > 0) return direct;
  const player = state?.players?.find?.((candidate) => Number(candidate?.id) === ownerId);
  const teamId = Number(player?.teamId);
  return Number.isInteger(teamId) && teamId > 0 ? teamId : ownerId;
}

function currentEntityViews(state) {
  const views = state?.entitiesInterpolated?.(1, { includePrediction: false });
  return Array.isArray(views)
    ? views.filter((entity) => !entity?.shotReveal && !entity?.visionOnly)
    : [];
}

function currentUnitViews(entityViews) {
  return entityViews.filter((entity) => isUnit(entity?.kind));
}

function closestApproach(a, b) {
  const relativeX = b.x - a.x;
  const relativeY = b.y - a.y;
  const velocityX = b.vx - a.vx;
  const velocityY = b.vy - a.vy;
  const speedSquared = velocityX * velocityX + velocityY * velocityY;
  const rawTick = speedSquared > 0
    ? -(relativeX * velocityX + relativeY * velocityY) / speedSquared
    : 0;
  const tick = Math.max(0, Math.min(INTERCEPT_HORIZON_TICKS, rawTick));
  const aFuture = { x: a.x + a.vx * tick, y: a.y + a.vy * tick };
  const bFuture = { x: b.x + b.vx * tick, y: b.y + b.vy * tick };
  return {
    tick,
    distance: distance(aFuture, bFuture),
  };
}

function contactPoints(units, a, b, radius) {
  return units
    .filter((unit) => (
      unit.teamId === a.teamId || unit.teamId === b.teamId
    ) && (
      distance(unit, a) <= radius || distance(unit, b) <= radius
    ))
    .map((unit) => ({ x: unit.x, y: unit.y }));
}

function contactPenalty(unit, tileSize) {
  return unit.kind === KIND.WORKER ? WORKER_SCORE_PENALTY_TILES * tileSize : 0;
}

function selectLikelyContact(units, tileSize, currentCenter) {
  let best = null;
  let bestScore = Number.POSITIVE_INFINITY;
  const closeDistance = CONTACT_DISTANCE_TILES * tileSize;
  const interceptDistance = INTERCEPT_DISTANCE_TILES * tileSize;
  const minClosingDistance = INTERCEPT_MIN_CLOSING_TILES * tileSize;
  const stickyDistance = CONTACT_STICKINESS_TILES * tileSize;

  for (let i = 0; i < units.length; i += 1) {
    const a = units[i];
    if (a.kind === KIND.SCOUT_PLANE) continue;
    for (let j = i + 1; j < units.length; j += 1) {
      const b = units[j];
      if (b.kind === KIND.SCOUT_PLANE || a.teamId === b.teamId) continue;
      const currentDistance = distance(a, b);
      const approach = closestApproach(a, b);
      const closingDistance = currentDistance - approach.distance;
      const closeNow = currentDistance <= closeDistance;
      const converging = approach.tick > 0
        && approach.distance <= interceptDistance
        && closingDistance >= minClosingDistance;
      if (!closeNow && !converging) continue;

      const center = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      const stickyBonus = currentCenter && distance(center, currentCenter) <= stickyDistance
        ? stickyDistance
        : 0;
      const score = approach.distance
        + currentDistance * 0.15
        + approach.tick * tileSize * 0.01
        + contactPenalty(a, tileSize)
        + contactPenalty(b, tileSize)
        - stickyBonus;
      if (score >= bestScore) continue;
      bestScore = score;
      best = {
        a,
        b,
        center,
        currentDistance,
        predictedDistance: approach.distance,
        etaTicks: approach.tick,
      };
    }
  }
  if (!best) return null;
  const { a, b, ...contact } = best;
  return {
    ...contact,
    points: contactPoints(
      units,
      a,
      b,
      CONTACT_CLUSTER_RADIUS_TILES * tileSize,
    ),
  };
}

function overviewMinimumScale(camera, state) {
  const viewport = camera?.projectionSnapshot?.()?.viewport;
  const tileSize = Number(state?.map?.tileSize);
  const mapWidth = Number(state?.map?.width) * tileSize;
  const mapHeight = Number(state?.map?.height) * tileSize;
  const widthScale = Number.isFinite(viewport?.widthCssPx) && mapWidth > 0
    ? viewport.widthCssPx / (mapWidth * OVERVIEW_MAX_MAP_FRACTION)
    : 0;
  const heightScale = Number.isFinite(viewport?.heightCssPx) && mapHeight > 0
    ? viewport.heightCssPx / (mapHeight * OVERVIEW_MAX_MAP_FRACTION)
    : 0;
  const minimum = Math.max(
    Number(camera?.minZoom) || 0,
    OVERVIEW_MIN_SCALE,
    widthScale,
    heightScale,
  );
  const maximum = Number(camera?.maxZoom);
  return Number.isFinite(maximum) ? Math.min(minimum, maximum) : minimum;
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
    this.currentContactCenter = null;
    this.currentCommandCenter = null;
    this.unitTracks = new Map();
    this.commandSignatures = new Map();
    this.commandSamples = [];
    this.transition = null;
    this.lastMoveKind = null;
    this.mode = null;
    this.contactDiagnostics = null;
  }

  setEnabled(enabled) {
    const next = !!enabled;
    if (next === this.enabled) return;
    this.enabled = next;
    this.transition = null;
    this.onEnabledChange?.(next);
    if (!next) {
      this.unitTracks.clear();
      this.currentFightCenter = null;
      this.currentContactCenter = null;
      this.currentCommandCenter = null;
      this.commandSignatures.clear();
      this.commandSamples = [];
      this.contactDiagnostics = null;
      this.mode = null;
      return;
    }
    const entityViews = currentEntityViews(this.state);
    this.updateUnitTracks(this.latestTick, entityViews);
    this.updateCommandActivity(this.latestTick, entityViews);
    this.lastDecisionTick = this.latestTick;
    this.decide(this.latestTick);
  }

  observeSnapshot(snapshot) {
    const tick = Number(snapshot?.tick);
    if (!Number.isFinite(tick)) return;
    if (this.latestTick != null && tick < this.latestTick) this.resetForSeek();
    this.latestTick = tick;
    const events = Array.isArray(snapshot?.events) ? snapshot.events : [];
    const needsEntityViews = this.enabled
      || events.some((event) => event?.e === EVENT.ARTILLERY_IMPACT);
    const entityViews = needsEntityViews ? currentEntityViews(this.state) : [];
    if (this.enabled) {
      this.updateUnitTracks(tick, entityViews);
      this.updateCommandActivity(tick, entityViews);
    }
    let newCombatActivity = false;
    for (const event of events) {
      const sample = eventSample(event, this.state, tick, entityViews);
      if (sample) {
        this.samples.push(sample);
        newCombatActivity = true;
      }
    }
    this.pruneSamples(tick);
    if (!this.enabled) return;
    const routineDecisionDue = this.lastDecisionTick == null
      || tick - this.lastDecisionTick >= DECISION_INTERVAL_TICKS;
    const quietMode = this.mode == null || this.mode === "overview" || this.mode === "activity";
    const quietDecisionDue = quietMode && (
      this.lastDecisionTick == null
      || tick - this.lastDecisionTick >= QUIET_DECISION_INTERVAL_TICKS
    );
    const combatInterrupt = quietMode && newCombatActivity;
    if (routineDecisionDue || quietDecisionDue || combatInterrupt || this.hasDistantFightCut()) {
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
      this.mode = "combat";
      this.currentFightCenter = { x: fight.x, y: fight.y };
      this.currentContactCenter = null;
      this.currentCommandCenter = null;
      this.contactDiagnostics = null;
      this.moveTo(fight.points, BATTLE_PADDING_CSS_PX, { immediate });
      return;
    }
    this.currentFightCenter = null;
    const contact = selectLikelyContact(
      [...this.unitTracks.values()],
      Math.max(1, Number(this.state?.map?.tileSize) || 32),
      this.currentContactCenter,
    );
    if (contact) {
      const tileSize = Math.max(1, Number(this.state?.map?.tileSize) || 32);
      this.mode = "contact";
      this.currentContactCenter = contact.center;
      this.currentCommandCenter = null;
      this.contactDiagnostics = {
        distanceTiles: contact.currentDistance / tileSize,
        predictedDistanceTiles: contact.predictedDistance / tileSize,
        etaTicks: contact.etaTicks,
      };
      this.moveTo(contact.points, CONTACT_PADDING_CSS_PX, { immediate });
      return;
    }
    const activity = selectFight(
      clusterSamples(this.commandSamples, radius),
      this.currentCommandCenter,
      radius,
    );
    if (activity) {
      this.mode = "activity";
      this.currentCommandCenter = { x: activity.x, y: activity.y };
      this.contactDiagnostics = null;
      this.moveTo(activity.points, BATTLE_PADDING_CSS_PX, {
        immediate,
        preserveScale: true,
      });
      return;
    }
    this.mode = "overview";
    this.currentContactCenter = null;
    this.currentCommandCenter = null;
    this.contactDiagnostics = null;
    this.widenView({ immediate });
  }

  moveTo(
    points,
    paddingCssPx,
    {
      immediate = false,
      allowCut = true,
      duration = PAN_DURATION_SECONDS,
      preserveScale = false,
    } = {},
  ) {
    const from = this.camera?.snapshot?.();
    const projection = this.camera?.projectionSnapshot?.();
    if (!from || !projection?.viewport) return;
    const viewportMinSpan = Math.min(
      Number(projection.viewport.widthCssPx),
      Number(projection.viewport.heightCssPx),
    );
    const effectivePadding = Number.isFinite(viewportMinSpan) && viewportMinSpan > 0
      ? Math.min(paddingCssPx, viewportMinSpan * MAX_PADDING_VIEWPORT_FRACTION)
      : paddingCssPx;
    const fitted = this.camera?.framingForWorldPoints?.(points, {
      paddingCssPx: effectivePadding,
    });
    if (!fitted) return;
    const minimumScale = Number(this.camera?.minZoom) || 0;
    const to = {
      ...fitted,
      framingScale: preserveScale
        ? from.framingScale
        : Math.max(
          minimumScale,
          fitted.framingScale / ACTIVE_SHOT_WORLD_SPAN_MULTIPLIER,
        ),
    };

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
    if (allowCut && distanceCss > viewportSpan * CUT_DISTANCE_VIEWPORTS) {
      this.camera.restore(to);
      this.transition = null;
      this.lastMoveKind = "cut";
      return;
    }

    const remainingDuration = this.transition
      ? Math.max(0, this.transition.duration - this.transition.elapsed)
      : duration;
    this.transition = {
      from,
      to,
      elapsed: 0,
      duration: remainingDuration,
    };
    this.lastMoveKind = "pan";
  }

  hasDistantFightCut() {
    const from = this.camera?.snapshot?.();
    const projection = this.camera?.projectionSnapshot?.();
    if (!from || !projection?.viewport || this.samples.length === 0) return false;
    const radius = Math.max(1, Number(this.state?.map?.tileSize) || 32) * CLUSTER_RADIUS_TILES;
    const fight = selectFight(clusterSamples(this.samples, radius), this.currentFightCenter, radius);
    if (!fight) return false;
    if (this.currentFightCenter
      && distance(fight, this.currentFightCenter) <= radius * 1.5) return false;
    const distanceCss = distance(from.focus, fight) * from.framingScale;
    const viewportSpan = Math.max(
      Number(projection.viewport.widthCssPx) || 0,
      Number(projection.viewport.heightCssPx) || 0,
      1,
    );
    return distanceCss > viewportSpan * CUT_DISTANCE_VIEWPORTS;
  }

  widenView({ immediate = false } = {}) {
    const from = this.camera?.snapshot?.();
    if (!from) return;
    const minimumScale = overviewMinimumScale(this.camera, this.state);
    if (immediate || from.framingScale <= minimumScale) {
      this.transition = null;
      this.lastMoveKind = "hold";
      return;
    }
    if (this.transition) {
      this.lastMoveKind = this.transition.kind || "zoom";
      return;
    }
    const targetScale = Math.max(minimumScale, from.framingScale * OVERVIEW_ZOOM_STEP);
    if (Math.abs(Math.log(targetScale / from.framingScale)) <= ZOOM_DEAD_ZONE_RATIO) {
      this.transition = null;
      this.lastMoveKind = "hold";
      return;
    }
    const to = {
      version: 1,
      focus: { ...from.focus },
      framingScale: targetScale,
      boundsPolicy: "mapOverscroll",
    };
    this.transition = {
      from,
      to,
      elapsed: 0,
      duration: OVERVIEW_DURATION_SECONDS,
      kind: "zoom",
    };
    this.lastMoveKind = "zoom";
  }

  updateUnitTracks(tick, entityViews = currentEntityViews(this.state)) {
    const next = new Map();
    for (const entity of currentUnitViews(entityViews)) {
      const point = finitePoint(entity);
      const id = Number(entity?.id);
      const teamId = teamIdForOwner(this.state, entity?.owner);
      if (!point || !Number.isInteger(id) || !isUnit(entity?.kind) || teamId == null) continue;
      const prior = this.unitTracks.get(id);
      const elapsedTicks = tick - Number(prior?.tick);
      let vx = 0;
      let vy = 0;
      if (prior && Number.isFinite(elapsedTicks) && elapsedTicks > 0) {
        const rawVx = (point.x - prior.x) / elapsedTicks;
        const rawVy = (point.y - prior.y) / elapsedTicks;
        vx = prior.hasVelocity
          ? prior.vx + (rawVx - prior.vx) * VELOCITY_SMOOTHING
          : rawVx;
        vy = prior.hasVelocity
          ? prior.vy + (rawVy - prior.vy) * VELOCITY_SMOOTHING
          : rawVy;
      }
      next.set(id, {
        id,
        owner: Number(entity.owner),
        teamId,
        kind: entity.kind,
        x: point.x,
        y: point.y,
        vx,
        vy,
        hasVelocity: !!prior,
        tick,
      });
    }
    this.unitTracks = next;
  }

  updateCommandActivity(tick, entityViews = currentEntityViews(this.state)) {
    const next = new Map();
    for (const entity of entityViews) {
      const id = Number(entity?.id);
      const point = finitePoint(entity);
      const signature = commandSignature(entity);
      if (!Number.isInteger(id) || !point || signature == null) continue;
      const prior = this.commandSignatures.get(id);
      if (prior != null && prior !== signature) {
        this.commandSamples.push(sampleFromPoints(
          [point],
          isBuilding(entity.kind) ? 2 : 1,
          tick,
        ));
      }
      next.set(id, signature);
    }
    this.commandSignatures = next;
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
    const oldestCommandTick = tick - COMMAND_ACTIVITY_WINDOW_TICKS;
    this.commandSamples = this.commandSamples
      .filter((sample) => sample.tick >= oldestCommandTick);
    if (this.commandSamples.length > MAX_COMMAND_ACTIVITY_SAMPLES) {
      this.commandSamples.splice(
        0,
        this.commandSamples.length - MAX_COMMAND_ACTIVITY_SAMPLES,
      );
    }
  }

  resetForSeek() {
    this.samples = [];
    this.lastDecisionTick = null;
    this.currentFightCenter = null;
    this.currentContactCenter = null;
    this.currentCommandCenter = null;
    this.unitTracks.clear();
    this.commandSignatures.clear();
    this.commandSamples = [];
    this.transition = null;
    this.mode = null;
    this.contactDiagnostics = null;
  }

  diagnostics() {
    return {
      enabled: this.enabled,
      sampleCount: this.samples.length,
      latestTick: this.latestTick,
      lastDecisionTick: this.lastDecisionTick,
      mode: this.mode,
      currentFightCenter: this.currentFightCenter ? { ...this.currentFightCenter } : null,
      currentContactCenter: this.currentContactCenter ? { ...this.currentContactCenter } : null,
      currentCommandCenter: this.currentCommandCenter ? { ...this.currentCommandCenter } : null,
      commandSampleCount: this.commandSamples.length,
      contact: this.contactDiagnostics ? { ...this.contactDiagnostics } : null,
      trackedUnitCount: this.unitTracks.size,
      moveKind: this.lastMoveKind,
      transitioning: !!this.transition,
    };
  }

  destroy() {
    this.enabled = false;
    this.samples = [];
    this.unitTracks.clear();
    this.commandSignatures.clear();
    this.commandSamples = [];
    this.transition = null;
    this.currentFightCenter = null;
    this.currentContactCenter = null;
    this.currentCommandCenter = null;
    this.contactDiagnostics = null;
    this.onEnabledChange = null;
  }
}
