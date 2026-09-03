// GameState — the single client-side model the renderer, HUD, minimap and
// input all read from. See docs/design/client-ui.md §4.1.
//
// It holds the two most recent server snapshots (for interpolation), the
// latest resources/events, and the local selection set. Selection is a
// client-only concept; the server never sees it directly.

import { admitSelectionIds } from "./command_budget.js";
import {
  composePredictionPatch,
  composeProgressPatch,
  normalizePredictionPatch,
  normalizeProgressPatch,
  progressPatchMatches,
} from "./prediction_frame.js";
import { MOVEMENT_PATH_DIAGNOSTICS, isBuilding, isResource, isUnit } from "./protocol.js";
import { admitControlGroupIds } from "./state_control_groups.js";
import { GroundDecalBuffer } from "./state_ground_decals.js";
import { autoBuild as ab } from "./state_auto_build.js";
import { pruneSelection } from "./state_selection.js";
import {
  isAllyOwner as queryIsAllyOwner,
  isEnemyOwner as queryIsEnemyOwner,
  isNeutralOwner as queryIsNeutralOwner,
  isOwnOwner as queryIsOwnOwner,
  isPassable as queryIsPassable,
  normalizeDiagnostics,
  normalizePlayer,
  normalizeResource,
  playerById as queryPlayerById,
  teamIdForPlayer as queryTeamIdForPlayer,
  terrainAt as queryTerrainAt,
  worldInBounds as queryWorldInBounds,
} from "./state_queries.js";
import { VisualEffectBackedState, VisualEffectBuffers } from "./state_visual_effects.js";
import { markActionableFiringReveal } from "./state_firing_reveal.js";
import { resetAuthoritativeRuntime } from "./state_runtime_reset.js";

const TWO_PI = Math.PI * 2;
const PREDICTION_SMOOTH_MS = 120;
const PREDICTION_SMOOTH_MAX_PX = 96;

function normalizeAngle(a) {
  let out = (a + Math.PI) % TWO_PI;
  if (out < 0) out += TWO_PI;
  return out - Math.PI;
}

function shortestAngleDelta(from, to) {
  return normalizeAngle(to - from);
}

function lerpAngle(from, to, t) {
  return normalizeAngle(from + shortestAngleDelta(from, to) * t);
}

function interpolateEntity(entity, prior, alpha) {
  if (!prior) return { ...entity };
  const next = {
    ...entity,
    x: prior.x + (entity.x - prior.x) * alpha,
    y: prior.y + (entity.y - prior.y) * alpha,
  };
  if (typeof prior.facing === "number" && typeof entity.facing === "number") {
    next.facing = lerpAngle(prior.facing, entity.facing, alpha);
  }
  if (typeof prior.weaponFacing === "number" && typeof entity.weaponFacing === "number") {
    next.weaponFacing = lerpAngle(prior.weaponFacing, entity.weaponFacing, alpha);
  }
  return next;
}

