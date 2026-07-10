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
  ABILITIES,
  MORTAR_INNER_RADIUS_TILES,
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
import { feedbackOwner, ownOrAllyOwner } from "./feedback_ownership.js";
import {
  attackFeedbackKindForWeapon,
  attackFeedbackOriginForWeapon,
} from "./attack_feedback_origin.js";
import { muzzleFeedbackStyle } from "./weapon_feedback_style.js";
import { drawLabToolPreview } from "./lab_tool_preview.js";
import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAntiTankGun,
  drawFacingWedge,
  drawFreeRotatedRect,
  drawImpassableEdge,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hash2,
  isImpassableAt,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  smoothstep01,
  tankBodyVisual,
  weaponRecoilOffset,
} from "./shared.js";

const MORTAR_WARNING_COLOR = 0x9f1f1f;
const FIELD_OF_FIRE_COLOR = 0x4aa3ff;
const ABILITY_RETURN_MARKER_COLOR = 0x82d8ff;
const ABILITY_LINE_SHOT_COLOR = 0x0b3a78;
const LINE_PROJECTILE_TRAIL_MAX_POINTS = 9;
const LINE_PROJECTILE_TRAIL_MIN_STEP_PX = 1.5;

export function _drawPlacement(view, fog) {
  const g = this._placementGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.placement");
  g.clear();
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

    g.lineStyle(2, color, 0.95);
    g.beginFill(color, 0.25);
    g.drawRoundedRect(x0, y0, w, h, 6);
    g.endFill();

    // Per-tile grid hint inside the footprint so the snap target is obvious.
    g.lineStyle(1, color, 0.4);
    for (let i = 1; i < footW; i++) {
      g.moveTo(x0 + i * ts, y0);
      g.lineTo(x0 + i * ts, y0 + h);
    }
    for (let j = 1; j < footH; j++) {
      g.moveTo(x0, y0 + j * ts);
      g.lineTo(x0 + w, y0 + j * ts);
    }
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
    g.lineStyle(4, resourceColor, 0.95);
    g.beginFill(resourceColor, 0.12);
    g.drawCircle(node.x, node.y, radius);
    g.endFill();
  }
}

export function _drawCommandFeedback(view) {
  const g = this._feedbackGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.feedback");
  g.clear();
  if (!view || typeof view.liveCommandFeedback !== "function") return;

  const now = performance.now();
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

    g.lineStyle(2, color, alpha);
    if (f.kind === "mortar" || f.kind === "artillery") {
      const tileSize = (this._map && this._map.tileSize) || 32;
      const splash = Number.isFinite(f.radiusTiles) ? f.radiusTiles * tileSize : 48;
      drawDashedCircle(g, f.x, f.y, splash, 14);
      g.drawCircle(f.x, f.y, r * 0.45);
      g.moveTo(f.x - r * 0.7, f.y);
      g.lineTo(f.x + r * 0.7, f.y);
      g.moveTo(f.x, f.y - r * 0.7);
      g.lineTo(f.x, f.y + r * 0.7);
      if (f.kind === "artillery") {
        g.lineStyle(1.5, 0xffd15c, alpha * 0.82);
        drawDashedCircle(g, f.x, f.y, splash * 0.45, 10);
      }
    } else if (f.kind === "attack") {
      g.moveTo(f.x - r, f.y - r);
      g.lineTo(f.x + r, f.y + r);
      g.moveTo(f.x + r, f.y - r);
      g.lineTo(f.x - r, f.y + r);
      g.drawCircle(f.x, f.y, r * 0.72);
    } else {
      g.drawCircle(f.x, f.y, r * 0.72);
      g.moveTo(f.x, f.y - r);
      g.lineTo(f.x + r * 0.72, f.y);
      g.lineTo(f.x, f.y + r);
      g.lineTo(f.x - r * 0.72, f.y);
      g.lineTo(f.x, f.y - r);
    }
    if (f.append) {
      g.lineStyle(1.5, color, alpha * 0.85);
      drawDashedCircle(g, f.x, f.y, r + 7, 10);
      const sx = f.x + r * 0.7;
      const sy = f.y - r * 0.7;
      g.lineStyle(2, color, alpha);
      g.moveTo(sx - 4, sy);
      g.lineTo(sx + 4, sy);
      g.moveTo(sx, sy - 4);
      g.lineTo(sx, sy + 4);
    }
  }
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
  g.lineStyle(4, COLORS.selectEnemy, 0.3);
  g.drawEllipse(p.x, p.y + cy, rx + 2, ry + 2);
  g.lineStyle(2, COLORS.selectEnemy, 0.98);
  g.drawEllipse(p.x, p.y + cy, rx, ry);
}

