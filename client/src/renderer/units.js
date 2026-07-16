import { SNAPSHOT_MS, STATS } from "../config.js";
import { KIND, SETUP, STATE } from "../protocol.js";
import { liveRigDefinitionFor, liveRigKeyForEntity, liveRigRoutesFor } from "./rigs/live_routing.js";
import { liveFrameStripFor } from "./rigs/frame_strip_routing.js";
import { livePngRigAtlasFor } from "./rigs/png_routing.js";
import { createRigRenderContext, sampleRigAnimation } from "./rigs/animation.js";
import { renderFrameStripUnit } from "./rigs/frame_strip_runtime.js";
import { pngAtlasRouteCoverage, renderPngUnitRig } from "./rigs/png_runtime.js";
import { renderLiveUnitRig } from "./rigs/runtime.js";
import {
  ARTILLERY_DEPLOYED_WEAPON_ANIM_MS,
  DEPLOYED_WEAPON_ANIM_MS,
} from "./palette.js";
import {
  angleDelta,
  clamp01,
  isVehicleBodyKind,
  rendererVisualNow,
  smoothstep01,
  tankBodyVisual,
} from "./shared.js";

const FRAME_STRIP_MOVEMENT_HOLD_MS = SNAPSHOT_MS * 3;

export function _deployedWeaponSetupVisual(e) {
  const now = rendererVisualNow(this);
  const setupState = e.setupState || SETUP.PACKED;
  const prev = this._setupVisuals.get(e.id);
  if (!prev || prev.state !== setupState) {
    this._setupVisuals.set(e.id, { state: setupState, changedAt: now });
  }
  const rec = this._setupVisuals.get(e.id);
  const elapsed = now - rec.changedAt;
  const durationMs = e.kind === KIND.ARTILLERY
    ? ARTILLERY_DEPLOYED_WEAPON_ANIM_MS
    : DEPLOYED_WEAPON_ANIM_MS;
  const progress = clamp01(elapsed / durationMs);
  const t = smoothstep01(progress);

  if (setupState === SETUP.SETTING_UP) {
    return { prongFactor: t, frameProgress: progress, barrel: false };
  }
  if (setupState === SETUP.TEARING_DOWN) {
    return { prongFactor: 1 - t, frameProgress: 1 - progress, barrel: false };
  }
  if (setupState === SETUP.DEPLOYED) {
    return { prongFactor: 1, frameProgress: 1, barrel: e.state !== STATE.MOVE };
  }
  return { prongFactor: 0, frameProgress: 0, barrel: false };
}

export function _sweepSetupVisuals(liveIds) {
  for (const id of [...this._setupVisuals.keys()]) {
    if (!liveIds.has(id)) this._setupVisuals.delete(id);
  }
}

export function _sweepTankMotion(liveIds) {
  for (const id of [...this._tankMotion.keys()]) {
    if (!liveIds.has(id)) this._tankMotion.delete(id);
  }
}

export function _sweepFrameStripMotion(liveIds) {
  for (const id of [...this._frameStripMotion.keys()]) {
    if (!liveIds.has(id)) this._frameStripMotion.delete(id);
  }
}

export function _tankMotionVisual(e, facing, state, body) {
  const prev = this._tankMotion.get(e.id);
  let leftPhase = prev ? prev.leftPhase : 0;
  let rightPhase = prev ? prev.rightPhase : 0;
  let leftDir = 0;
  let rightDir = 0;
  let activity = 0;

  if (prev) {
    const dx = e.x - prev.x;
    const dy = e.y - prev.y;
    const dist = Math.hypot(dx, dy);
    const turn = angleDelta(prev.facing, facing);
    const avgFacing = prev.facing + turn * 0.5;
    const forward = Math.cos(avgFacing);
    const forwardY = Math.sin(avgFacing);
    const forwardMove = dx * forward + dy * forwardY;
    const lateralMove = -dx * forwardY + dy * forward;
    const drive = Math.abs(forwardMove) >= Math.abs(lateralMove) * 0.5
      ? forwardMove
      : Math.sign(forwardMove || 1) * dist;
    const turnTravel = turn * body.halfWidth;
    const leftDelta = drive - turnTravel;
    const rightDelta = drive + turnTravel;
    leftPhase += leftDelta;
    rightPhase += rightDelta;
    leftDir = Math.sign(leftDelta);
    rightDir = Math.sign(rightDelta);
    activity = clamp01((Math.abs(leftDelta) + Math.abs(rightDelta)) / 4);
  }

  const ownTank = e.owner === state.playerId;
  const oil = state.resources ? state.resources.oil : null;
  const oilStarved = ownTank && oil === 0 && (e.state === STATE.MOVE || e.state === STATE.ATTACK);
  const lowOil = ownTank && typeof oil === "number" && oil > 0 && oil <= 5;
  const next = { x: e.x, y: e.y, facing, leftPhase, rightPhase };
  this._tankMotion.set(e.id, next);
  return { leftPhase, rightPhase, leftDir, rightDir, activity, lowOil, oilStarved };
}

