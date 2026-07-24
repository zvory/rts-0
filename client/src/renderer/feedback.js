import { gfxNoFill, gfxCircle, gfxEllipse, gfxPoly, gfxRect, gfxRoundRect, gfxStrokeLine, gfxStrokePaths, gfxReset, gfxFill, gfxStroke } from "./native_graphics.js";
import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
  STATS,
  RESOURCE_AMOUNTS,
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  MORTAR_INNER_RADIUS_TILES,
  MORTAR_FIELD_OF_FIRE_RAD,
  MORTAR_MIN_RANGE_TILES,
  MORTAR_RANGE_TILES,
  MINING_CC_RANGE_TILES,
  isProducerBuilding,
} from "../config.js";
import {
  ABILITY,
  ABILITY_OBJECT_KIND,
  KIND,
  ORDER_STAGE,
  SETUP,
  STATE,
  WEAPON_KIND,
  isBuilding,
  isResource,
  isUnit,
} from "../protocol.js";
import {
  DEPLOYED_WEAPON_ANIM_MS,
  SWEEP_EVICT_FRAMES,
  WEAPON_RECOIL_PX,
  ZERO_OFFSET,
} from "./palette.js";
import {
  drawArtilleryFireTargetPreview,
  isArtilleryFirePreview,
} from "./artillery_fire_preview.js";
import { MAGIC_ANCHOR_COLOR, drawMagicAnchor } from "./magic_anchor_effect.js";
import { feedbackOwner } from "./feedback_ownership.js";
import {
  attackFeedbackKindForWeapon,
  attackFeedbackOriginForWeapon,
} from "./attack_feedback_origin.js";
import { muzzleFeedbackStyle } from "./weapon_feedback_style.js";
import { drawLabToolPreview } from "./lab_tool_preview.js";
import { drawBreakthroughAura } from "./breakthrough_aura.js";
import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAntiTankGun,
  drawFacingWedge,
  drawFreeRotatedRect,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hash2,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  rendererVisualNow,
  smoothstep01,
  tankBodyVisual,
  weaponRecoilOffset,
} from "./shared.js";
import { drawImpassableEdge, isImpassableAt } from "./terrain_palette.js";
import { drawFormationMovePreview } from "./formation_line_preview.js";

export { _drawBreakthroughAuras } from "./breakthrough_aura.js";

const MORTAR_WARNING_COLOR = 0x9f1f1f;
const FIELD_OF_FIRE_COLOR = 0x4aa3ff;
const ABILITY_RETURN_MARKER_COLOR = 0x82d8ff;
const ABILITY_LINE_SHOT_COLOR = 0x0b3a78;
const LINE_PROJECTILE_TRAIL_MAX_POINTS = 9;
const LINE_PROJECTILE_TRAIL_MIN_STEP_PX = 1.5;

export function _drawPlacement(view, fog) {
  const g = this._placementGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.placement");
  gfxReset(g.clear());
  const ts = (this._map && this._map.tileSize) || 32;
  const p = view?.placement;
  if (p) drawBuildPlacementPreview(g, view, p, ts);
  drawLabToolPreview(g, view?.labToolPreview, ts);
  void fog;
}

function drawBuildPlacementPreview(g, view, p, ts) {
  const stat = STATS[p.building] || {};
  const footW = stat.footW || 2;
  const footH = stat.footH || 2;
  const w = footW * ts;
  const h = footH * ts;
  const sites = Array.isArray(p.lineSites) && p.lineSites.length > 0
    ? p.lineSites
    : [{ tileX: p.tileX, tileY: p.tileY, valid: p.valid }];

  for (const site of sites) {
    const x0 = site.tileX * ts;
    const y0 = site.tileY * ts;
    const color = site.valid ? COLORS.placeOk : COLORS.placeBad;

    gfxStroke(g, 2, color, 0.95);
    gfxFill(g, color, 0.25);
    gfxRoundRect(g, x0, y0, w, h, 6);
    gfxNoFill(g);

    // Per-tile grid hint inside the footprint so the snap target is obvious.
    const gridPaths = [];
    for (let i = 1; i < footW; i++) {
      gridPaths.push([[x0 + i * ts, y0], [x0 + i * ts, y0 + h]]);
    }
    for (let j = 1; j < footH; j++) {
      gridPaths.push([[x0, y0 + j * ts], [x0 + w, y0 + j * ts]]);
    }
    gfxStrokePaths(g, gridPaths, 1, color, 0.4);
  }

  if (p.building !== KIND.CITY_CENTRE && p.building !== KIND.ZAMOK) return;

  const x0 = p.tileX * ts;
  const y0 = p.tileY * ts;
  const cx = x0 + w / 2;
  const cy = y0 + h / 2;
  const rangePx = MINING_CC_RANGE_TILES * ts;
  const rangeSq = rangePx * rangePx;
  const resourceColor = 0x4aa3ff;
  for (const node of view.map?.resources || []) {
    if (!node || !isResource(node.kind) || node.remaining === 0) continue;
    const dx = node.x - cx;
    const dy = node.y - cy;
    if (dx * dx + dy * dy > rangeSq + 0.001) continue;

    const resourceStat = STATS[node.kind] || {};
    const radius = Math.max(13, (resourceStat.size || 12) + 7);
    gfxStroke(g, 4, resourceColor, 0.95);
    gfxFill(g, resourceColor, 0.12);
    gfxCircle(g, node.x, node.y, radius);
    gfxNoFill(g);
  }
}

export function _drawCommandFeedback(view) {
  const g = this._feedbackGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.feedback");
  gfxReset(g.clear());
  if (!view || typeof view.liveCommandFeedback !== "function") return;

  const now = rendererVisualNow(this);
  for (const f of view.liveCommandFeedback(now)) {
    if (f.ownerId != null && typeof view.isFeedbackOwner === "function" && !view.isFeedbackOwner(f.ownerId)) {
      continue;
    }
    const age = now - f.createdAt;
    const t = clamp01(age / 650);
    const alpha = (1 - t) * 0.95;
    const r = 12 + t * 10;
    const color = f.kind === "mortar" || f.kind === "artillery"
      ? MORTAR_WARNING_COLOR
      : f.kind === "attack"
        ? COLORS.selectEnemy
        : COLORS.selectOwn;

    if (f.kind === "mortar" || f.kind === "artillery") {
      const tileSize = (this._map && this._map.tileSize) || 32;
      const splash = Number.isFinite(f.radiusTiles) ? f.radiusTiles * tileSize : 48;
      drawDashedCircle(g, f.x, f.y, splash, 14, 2, color, alpha);
      gfxStroke(g, 2, color, alpha);
      gfxCircle(g, f.x, f.y, r * 0.45);
      gfxStrokePaths(g, [
        [[f.x - r * 0.7, f.y], [f.x + r * 0.7, f.y]],
        [[f.x, f.y - r * 0.7], [f.x, f.y + r * 0.7]],
      ], 2, color, alpha);
      if (f.kind === "artillery") {
        drawDashedCircle(g, f.x, f.y, splash * 0.45, 10, 1.5, 0xffd15c, alpha * 0.82);
      }
    } else if (f.kind === "attack") {
      gfxStrokePaths(g, [
        [[f.x - r, f.y - r], [f.x + r, f.y + r]],
        [[f.x + r, f.y - r], [f.x - r, f.y + r]],
      ], 2, color, alpha);
      gfxStroke(g, 2, color, alpha);
      gfxCircle(g, f.x, f.y, r * 0.72);
    } else {
      gfxStroke(g, 2, color, alpha);
      gfxCircle(g, f.x, f.y, r * 0.72);
      gfxStrokePaths(g, [[[f.x, f.y - r], [f.x + r * 0.72, f.y], [f.x, f.y + r],
        [f.x - r * 0.72, f.y], [f.x, f.y - r]]], 2, color, alpha);
    }
    if (f.append) {
      drawDashedCircle(g, f.x, f.y, r + 7, 10, 1.5, color, alpha * 0.85);
      const sx = f.x + r * 0.7;
      const sy = f.y - r * 0.7;
      gfxStrokePaths(g, [[[sx - 4, sy], [sx + 4, sy]], [[sx, sy - 4], [sx, sy + 4]]],
        2, color, alpha);
    }
  }
  drawFormationMovePreview(g, view.formationMovePreview);
}