export function _drawOrderPlan(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  const moveColor = COLORS.selectOwn;
  const attackColor = COLORS.selectEnemy;

  for (const e of state.selectedEntities()) {
    if (!feedbackOwner(state, e.owner) || !isUnit(e.kind)) continue;
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
      g.lineStyle(2, color, alpha);
      if (attackMove || artilleryFire) {
        dashedLine(g, fromX, fromY, marker.x, marker.y, 12, 8);
      } else {
        g.moveTo(fromX, fromY);
        g.lineTo(marker.x, marker.y);
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
    g.lineStyle(3, currentColor, 0.9);
    dashedLine(g, e.x, e.y, current.x, current.y, 10, 6);

    if (waypoints.length > 1) {
      g.lineStyle(2, pathColor, 0.72);
      g.moveTo(current.x, current.y);
      for (let i = 1; i < waypoints.length; i += 1) {
        g.lineTo(waypoints[i].x, waypoints[i].y);
      }
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
    const facing = Math.atan2(preview.mouseY - e.y, preview.mouseX - e.x);
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

export function _drawBreakthroughAuras(state, entities = []) {
  if (!state || !Array.isArray(entities)) return;
  const g = this._feedbackGfx;
  const definition = ABILITIES[ABILITY.BREAKTHROUGH];
  const tileSize = (this._map && this._map.tileSize) || 32;
  const radiusPx = (definition?.radiusTiles || 0) * tileSize;
  if (radiusPx <= 0) return;

  for (const e of entities) {
    if (e.kind !== KIND.COMMAND_CAR || !(breakthroughAuraExpiresIn(e) > 0)) continue;
    if (!ownOrAllyOwner(state, e.owner)) continue;
    drawBreakthroughAura(g, e.x, e.y, radiusPx, 0.78);
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
      g.lineStyle(1.5, rangeColor, 0.85);
      dashedCircle(g, carrier.x, carrier.y, preview.rangePx, 64);
      if (preview.minRangePx > 0) {
        g.lineStyle(1.3, minRangeColor, 0.82);
        dashedCircle(g, carrier.x, carrier.y, preview.minRangePx, 42);
      }
    }
  }

  if (Array.isArray(preview.returnMarkers)) {
    for (const marker of preview.returnMarkers) {
      if (!finiteNumber(marker.x) || !finiteNumber(marker.y)) continue;
      drawReturnMarker(g, marker.x, marker.y, marker.radiusPx || 13, ABILITY_RETURN_MARKER_COLOR, 0.72);
      g.lineStyle(1.5, ABILITY_RETURN_MARKER_COLOR, 0.45);
      dashedLine(g, marker.x, marker.y, preview.mouseX, preview.mouseY, 8, 6);
    }
  }

  if (Array.isArray(preview.pathOrigins)) {
    for (const origin of preview.pathOrigins) {
      if (!finiteNumber(origin.x) || !finiteNumber(origin.y)) continue;
      const color = origin.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR
        ? MAGIC_ANCHOR_COLOR
        : FIELD_OF_FIRE_COLOR;
      g.lineStyle(2, color, origin.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR ? 0.72 : 0.55);
      dashedLine(g, origin.x, origin.y, preview.mouseX, preview.mouseY, 10, 5);
      g.beginFill(color, 0.2);
      g.drawCircle(origin.x, origin.y, origin.radiusPx || 6);
      g.endFill();
    }
  }

  const cursorInvalid = preview.hoverInsideMinRange === true;
  const cursorColor = preview.hoverInRange ? COLORS.selectOwn : cursorInvalid ? minRangeColor : COLORS.selectNeutral;
  const radiusPx = preview.radiusPx || 24;
  g.lineStyle(2, cursorColor, 0.95);
  g.beginFill(cursorColor, 0.18);
  g.drawCircle(preview.mouseX, preview.mouseY, radiusPx);
  g.endFill();
  g.lineStyle(2, cursorColor, 0.85);
  if (cursorInvalid) {
    const arm = radiusPx * 0.44;
    g.moveTo(preview.mouseX - arm, preview.mouseY - arm);
    g.lineTo(preview.mouseX + arm, preview.mouseY + arm);
    g.moveTo(preview.mouseX + arm, preview.mouseY - arm);
    g.lineTo(preview.mouseX - arm, preview.mouseY + arm);
  } else {
    g.moveTo(preview.mouseX - radiusPx * 0.45, preview.mouseY);
    g.lineTo(preview.mouseX + radiusPx * 0.45, preview.mouseY);
    g.moveTo(preview.mouseX, preview.mouseY - radiusPx * 0.45);
    g.lineTo(preview.mouseX, preview.mouseY + radiusPx * 0.45);
  }
}

function drawBreakthroughAura(g, x, y, radiusPx, alpha = 0.8) {
  const color = 0xf2d16b;
  g.lineStyle(2.5, color, alpha);
  g.drawCircle(x, y, radiusPx);
}

function breakthroughAuraExpiresIn(entity) {
  if (!Array.isArray(entity?.abilities)) return 0;
  const ability = entity.abilities.find((entry) => entry?.ability === ABILITY.BREAKTHROUGH);
  return Number.isFinite(ability?.expiresIn) ? ability.expiresIn : 0;
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
  g.lineStyle(2, color, alpha);
  g.beginFill(color, 0.09);
  g.drawCircle(x, y, radius);
  g.endFill();
  g.moveTo(x, y - radius * 0.7);
  g.lineTo(x + radius * 0.7, y);
  g.lineTo(x, y + radius * 0.7);
  g.lineTo(x - radius * 0.7, y);
  g.lineTo(x, y - radius * 0.7);
}

function drawLineProjectile(g, object, radius, trails) {
  const points = lineProjectileTrailPoints(object, trails);
  if (points.length >= 2) {
    drawLineProjectileTrail(g, points, radius);
  }

  const previous = points.length >= 2 ? points[points.length - 2] : null;
  if (previous) {
    g.lineStyle(Math.max(3.5, radius * 0.72), ABILITY_LINE_SHOT_COLOR, 0.95);
    g.moveTo(previous.x, previous.y);
    g.lineTo(object.x, object.y);
  }

  g.lineStyle(2, ABILITY_LINE_SHOT_COLOR, 0.96);
  g.beginFill(ABILITY_LINE_SHOT_COLOR, 0.68);
  g.drawCircle(object.x, object.y, radius);
  g.endFill();
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
    g.lineStyle(width, ABILITY_LINE_SHOT_COLOR, alpha);
    g.moveTo(points[i - 1].x, points[i - 1].y);
    g.lineTo(points[i].x, points[i].y);
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

function dashedCircle(g, cx, cy, radius, segments) {
  if (!(radius > 0)) return;
  const count = Math.max(12, segments | 0);
  for (let i = 0; i < count; i += 2) {
    const a0 = (i / count) * Math.PI * 2;
    const a1 = ((i + 1) / count) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a0) * radius, cy + Math.sin(a0) * radius);
    g.lineTo(cx + Math.cos(a1) * radius, cy + Math.sin(a1) * radius);
  }
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
  g.beginFill(color, alpha);
  g.drawPolygon(points);
  g.endFill();
}

export function _drawSmokes(state) {
  const smokes = state?.smokes;
  if (!Array.isArray(smokes) || smokes.length === 0) return;
  const g = this._smokeGfx;
  if (!g) return;
  const ts = (this._map && this._map.tileSize) || 32;
  const now = performance.now();
  g.lineStyle(0, 0x000000, 0);
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
  const now = performance.now();
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

    g.lineStyle(2, 0x111111, alpha * 0.45);
    g.moveTo(px - ux * tail, py - uy * tail);
    g.lineTo(px, py);
    g.lineStyle(0, 0x000000, 0);
    g.beginFill(0x050505, alpha);
    g.drawCircle(px, py, 2.7);
    g.endFill();
    g.beginFill(0x2b2b2b, alpha * 0.7);
    g.drawCircle(px - ux * 1.2 - uy * 0.7, py - uy * 1.2 + ux * 0.7, 1.2);
    g.endFill();
  }
}

export function _drawMortarLaunches(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarLaunches !== "function") return;
  const now = performance.now();
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
    g.lineStyle(0, 0x000000, 0);
    g.beginFill(0xfff3b0, 0.88 * flashFade);
    g.drawPolygon([
      launch.x + ux * 2 - uy * 2.8,
      launch.y + uy * 2 + ux * 2.8,
      launch.x + ux * flashLen,
      launch.y + uy * flashLen,
      launch.x + ux * 5 + uy * flashWidth,
      launch.y + uy * 5 - ux * flashWidth,
    ]);
    g.endFill();
    g.beginFill(0xff8b23, 0.48 * flashFade);
    g.drawPolygon([
      launch.x - uy * 4.5,
      launch.y + ux * 4.5,
      launch.x + ux * 16,
      launch.y + uy * 16,
      launch.x + uy * 4.5,
      launch.y - ux * 4.5,
      launch.x - ux * 5,
      launch.y - uy * 5,
    ]);
    g.endFill();
    g.beginFill(0x8a806b, 0.24 * fade);
    g.drawPolygon([
      launch.x - r * 0.95, launch.y - r * 0.14,
      launch.x - r * 0.5, launch.y - r * 0.58,
      launch.x + r * 0.22, launch.y - r * 0.5,
      launch.x + r * 0.86, launch.y - r * 0.16,
      launch.x + r * 0.64, launch.y + r * 0.38,
      launch.x - r * 0.18, launch.y + r * 0.52,
      launch.x - r * 0.82, launch.y + r * 0.28,
    ]);
    g.endFill();
    g.beginFill(0xc0b092, 0.18 * fade);
    g.drawCircle(launch.x - 4, launch.y + 1, 4.5);
    g.drawCircle(launch.x + 4, launch.y - 2, 3.8);
    g.endFill();
  }
}

