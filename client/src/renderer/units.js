import { STATS } from "../config.js";
import { KIND, SETUP, STATE } from "../protocol.js";
import { liveRigDefinitionFor, liveRigRoutesFor } from "./rigs/live_routing.js";
import { livePngRigAtlasFor } from "./rigs/png_routing.js";
import { createRigRenderContext } from "./rigs/animation.js";
import { pngAtlasCanRenderRoute, renderPngUnitRig } from "./rigs/png_runtime.js";
import { renderLiveUnitRig } from "./rigs/runtime.js";
import {
  ARTILLERY_DEPLOYED_WEAPON_ANIM_MS,
  DEPLOYED_WEAPON_ANIM_MS,
} from "./palette.js";
import {
  angleDelta,
  clamp01,
  isVehicleBodyKind,
  smoothstep01,
  tankBodyVisual,
} from "./shared.js";

export function _deployedWeaponSetupVisual(e) {
  const now = performance.now();
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
  const t = smoothstep01(elapsed / durationMs);

  if (setupState === SETUP.SETTING_UP) {
    return { prongFactor: t, barrel: false };
  }
  if (setupState === SETUP.TEARING_DOWN) {
    return { prongFactor: 1 - t, barrel: false };
  }
  if (setupState === SETUP.DEPLOYED) {
    return { prongFactor: 1, barrel: e.state !== STATE.MOVE };
  }
  return { prongFactor: 0, barrel: false };
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

function unitVehicleBody(kind, stat) {
  if (kind === KIND.ARTILLERY) return tankBodyVisual(stat);
  return isVehicleBodyKind(kind) ? tankBodyVisual(stat) : null;
}

export function _drawUnit(e, colorByOwner, state, pools = {}) {
  const definition = liveRigDefinitionFor(this._liveRigDefinitionsByKind, e.kind);
  if (!definition) {
    throw new Error(`missing live SVG rig definition for unit kind ${e.kind}`);
  }

  const routes = liveRigRoutesFor(e.kind, pools);
  if (routes.length === 0) {
    throw new Error(`missing live SVG rig route for unit kind ${e.kind}`);
  }

  const pngAtlas = livePngRigAtlasFor(this._livePngRigAtlasesByKind, e.kind);
  const pngAtlasTexture = this._livePngRigAtlasTextures?.get?.(e.kind) ?? null;
  if (pngAtlas && pngAtlasTexture) {
    const renderContext = this._rigRenderContextFor?.(e, colorByOwner, state) ?? {};
    const rendered = [];
    for (const route of routes) {
      if (pngAtlasCanRenderRoute(definition, pngAtlas, route)) {
        rendered.push(...(renderPngUnitRig(this, e, colorByOwner, state, definition, {
          atlas: pngAtlas,
          atlasTexture: pngAtlasTexture,
          routes: [route],
          alpha: pools.alpha,
          renderContext,
        }) || []));
      } else {
        rendered.push(...(renderLiveUnitRig(this, e, colorByOwner, state, definition, {
          routes: [route],
          alpha: pools.alpha,
          renderContext,
        }) || []));
      }
    }
    return rendered;
  }

  return renderLiveUnitRig(this, e, colorByOwner, state, definition, {
    routes,
    alpha: pools.alpha,
  });
}

export function _rigRenderContextFor(e, colorByOwner, state) {
  const facing = typeof e.facing === "number" ? e.facing : 0;
  const stat = STATS[e.kind] || {};
  const body = unitVehicleBody(e.kind, stat);
  return createRigRenderContext(e, {
    state,
    colorByOwner,
    setupVisual: this._deployedWeaponSetupVisual(e),
    vehicleMotion: body ? this._tankMotionVisual(e, facing, state, body) : null,
    selected: state.selection?.has?.(e.id) ?? false,
    map: this._map,
    occupiedTrench: hasOccupiedTrench(e),
  });
}

function hasOccupiedTrench(entity) {
  const id = Number(entity?.occupiedTrenchId);
  return Number.isInteger(id) && id > 0;
}

export function _drawShotRevealUnit(e, colorByOwner, state) {
  const now = performance.now();
  const age = Math.max(0, now - (e.shotRevealCreatedAt || now));
  const ttl = Math.max(1, (e.shotRevealExpiresAt || now + 1) - (e.shotRevealCreatedAt || now));
  const t = clamp01(age / ttl);
  const alpha = 0.82 * (1 - smoothstep01(Math.max(0, t - 0.62) / 0.38));
  this._drawUnit(e, colorByOwner, state, {
    shadow: "shotRevealShadows",
    unit: "shotReveals",
    effects: "shotReveals",
    liveRigShadow: "liveShotRevealRigShadows",
    liveRigUnit: "liveShotRevealRigs",
    liveRigEffects: "liveShotRevealRigEffects",
    alpha,
  });
}