export class GameState extends VisualEffectBackedState {
  /**
   * @param {object} startInfo the §2.3 `start` payload.
   */
  constructor(startInfo, { renderClock = null } = {}) {
    super();
    this.renderClock = renderClock;
    /** @type {number} our player id (repeat of welcome, for convenience). */
    this.playerId = startInfo.playerId;
    /** @type {boolean} true when this client is observing instead of playing. */
    this.spectator = !!startInfo.spectator;
    /** @type {object} the full §2.3 start payload, kept for reference. */
    this.startInfo = startInfo;
    /** @type {{width:number,height:number,tileSize:number,terrain:number[]}} */
    this.map = {
      ...startInfo.map,
      resources: (startInfo.map?.resources || []).map((node, index) =>
        normalizeResource(node, index),
      ),
    };
    /** @type {Map<number, object>} id -> resource node state. */
    this.resourceById = new Map();
    for (const node of this.map.resources) this.resourceById.set(node.id, node);
    /** @type {Array<{id:number,teamId:number,factionId:string,name:string,color:string,isAi:boolean,startTileX:number,startTileY:number}>} */
    this.players = (startInfo.players || []).map((player) => normalizePlayer(player));

    // --- snapshot buffering for interpolation ---
    /** @type {object|null} previous snapshot (older of the two we keep). */
    this._prev = null;
    /** @type {object|null} current snapshot (most recent received). */
    this._cur = null;
    /** performance.now() stamp when `_prev` arrived. */
    this._prevRecvTime = 0;
    /** performance.now() stamp when `_cur` arrived. */
    this._curRecvTime = 0;
    /** @type {Map<number, object>} id -> entity for the current snapshot. */
    this._curById = new Map();
    /** @type {Map<number, object>} id -> entity for the previous snapshot. */
    this._prevById = new Map();

    // --- derived latest state ---
    /** @type {{steel:number,oil:number,supplyUsed:number,supplyCap:number}} */
    this.resources = { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
    this.autoBuild = ab();
    /** @type {Array<{id:number,steel:number,oil:number,supplyUsed:number,supplyCap:number,apm:number}>} */
    this.playerResources = [];
    /** @type {Array<object>} latest snapshot's transient events. */
    this.events = [];
    /** @type {string[]} upgrades completed for this player. */
    this.upgrades = [];

    // --- selection (client-only) ---
    /** @type {Set<number>} */
    this.selection = new Set();
    /** @type {null | {used:number, cap:number, seq:number}} latest playable selection budget overflow. */
    this.selectionBudgetOverflow = null;
    this._selectionBudgetOverflowSeq = 0;
    const diagnostics = normalizeDiagnostics(startInfo.diagnostics);
    /** @type {{movementPaths:string, observerAnalysis:boolean}} diagnostic affordances for this recipient. */
    this.diagnostics = diagnostics;
    /** @type {boolean} true when the server says movement path diagnostics are available. */
    this.debugPathOverlaysAvailable =
      diagnostics.movementPaths !== MOVEMENT_PATH_DIAGNOSTICS.NONE;
    /** @type {boolean} local gear-menu preference for drawing movement path diagnostics. */
    this.debugPathOverlaysEnabled = this.debugPathOverlaysAvailable;
    /** @type {boolean} true for recipients that may draw every server-projected debug path. */
    this.showAllDebugPathOverlays =
      diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.ALL;
    /** @type {boolean} local gear-menu preference for drawing selected unit firing ranges. */
    this.showUnitRangesEnabled = true;
    /** @type {Array<Array<number>>} ten local control groups, slot 9 is key 0. */
    this.controlGroups = Array.from({ length: 10 }, () => []);
    /** @type {Array<{id:number,x:number,y:number,radiusTiles:number,expiresIn:number}>} */
    this.smokes = [];
    /** @type {Array<{id:number,owner:number,ability:string,kind:string,x:number,y:number,expiresIn?:number,sourceCasterId?:number,ownerState?:object}>} */
    this.abilityObjects = [];
    /** @type {Array<{id:number,x:number,y:number,radiusTiles:number}>} */
    this.trenches = [];
    /** @type {Array<{id:number,owner:number,kind:string,x:number,y:number,footprint:Array<[number,number]>,observedTick:number}>} */
    this.rememberedBuildings = [];
    /** @type {Array<{id:number,owner:number,x:number,y:number,facing:number,observedTick:number}>} */
    this.rememberedAntiTankGuns = [];
    /** @type {number[]|Uint8Array} row-major current server-authoritative visibility. */
    this.visibleTiles = [];
    /** @type {number[]|Uint8Array} row-major server-authoritative exploration history. */
    this.exploredTiles = [];
    /** @type {object|null} current server-authoritative player-relative observer projection. */
    this.observerView = startInfo.observerView || null;

    this.groundDecals = new GroundDecalBuffer();
    /** @type {Map<number, object>} owned entity id -> sparse local prediction patch. */
    this.predictionPatchById = new Map();
    /** @type {Map<number, object>} owned building id -> sparse display-progress patch. */
    this.predictionProgressById = new Map();
    this.predictionCorrectionById = new Map();
    this.predictionDiagnostics = null;
    this.predictionProgressPaused = false;
    this.optimisticProduction = [];
    this.optimisticProductionByBuilding = new Map();
    this.optimisticRallyByBuilding = new Map();
  }

  /** World pixels per tile. */
  get tileSize() {
    return this.map.tileSize;
  }

  /** Replace static map/start state after an authoritative Lab battle reset without rebuilding Match. */
  resetForLabMap({ map, players, tick = 0 }) {
    if (!this.updateForLabMap({ map, players, tick })) return false;

    resetAuthoritativeRuntime(this);
    return true;
  }

  /** Clear timeline-derived state before an authoritative replay seek begins streaming again. */
  resetForReplaySeek() {
    const initialResources = (this.startInfo?.map?.resources || []).map((node, index) =>
      normalizeResource(node, index),
    );
    this.map = { ...this.map, resources: initialResources };
    this.resourceById = new Map();
    for (const node of initialResources) this.resourceById.set(node.id, node);
    resetAuthoritativeRuntime(this);
  }

  /** Replace authoritative static map data while preserving the live entity/snapshot buffers. */
  updateForLabMap({ map, players, tick = this.tick }) {
    if (!map || !Array.isArray(map.terrain) || !Array.isArray(players)) return false;
    this.map = {
      ...map,
      resources: (map.resources || []).map((node, index) => normalizeResource(node, index)),
    };
    this.players = players.map((player) => normalizePlayer(player));
    this.startInfo = { ...this.startInfo, tick, map, players };
    this.resourceById = new Map();
    for (const node of this.map.resources) this.resourceById.set(node.id, node);
    return true;
  }

  playerById(id) {
    return queryPlayerById(this.players, id);
  }

  get localPlayer() {
    return this.playerById(this.playerId);
  }

  get localFactionId() {
    return this.localPlayer?.factionId || null;
  }

  teamIdForPlayer(id) {
    return queryTeamIdForPlayer(this.players, id);
  }

  isOwnOwner(owner) {
    return queryIsOwnOwner(this.playerId, owner);
  }

  isAllyOwner(owner) {
    return queryIsAllyOwner(this.players, this.playerId, owner);
  }

  isEnemyOwner(owner) {
    return queryIsEnemyOwner(this.players, this.playerId, owner);
  }

  isNeutralOwner(owner) {
    return queryIsNeutralOwner(owner);
  }

  /**
   * Receive time of the previous snapshot, or null when no prior snapshot
   * exists yet. main.js computeAlpha() reads this to interpolate.
   * @returns {number|null}
   */
  get prevRecvTime() {
    return this._prev ? this._prevRecvTime : null;
  }

  /**
   * Receive time of the current snapshot, or null when none received yet.
   * @returns {number|null}
   */
  get currRecvTime() {
    return this._cur ? this._curRecvTime : null;
  }

  /**
   * Current authoritative simulation tick, or 0 before the first snapshot.
   * Views should use this for sim-time readouts so replay seeks and room-time
   * speed changes are reflected from server state instead of wall time.
   * @returns {number}
   */
  get tick() {
    const tick = this._cur?.tick;
    return Number.isFinite(tick) ? Math.max(0, Math.trunc(tick)) : 0;
  }

  // --- snapshots ----------------------------------------------------------

  /**
   * Ingest a new snapshot. The current snapshot becomes the previous one and
   * the new message becomes current, each stamped with the receive time so
   * entitiesInterpolated() can position entities between them. Resources,
   * events and the id->entity index are refreshed from the latest snapshot.
   * @param {object} msg a §2.4 snapshot payload.
   */
  applySnapshot(msg, visualNow = this.visualNow(), { controlPolicy = null } = {}) {
    // Snapshots can arrive batched in a single event-loop turn (a throttled or
    // backgrounded tab drains its socket buffer at once) and performance.now()
    // is clamped to a coarse resolution, so two consecutive snapshots could
    // otherwise share a receive time. Force the receive clock strictly forward
    // so the interpolation window (curr - prev) is always positive and never
    // collapses to a degenerate, alpha-pinned-to-1 span. Real time reasserts
    // itself via Math.max as soon as performance.now() passes the floor.
    const receivedAt = Math.max(performance.now(), this._curRecvTime + 1);

    this._prev = this._cur;
    this._prevRecvTime = this._curRecvTime;
    this._prevById = this._curById;

    // A progress patch belongs to exactly one authoritative baseline. Drop it before any
    // snapshot-time entity lookup can observe a stale production identity or completed build.
    this.predictionProgressById.clear();
    const events = msg.events || [];
    this._applyResourceDeltas(msg.resourceDeltas || []);
    this._applyResourceDeaths(events);
    const visibleTiles = Array.isArray(msg.visibleTiles) || msg.visibleTiles instanceof Uint8Array
      ? msg.visibleTiles
      : [];
    const wireEntities = (msg.entities || [])
      .filter((e) => !isResource(e.kind))
      .map((e) => markActionableFiringReveal(e, this.map, visibleTiles, this.playerId));
    this.visualEffects.applyAttackReveals(events, visualNow);
    const visibleIds = new Set(wireEntities.map((e) => e.id));
    const entities = wireEntities
      .concat(this._resourceEntityViews())
      .concat(this.visualEffects.shotRevealEntityViews(visualNow, visibleIds));

    this._cur = { ...msg, entities };
    this._curRecvTime = receivedAt;
    this._curById = new Map();
    for (const e of entities) this._curById.set(e.id, e);
    this.visualEffects.pruneRecoilForSnapshot(this._curById);
    this.resources = {
      steel: msg.steel | 0,
      oil: msg.oil | 0,
      supplyUsed: msg.supplyUsed | 0,
      supplyCap: msg.supplyCap | 0,
    };
    this.autoBuild = ab(msg.autoBuild, msg.netStatus);
    this.playerResources = msg.playerResources || [];
    this.upgrades = Array.isArray(msg.upgrades) ? msg.upgrades : [];
    this.smokes = Array.isArray(msg.smokes) ? msg.smokes : [];
    this.abilityObjects = Array.isArray(msg.abilityObjects) ? msg.abilityObjects : [];
    this.trenches = Array.isArray(msg.trenches) ? msg.trenches : [];
    this.rememberedBuildings = Array.isArray(msg.rememberedBuildings)
      ? msg.rememberedBuildings
      : [];
    this.rememberedAntiTankGuns = Array.isArray(msg.rememberedAntiTankGuns)
      ? msg.rememberedAntiTankGuns
      : [];
    this.visibleTiles = visibleTiles;
    this.exploredTiles = Array.isArray(msg.exploredTiles) || msg.exploredTiles instanceof Uint8Array
      ? msg.exploredTiles
      : [];
    this.events = events;
    this._pruneSelection({ controlPolicy, enforceCommandBudget: true });
    this._pruneControlGroups({ controlPolicy });

    this.visualEffects.applySnapshotEvents(this.events, visualNow, (id) => this.entityById(id));
  }

  setObserverView(observerView = null) {
    this.observerView =
      observerView && typeof observerView === "object" ? { ...observerView } : null;
  }

  applyAuthoritativeGroundDecals(message) {
    return this.groundDecals.applyAuthoritativeBatch(message, {
      players: this.players,
      tileSize: this.map?.tileSize,
    });
  }

  resetAuthoritativeGroundDecals() {
    this.groundDecals.clear();
  }

  requeueAuthoritativeGroundDecals() {
    return this.groundDecals.requeueAuthoritative();
  }

  consumePendingGroundDecals() {
    return this.groundDecals.consumePending();
  }

  reconcilePendingGroundDecals() {
    return this.groundDecals.reconcileBatch();
  }

  acknowledgeReconciledGroundDecals(revision) {
    return this.groundDecals.acknowledgeReconciled(revision);
  }

  /**
   * Entities of the current snapshot with x,y linearly interpolated toward
   * their current positions from where they were in the previous snapshot.
   * All other fields are carried through unchanged. Entities with no prior
   * sample (newly visible) use their current position.
   * @param {number} alpha blend factor in [0,1]; 0 = previous, 1 = current.
   * @returns {Array<object>}
   */
  entitiesInterpolated(alpha, { includePrediction = true } = {}) {
    if (!this._cur) return [];
    const t = alpha < 0 ? 0 : alpha > 1 ? 1 : alpha;
    const now = this.visualNow();
    const out = [];
    for (const e of this._cur.entities || []) {
      const next = interpolateEntity(e, this._prevById.get(e.id), t);
      out.push(includePrediction ? this._applyDisplayEntity(this._applyPredictedEntity(next, now), now) : next);
    }
    return out;
  }

  entityVariants(alpha) {
    if (!this._cur) {
      return { interpolatedEntities: [], currentEntities: [], authoritativeEntities: [], entityTraversals: 0 };
    }
    const t = alpha < 0 ? 0 : alpha > 1 ? 1 : alpha;
    const renderNow = this.visualNow();
    const currentNow = t === 1 ? renderNow : this.visualNow();
    // The authoritative legacy call sampled the clock even though it did not
    // apply prediction. Preserve that observable ordering for injected clocks.
    this.visualNow();
    const interpolatedEntities = [];
    const currentEntities = t === 1 ? interpolatedEntities : [];
    const authoritativeEntities = [];
    for (const entity of this._cur.entities || []) {
      const prior = this._prevById.get(entity.id);
      const interpolated = interpolateEntity(entity, prior, t);
      interpolatedEntities.push(
        this._applyDisplayEntity(this._applyPredictedEntity(interpolated, renderNow), renderNow),
      );
      if (t !== 1) {
        const current = interpolateEntity(entity, prior, 1);
        currentEntities.push(
          this._applyDisplayEntity(this._applyPredictedEntity(current, currentNow), currentNow),
        );
      }
      authoritativeEntities.push(interpolateEntity(entity, prior, 1));
    }
    return {
      interpolatedEntities,
      currentEntities,
      authoritativeEntities,
      entityTraversals: 1,
    };
  }

  /**
   * Look up an entity by id in the current snapshot.
   * @param {number} id
   * @returns {object|undefined}
   */
  entityById(id) {
    const entity = this._curById.get(id);
    if (!entity) return entity;
    const now = this.visualNow();
    return this._applyDisplayEntity(this._applyPredictedEntity({ ...entity }, now), now);
  }

  progressPredictionDebug() {
    let productionBars = 0;
    let constructionBars = 0;
    for (const patch of this.predictionProgressById.values()) {
      if (patch.kind === "construction") constructionBars += 1;
      else productionBars += 1;
    }
    return {
      activeBars: this.predictionProgressById.size,
      paused: this.predictionProgressPaused,
      productionBars,
      constructionBars,
      correctionCount: Number(this.predictionDiagnostics?.progressCorrectionCount) || 0,
      lastCorrection: Number(this.predictionDiagnostics?.progressLastCorrection) || 0,
      maxCorrection: Number(this.predictionDiagnostics?.progressMaxCorrection) || 0,
      averageCorrection: Number(this.predictionDiagnostics?.progressAverageCorrection) || 0,
    };
  }

  setProgressPredictionPaused(paused) {
    this.predictionProgressPaused = paused === true;
  }

  applyPredictionDisplayOverlay(overlay = null) {
    if (!overlay || typeof overlay !== "object") {
      this._applyOptimisticCommandOverlay(null);
      this._clearPredictionFrameOverlay();
      return;
    }
    if (Object.prototype.hasOwnProperty.call(overlay, "optimisticCommands")) {
      this._applyOptimisticCommandOverlay(overlay.optimisticCommands);
    }
    if (Object.prototype.hasOwnProperty.call(overlay, "predictionFrame")) {
      if (overlay.predictionFrame) {
        this._applyPredictionFrameOverlay(overlay.predictionFrame, overlay.diagnostics ?? null, {
          smoothCorrections: !!overlay.smoothCorrections,
          includePose: overlay.includePose !== false,
        });
      } else {
        this._clearPredictionFrameOverlay();
      }
    }
  }

  setOptimisticCommandState(state = null) {
    this.applyPredictionDisplayOverlay({ optimisticCommands: state });
  }

  _applyOptimisticCommandOverlay(state = null) {
    this.optimisticProduction = [];
    this.optimisticProductionByBuilding.clear();
    this.optimisticRallyByBuilding.clear();
    for (const entry of state?.production || []) {
      if (typeof entry?.building !== "number") continue;
      const production = { ...entry, predicted: true };
      this.optimisticProduction.push(production);
      this.optimisticProductionByBuilding.set(entry.building, production);
    }
    for (const entry of state?.rally || []) {
      if (typeof entry?.building !== "number" || !Array.isArray(entry.plan)) continue;
      this.optimisticRallyByBuilding.set(entry.building, {
        ...entry,
        plan: entry.plan.map((stage) => ({ ...stage, predicted: true })),
        predicted: true,
      });
    }
  }

  setPredictionFrame(frame, diagnostics = null, { smoothCorrections = false, includePose = true } = {}) {
    this.applyPredictionDisplayOverlay({ predictionFrame: frame, diagnostics, smoothCorrections, includePose });
  }

  _applyPredictionFrameOverlay(frame, diagnostics = null, { smoothCorrections = false, includePose = true } = {}) {
    const patches = new Map();
    if (includePose) {
      for (const candidate of frame?.entities || []) {
        const authoritative = this._curById.get(candidate?.id);
        if (!authoritative || authoritative.owner !== this.playerId || !isUnit(authoritative.kind)) continue;
        const patch = normalizePredictionPatch(candidate);
        if (patch) patches.set(patch.id, patch);
      }
    }
    const progress = new Map();
    for (const candidate of frame?.progress || []) {
      const authoritative = this._curById.get(candidate?.id);
      const patch = normalizeProgressPatch(candidate);
      if (!patch || !progressPatchMatches(authoritative, patch, this.playerId, { isBuilding })) continue;
      progress.set(patch.id, patch);
    }
    const now = this.visualNow();
    const corrections = new Map();
    if (smoothCorrections) {
      for (const [id, next] of patches) {
        const prev = this.predictionPatchById.get(id);
        if (!prev) continue;
        const dx = prev.x - next.x;
        const dy = prev.y - next.y;
        const distance = Math.hypot(dx, dy);
        if (distance > 0.01 && distance <= PREDICTION_SMOOTH_MAX_PX) {
          corrections.set(id, { dx, dy, startedAt: now, durationMs: PREDICTION_SMOOTH_MS });
        }
      }
    }
    this.predictionPatchById = patches;
    this.predictionProgressById = progress;
    this.predictionCorrectionById = corrections;
    this.predictionDiagnostics = diagnostics;
  }

  clearPredictionFrame() {
    this.applyPredictionDisplayOverlay({ predictionFrame: null });
  }

  clearPredictionPose() {
    this.predictionPatchById.clear();
    this.predictionCorrectionById.clear();
  }

  _clearPredictionFrameOverlay() {
    this.predictionPatchById.clear();
    this.predictionProgressById.clear();
    this.predictionCorrectionById.clear();
    this.predictionDiagnostics = null;
  }

  _applyPredictedEntity(entity, now) {
    const patch = this.predictionPatchById.get(entity.id);
    if (!patch || entity.owner !== this.playerId || !isUnit(entity.kind)) return entity;
    const out = composePredictionPatch(entity, patch);
    const correction = this.predictionCorrectionById.get(entity.id);
    if (correction) {
      const age = now - correction.startedAt;
      if (age >= correction.durationMs) {
        this.predictionCorrectionById.delete(entity.id);
      } else {
        const remaining = 1 - age / correction.durationMs;
        out.x += correction.dx * remaining;
        out.y += correction.dy * remaining;
      }
    }
    return out;
  }

  _applyDisplayEntity(entity, now) {
    return this._applyOptimisticEntity(this._applyPredictedProgress(entity));
  }

  _applyPredictedProgress(entity) {
    const patch = this.predictionProgressById.get(entity?.id);
    if (!patch || !progressPatchMatches(entity, patch, this.playerId, { isBuilding })) return entity;
    return composeProgressPatch(entity, patch);
  }

  setRenderClock(renderClock) {
    if (!renderClock || typeof renderClock.now !== "function") throw new TypeError("GameState requires a render clock.");
    this.renderClock = renderClock;
  }

  visualNow() {
    return this.renderClock?.now?.() ?? performance.now();
  }

  _applyOptimisticEntity(entity) {
    if (!entity || entity.owner !== this.playerId || !isBuilding(entity.kind)) return entity;
    const out = { ...entity };
    const production = this.optimisticProductionByBuilding.get(entity.id);
    if (production) {
      out.prodQueue = Math.max(out.prodQueue ?? 0, production.optimisticQueue ?? 1);
      if (!out.prodKind) out.prodKind = production.unit;
      if (out.prodProgress == null) out.prodProgress = 0;
      out.optimisticProduction = true;
    }
    const rally = this.optimisticRallyByBuilding.get(entity.id);
    if (rally) {
      out.rallyPlan = rally.plan.map((stage) => ({ ...stage }));
      out.optimisticRally = true;
    }
    return out;
  }

  _applyResourceDeltas(deltas) {
    for (const delta of deltas) {
      if (!delta || typeof delta.id !== "number") continue;
      const node = this.resourceById.get(delta.id);
      if (!node || typeof delta.remaining !== "number") continue;
      node.remaining = delta.remaining;
    }
  }

  _applyResourceDeaths(events) {
    for (const ev of events) {
      if (!ev || ev.e !== "death" || typeof ev.id !== "number") continue;
      const node = this.resourceById.get(ev.id);
      if (node) node.remaining = 0;
    }
  }

  _resourceEntityViews() {
    return (this.map.resources || []).flatMap((n) => (n.remaining === 0 ? [] : [{ ...n }]));
  }

  // --- selection (client-only) -------------------------------------------

  /**
   * Resolve the current selection to live entities, dropping ids that no
   * longer exist in the current snapshot (e.g. units that died).
   * @returns {Array<object>}
   */
  selectedEntities() {
    this._pruneSelection();
    const out = [];
    for (const id of this.selection) {
      const e = this._curById.get(id);
      if (e && !e.shotReveal && !e.visionOnly) out.push(this._applyDisplayEntity(e, this.visualNow()));
    }
    return out;
  }

  /**
   * Replace the selection with the given ids.
   * @param {Iterable<number>} ids
   */
  setSelection(ids, options = {}) {
    const admitted = admitSelectionIds(this, ids, options);
    this.selection = new Set(admitted.ids);
    this._recordSelectionBudgetOverflow(admitted);
  }

  /**
   * Add ids to the existing selection.
   * @param {Iterable<number>} ids
   */
  addToSelection(ids, options = {}) {
    this._pruneSelection();
    const admitted = admitSelectionIds(this, ids, { ...options, baseIds: this.selection });
    this.selection = new Set(admitted.ids);
    this._recordSelectionBudgetOverflow(admitted);
  }

  /**
   * Remove ids from the existing selection.
   * @param {Iterable<number>} ids
   */
  removeFromSelection(ids) {
    this._pruneSelection();
    for (const id of ids) {
      this.selection.delete(id);
    }
    this.selectionBudgetOverflow = null;
  }

  /** Clear the selection. */
  clearSelection() {
    this.selection.clear();
    this.selectionBudgetOverflow = null;
  }

  /** Drop missing ids and optionally restore a legal command budget. */
  _pruneSelection(options = {}) {
    const trimmed = pruneSelection(this, options);
    if (trimmed) this._recordSelectionBudgetOverflow(trimmed);
  }

  _recordSelectionBudgetOverflow(admitted) {
    this.selectionBudgetOverflow = admitted?.overflow
      ? { used: admitted.used, cap: admitted.cap, seq: ++this._selectionBudgetOverflowSeq }
      : null;
  }

  /**
   * Replace a control group with currently-live own units/buildings.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @param {Iterable<number>} ids selected ids to store.
   * @returns {Array<number>} stored ids.
   */
  setControlGroup(slot, ids, options = {}) {
    if (!this._validControlGroupSlot(slot)) return [];
    const admitted = this._admitControlGroupIds(ids, options);
    this.controlGroups[slot] = admitted.ids;
    this._recordSelectionBudgetOverflow(admitted);
    return admitted.ids.slice();
  }

  /**
   * Add currently-live own units/buildings to a control group, ignoring overflow.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @param {Iterable<number>} ids selected ids to add.
   * @returns {Array<number>} stored ids after the add.
   */
  addToControlGroup(slot, ids, options = {}) {
    if (!this._validControlGroupSlot(slot)) return [];
    this._pruneControlGroup(slot, options);
    const admitted = this._admitControlGroupIds(ids, { ...options, baseIds: this.controlGroups[slot] || [] });
    this.controlGroups[slot] = admitted.ids;
    this._recordSelectionBudgetOverflow(admitted);
    return admitted.ids.slice();
  }

  /**
   * Resolve a control group to live entities, pruning dead/stale ids first.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @param {{controlPolicy?: object|null}} [options] injected ownership policy.
   * @returns {Array<object>}
   */
  controlGroupEntities(slot, options = {}) {
    if (!this._validControlGroupSlot(slot)) return [];
    this._pruneControlGroup(slot, options);
    const out = [];
    for (const id of this.controlGroups[slot]) {
      const e = this._curById.get(id);
      if (e) out.push(e);
    }
    return out;
  }

  /**
   * Select a control group if it has live members.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @returns {Array<number>} selected ids.
   */
  selectControlGroup(slot, options = {}) {
    if (!this._validControlGroupSlot(slot)) return [];
    const pruned = this._pruneControlGroup(slot, options);
    const ids = this.controlGroups[slot] || [];
    if (ids.length === 0) return [];
    this.setSelection(ids, options);
    if (pruned?.overflow) this._recordSelectionBudgetOverflow(pruned);
    return Array.from(this.selection);
  }

  _validControlGroupSlot(slot) {
    return Number.isInteger(slot) && slot >= 0 && slot < this.controlGroups.length;
  }

  _admitControlGroupIds(ids, { baseIds = [], entityById = null, controlPolicy = null } = {}) {
    return admitControlGroupIds(this, ids, { baseIds, entityById, controlPolicy });
  }

  _pruneControlGroups(options = {}) {
    for (let i = 0; i < this.controlGroups.length; i++) this._pruneControlGroup(i, options);
  }

  _pruneControlGroup(slot, options = {}) {
    const group = this.controlGroups[slot];
    if (!group || group.length === 0) return null;
    const admitted = this._admitControlGroupIds(group, options);
    if (admitted.ids.length !== group.length || admitted.ids.some((id, index) => id !== group[index])) {
      this.controlGroups[slot] = admitted.ids;
    }
    return admitted;
  }

  // --- map helpers --------------------------------------------------------

  /**
   * Whether a world point lies inside the map bounds.
   * @param {number} wx world x in pixels
   * @param {number} wy world y in pixels
   * @returns {boolean}
   */
  worldInBounds(wx, wy) {
    return queryWorldInBounds(this.map, wx, wy);
  }

  /**
   * Terrain code at a tile, or null if out of bounds.
   * @param {number} tileX
   * @param {number} tileY
   * @returns {number|null} a TERRAIN code, or null.
   */
  terrainAt(tileX, tileY) {
    return queryTerrainAt(this.map, tileX, tileY);
  }

  /**
   * Whether a tile is passable per the PASSABLE table (useful to input/index.js for
   * placement validity). Out-of-bounds tiles are impassable.
   * @param {number} tileX
   * @param {number} tileY
   * @returns {boolean}
   */
  isPassable(tileX, tileY) {
    return queryIsPassable(this.map, tileX, tileY);
  }
}