export function _drawMortarTargets(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarTargets !== "function") return;
  const now = performance.now();
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
      g.lineStyle(1.8, MORTAR_WARNING_COLOR, 0.72 * fade);
      dashedLine(g, target.fromX, target.fromY, target.x, target.y, 10, 7);
    }
    g.lineStyle(2.3, MORTAR_WARNING_COLOR, 0.9 * fade);
    drawDashedCircle(g, target.x, target.y, radius * pulse, 24);
    g.lineStyle(2, MORTAR_WARNING_COLOR, 0.86 * fade);
    g.moveTo(target.x - 14, target.y);
    g.lineTo(target.x + 14, target.y);
    g.moveTo(target.x, target.y - 14);
    g.lineTo(target.x, target.y + 14);
    g.lineStyle(1.4, 0x421010, 0.52 * fade);
    drawDashedCircle(g, target.x, target.y, radius * 0.45, 12);
  }
}

export function _drawMortarShells(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarShells !== "function") return;
  const now = performance.now();
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

    g.lineStyle(0, 0x000000, 0);
    g.beginFill(0x050505, shadowAlpha);
    g.drawEllipse(x, y, 4.4, 2.2);
    g.endFill();
    g.beginFill(0x050505, 1);
    drawFreeRotatedRect(g, x, y, shellLen, shellWidth, angle);
    g.endFill();
    g.beginFill(0x2d2d2d, 1);
    drawFreeRotatedRect(
      g,
      x - uy * shellWidth * 0.24,
      y + ux * shellWidth * 0.24,
      shellLen * 0.55,
      shellWidth * 0.35,
      angle,
    );
    g.endFill();
  }
}