export function _drawAttackTargetPreview(view) {
  const p = view?.attackTargetPreview;
  if (!p || !finiteNumber(p.x) || !finiteNumber(p.y)) return;
  const g = this._feedbackGfx;
  const ring = typeof this._ringRadius === "function"
    ? this._ringRadius({ kind: p.kind })
    : { rx: 16, ry: 11, cy: 4 };
  const rx = Number.isFinite(ring?.rx) ? ring.rx : 16;
  const ry = Number.isFinite(ring?.ry) ? ring.ry : 11;
  const cy = Number.isFinite(ring?.cy) ? ring.cy : 4;
  gfxStroke(g, 4, COLORS.selectEnemy, 0.3);
  gfxEllipse(g, p.x, p.y + cy, rx + 2, ry + 2);
  gfxStroke(g, 2, COLORS.selectEnemy, 0.98);
  gfxEllipse(g, p.x, p.y + cy, rx, ry);
}

export function _drawOrderPlan(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  const moveColor = COLORS.selectOwn;
  const attackColor = COLORS.selectEnemy;

  for (const e of state.selectedEntities()) {
    if (!isUnit(e.kind)) continue;
    const markers = Array.isArray(e.orderPlan)
      ? e.orderPlan.filter((m) => Number.isFinite(m?.x) && Number.isFinite(m?.y))
      : [];
    if (markers.length === 0) continue;

    let fromX = e.x;
    let fromY = e.y;
    for (let i = 0; i < markers.length; i += 1) {
      const marker = markers[i];
      const artilleryFire =
        marker.kind === ORDER_STAGE.POINT_FIRE || marker.kind === ORDER_STAGE.BLANKET_FIRE;
      const hostile = marker.kind === ORDER_STAGE.ATTACK || marker.kind === ORDER_STAGE.ATTACK_MOVE || artilleryFire;
      const attackMove = marker.kind === ORDER_STAGE.ATTACK_MOVE;
      const color = artilleryFire ? 0xffd15c : hostile ? attackColor : moveColor;
      const alpha = i === 0 ? 0.68 : 0.48;
      if (attackMove || artilleryFire) {
        dashedLine(g, fromX, fromY, marker.x, marker.y, 12, 8, 2, color, alpha);
      } else {
        gfxStrokeLine(g, fromX, fromY, marker.x, marker.y, 2, color, alpha);
      }

      if (artilleryFire) {
        drawPointFireMarker(g, marker.x, marker.y, color, 0.92);
      } else {
        drawQueuedPointMarker(g, marker.x, marker.y, color, hostile);
      }
      fromX = marker.x;
      fromY = marker.y;
    }
  }
}

export function _drawDebugPathOverlay(state, entities = null) {
  if (!state || typeof state.selectedEntities !== "function") return;
  if (!state.debugPathOverlaysEnabled) return;
  const g = this._feedbackGfx;
  const pathColor = 0x33d6ff;
  const currentColor = 0xffe066;
  const goalColor = 0xff8a4c;
  const candidates = state.showAllDebugPathOverlays && Array.isArray(entities)
    ? entities
    : state.selectedEntities();

  for (const e of candidates) {
    if (!feedbackOwner(state, e.owner) || !isUnit(e.kind) || e.state !== STATE.MOVE) continue;
    const debugPath = e.debugPath;
    const waypoints = Array.isArray(debugPath?.waypoints)
      ? debugPath.waypoints.filter((p) => finiteNumber(p?.x) && finiteNumber(p?.y))
      : [];
    if (waypoints.length === 0) continue;

    const current = waypoints[0];
    dashedLine(g, e.x, e.y, current.x, current.y, 10, 6, 3, currentColor, 0.9);

    if (waypoints.length > 1) {
      gfxStrokePaths(g, [[...waypoints.map(({ x, y }) => [x, y])]], 2, pathColor, 0.72);
    }

    for (let i = 0; i < waypoints.length; i += 1) {
      const p = waypoints[i];
      if (i === 0) {
        drawDebugCurrentWaypoint(g, p.x, p.y, currentColor);
      } else {
        drawDebugWaypoint(g, p.x, p.y, pathColor, i);
      }
    }

    const goal = debugPath?.goal;
    if (finiteNumber(goal?.x) && finiteNumber(goal?.y)) {
      const last = waypoints[waypoints.length - 1];
      const goalMatchesPathEnd = Math.hypot(goal.x - last.x, goal.y - last.y) < 0.5;
      drawDebugGoal(g, goal.x, goal.y, goalColor, goalMatchesPathEnd ? 0.35 : 0.9);
    }

    if (Number.isFinite(debugPath?.totalWaypoints) && debugPath.totalWaypoints > waypoints.length) {
      const last = waypoints[waypoints.length - 1];
      drawDebugTruncatedTail(g, last.x, last.y, pathColor);
    }
  }
}

