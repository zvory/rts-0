import { frameStripMovementVisual } from "./frame_strip_movement.js";
import { supportWeaponSetupVisual } from "./support_weapon_setup_visual.js";
import { STATS } from "../config.js";
import { KIND } from "../protocol.js";
import {
  isRasterRigKey,
  liveRigDefinitionFor,
  liveRigKeyForEntity,
  liveRigRoutePlanFor,
} from "./rigs/live_routing.js";
import { liveFrameStripFor, liveFrameStripKeyForEntity } from "./rigs/frame_strip_routing.js";
import { livePngRigAtlasFor } from "./rigs/png_routing.js";
import { createRigRenderContext, sampleRigAnimationInto } from "./rigs/animation.js";
import {
  animationStageFor,
  frameStripDrawPlanFor,
  pngDrawPlanFor,
  reconcileActiveLiveRigPools,
} from "./rigs/draw_plans.js";
import { renderFrameStripUnit } from "./rigs/frame_strip_runtime.js";
import { renderPngUnitRig } from "./rigs/png_runtime.js";
import { renderLiveUnitRig } from "./rigs/runtime.js";
import { hasProjectedUnitShadow, shadowSunModel } from "./unified_gpu_shadows.js";
import {
  angleDelta,
  clamp01,
  isVehicleBodyKind,
  rendererVisualNow,
  smoothstep01,
  tankBodyVisual,
} from "./shared.js";


export function _deployedWeaponSetupVisual(e) {
  return supportWeaponSetupVisual(e, this?._setupVisuals, this ? rendererVisualNow(this) : 0);
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

  const next = { x: e.x, y: e.y, facing, leftPhase, rightPhase };
  this._tankMotion.set(e.id, next);
  return { leftPhase, rightPhase, leftDir, rightDir, activity };
}

export function _frameStripMovementVisual(e, state) {
  return frameStripMovementVisual(e, state, this._frameStripMotion, rendererVisualNow(this));
}

function unitVehicleBody(kind, stat) {
  if (kind === KIND.ARTILLERY) return tankBodyVisual(stat);
  return isVehicleBodyKind(kind) ? tankBodyVisual(stat) : null;
}