export function _drawMortarImpacts(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarImpacts !== "function") return;
  const now = performance.now();
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
    g.lineStyle(0, 0x000000, 0);

    g.beginFill(0xffb22e, 0.28 * blastFade);
    drawJaggedBlob(g, impact.x, impact.y, outerRadius * 1.05, 18, impact.seed + 11, 0.7, 1.0);
    g.endFill();
    g.beginFill(0xffd65a, 0.2 * blastFade);
    drawJaggedBlob(g, impact.x, impact.y, outerRadius * 0.7, 14, impact.seed + 23, 0.74, 1.0);
    g.endFill();

    g.beginFill(0x6f5c45, 0.3 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, dustRadius, 26, impact.seed + 31, 0.62, 1.0);
    g.endFill();
    g.beginFill(0xa08d70, 0.2 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, dustRadius * 0.74, 22, impact.seed + 43, 0.68, 1.0);
    g.endFill();

    g.beginFill(0x2a2119, 0.24 * dustFade);
    drawJaggedBlob(g, impact.x, impact.y, innerRadius * 1.55, 14, impact.seed + 37, 0.72, 1.0);
    g.endFill();
    g.lineStyle(3, 0xffffff, 0.95 * blastFade);
    drawJaggedRing(g, impact.x, impact.y, innerRadius, 12, impact.seed + 41, 0.72, 1.18);
    g.lineStyle(1.8, 0xfff2d0, 0.7 * blastFade);
    drawJaggedRing(g, impact.x, impact.y, innerRadius * 0.72, 9, impact.seed + 53, 0.78, 1.08);

    drawShrapnel(g, impact.x, impact.y, innerRadius * 0.78, outerRadius, impact.seed, 0.56 * dustFade);
  }
}