export function _drawAntiTankGunSetupPreview(view) {
  if (!view || typeof view.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  const tileSize = (this._map && this._map.tileSize) || 32;
  const artilleryFirePreview = isArtilleryFirePreview(view.abilityTargetPreview)
    ? view.abilityTargetPreview
    : null;

  if (artilleryFirePreview) {
    const locks = Array.isArray(artilleryFirePreview.artilleryLocks)
      ? artilleryFirePreview.artilleryLocks
      : [];
    for (const lock of locks) {
      if (!lock.insideCurrentCone || !finiteNumber(lock.currentFacing)) continue;
      const weapon = fieldOfFireProfile(KIND.ARTILLERY, tileSize);
      if (!weapon) continue;
      drawFacingWedge(
        g,
        lock.originX,
        lock.originY,
        weapon.maxRadius,
        lock.currentFacing,
        weapon.arc,
        FIELD_OF_FIRE_COLOR,
        0.045,
        0.2,
        weapon.minRadius,
      );
    }
  }

  const preview = view.antiTankGunSetupPreview;
  if (!preview || !Array.isArray(preview.guns)) return;
  for (const e of preview.guns) {
    if (!finiteNumber(e.x) || !finiteNumber(e.y)) continue;
    const aimX = finiteNumber(e.setupAimX) ? e.setupAimX : preview.mouseX;
    const aimY = finiteNumber(e.setupAimY) ? e.setupAimY : preview.mouseY;
    const facing = Math.atan2(aimY - e.y, aimX - e.x);
    if (!Number.isFinite(facing)) continue;
    const weapon = fieldOfFireProfile(e.kind, tileSize);
    if (!weapon) continue;
    drawFacingWedge(
      g,
      e.x,
      e.y,
      weapon.maxRadius,
      facing,
      weapon.arc,
      FIELD_OF_FIRE_COLOR,
      0.16,
      0.58,
      weapon.minRadius,
    );
  }
}

export function _drawAbilityTargetPreview(view) {
  const preview = view?.abilityTargetPreview;
  if (!preview || !Array.isArray(preview.carriers)) return;
  const g = this._feedbackGfx;
  if (isArtilleryFirePreview(preview) && drawArtilleryFireTargetPreview(g, preview, this._map)) {
    return;
  }
  const areaOrigins = Array.isArray(preview.areaOrigins) ? preview.areaOrigins : [];
  if (areaOrigins.length > 0 && preview.radiusPx > 0) {
    for (const origin of areaOrigins) {
      if (!finiteNumber(origin.x) || !finiteNumber(origin.y)) continue;
      drawBreakthroughAura(g, origin.x, origin.y, preview.radiusPx, 0.92);
    }
    return;
  }
  if (!finiteNumber(preview.mouseX) || !finiteNumber(preview.mouseY)) return;
  const rangeColor = FIELD_OF_FIRE_COLOR;
  const minRangeColor = 0x8f2d2a;
  const rangeOrigins = preview.ability === ABILITY.POINT_FIRE
    ? preview.carriers
    : Array.isArray(preview.rangeOrigins) ? preview.rangeOrigins : preview.carriers;

  for (const carrier of rangeOrigins) {
    if (!finiteNumber(carrier.x) || !finiteNumber(carrier.y)) continue;
    const facing = Math.atan2(preview.mouseY - carrier.y, preview.mouseX - carrier.x);
    if (preview.ability === ABILITY.POINT_FIRE && Number.isFinite(facing)) {
      const weapon = fieldOfFireProfile(carrier.kind, (this._map && this._map.tileSize) || 32);
      const staticFacing = finiteNumber(carrier.setupFacing)
        ? carrier.setupFacing
        : finiteNumber(carrier.facing)
          ? carrier.facing
          : null;
      if (
        !weapon ||
        carrier.setupState !== SETUP.DEPLOYED ||
        (staticFacing != null &&
          pointInsideFieldOfFire(carrier, preview.mouseX, preview.mouseY, weapon, staticFacing))
      ) {
        continue;
      }
      drawFacingWedge(
        g,
        carrier.x,
        carrier.y,
        preview.rangePx,
        facing,
        ARTILLERY_FIELD_OF_FIRE_RAD,
        FIELD_OF_FIRE_COLOR,
        0.06,
        0.38,
        preview.minRangePx,
      );
    } else {
      dashedCircle(g, carrier.x, carrier.y, preview.rangePx, 64, 1.5, rangeColor, 0.85);
      if (preview.minRangePx > 0) {
        dashedCircle(g, carrier.x, carrier.y, preview.minRangePx, 42, 1.3, minRangeColor, 0.82);
      }
    }
  }

  if (Array.isArray(preview.returnMarkers)) {
    for (const marker of preview.returnMarkers) {
      if (!finiteNumber(marker.x) || !finiteNumber(marker.y)) continue;
      drawReturnMarker(g, marker.x, marker.y, marker.radiusPx || 13, ABILITY_RETURN_MARKER_COLOR, 0.72);
      dashedLine(g, marker.x, marker.y, preview.mouseX, preview.mouseY, 8, 6,
        1.5, ABILITY_RETURN_MARKER_COLOR, 0.45);
    }
  }

  if (Array.isArray(preview.pathOrigins)) {
    for (const origin of preview.pathOrigins) {
      if (!finiteNumber(origin.x) || !finiteNumber(origin.y)) continue;
      const color = origin.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR
        ? MAGIC_ANCHOR_COLOR
        : FIELD_OF_FIRE_COLOR;
      dashedLine(g, origin.x, origin.y, preview.mouseX, preview.mouseY, 10, 5,
        2, color, origin.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR ? 0.72 : 0.55);
      gfxFill(g, color, 0.2);
      gfxCircle(g, origin.x, origin.y, origin.radiusPx || 6);
      gfxNoFill(g);
    }
  }

  const cursorInvalid = preview.hoverInsideMinRange === true;
  const cursorColor = preview.hoverInRange ? COLORS.selectOwn : cursorInvalid ? minRangeColor : COLORS.selectNeutral;
  const radiusPx = preview.radiusPx || 24;
  gfxStroke(g, 2, cursorColor, 0.95);
  gfxFill(g, cursorColor, 0.18);
  gfxCircle(g, preview.mouseX, preview.mouseY, radiusPx);
  gfxNoFill(g);
  if (cursorInvalid) {
    const arm = radiusPx * 0.44;
    gfxStrokePaths(g, [
      [[preview.mouseX - arm, preview.mouseY - arm], [preview.mouseX + arm, preview.mouseY + arm]],
      [[preview.mouseX + arm, preview.mouseY - arm], [preview.mouseX - arm, preview.mouseY + arm]],
    ], 2, cursorColor, 0.85);
  } else {
    gfxStrokePaths(g, [
      [[preview.mouseX - radiusPx * 0.45, preview.mouseY], [preview.mouseX + radiusPx * 0.45, preview.mouseY]],
      [[preview.mouseX, preview.mouseY - radiusPx * 0.45], [preview.mouseX, preview.mouseY + radiusPx * 0.45]],
    ], 2, cursorColor, 0.85);
  }
}

export function _drawAbilityObjects(state) {
  const objects = state?.abilityObjects;
  if (!Array.isArray(objects) || objects.length === 0) {
    if (this._lineProjectileTrails) this._lineProjectileTrails.clear();
    return;
  }
  const g = this._abilityObjectGfx;
  if (!g) return;
  const ts = (this._map && this._map.tileSize) || 32;
  const activeLineProjectileIds = new Set();

  for (const object of objects) {
    if (!finiteNumber(object?.x) || !finiteNumber(object?.y)) continue;
    if (object.kind === ABILITY_OBJECT_KIND.RETURN_MARKER) {
      drawReturnMarker(g, object.x, object.y, 13, ABILITY_RETURN_MARKER_COLOR, 0.82);
    } else if (object.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR) {
      drawMagicAnchor(g, object, Math.max(10, object.ownerState?.radius || ts * 0.38));
    } else if (object.kind === ABILITY_OBJECT_KIND.LINE_PROJECTILE) {
      const r = Math.max(5, object.ownerState?.radius || ts * 0.18);
      activeLineProjectileIds.add(object.id);
      drawLineProjectile(g, object, r, this._lineProjectileTrails);
    }
  }

  if (this._lineProjectileTrails) {
    for (const id of this._lineProjectileTrails.keys()) {
      if (!activeLineProjectileIds.has(id)) this._lineProjectileTrails.delete(id);
    }
  }
}

function drawReturnMarker(g, x, y, radius, color, alpha) {
  gfxStroke(g, 2, color, alpha);
  gfxFill(g, color, 0.09);
  gfxCircle(g, x, y, radius);
  gfxNoFill(g);
  gfxStrokePaths(g, [[[x, y - radius * 0.7], [x + radius * 0.7, y], [x, y + radius * 0.7],
    [x - radius * 0.7, y], [x, y - radius * 0.7]]], 2, color, alpha);
}

function drawLineProjectile(g, object, radius, trails) {
  const points = lineProjectileTrailPoints(object, trails);
  if (points.length >= 2) {
    drawLineProjectileTrail(g, points, radius);
  }

  const previous = points.length >= 2 ? points[points.length - 2] : null;
  if (previous) {
    gfxStrokeLine(g, previous.x, previous.y, object.x, object.y,
      Math.max(3.5, radius * 0.72), ABILITY_LINE_SHOT_COLOR, 0.95);
  }

  gfxStroke(g, 2, ABILITY_LINE_SHOT_COLOR, 0.96);
  gfxFill(g, ABILITY_LINE_SHOT_COLOR, 0.68);
  gfxCircle(g, object.x, object.y, radius);
  gfxNoFill(g);
}

function lineProjectileTrailPoints(object, trails) {
  if (!trails || object.id == null) return [{ x: object.x, y: object.y }];
  const lastTicksOut = finiteNumber(object.ownerState?.ticksOut) ? object.ownerState.ticksOut : null;
  let trail = trails.get(object.id);
  if (!trail || (lastTicksOut !== null && trail.lastTicksOut !== null && lastTicksOut < trail.lastTicksOut)) {
    trail = { points: [], lastTicksOut: null };
    trails.set(object.id, trail);
  }
  const last = trail.points[trail.points.length - 1];
  const moved = !last || Math.hypot(object.x - last.x, object.y - last.y) >= LINE_PROJECTILE_TRAIL_MIN_STEP_PX;
  if (moved) {
    trail.points.push({ x: object.x, y: object.y });
    if (trail.points.length > LINE_PROJECTILE_TRAIL_MAX_POINTS) {
      trail.points.splice(0, trail.points.length - LINE_PROJECTILE_TRAIL_MAX_POINTS);
    }
  }
  trail.lastTicksOut = lastTicksOut;
  return trail.points;
}

function drawLineProjectileTrail(g, points, radius) {
  const maxSegment = points.length - 1;
  for (let i = 1; i < points.length; i += 1) {
    const age = (maxSegment - i) / Math.max(1, maxSegment);
    const alpha = 0.14 + (1 - age) * 0.38;
    const width = Math.max(2, radius * (0.36 + (1 - age) * 0.42));
    gfxStrokeLine(g, points[i - 1].x, points[i - 1].y, points[i].x, points[i].y,
      width, ABILITY_LINE_SHOT_COLOR, alpha);
  }
}

function fieldOfFireProfile(kind, tileSize) {
  if (kind === KIND.ARTILLERY) {
    return {
      minRadius: ARTILLERY_MIN_RANGE_TILES * tileSize,
      maxRadius: ARTILLERY_MAX_RANGE_TILES * tileSize,
      arc: ARTILLERY_FIELD_OF_FIRE_RAD,
    };
  }
  if (kind === KIND.ANTI_TANK_GUN) {
    return {
      minRadius: 0,
      maxRadius: ANTI_TANK_GUN_DEPLOYED_RANGE_TILES * tileSize,
      arc: ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
    };
  }
  if (kind === KIND.MORTAR_TEAM) {
    return {
      minRadius: MORTAR_MIN_RANGE_TILES * tileSize,
      maxRadius: MORTAR_RANGE_TILES * tileSize,
      arc: MORTAR_FIELD_OF_FIRE_RAD,
    };
  }
  return null;
}

function pointInsideFieldOfFire(e, x, y, weapon, facing) {
  if (!finiteNumber(e?.x) || !finiteNumber(e?.y) || !finiteNumber(x) || !finiteNumber(y)) return false;
  if (!finiteNumber(facing)) return false;
  const dx = x - e.x;
  const dy = y - e.y;
  const dist = Math.hypot(dx, dy);
  if (dist < (weapon.minRadius || 0) || dist > weapon.maxRadius) return false;
  const targetFacing = Math.atan2(dy, dx);
  return Math.abs(angleDelta(facing, targetFacing)) <= weapon.arc / 2 + 0.001;
}

function dashedCircle(g, cx, cy, radius, segments, width, color, alpha) {
  if (!(radius > 0)) return;
  const count = Math.max(12, segments | 0);
  const paths = [];
  for (let i = 0; i < count; i += 2) {
    const a0 = (i / count) * Math.PI * 2;
    const a1 = ((i + 1) / count) * Math.PI * 2;
    paths.push([[cx + Math.cos(a0) * radius, cy + Math.sin(a0) * radius],
      [cx + Math.cos(a1) * radius, cy + Math.sin(a1) * radius]]);
  }
  gfxStrokePaths(g, paths, width, color, alpha);
}

const SMOKE_RING_BILLOWS = [
  [-0.54, -0.18, 0.43, 0.74, 0x4d4d50, 0.31],
  [-0.45, -0.38, 0.34, 0.68, 0x626266, 0.23],
  [-0.28, -0.49, 0.39, 0.69, 0x707073, 0.25],
  [0.0, -0.58, 0.33, 0.76, 0x8a8a8d, 0.2],
  [0.18, -0.47, 0.45, 0.8, 0x5c5c60, 0.3],
  [0.42, -0.36, 0.35, 0.66, 0x727276, 0.22],
  [0.52, -0.16, 0.4, 0.72, 0x848487, 0.22],
  [0.59, 0.08, 0.32, 0.79, 0x57575b, 0.25],
  [0.44, 0.3, 0.46, 0.75, 0x66666a, 0.28],
  [0.25, 0.49, 0.34, 0.67, 0x8d8d90, 0.19],
  [0.02, 0.52, 0.42, 0.7, 0x77777b, 0.24],
  [-0.25, 0.5, 0.35, 0.81, 0x5e5e62, 0.24],
  [-0.44, 0.34, 0.4, 0.78, 0x555559, 0.29],
  [-0.59, 0.09, 0.33, 0.71, 0x7d7d80, 0.21],
];
const SMOKE_CORE_BILLOWS = [
  [-0.2, -0.08, 0.62, 0.64, 0x3f3f43, 0.38],
  [0.22, 0.04, 0.58, 0.58, 0x6c6c70, 0.3],
  [-0.02, 0.22, 0.5, 0.52, 0x8b8b8e, 0.22],
  [0.08, -0.28, 0.46, 0.46, 0xa0a0a2, 0.17],
];

function smokeHash(seed) {
  const n = Math.sin(seed * 12.9898) * 43758.5453;
  return n - Math.floor(n);
}

function drawSmokeBillow(g, cx, cy, radius, sides, phase, seed, color, alpha) {
  const points = [];
  const twist = phase * 0.00018 * (smokeHash(seed + 11) > 0.5 ? 1 : -1);
  for (let i = 0; i < sides; i++) {
    const t = i / sides;
    const a = t * Math.PI * 2 + twist;
    const jitter =
      0.86 +
      smokeHash(seed + i * 5.17) * 0.18 +
      Math.sin(phase * 0.0012 + seed + i * 1.8) * 0.025;
    points.push(cx + Math.cos(a) * radius * jitter, cy + Math.sin(a) * radius * jitter);
  }
  gfxFill(g, color, alpha);
  gfxPoly(g, points);
  gfxNoFill(g);
}

export function _drawSmokes(state) {
  const smokes = state?.smokes;
  if (!Array.isArray(smokes) || smokes.length === 0) return;
  const g = this._smokeGfx;
  if (!g) return;
  const ts = (this._map && this._map.tileSize) || 32;
  const now = rendererVisualNow(this);
  gfxStroke(g, 0, 0x000000, 0);
  for (const smoke of smokes) {
    if (!finiteNumber(smoke.x) || !finiteNumber(smoke.y)) continue;
    const r = Math.max(8, (smoke.radiusTiles || 0) * ts);
    const base = (smoke.id || 1) * 17.31;
    const phase = now + base * 83;

    // Low-poly overlapping gray billows: readable tactical radius, no hard outline.
    drawSmokeBillow(g, smoke.x, smoke.y, r * 0.98, 13, phase * 0.72, base, 0x262629, 0.34);
    for (let i = 0; i < SMOKE_RING_BILLOWS.length; i++) {
      const [ox, oy, scale, drift, color, alpha] = SMOKE_RING_BILLOWS[i];
      const seed = base + i * 3.73;
      const dx = (Math.sin(phase * 0.00035 + seed) * 0.025 + ox) * r;
      const dy = (Math.cos(phase * 0.00032 + seed * 1.4) * 0.025 + oy) * r;
      drawSmokeBillow(
        g,
        smoke.x + dx,
        smoke.y + dy,
        r * scale,
        7,
        phase * drift,
        seed,
        color,
        alpha,
      );
    }
    for (let i = 0; i < SMOKE_CORE_BILLOWS.length; i++) {
      const [ox, oy, scale, drift, color, alpha] = SMOKE_CORE_BILLOWS[i];
      const seed = base + 41 + i * 4.91;
      const dx = (Math.sin(phase * 0.00042 + seed) * 0.04 + ox) * r;
      const dy = (Math.cos(phase * 0.00039 + seed * 1.3) * 0.04 + oy) * r;
      drawSmokeBillow(
        g,
        smoke.x + dx,
        smoke.y + dy,
        r * scale,
        8,
        phase * drift,
        seed,
        color,
        alpha,
      );
    }
  }
}

export function _drawSmokeCanisters(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveSmokeCanisters !== "function") return;
  const now = rendererVisualNow(this);
  const canisters = state.liveSmokeCanisters(now);
  if (!canisters.length) return;

  for (const c of canisters) {
    const duration = Math.max(1, c.durationMs || 1);
    const t = clamp01((now - c.createdAt) / duration);
    const eased = t * t * (3 - 2 * t);
    const x = c.fromX + (c.toX - c.fromX) * eased;
    const y = c.fromY + (c.toY - c.fromY) * eased;
    const dx = c.toX - c.fromX;
    const dy = c.toY - c.fromY;
    const len = Math.hypot(dx, dy);
    const ux = len > 0.001 ? dx / len : 1;
    const uy = len > 0.001 ? dy / len : 0;
    const arc = Math.sin(Math.PI * t) * Math.min(18, Math.max(4, len * 0.04));
    const px = x - uy * arc;
    const py = y + ux * arc;
    const tail = Math.min(28, Math.max(8, len * 0.08));
    const alpha = 0.95 - t * 0.25;

    gfxStrokeLine(g, px - ux * tail, py - uy * tail, px, py, 2, 0x111111, alpha * 0.45);
    gfxStroke(g, 0, 0x000000, 0);
    gfxFill(g, 0x050505, alpha);
    gfxCircle(g, px, py, 2.7);
    gfxNoFill(g);
    gfxFill(g, 0x2b2b2b, alpha * 0.7);
    gfxCircle(g, px - ux * 1.2 - uy * 0.7, py - uy * 1.2 + ux * 0.7, 1.2);
    gfxNoFill(g);
  }
}