export function _drawUnit(e, colorByOwner, state, pools = {}) {
  const projectedShadow = Boolean(pools.projectedShadow && hasProjectedUnitShadow(e.kind));
  const visualOverride = pools.visualOverride || null;
  const rigKey = liveRigKeyForEntity(e);
  const definition = visualOverride?.definition || liveRigDefinitionFor(this._liveRigDefinitionsByKind, rigKey);
  if (!definition) {
    throw new Error(`missing live rig definition for unit kind ${e.kind}`);
  }

  const routePlan = liveRigRoutePlanFor(rigKey, pools);
  if (routePlan.routes.length === 0) {
    throw new Error(`missing live rig route for unit kind ${e.kind}`);
  }

  const frameStripKey = liveFrameStripKeyForEntity(e);
  const visualFrameStrip = !visualOverride ? pools.visualFrameStrip
    || (e.kind === KIND.RIFLEMAN ? this._experimentalRiflemanFrameStrip?.() : null) || null : null;
  const frameStrip = visualFrameStrip?.strip || (visualOverride ? null : liveFrameStripFor(this._liveFrameStripsByKind, frameStripKey));
  const frameStripTexture = visualFrameStrip?.texture || this._liveFrameStripTextures?.get?.(frameStripKey) || null;
  if (frameStrip && !frameStripTexture) {
    throw new Error(`missing live PNG frame-strip texture for unit kind ${e.kind}`);
  }
  if (frameStrip) {
    const renderContext = pools.renderContext || this._rigRenderContextFor?.(e, colorByOwner, state) || {};
    applyRigAlpha(renderContext, pools.alpha);
    const frameStripMovement = pools.renderContext ? null : this._frameStripMovementVisual?.(e, state);
    if (frameStripMovement) {
      renderContext.frameStripMoving = frameStripMovement.moving;
      renderContext.frameStripMovementActivity = frameStripMovement.activity;
    }
    rememberRigRenderContext(this, e, pools, renderContext);
    const drawPlan = frameStripDrawPlanFor(routePlan);
    if (drawPlan.shadowRoute && !projectedShadow && frameStrip.shadowHeightTiles) {
      // A flat airborne sprite casts the same silhouette, translated away from the sun.
      const sun = shadowSunModel(this._map?.sun);
      const distance = frameStrip.shadowHeightTiles * (this._map?.tileSize || 32) / sun.slope;
      renderFrameStripUnit(this, e, frameStrip, frameStripTexture, {
        poolName: drawPlan.shadowRoute.poolName,
        layerName: drawPlan.shadowRoute.layerName,
        alpha: pools.alpha,
        renderContext,
        silhouetteShadow: true,
        shadowOffset: { x: -sun.directionX * distance, y: -sun.directionY * distance },
      });
    } else if (drawPlan.shadowRoute && !projectedShadow && !frameStrip.groundContactShadow) {
      const sampledAnimation = sampleRigAnimationInto(
        animationStageFor(definition, drawPlan.sampledParts),
        e,
        renderContext,
      );
      renderLiveUnitRig(this, e, colorByOwner, state, definition, {
        route: drawPlan.shadowRoute,
        alpha: pools.alpha,
        renderContext,
        sampledAnimation,
        collectResults: false,
      });
    }
    renderFrameStripUnit(this, e, frameStrip, frameStripTexture, {
      poolName: drawPlan.unitRoute.poolName,
      layerName: drawPlan.unitRoute.layerName,
      alpha: pools.alpha,
      renderContext,
    });
    reconcileActiveLiveRigPools(
      this,
      e.id,
      projectedShadow
        ? drawPlan.activePoolNames.filter((name) => name !== drawPlan.shadowRoute?.poolName)
        : drawPlan.activePoolNames,
    );
    return null;
  }

  const pngAtlas = visualOverride ? null : livePngRigAtlasFor(this._livePngRigAtlasesByKind, rigKey);
  const pngAtlasTexture = this._livePngRigAtlasTextures?.get?.(rigKey) ?? null;
  if (!visualOverride && isRasterRigKey(rigKey) && !frameStrip && !pngAtlas) {
    throw new Error(`missing production PNG route for unit kind ${e.kind}`);
  }
  if (pngAtlas && !pngAtlasTexture) {
    throw new Error(`missing live PNG atlas texture for unit kind ${e.kind}`);
  }
  if (pngAtlas) {
    const renderContext = pools.renderContext || this._rigRenderContextFor?.(e, colorByOwner, state) || {};
    applyRigAlpha(renderContext, pools.alpha);
    rememberRigRenderContext(this, e, pools, renderContext);
    const drawPlan = pngDrawPlanFor(definition, pngAtlas, routePlan);
    const sampledAnimation = sampleRigAnimationInto(
      animationStageFor(definition, drawPlan.sampledParts),
      e,
      renderContext,
    );
    for (const step of drawPlan.steps) {
      if (projectedShadow && step.route === routePlan.shadowRoute) continue;
      if (step.runtime === "png") {
        renderPngUnitRig(this, e, colorByOwner, state, definition, {
          atlas: pngAtlas,
          atlasTexture: pngAtlasTexture,
          route: step.route,
          alpha: pools.alpha,
          renderContext,
          sampledAnimation,
          routesCovered: true,
          collectResults: false,
        });
      } else {
        renderLiveUnitRig(this, e, colorByOwner, state, definition, {
          route: step.route,
          alpha: pools.alpha,
          renderContext,
          sampledAnimation,
          collectResults: false,
        });
      }
    }
    reconcileActiveLiveRigPools(
      this,
      e.id,
      projectedShadow
        ? drawPlan.activePoolNames.filter((name) => name !== routePlan.shadowRoute?.poolName)
        : drawPlan.activePoolNames,
    );
    return null;
  }

  const routes = projectedShadow
    ? routePlan.routes.filter((route) => route !== routePlan.shadowRoute)
    : routePlan.routes;
  reconcileActiveLiveRigPools(this, e.id, routes.map((route) => route.poolName));
  const renderContext = pools.renderContext || this._rigRenderContextFor?.(e, colorByOwner, state) || {};
  applyRigAlpha(renderContext, pools.alpha);
  rememberRigRenderContext(this, e, pools, renderContext);
  const sampledAnimation = sampleRigAnimationInto(
    animationStageFor(definition, routePlan.allParts),
    e,
    renderContext,
  );
  return renderLiveUnitRig(this, e, colorByOwner, state, definition, {
    routes,
    alpha: pools.alpha,
    renderContext,
    sampledAnimation,
    collectResults: false,
  });
}

function applyRigAlpha(renderContext, alpha) {
  if (typeof alpha === "number") renderContext.shotRevealAlpha = alpha;
}

function rememberRigRenderContext(renderer, entity, pools, renderContext) {
  if (!pools.rememberRenderContext) return;
  renderer._unitRenderContexts?.set?.(entity.id, { ...renderContext });
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

export function _drawShotRevealUnit(e, colorByOwner, state, pools = {}) {
  const now = rendererVisualNow(this);
  const timedReveal = Number.isFinite(e.shotRevealCreatedAt) && Number.isFinite(e.shotRevealExpiresAt);
  const age = timedReveal ? Math.max(0, now - e.shotRevealCreatedAt) : 0;
  const ttl = timedReveal ? Math.max(1, e.shotRevealExpiresAt - e.shotRevealCreatedAt) : 1;
  const t = clamp01(age / ttl);
  const alpha = timedReveal ? 1 - smoothstep01(Math.max(0, t - 0.8) / 0.2) : 1;
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