export function _drawArtilleryTargets(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveArtilleryTargets !== "function") return;
  const now = performance.now();
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
    g.lineStyle(2.5, 0xffd15c, 0.9 * fade);
    drawDashedCircle(g, target.x, target.y, radius, 28);
    g.lineStyle(2, 0xfff2d0, 0.78 * fade);
    g.moveTo(target.x - 18, target.y);
    g.lineTo(target.x + 18, target.y);
    g.moveTo(target.x, target.y - 18);
    g.lineTo(target.x, target.y + 18);
    g.lineStyle(1.5, 0x2a2119, 0.6 * fade);
    drawDashedCircle(g, target.x, target.y, radius * (0.34 + 0.08 * Math.sin(t * Math.PI)), 12);
    g.lineStyle(2, 0x2a2119, 0.58 * fade);
    g.moveTo(shellX - 12, shellY - 18);
    g.lineTo(shellX, shellY);
    g.lineStyle(0, 0x000000, 0);
    g.beginFill(0xfff2d0, 0.9 * fade);
    g.drawCircle(shellX, shellY, 3.5 + descend * 1.5);
    g.endFill();
    g.beginFill(0x2a2119, 0.2 * fade);
    g.drawCircle(target.x, target.y, 3 + descend * 8);
    g.endFill();
  }
}

export function _drawArtilleryLaunches(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveArtilleryLaunches !== "function") return;
  const now = performance.now();
  const launches = state.liveArtilleryLaunches(now);
  if (!launches.length) return;

  for (const launch of launches) {
    const age = now - launch.createdAt;
    const t = clamp01(age / 820);
    const fade = 1 - smoothstep01(Math.max(0, t - 0.42) / 0.58);
    const burst = 1 + smoothstep01(t) * 1.25;
    const rearX = launch.x - Math.cos(launch.facing) * 22;
    const rearY = launch.y - Math.sin(launch.facing) * 22;
    g.lineStyle(0, 0x000000, 0);
    g.beginFill(0x6f5c45, 0.32 * fade);
    drawJaggedBlob(g, rearX, rearY, 28 * burst, 18, launch.seed + 17, 0.58, 1.0);
    g.endFill();
    g.beginFill(0xa08d70, 0.22 * fade);
    drawJaggedBlob(g, launch.x, launch.y, 20 * burst, 14, launch.seed + 31, 0.62, 1.0);
    g.endFill();
    g.beginFill(0x2a2119, 0.16 * fade);
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
    g.endFill();
  }
}