export function _drawMortarLaunches(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarLaunches !== "function") return;
  const now = rendererVisualNow(this);
  const launches = state.liveMortarLaunches(now);
  if (!launches.length) return;

  for (const launch of launches) {
    const age = now - launch.createdAt;
    const fade = 1 - clamp01(age / 360);
    const r = 11;
    const dx = finiteNumber(launch.toX) ? launch.toX - launch.x : 1;
    const dy = finiteNumber(launch.toY) ? launch.toY - launch.y : 0;
    const len = Math.hypot(dx, dy) || 1;
    const ux = dx / len;
    const uy = dy / len;
    const flashFade = 1 - clamp01(age / 150);
    const flashLen = 22;
    const flashWidth = 8;
    gfxStroke(g, 0, 0x000000, 0);
    gfxFill(g, 0xfff3b0, 0.88 * flashFade);
    gfxPoly(g, [
      launch.x + ux * 2 - uy * 2.8,
      launch.y + uy * 2 + ux * 2.8,
      launch.x + ux * flashLen,
      launch.y + uy * flashLen,
      launch.x + ux * 5 + uy * flashWidth,
      launch.y + uy * 5 - ux * flashWidth,
    ]);
    gfxNoFill(g);
    gfxFill(g, 0xff8b23, 0.48 * flashFade);
    gfxPoly(g, [
      launch.x - uy * 4.5,
      launch.y + ux * 4.5,
      launch.x + ux * 16,
      launch.y + uy * 16,
      launch.x + uy * 4.5,
      launch.y - ux * 4.5,
      launch.x - ux * 5,
      launch.y - uy * 5,
    ]);
    gfxNoFill(g);
    gfxFill(g, 0x8a806b, 0.24 * fade);
    gfxPoly(g, [
      launch.x - r * 0.95, launch.y - r * 0.14,
      launch.x - r * 0.5, launch.y - r * 0.58,
      launch.x + r * 0.22, launch.y - r * 0.5,
      launch.x + r * 0.86, launch.y - r * 0.16,
      launch.x + r * 0.64, launch.y + r * 0.38,
      launch.x - r * 0.18, launch.y + r * 0.52,
      launch.x - r * 0.82, launch.y + r * 0.28,
    ]);
    gfxNoFill(g);
    gfxFill(g, 0xc0b092, 0.18 * fade);
    gfxCircle(g, launch.x - 4, launch.y + 1, 4.5);
    gfxCircle(g, launch.x + 4, launch.y - 2, 3.8);
    gfxNoFill(g);
  }
}

