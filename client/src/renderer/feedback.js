import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
  STATS,
  PLAYER_PALETTE,
  RESOURCE_AMOUNTS,
  AT_GUN_DEPLOYED_RANGE_TILES,
  AT_GUN_FIELD_OF_FIRE_RAD,
  MINING_CC_RANGE_TILES,
  isProducerBuilding,
} from "../config.js";
import { KIND, SETUP, STATE, isBuilding, isResource } from "../protocol.js";
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
    const color = f.kind === "attack" ? COLORS.selectEnemy : COLORS.selectOwn;

    g.lineStyle(2, color, alpha);
    if (f.kind === "attack") {
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

export function _drawRallyPoints(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
  const g = this._feedbackGfx;
  const color = COLORS.selectOwn;
  for (const e of state.selectedEntities()) {
    if (e.owner !== state.playerId) continue;
    if (!isBuilding(e.kind) || !isProducerBuilding(e.kind)) continue;
    const rally = e.rally;
    if (!rally) continue;
    const [rx, ry] = rally;

    // Link from the building to the rally point.
    g.lineStyle(2, color, 0.5);
    g.moveTo(e.x, e.y);
    g.lineTo(rx, ry);

    // Flag: pole + pennant + base dot.
    g.lineStyle(2.5, color, 0.95);
    g.moveTo(rx, ry);
    g.lineTo(rx, ry - 20);
    g.beginFill(color, 0.9);
    g.drawPolygon([rx, ry - 20, rx + 13, ry - 16, rx, ry - 11]);
    g.endFill();
    g.lineStyle(0);
    g.beginFill(color, 0.85);
    g.drawCircle(rx, ry, 3);
    g.endFill();
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

    const age = now - f.createdAt;
    const t = clamp01(age / 240);
    const fade = 1 - t;

    const baseR = muzzleFlashRadius(attacker.kind);
    if (baseR <= 0) continue;

    const facing = isVehicleBodyKind(attacker.kind) && typeof attacker.weaponFacing === "number"
      ? attacker.weaponFacing
      : typeof attacker.facing === "number"
      ? attacker.facing
      : target
      ? Math.atan2(target.y - attacker.y, target.x - attacker.x)
      : 0;
    const stat = STATS[attacker.kind] || {};
    const reach = isBuilding(attacker.kind)
      ? Math.max(stat.footW || 2, stat.footH || 2) * ((this._map && this._map.tileSize) || 32) * 0.5
      : (stat.size || 9) * 1.1;
    const mx = attacker.x + Math.cos(facing) * reach;
    const my = attacker.y + Math.sin(facing) * reach;

    if (target) {
      const dx = target.x - mx;
      const dy = target.y - my;
      const shotLen = Math.hypot(dx, dy);
      // Mirror the server overpenetration band: a round that hits a tank stops dead (no tail),
      // and AT teams punch twice as deep as everyone else.
      const tileSize = (this._map && this._map.tileSize) || 32;
      const penFactor = target.kind === KIND.TANK ? 0 : attacker.kind === KIND.AT_TEAM ? 0.5 : 0.25;
      const tailLen = (stat.rangeTiles || 0) * tileSize * penFactor;

      g.lineStyle(1.5, 0xffe066, 0.9 * fade);
      g.moveTo(mx, my);
      g.lineTo(target.x, target.y);

      if (shotLen > 0.001 && tailLen > 0) {
        const ux = dx / shotLen;
        const uy = dy / shotLen;
        const ex = target.x + ux * tailLen;
        const ey = target.y + uy * tailLen;
        g.lineStyle(1.0, 0xffd84a, 0.42 * fade);
        g.moveTo(target.x, target.y);
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