export function _drawArtilleryImpacts(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveArtilleryImpacts !== "function") return;
  const now = performance.now();
  const impacts = state.liveArtilleryImpacts(now);
  if (!impacts.length) return;
  const ts = (this._map && this._map.tileSize) || 32;

  for (const impact of impacts) {
    const age = now - impact.createdAt;
    const fade = 1 - clamp01(age / 850);
    const outerRadius = Math.max(48, impact.radiusTiles * ts);
    const shock = outerRadius * (1.0 + (1 - fade) * 0.34);
    g.lineStyle(4, 0xfff2d0, 0.92 * fade);
    drawJaggedRing(g, impact.x, impact.y, shock * 0.45, 16, impact.seed + 3, 0.78, 1.15);
    g.beginFill(0xff7a28, 0.28 * fade);
    drawJaggedBlob(g, impact.x, impact.y, shock, 22, impact.seed + 11, 0.62, 1.0);
    g.endFill();
    g.beginFill(0x3b2a1c, 0.34 * fade);
    drawJaggedBlob(g, impact.x, impact.y, shock * 0.62, 14, impact.seed + 23, 0.72, 1.0);
    g.endFill();
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
  g.drawPolygon(poly);
}

function drawJaggedRing(g, cx, cy, radius, points, seed, minScale, maxScale) {
  for (let i = 0; i <= points; i += 1) {
    const j = i % points;
    const a = (j / points) * Math.PI * 2;
    const n = hash2(seed + j * 19, seed + j * 7);
    const r = radius * (minScale + (maxScale - minScale) * n);
    const x = cx + Math.cos(a) * r;
    const y = cy + Math.sin(a) * r;
    if (i === 0) g.moveTo(x, y);
    else g.lineTo(x, y);
  }
}

function drawShrapnel(g, cx, cy, innerRadius, outerRadius, seed, alpha = 0.56) {
  g.lineStyle(1.5, 0x2b2119, alpha);
  const count = 18;
  for (let i = 0; i < count; i += 1) {
    const a = (i / count) * Math.PI * 2 + hash2(seed + i * 5, seed + 99) * 0.18;
    const start = innerRadius + hash2(seed + i * 13, seed + 3) * innerRadius * 0.55;
    const len = 5 + hash2(seed + i * 29, seed + 77) * 11;
    const end = Math.min(outerRadius * 0.94, start + len);
    g.moveTo(cx + Math.cos(a) * start, cy + Math.sin(a) * start);
    g.lineTo(cx + Math.cos(a) * end, cy + Math.sin(a) * end);
  }
}

export function _drawRallyPoints(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  for (const e of state.selectedEntities()) {
    if (!feedbackOwner(state, e.owner)) continue;
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
      g.lineStyle(e.optimisticRally ? 2.5 : 2, color, i === 0 ? 0.55 : 0.35);
      g.moveTo(fromX, fromY);
      g.lineTo(stage.x, stage.y);
      if (i === 0 && !attackMove) {
        // Flag: pole + pennant + base dot for the active move rally.
        g.lineStyle(2.5, color, 0.95);
        g.moveTo(stage.x, stage.y);
        g.lineTo(stage.x, stage.y - 20);
        g.beginFill(color, 0.9);
        g.drawPolygon([stage.x, stage.y - 20, stage.x + 13, stage.y - 16, stage.x, stage.y - 11]);
        g.endFill();
        g.lineStyle(0);
        g.beginFill(color, 0.85);
        g.drawCircle(stage.x, stage.y, 3);
        g.endFill();
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
    g.lineStyle(2.5, color, 0.95);
    g.drawCircle(x, y, 7);
    g.moveTo(x - 6, y - 6);
    g.lineTo(x + 6, y + 6);
    g.moveTo(x + 6, y - 6);
    g.lineTo(x - 6, y + 6);
    return;
  }

  g.lineStyle(2.5, color, 0.95);
  g.beginFill(color, 0.18);
  g.drawPolygon([x, y - 8, x + 8, y, x, y + 8, x - 8, y]);
  g.endFill();
  g.lineStyle(0);
  g.beginFill(color, 0.9);
  g.drawCircle(x, y, 2.5);
  g.endFill();
}

function drawPointFireMarker(g, x, y, color, alpha = 0.95) {
  g.lineStyle(2.5, color, alpha);
  g.drawCircle(x, y, 10);
  g.moveTo(x - 13, y);
  g.lineTo(x + 13, y);
  g.moveTo(x, y - 13);
  g.lineTo(x, y + 13);
  g.lineStyle(1.5, color, alpha * 0.78);
  drawDashedCircle(g, x, y, 18, 12);
}

function drawDebugCurrentWaypoint(g, x, y, color) {
  g.lineStyle(3, color, 0.98);
  g.beginFill(color, 0.18);
  g.drawCircle(x, y, 10);
  g.endFill();
  g.lineStyle(1.5, color, 0.9);
  g.drawCircle(x, y, 15);
  g.moveTo(x - 13, y);
  g.lineTo(x + 13, y);
  g.moveTo(x, y - 13);
  g.lineTo(x, y + 13);
}

function drawDebugWaypoint(g, x, y, color, index) {
  const radius = index % 2 === 0 ? 5.5 : 4.5;
  g.lineStyle(2, color, 0.85);
  g.beginFill(color, 0.14);
  g.drawCircle(x, y, radius);
  g.endFill();
}

function drawDebugGoal(g, x, y, color, alpha) {
  g.lineStyle(2.5, color, alpha);
  g.drawRect(x - 8, y - 8, 16, 16);
  g.moveTo(x - 11, y);
  g.lineTo(x + 11, y);
  g.moveTo(x, y - 11);
  g.lineTo(x, y + 11);
}

function drawDebugTruncatedTail(g, x, y, color) {
  g.lineStyle(2, color, 0.72);
  g.moveTo(x + 10, y - 6);
  g.lineTo(x + 16, y);
  g.lineTo(x + 10, y + 6);
}

function drawDashedCircle(g, x, y, radius, segments) {
  const count = Math.max(6, segments | 0);
  for (let i = 0; i < count; i += 2) {
    const a0 = (i / count) * Math.PI * 2;
    const a1 = ((i + 1) / count) * Math.PI * 2;
    g.moveTo(x + Math.cos(a0) * radius, y + Math.sin(a0) * radius);
    g.lineTo(x + Math.cos(a1) * radius, y + Math.sin(a1) * radius);
  }
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
    g.lineStyle(4, 0x4aa3ff, 0.95);
    g.beginFill(0x4aa3ff, 0.18);
    g.drawCircle(p.resourceX, p.resourceY, 9);
    g.endFill();
    return;
  }

  g.lineStyle(2.5, 0xd64d45, 0.9);
  dashedLine(g, p.resourceX, p.resourceY, ccEndpoint.x, ccEndpoint.y, 14, 9);
}