export function _drawMortarTargets(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarTargets !== "function") return;
  const now = rendererVisualNow(this);
  const targets = state.liveMortarTargets(now);
  if (!targets.length) return;
  const ts = (this._map && this._map.tileSize) || 32;

  for (const target of targets) {
    const duration = Math.max(1, target.durationMs || 1);
    const age = now - target.createdAt;
    const t = clamp01(age / duration);
    const fade = 1 - smoothstep01(Math.max(0, t - 0.78) / 0.22);
    const radius = Math.max(20, (target.radiusTiles || 1.5) * ts);
    const pulse = 1 + Math.sin(t * Math.PI * 5) * 0.035;

    if (finiteNumber(target.fromX) && finiteNumber(target.fromY)) {
      dashedLine(g, target.fromX, target.fromY, target.x, target.y, 10, 7,
        1.8, MORTAR_WARNING_COLOR, 0.72 * fade);
    }
    drawDashedCircle(g, target.x, target.y, radius * pulse, 24,
      2.3, MORTAR_WARNING_COLOR, 0.9 * fade);
    gfxStrokePaths(g, [
      [[target.x - 14, target.y], [target.x + 14, target.y]],
      [[target.x, target.y - 14], [target.x, target.y + 14]],
    ], 2, MORTAR_WARNING_COLOR, 0.86 * fade);
    drawDashedCircle(g, target.x, target.y, radius * 0.45, 12, 1.4, 0x421010, 0.52 * fade);
  }
}