export function _frameStripMovementVisual(e, state) {
  const now = rendererVisualNow(this);
  const previousMotion = this._frameStripMotion?.get?.(e.id);
  const snapshotMotion = snapshotMovementSample(e, state);
  const renderMoving = renderedPositionChanged(e, this._frameStripMotion);
  const moveState = e?.state === STATE.MOVE;
  const freshSnapshot = snapshotMotion != null &&
    authoritativeSampleChanged(snapshotMotion, previousMotion);
  const observedMovement = moveState && (
    renderMoving ||
    (freshSnapshot && snapshotMotion.moving) ||
    (snapshotMotion == null && previousMotion == null)
  );
  let lastMovementAt = null;
  if (moveState) {
    lastMovementAt = observedMovement ? now : previousMotion?.lastMovementAt ?? null;
  }
  if (this._frameStripMotion) {
    this._frameStripMotion.set(e.id, {
      x: finite(e.x, 0),
      y: finite(e.y, 0),
      snapshotTick: snapshotMotion?.tick ?? null,
      snapshotX: snapshotMotion?.x ?? null,
      snapshotY: snapshotMotion?.y ?? null,
      lastMovementAt,
    });
  }
  const active = lastMovementAt != null &&
    now - lastMovementAt <= FRAME_STRIP_MOVEMENT_HOLD_MS;
  return {
    moving: Boolean(active),
    activity: active ? 1 : 0,
  };
}

function unitVehicleBody(kind, stat) {
  if (kind === KIND.ARTILLERY) return tankBodyVisual(stat);
  return isVehicleBodyKind(kind) ? tankBodyVisual(stat) : null;
}