export function _drawMuzzleFlashes(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMuzzleFlashes !== "function") return;
  const now = performance.now();
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

      g.lineStyle(style.tracerWidth, style.tracerColor, style.tracerAlpha * fade);
      g.moveTo(mx, my);
      g.lineTo(targetPos.x, targetPos.y);
      if (style.tracerCoreWidth > 0) {
        g.lineStyle(style.tracerCoreWidth, style.tracerCoreColor, style.tracerCoreAlpha * fade);
        g.moveTo(mx, my);
        g.lineTo(targetPos.x, targetPos.y);
      }

      if (shotLen > 0.001 && tailLen > 0) {
        const ux = dx / shotLen;
        const uy = dy / shotLen;
        const ex = targetPos.x + ux * tailLen;
        const ey = targetPos.y + uy * tailLen;
        g.lineStyle(style.tailWidth, style.tailColor, style.tailAlpha * fade);
        g.moveTo(targetPos.x, targetPos.y);
        g.lineTo(ex, ey);
      }
    }

    const rigOwnsTankCannonFlash = attacker.kind === KIND.TANK && f.weaponKind !== WEAPON_KIND.TANK_COAX;
    const suppressCircularFlash = feedbackKind === KIND.RIFLEMAN;
    if (!rigOwnsTankCannonFlash && !suppressCircularFlash) {
      // Flash: bright core that scales up slightly then fades.
      const r = baseR * (0.7 + 0.45 * t);
      g.lineStyle(0);
      g.beginFill(0xfff2a8, 0.85 * fade);
      g.drawCircle(mx, my, r);
      g.endFill();
      g.beginFill(0xffd84a, 0.55 * fade);
      g.drawCircle(mx, my, r * 0.55);
      g.endFill();
    }
  }
}

export function drawSelectionBox(rect) {
  const g = this._dragGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.dragSelection");
  g.clear();
  if (!rect) return;
  const { x, y, w, h } = normRect(rect);
  g.lineStyle(1.5, COLORS.dragBox, 0.95);
  g.beginFill(COLORS.dragBox, 0.12);
  g.drawRect(x, y, w, h);
  g.endFill();
}