export function _drawMortarShells(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarShells !== "function") return;
  const now = rendererVisualNow(this);
  const shells = state.liveMortarShells(now);
  if (!shells.length) return;

  for (const shell of shells) {
    const duration = Math.max(1, shell.durationMs || 1);
    const t = clamp01((now - shell.createdAt) / duration);
    const dx = shell.toX - shell.fromX;
    const dy = shell.toY - shell.fromY;
    const len = Math.hypot(dx, dy);
    const x = shell.fromX + dx * t;
    const y = shell.fromY + dy * t;
    const stretch = Math.sin(Math.PI * t);
    const shellLen = (5.5 + stretch * 6.5) * 1.25;
    const shellWidth = (4.2 - stretch * 1.2) * 1.25;
    const angle = len > 0.001 ? Math.atan2(dy, dx) : 0;
    const ux = Math.cos(angle);
    const uy = Math.sin(angle);
    const shadowAlpha = 0.22 * (1 - stretch * 0.55);

    gfxStroke(g, 0, 0x000000, 0);
    gfxFill(g, 0x050505, shadowAlpha);
    gfxEllipse(g, x, y, 4.4, 2.2);
    gfxNoFill(g);
    gfxFill(g, 0x050505, 1);
    drawFreeRotatedRect(g, x, y, shellLen, shellWidth, angle);
    gfxNoFill(g);
    gfxFill(g, 0x2d2d2d, 1);
    drawFreeRotatedRect(
      g,
      x - uy * shellWidth * 0.24,
      y + ux * shellWidth * 0.24,
      shellLen * 0.55,
      shellWidth * 0.35,
      angle,
    );
    gfxNoFill(g);
  }
}

export function _drawMortarImpacts(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarImpacts !== "function") return;
  const now = rendererVisualNow(this);
  const impacts = state.liveMortarImpacts(now);
  if (!impacts.length) return;
  const ts = (this._map && this._map.tileSize) || 32;
  const innerRadius = MORTAR_INNER_RADIUS_TILES * ts;

  for (const impact of impacts) {
    const age = now - impact.createdAt;
    const t = clamp01(age / 1000);
    const blastFade = 1 - smoothstep01(Math.max(0, t - 0.36) / 0.28);
    const dustFade = 1 - smoothstep01(Math.max(0, t - 0.48) / 0.52);
    const outerRadius = Math.max(innerRadius + 8, impact.radiusTiles * ts);
    const dustRadius = outerRadius * 2;
    gfxStroke(g, 0, 0x000000, 0);

    gfxFill(g, 0xffb22e, 0.28 * blastFade);
    drawJaggedBlob(g, impact.x, impact.y, outerRadius * 1.05, 18, impact.seed + 11, 0.7, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0xffd65a, 0.2 * blastFade);
    drawJaggedBlob(g, impact.x, impact.y, outerRadius * 0.7, 14, impact.seed + 23, 0.74, 1.0);
    gfxNoFill(g);

    gfxFill(g, 0x6f5c45, 0.3 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, dustRadius, 26, impact.seed + 31, 0.62, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0xa08d70, 0.2 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, dustRadius * 0.74, 22, impact.seed + 43, 0.68, 1.0);
    gfxNoFill(g);

    gfxFill(g, 0x2a2119, 0.24 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, innerRadius * 1.55, 14, impact.seed + 37, 0.72, 1.0);
    gfxNoFill(g);
    drawJaggedRing(g, impact.x, impact.y, innerRadius, 12, impact.seed + 41, 0.72, 1.18,
      3, 0xffffff, 0.95 * blastFade);
    drawJaggedRing(g, impact.x, impact.y, innerRadius * 0.72, 9, impact.seed + 53, 0.78, 1.08,
      1.8, 0xfff2d0, 0.7 * blastFade);

    drawShrapnel(g, impact.x, impact.y, innerRadius * 0.78, outerRadius, impact.seed, 0.56 * dustFade);
  }
}

export function _drawArtilleryTargets(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveArtilleryTargets !== "function") return;
  const now = rendererVisualNow(this);
  const targets = state.liveArtilleryTargets(now);
  if (!targets.length) return;
  const ts = (this._map && this._map.tileSize) || 32;

  for (const target of targets) {
    const ttlMs = Math.max(900, ((target.delayTicks || 0) / 30) * 1000 + 350);
    const t = clamp01((now - target.createdAt) / ttlMs);
    const fade = 1 - smoothstep01(Math.max(0, t - 0.72) / 0.28);
    const radius = Math.max(24, (target.radiusTiles || 3) * ts);
    const descend = smoothstep01(t);
    const shellX = target.x - 26 + descend * 26;
    const shellY = target.y - 92 + descend * 92;
    drawDashedCircle(g, target.x, target.y, radius, 28, 2.5, 0xffd15c, 0.9 * fade);
    gfxStrokePaths(g, [
      [[target.x - 18, target.y], [target.x + 18, target.y]],
      [[target.x, target.y - 18], [target.x, target.y + 18]],
    ], 2, 0xfff2d0, 0.78 * fade);
    drawDashedCircle(g, target.x, target.y,
      radius * (0.34 + 0.08 * Math.sin(t * Math.PI)), 12, 1.5, 0x2a2119, 0.6 * fade);
    gfxStrokeLine(g, shellX - 12, shellY - 18, shellX, shellY, 2, 0x2a2119, 0.58 * fade);
    gfxStroke(g, 0, 0x000000, 0);
    gfxFill(g, 0xfff2d0, 0.9 * fade);
    gfxCircle(g, shellX, shellY, 3.5 + descend * 1.5);
    gfxNoFill(g);
    gfxFill(g, 0x2a2119, 0.2 * fade);
    gfxCircle(g, target.x, target.y, 3 + descend * 8);
    gfxNoFill(g);
  }
}