export function _drawUnit(e, colorByOwner, state, pools = {}) {
  const visualOverride = pools.visualOverride || null;
  const rigKey = liveRigKeyForEntity(e);
  const definition = visualOverride?.definition || liveRigDefinitionFor(this._liveRigDefinitionsByKind, rigKey);
  if (!definition) {
    throw new Error(`missing live SVG rig definition for unit kind ${e.kind}`);
  }

  const routes = liveRigRoutesFor(rigKey, pools);
  if (routes.length === 0) {
    throw new Error(`missing live SVG rig route for unit kind ${e.kind}`);
  }

  const visualFrameStrip = !visualOverride ? pools.visualFrameStrip || null : null;
  const frameStrip = visualFrameStrip?.strip || (visualOverride ? null : liveFrameStripFor(this._liveFrameStripsByKind, rigKey));
  const frameStripTexture = visualFrameStrip?.texture || this._liveFrameStripTextures?.get?.(rigKey) || null;
  if (frameStrip && frameStripTexture) {
    const renderContext = this._rigRenderContextFor?.(e, colorByOwner, state) ?? {};
    applyRigAlpha(renderContext, pools.alpha);
    const frameStripMovement = this._frameStripMovementVisual?.(e, state);
    if (frameStripMovement) {
      renderContext.frameStripMoving = frameStripMovement.moving;
      renderContext.frameStripMovementActivity = frameStripMovement.activity;
    }
    const rendered = [];
    const activePoolNames = new Set();
    const shadowRoute = routes.find((route) => route.parts?.includes?.("part.shadow"));
    if (shadowRoute) {
      const sampledAnimation = sampleRigAnimation(definition, e, renderContext, {
        includeParts: shadowRoute.parts,
      });
      activePoolNames.add(shadowRoute.poolName);
      rendered.push(...(renderLiveUnitRig(this, e, colorByOwner, state, definition, {
        routes: [shadowRoute],
        alpha: pools.alpha,
        renderContext,
        sampledAnimation,
      }) || []));
    }
    const poolName = pools.liveRigUnit || "liveUnitRigs";
    activePoolNames.add(poolName);
    const stripInstance = renderFrameStripUnit(this, e, frameStrip, frameStripTexture, {
      poolName,
      layerName: pools.unit || "units",
      alpha: pools.alpha,
      renderContext,
    });
    if (stripInstance) rendered.push(stripInstance);
    destroyInactiveLiveRigInstances(this, e.id, activePoolNames);
    return rendered;
  }

  const pngAtlas = visualOverride ? null : livePngRigAtlasFor(this._livePngRigAtlasesByKind, rigKey);
  const pngAtlasTexture = this._livePngRigAtlasTextures?.get?.(rigKey) ?? null;
  if (pngAtlas && pngAtlasTexture) {
    const renderContext = this._rigRenderContextFor?.(e, colorByOwner, state) ?? {};
    applyRigAlpha(renderContext, pools.alpha);
    const rendered = [];
    const activePoolNames = new Set();
    const routedCoverage = routes.map((route) => ({
      route,
      coverage: pngAtlasRouteCoverage(definition, pngAtlas, route),
    }));
    const sampledParts = new Set();
    for (const { coverage } of routedCoverage) {
      for (const partId of coverage.animationParts) sampledParts.add(partId);
      for (const partId of coverage.missingParts) sampledParts.add(partId);
    }
    const sampledAnimation = sampleRigAnimation(definition, e, renderContext, {
      includeParts: sampledParts,
    });
    for (const { route, coverage } of routedCoverage) {
      if (coverage.coveredParts.length > 0) {
        const pngRoute = coverage.missingParts.length === 0
          ? route
          : { ...route, parts: coverage.coveredParts };
        activePoolNames.add(pngRoute.poolName);
        rendered.push(...(renderPngUnitRig(this, e, colorByOwner, state, definition, {
          atlas: pngAtlas,
          atlasTexture: pngAtlasTexture,
          routes: [pngRoute],
          alpha: pools.alpha,
          renderContext,
          sampledAnimation,
          routesCovered: true,
        }) || []));
      }
      if (coverage.missingParts.length > 0) {
        const svgRoute = coverage.coveredParts.length > 0
          ? { ...pngFallbackSvgRoute(route, pools), parts: coverage.missingParts }
          : { ...route, parts: coverage.missingParts };
        activePoolNames.add(svgRoute.poolName);
        rendered.push(...(renderLiveUnitRig(this, e, colorByOwner, state, definition, {
          routes: [svgRoute],
          alpha: pools.alpha,
          renderContext,
          sampledAnimation,
        }) || []));
      }
    }
    destroyInactiveLiveRigInstances(this, e.id, activePoolNames);
    return rendered;
  }

  destroyInactiveLiveRigInstances(this, e.id, routePoolNames(routes));
  const renderContext = this._rigRenderContextFor?.(e, colorByOwner, state) ?? {};
  applyRigAlpha(renderContext, pools.alpha);
  const sampledAnimation = sampleRigAnimation(definition, e, renderContext, {
    includeParts: routePartIds(routes),
  });
  return renderLiveUnitRig(this, e, colorByOwner, state, definition, {
    routes,
    alpha: pools.alpha,
    renderContext,
    sampledAnimation,
  });
}

function applyRigAlpha(renderContext, alpha) {
  if (typeof alpha === "number") renderContext.shotRevealAlpha = alpha;
}

function routePartIds(routes) {
  const parts = new Set();
  for (const route of routes || []) {
    for (const partId of route.parts || []) parts.add(partId);
  }
  return parts;
}

function pngFallbackSvgRoute(route, pools = {}) {
  return {
    ...route,
    poolName: pools.liveRigOverlay || "liveUnitRigOverlays",
    layerName: pools.overlay || route.layerName,
  };
}

function routePoolNames(...routeGroups) {
  const out = new Set();
  for (const routes of routeGroups) {
    for (const route of routes || []) out.add(route.poolName);
  }
  return out;
}

