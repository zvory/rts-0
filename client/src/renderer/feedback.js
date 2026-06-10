import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
  STATS,
  PLAYER_PALETTE,
  RESOURCE_AMOUNTS,
  AT_GUN_DEPLOYED_RANGE_TILES,
  AT_GUN_FIELD_OF_FIRE_RAD,
  MORTAR_INNER_RADIUS_TILES,
  MINING_CC_RANGE_TILES,
  isProducerBuilding,
} from "../config.js";
import { KIND, ORDER_STAGE, SETUP, STATE, isBuilding, isResource, isUnit } from "../protocol.js";
import {
  DEPLOYED_WEAPON_ANIM_MS,
  SWEEP_EVICT_FRAMES,
  WEAPON_RECOIL_PX,
  ZERO_OFFSET,
} from "./palette.js";
import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAtGun,
  drawFacingWedge,
  drawImpassableEdge,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawRotatedRect,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hash2,
  hexToInt,
  isImpassableAt,
  isVehicleBodyKind,
  muzzleFlashRadius,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  smoothstep01,
  tankBodyVisual,
  terrainColor,
  terrainOverlayColor,
  weaponRecoilOffset,
} from "./shared.js";

export function _drawPlacement(state, fog) {
  const g = this._placementGfx;
  g.clear();
  const p = state.placement;
  if (!p) return;
  const ts = (this._map && this._map.tileSize) || 32;
  const stat = STATS[p.building] || {};
  const w = (stat.footW || 2) * ts;
  const h = (stat.footH || 2) * ts;
  const x0 = p.tileX * ts;
  const y0 = p.tileY * ts;
  const color = p.valid ? COLORS.placeOk : COLORS.placeBad;

  g.lineStyle(2, color, 0.95);
  g.beginFill(color, 0.25);
  g.drawRoundedRect(x0, y0, w, h, 6);
  g.endFill();

  // Per-tile grid hint inside the footprint so the snap target is obvious.
  g.lineStyle(1, color, 0.4);
  for (let i = 1; i < (stat.footW || 2); i++) {
    g.moveTo(x0 + i * ts, y0);
    g.lineTo(x0 + i * ts, y0 + h);
  }
  for (let j = 1; j < (stat.footH || 2); j++) {
    g.moveTo(x0, y0 + j * ts);
    g.lineTo(x0 + w, y0 + j * ts);
  }

  if (p.building !== KIND.CITY_CENTRE) return;

  const cx = x0 + w / 2;
  const cy = y0 + h / 2;
  const rangePx = MINING_CC_RANGE_TILES * ts;
  const rangeSq = rangePx * rangePx;
  const resourceColor = 0x4aa3ff;
  for (const node of state.map?.resources || []) {
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

export function _drawCommandFeedback(state) {
  const g = this._feedbackGfx;
  g.clear();
  if (!state || typeof state.liveCommandFeedback !== "function") return;

  const now = performance.now();
  for (const f of state.liveCommandFeedback(now)) {
    const age = now - f.createdAt;
    const t = clamp01(age / 650);
    const alpha = (1 - t) * 0.95;
    const r = 12 + t * 10;
    const color = f.kind === "attack" || f.kind === "mortar" ? COLORS.selectEnemy : COLORS.selectOwn;

    g.lineStyle(2, color, alpha);
    if (f.kind === "mortar") {
      const splash = Number.isFinite(f.radiusTiles) ? f.radiusTiles * 32 : 48;
      drawDashedCircle(g, f.x, f.y, splash, 14);
      g.drawCircle(f.x, f.y, r * 0.45);
      g.moveTo(f.x - r * 0.7, f.y);
      g.lineTo(f.x + r * 0.7, f.y);
      g.moveTo(f.x, f.y - r * 0.7);
      g.lineTo(f.x, f.y + r * 0.7);
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

export function _drawOrderPlan(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  const moveColor = COLORS.selectOwn;
  const attackColor = COLORS.selectEnemy;

  for (const e of state.selectedEntities()) {
    if (e.owner !== state.playerId || !isUnit(e.kind)) continue;
    const markers = Array.isArray(e.orderPlan)
      ? e.orderPlan.filter((m) => Number.isFinite(m?.x) && Number.isFinite(m?.y))
      : [];
    if (markers.length === 0) continue;

    let fromX = e.x;
    let fromY = e.y;
    for (let i = 0; i < markers.length; i += 1) {
      const marker = markers[i];
      const hostile = marker.kind === ORDER_STAGE.ATTACK || marker.kind === ORDER_STAGE.ATTACK_MOVE;
      const attackMove = marker.kind === ORDER_STAGE.ATTACK_MOVE;
      const color = hostile ? attackColor : moveColor;
      const alpha = i === 0 ? 0.68 : 0.48;
      g.lineStyle(2, color, alpha);
      if (attackMove) {
        dashedLine(g, fromX, fromY, marker.x, marker.y, 12, 8);
      } else {
        g.moveTo(fromX, fromY);
        g.lineTo(marker.x, marker.y);
      }

      drawQueuedPointMarker(g, marker.x, marker.y, color, hostile);
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
    if (e.owner !== state.playerId || !isUnit(e.kind) || e.state !== STATE.MOVE) continue;
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

export function _drawAtGunSetupPreview(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  const tileSize = (this._map && this._map.tileSize) || 32;
  const radius = AT_GUN_DEPLOYED_RANGE_TILES * tileSize;
  const color = 0x4aa3ff;

  for (const e of state.selectedEntities()) {
    if (e.owner !== state.playerId || e.kind !== KIND.AT_TEAM) continue;
    if (e.setupState !== SETUP.DEPLOYED) continue;
    const facing = finiteNumber(e.setupFacing) ? e.setupFacing : finiteNumber(e.facing) ? e.facing : null;
    if (facing == null) continue;
    drawFacingWedge(g, e.x, e.y, radius, facing, AT_GUN_FIELD_OF_FIRE_RAD, color, 0.08, 0.26);
  }

  const preview = state.atGunSetupPreview;
  if (!preview || !Array.isArray(preview.guns)) return;
  for (const e of preview.guns) {
    if (!finiteNumber(e.x) || !finiteNumber(e.y)) continue;
    const facing = Math.atan2(preview.mouseY - e.y, preview.mouseX - e.x);
    if (!Number.isFinite(facing)) continue;
    drawFacingWedge(g, e.x, e.y, radius, facing, AT_GUN_FIELD_OF_FIRE_RAD, color, 0.16, 0.58);
  }
}

export function _drawAbilityTargetPreview(state) {
  const preview = state?.abilityTargetPreview;
  if (!preview || !Array.isArray(preview.carriers)) return;
  const g = this._feedbackGfx;
  const rangeColor = 0x6fa3ff;

  for (const carrier of preview.carriers) {
    if (!finiteNumber(carrier.x) || !finiteNumber(carrier.y)) continue;
    g.lineStyle(1.5, rangeColor, 0.85);
    dashedCircle(g, carrier.x, carrier.y, preview.rangePx, 64);
  }

  const cursorColor = preview.hoverInRange ? COLORS.selectOwn : COLORS.selectNeutral;
  const radiusPx = preview.radiusPx || 24;
  g.lineStyle(2, cursorColor, 0.95);
  g.beginFill(cursorColor, 0.18);
  g.drawCircle(preview.mouseX, preview.mouseY, radiusPx);
  g.endFill();
  g.lineStyle(2, cursorColor, 0.85);
  g.moveTo(preview.mouseX - radiusPx * 0.45, preview.mouseY);
  g.lineTo(preview.mouseX + radiusPx * 0.45, preview.mouseY);
  g.moveTo(preview.mouseX, preview.mouseY - radiusPx * 0.45);
  g.lineTo(preview.mouseX, preview.mouseY + radiusPx * 0.45);
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
    g.lineStyle(0, 0x000000, 0);
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

export function _drawMortarImpacts(state) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMortarImpacts !== "function") return;
  const now = performance.now();
  const impacts = state.liveMortarImpacts(now);
  if (!impacts.length) return;
  const ts = (this._map && this._map.tileSize) || 32;
  const innerRadius = MORTAR_INNER_RADIUS_TILES * ts;

  for (const impact of impacts) {
    const outerRadius = Math.max(innerRadius + 8, impact.radiusTiles * ts);
    g.lineStyle(0, 0x000000, 0);
    g.beginFill(0xff4500, 0.23);
    drawJaggedBlob(g, impact.x, impact.y, outerRadius, 18, impact.seed + 11, 0.7, 1.0);
    g.endFill();
    g.beginFill(0xff8c3a, 0.16);
    drawJaggedBlob(g, impact.x, impact.y, outerRadius * 0.72, 14, impact.seed + 23, 0.74, 1.0);
    g.endFill();

    g.beginFill(0x2a2119, 0.28);
    drawJaggedBlob(g, impact.x, impact.y, innerRadius * 1.18, 10, impact.seed + 37, 0.72, 1.0);
    g.endFill();
    g.lineStyle(3, 0xffffff, 0.95);
    drawJaggedRing(g, impact.x, impact.y, innerRadius, 12, impact.seed + 41, 0.72, 1.18);
    g.lineStyle(1.8, 0xfff2d0, 0.7);
    drawJaggedRing(g, impact.x, impact.y, innerRadius * 0.72, 9, impact.seed + 53, 0.78, 1.08);

    drawShrapnel(g, impact.x, impact.y, innerRadius * 0.78, outerRadius, impact.seed);
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

function drawShrapnel(g, cx, cy, innerRadius, outerRadius, seed) {
  g.lineStyle(1.5, 0x2b2119, 0.56);
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
  const color = COLORS.selectOwn;
  for (const e of state.selectedEntities()) {
    if (e.owner !== state.playerId) continue;
    if (!isBuilding(e.kind) || !isProducerBuilding(e.kind)) continue;
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
      g.lineStyle(2, color, i === 0 ? 0.55 : 0.35);
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

export function _drawResourceMiningPreview(state) {
  if (!state || !state.resourceMiningPreview) return;
  const g = this._feedbackGfx;
  const p = state.resourceMiningPreview;
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

    const baseR = muzzleFlashRadius(attacker.kind);
    if (baseR <= 0) continue;

    const facing = isVehicleBodyKind(attacker.kind) && typeof attacker.weaponFacing === "number"
      ? attacker.weaponFacing
      : typeof attacker.facing === "number"
      ? attacker.facing
      : targetPos
      ? Math.atan2(targetPos.y - attacker.y, targetPos.x - attacker.x)
      : 0;
    const stat = STATS[attacker.kind] || {};
    const reach = isBuilding(attacker.kind)
      ? Math.max(stat.footW || 2, stat.footH || 2) * ((this._map && this._map.tileSize) || 32) * 0.5
      : attacker.kind === KIND.AT_TEAM
        ? (stat.size || 9) * 1.9
      : (stat.size || 9) * 1.1;
    const mx = attacker.x + Math.cos(facing) * reach;
    const my = attacker.y + Math.sin(facing) * reach;

    if (targetPos) {
      const dx = targetPos.x - mx;
      const dy = targetPos.y - my;
      const shotLen = Math.hypot(dx, dy);
      // Mirror the server overpenetration band: a round that hits a tank stops dead (no tail),
      // and AT teams punch twice as deep as everyone else.
      const tileSize = (this._map && this._map.tileSize) || 32;
      const penFactor = target?.kind === KIND.TANK ? 0 : attacker.kind === KIND.AT_TEAM ? 0.5 : 0.25;
      const tailLen = (stat.rangeTiles || 0) * tileSize * penFactor;
      const tracerWidth = attacker.kind === KIND.AT_TEAM ? 2.5 : 1.5;

      g.lineStyle(tracerWidth, 0xffe066, 0.92 * fade);
      g.moveTo(mx, my);
      g.lineTo(targetPos.x, targetPos.y);

      if (shotLen > 0.001 && tailLen > 0) {
        const ux = dx / shotLen;
        const uy = dy / shotLen;
        const ex = targetPos.x + ux * tailLen;
        const ey = targetPos.y + uy * tailLen;
        g.lineStyle(attacker.kind === KIND.AT_TEAM ? 1.4 : 1.0, 0xffd84a, 0.46 * fade);
        g.moveTo(targetPos.x, targetPos.y);
        g.lineTo(ex, ey);
      }
    }

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

export function drawSelectionBox(rect) {
  const g = this._dragGfx;
  g.clear();
  if (!rect) return;
  const { x, y, w, h } = normRect(rect);
  g.lineStyle(1.5, COLORS.dragBox, 0.95);
  g.beginFill(COLORS.dragBox, 0.12);
  g.drawRect(x, y, w, h);
  g.endFill();
}