export function _drawArtilleryLaunches(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveArtilleryLaunches !== "function") return;
  const now = rendererVisualNow(this);
  const launches = state.liveArtilleryLaunches(now);
  if (!launches.length) return;

  for (const launch of launches) {
    const age = now - launch.createdAt;
    const t = clamp01(age / 820);
    const fade = 1 - smoothstep01(Math.max(0, t - 0.42) / 0.58);
    const burst = 1 + smoothstep01(t) * 1.25;
    const rearX = launch.x - Math.cos(launch.facing) * 22;
    const rearY = launch.y - Math.sin(launch.facing) * 22;
    gfxStroke(g, 0, 0x000000, 0);
    gfxFill(g, 0x6f5c45, 0.32 * fade);
    drawJaggedBlob(g, rearX, rearY, 28 * burst, 18, launch.seed + 17, 0.58, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0xa08d70, 0.22 * fade);
    drawJaggedBlob(g, launch.x, launch.y, 20 * burst, 14, launch.seed + 31, 0.62, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0x2a2119, 0.16 * fade);
    drawJaggedBlob(
      g,
      rearX - Math.cos(launch.facing) * 10,
      rearY - Math.sin(launch.facing) * 10,
      15 * burst,
      10,
      launch.seed + 43,
      0.7,
      1.0,
    );
    gfxNoFill(g);
  }
}

export function _drawArtilleryImpacts(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveArtilleryImpacts !== "function") return;
  const now = rendererVisualNow(this);
  const impacts = state.liveArtilleryImpacts(now);
  if (!impacts.length) return;
  const ts = (this._map && this._map.tileSize) || 32;

  for (const impact of impacts) {
    const age = now - impact.createdAt;
    const fade = 1 - clamp01(age / 850);
    const outerRadius = Math.max(48, impact.radiusTiles * ts);
    const shock = outerRadius * (1.0 + (1 - fade) * 0.34);
    drawJaggedRing(g, impact.x, impact.y, shock * 0.45, 16, impact.seed + 3, 0.78, 1.15,
      4, 0xfff2d0, 0.92 * fade);
    gfxFill(g, 0xff7a28, 0.28 * fade);
    drawJaggedBlob(g, impact.x, impact.y, shock, 22, impact.seed + 11, 0.62, 1.0);
    gfxNoFill(g);
    gfxFill(g, 0x3b2a1c, 0.34 * fade);
    drawJaggedBlob(g, impact.x, impact.y, shock * 0.62, 14, impact.seed + 23, 0.72, 1.0);
    gfxNoFill(g);
    drawShrapnel(g, impact.x, impact.y, outerRadius * 0.28, outerRadius * 1.08, impact.seed + 37);
  }
}