const UNIT_RIG_POOL_NAMES = Object.freeze([
  "liveUnitRigShadows",
  "liveUnitRigs",
  "liveUnitRigOverlays",
  "liveUnitRigEffects",
  "liveShotRevealRigShadows",
  "liveShotRevealRigs",
  "liveShotRevealRigOverlays",
  "liveShotRevealRigEffects",
]);

function destroyInactiveLiveRigInstances(renderer, entityId, activePoolNames = new Set()) {
  for (const poolName of UNIT_RIG_POOL_NAMES) {
    if (activePoolNames.has(poolName)) continue;
    const pool = renderer._liveRigPools?.[poolName];
    const instance = pool?.get?.(entityId);
    if (!instance) continue;
    instance.destroy?.();
    pool.delete(entityId);
    renderer._seen?.[poolName]?.delete?.(entityId);
    renderer._recordRenderDiagnostic?.(`renderer.rig.instance.destroyed.unused.${poolName}`);
  }
}

export function _rigRenderContextFor(e, colorByOwner, state) {
  const facing = typeof e.facing === "number" ? e.facing : 0;
  const stat = STATS[e.kind] || {};
  const body = unitVehicleBody(e.kind, stat);
  const context = createRigRenderContext(e, {
    state,
    colorByOwner,
    setupVisual: this._deployedWeaponSetupVisual(e),
    vehicleMotion: body ? this._tankMotionVisual(e, facing, state, body) : null,
    selected: state.selection?.has?.(e.id) ?? false,
    map: this._map,
    occupiedTrench: hasOccupiedTrench(e),
    now: rendererVisualNow(this),
  });
  return context;
}

function hasOccupiedTrench(entity) {
  const id = Number(entity?.occupiedTrenchId);
  return Number.isInteger(id) && id > 0;
}

function snapshotMovementSample(entity, state) {
  const current = state?._curById?.get?.(entity?.id);
  const previous = state?._prevById?.get?.(entity?.id);
  if (!current || !previous) return null;
  if (!Number.isFinite(current.x) || !Number.isFinite(current.y)) return null;
  if (!Number.isFinite(previous.x) || !Number.isFinite(previous.y)) return null;
  return {
    moving: distanceSq(current.x - previous.x, current.y - previous.y) > 0.0025,
    tick: Number.isFinite(state?.tick) ? state.tick : null,
    x: current.x,
    y: current.y,
  };
}

function authoritativeSampleChanged(sample, previousMotion) {
  if (!previousMotion) return true;
  if (sample.tick != null && previousMotion.snapshotTick != null) {
    return sample.tick !== previousMotion.snapshotTick;
  }
  return sample.x !== previousMotion.snapshotX || sample.y !== previousMotion.snapshotY;
}

function renderedPositionChanged(entity, motion) {
  const previous = motion?.get?.(entity?.id);
  if (!previous) return false;
  if (!Number.isFinite(entity?.x) || !Number.isFinite(entity?.y)) return false;
  return distanceSq(entity.x - previous.x, entity.y - previous.y) > 0.0025;
}

function distanceSq(dx, dy) {
  return dx * dx + dy * dy;
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}

export function _drawShotRevealUnit(e, colorByOwner, state, pools = {}) {
  const now = rendererVisualNow(this);
  const age = Math.max(0, now - (e.shotRevealCreatedAt || now));
  const ttl = Math.max(1, (e.shotRevealExpiresAt || now + 1) - (e.shotRevealCreatedAt || now));
  const t = clamp01(age / ttl);
  const alpha = 0.82 * (1 - smoothstep01(Math.max(0, t - 0.62) / 0.38));
  this._drawUnit(e, colorByOwner, state, {
    visualOverride: pools.visualOverride || null,
    visualFrameStrip: pools.visualFrameStrip || null,
    shadow: "shotRevealShadows",
    unit: "shotReveals",
    effects: "shotReveals",
    liveRigShadow: "liveShotRevealRigShadows",
    liveRigUnit: "liveShotRevealRigs",
    liveRigOverlay: "liveShotRevealRigOverlays",
    liveRigEffects: "liveShotRevealRigEffects",
    alpha,
  });
}