function drawJaggedBlob(g, cx, cy, radius, points, seed, minScale, maxScale) {
  const poly = [];
  for (let i = 0; i < points; i += 1) {
    const a = (i / points) * Math.PI * 2;
    const n = hash2(seed + i * 17, seed - i * 31);
    const r = radius * (minScale + (maxScale - minScale) * n);
    poly.push(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  gfxPoly(g, poly);
}

function drawJaggedRing(g, cx, cy, radius, points, seed, minScale, maxScale, width, color, alpha) {
  const path = [];
  for (let i = 0; i <= points; i += 1) {
    const j = i % points;
    const a = (j / points) * Math.PI * 2;
    const n = hash2(seed + j * 19, seed + j * 7);
    const r = radius * (minScale + (maxScale - minScale) * n);
    const x = cx + Math.cos(a) * r;
    const y = cy + Math.sin(a) * r;
    path.push([x, y]);
  }
  gfxStrokePaths(g, [path], width, color, alpha);
}

function drawShrapnel(g, cx, cy, innerRadius, outerRadius, seed, alpha = 0.56) {
  const count = 18;
  const paths = [];
  for (let i = 0; i < count; i += 1) {
    const a = (i / count) * Math.PI * 2 + hash2(seed + i * 5, seed + 99) * 0.18;
    const start = innerRadius + hash2(seed + i * 13, seed + 3) * innerRadius * 0.55;
    const len = 5 + hash2(seed + i * 29, seed + 77) * 11;
    const end = Math.min(outerRadius * 0.94, start + len);
    paths.push([[cx + Math.cos(a) * start, cy + Math.sin(a) * start],
      [cx + Math.cos(a) * end, cy + Math.sin(a) * end]]);
  }
  gfxStrokePaths(g, paths, 1.5, 0x2b2119, alpha);
}

export function _drawRallyPoints(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  for (const e of state.selectedEntities()) {
    if (!isBuilding(e.kind) || !isProducerBuilding(e.kind)) continue;
    const color = e.optimisticRally ? 0x7ee7ff : COLORS.selectOwn;
    const plan = Array.isArray(e.rallyPlan) && e.rallyPlan.length > 0
      ? e.rallyPlan
      : e.rally
        ? [{ kind: "move", x: e.rally[0], y: e.rally[1] }]
        : [];
    if (plan.length === 0) continue;

    let fromX = e.x;
    let fromY = e.y;
    for (let i = 0; i < plan.length; i += 1) {
      const stage = plan[i];
      const attackMove = stage.kind === "attackMove";
      gfxStrokeLine(g, fromX, fromY, stage.x, stage.y,
        e.optimisticRally ? 2.5 : 2, color, i === 0 ? 0.55 : 0.35);
      if (i === 0 && !attackMove) {
        // Flag: pole + pennant + base dot for the active move rally.
        gfxStrokeLine(g, stage.x, stage.y, stage.x, stage.y - 20, 2.5, color, 0.95);
        gfxFill(g, color, 0.9);
        gfxPoly(g, [stage.x, stage.y - 20, stage.x + 13, stage.y - 16, stage.x, stage.y - 11]);
        gfxNoFill(g);
        gfxStroke(g, 0);
        gfxFill(g, color, 0.85);
        gfxCircle(g, stage.x, stage.y, 3);
        gfxNoFill(g);
      } else {
        drawQueuedPointMarker(g, stage.x, stage.y, color, attackMove);
      }
      fromX = stage.x;
      fromY = stage.y;
    }
  }
}

function drawQueuedPointMarker(g, x, y, color, attackMove) {
  if (attackMove) {
    gfxStroke(g, 2.5, color, 0.95);
    gfxCircle(g, x, y, 7);
    gfxStrokePaths(g, [[[x - 6, y - 6], [x + 6, y + 6]], [[x + 6, y - 6], [x - 6, y + 6]]],
      2.5, color, 0.95);
    return;
  }

  gfxStroke(g, 2.5, color, 0.95);
  gfxFill(g, color, 0.18);
  gfxPoly(g, [x, y - 8, x + 8, y, x, y + 8, x - 8, y]);
  gfxNoFill(g);
  gfxStroke(g, 0);
  gfxFill(g, color, 0.9);
  gfxCircle(g, x, y, 2.5);
  gfxNoFill(g);
}

function drawPointFireMarker(g, x, y, color, alpha = 0.95) {
  gfxStroke(g, 2.5, color, alpha);
  gfxCircle(g, x, y, 10);
  gfxStrokePaths(g, [[[x - 13, y], [x + 13, y]], [[x, y - 13], [x, y + 13]]],
    2.5, color, alpha);
  drawDashedCircle(g, x, y, 18, 12, 1.5, color, alpha * 0.78);
}

function drawDebugCurrentWaypoint(g, x, y, color) {
  gfxStroke(g, 3, color, 0.98);
  gfxFill(g, color, 0.18);
  gfxCircle(g, x, y, 10);
  gfxNoFill(g);
  gfxStroke(g, 1.5, color, 0.9);
  gfxCircle(g, x, y, 15);
  gfxStrokePaths(g, [[[x - 13, y], [x + 13, y]], [[x, y - 13], [x, y + 13]]],
    1.5, color, 0.9);
}

function drawDebugWaypoint(g, x, y, color, index) {
  const radius = index % 2 === 0 ? 5.5 : 4.5;
  gfxStroke(g, 2, color, 0.85);
  gfxFill(g, color, 0.14);
  gfxCircle(g, x, y, radius);
  gfxNoFill(g);
}

function drawDebugGoal(g, x, y, color, alpha) {
  gfxStroke(g, 2.5, color, alpha);
  gfxRect(g, x - 8, y - 8, 16, 16);
  gfxStrokePaths(g, [[[x - 11, y], [x + 11, y]], [[x, y - 11], [x, y + 11]]],
    2.5, color, alpha);
}

function drawDebugTruncatedTail(g, x, y, color) {
  gfxStrokePaths(g, [[[x + 10, y - 6], [x + 16, y], [x + 10, y + 6]]], 2, color, 0.72);
}

function drawDashedCircle(g, x, y, radius, segments, width, color, alpha) {
  const count = Math.max(6, segments | 0);
  const paths = [];
  for (let i = 0; i < count; i += 2) {
    const a0 = (i / count) * Math.PI * 2;
    const a1 = ((i + 1) / count) * Math.PI * 2;
    paths.push([[x + Math.cos(a0) * radius, y + Math.sin(a0) * radius],
      [x + Math.cos(a1) * radius, y + Math.sin(a1) * radius]]);
  }
  gfxStrokePaths(g, paths, width, color, alpha);
}

export function _drawResourceMiningPreview(view) {
  if (!view || !view.resourceMiningPreview) return;
  const g = this._feedbackGfx;
  const p = view.resourceMiningPreview;
  const ccStat = STATS[KIND.CITY_CENTRE] || {};
  const ts = (this._map && this._map.tileSize) || 32;
  const ccEndpoint = rectEdgePointTowardCenter(
    p.resourceX,
    p.resourceY,
    p.ccX,
    p.ccY,
    ((ccStat.footW || 3) * ts) / 2,
    ((ccStat.footH || 3) * ts) / 2,
  );

  if (p.inRange) {
    gfxStroke(g, 4, 0x4aa3ff, 0.95);
    gfxFill(g, 0x4aa3ff, 0.18);
    gfxCircle(g, p.resourceX, p.resourceY, 9);
    gfxNoFill(g);
    return;
  }

  dashedLine(g, p.resourceX, p.resourceY, ccEndpoint.x, ccEndpoint.y, 14, 9, 2.5, 0xd64d45, 0.9);
}

export function _drawMuzzleFlashes(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMuzzleFlashes !== "function") return;
  const now = rendererVisualNow(this);
  const flashes = state.liveMuzzleFlashes(now);
  if (!flashes.length) return;

  for (const f of flashes) {
    const attacker = state.entityById(f.from);
    if (!attacker) continue;
    const target = state.entityById(f.to);
    const targetPos = target || f.targetPos;

    const age = now - f.createdAt;
    const t = clamp01(age / 240);
    const fade = 1 - t;

    const feedbackKind = attackFeedbackKindForWeapon(attacker.kind, f.weaponKind);
    const style = muzzleFeedbackStyle(feedbackKind, f.weaponKind);
    const baseR = style.flashRadius;
    if (baseR <= 0) continue;

    const stat = STATS[feedbackKind] || STATS[attacker.kind] || {};
    const origin = attackFeedbackOriginForWeapon({
      definitionsByKind: this._liveRigDefinitionsByKind,
      attacker,
      weaponKind: f.weaponKind,
      targetPos,
      state,
      now,
      map: this._map,
      stat,
    });
    const mx = origin.x;
    const my = origin.y;

    if (targetPos) {
      const dx = targetPos.x - mx;
      const dy = targetPos.y - my;
      const shotLen = Math.hypot(dx, dy);
      // Mirror the server overpenetration band: a round that hits a tank stops dead (no tail),
      // and Anti-Tank Guns punch twice as deep as everyone else.
      const tileSize = (this._map && this._map.tileSize) || 32;
      const penFactor = target?.kind === KIND.TANK ? 0 : feedbackKind === KIND.ANTI_TANK_GUN ? 0.5 : 0.25;
      const tailLen = (stat.rangeTiles || 0) * tileSize * penFactor;

      gfxStrokeLine(g, mx, my, targetPos.x, targetPos.y,
        style.tracerWidth, style.tracerColor, style.tracerAlpha * fade);
      if (style.tracerCoreWidth > 0) {
        gfxStrokeLine(g, mx, my, targetPos.x, targetPos.y,
          style.tracerCoreWidth, style.tracerCoreColor, style.tracerCoreAlpha * fade);
      }

      if (shotLen > 0.001 && tailLen > 0) {
        const ux = dx / shotLen;
        const uy = dy / shotLen;
        const ex = targetPos.x + ux * tailLen;
        const ey = targetPos.y + uy * tailLen;
        gfxStrokeLine(g, targetPos.x, targetPos.y, ex, ey,
          style.tailWidth, style.tailColor, style.tailAlpha * fade);
      }
    }

    const rigOwnsTankCannonFlash = attacker.kind === KIND.TANK && f.weaponKind !== WEAPON_KIND.TANK_COAX;
    const suppressCircularFlash = feedbackKind === KIND.RIFLEMAN;
    if (!rigOwnsTankCannonFlash && !suppressCircularFlash) {
      // Flash: bright core that scales up slightly then fades.
      const r = baseR * (0.7 + 0.45 * t);
      gfxStroke(g, 0);
      gfxFill(g, 0xfff2a8, 0.85 * fade);
      gfxCircle(g, mx, my, r);
      gfxNoFill(g);
      gfxFill(g, 0xffd84a, 0.55 * fade);
      gfxCircle(g, mx, my, r * 0.55);
      gfxNoFill(g);
    }
  }
}

export function drawSelectionBox(rect) {
  const g = this._dragGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.dragSelection");
  gfxReset(g.clear());
  if (!rect) return;
  const { x, y, w, h } = normRect(rect);
  gfxStroke(g, 1.5, COLORS.dragBox, 0.95);
  gfxFill(g, COLORS.dragBox, 0.12);
  gfxRect(g, x, y, w, h);
  gfxNoFill(g);
}
