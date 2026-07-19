import {
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  COLORS,
} from "../config.js";
import { ABILITY } from "../protocol.js";
import {
  dashedLine,
  drawFacingWedge,
  finiteNumber,
} from "./shared.js";

const FIELD_OF_FIRE_COLOR = 0x4aa3ff;

export function isArtilleryFirePreview(preview) {
  return preview?.ability === ABILITY.POINT_FIRE || preview?.ability === ABILITY.BLANKET_FIRE;
}

export function drawArtilleryFireTargetPreview(g, preview, map) {
  const locks = Array.isArray(preview.artilleryLocks) ? preview.artilleryLocks : [];
  if (locks.length === 0) return false;
  const tileSize = map?.tileSize || 32;
  const weapon = artilleryFieldOfFireProfile(tileSize);
  const radiusPx = Number.isFinite(preview.radiusPx) ? preview.radiusPx : 0;
  const targetColor = preview.hoverInRange ? COLORS.selectOwn : COLORS.selectNeutral;
  for (const lock of locks) {
    if (!finiteNumber(lock.x) || !finiteNumber(lock.y)) continue;
    if (lock.needsRedeploy && finiteNumber(lock.originX) && finiteNumber(lock.originY)) {
      drawFacingWedge(
        g,
        lock.originX,
        lock.originY,
        lock.rangePx || weapon.maxRadius,
        lock.facing,
        weapon.arc,
        FIELD_OF_FIRE_COLOR,
        0.06,
        0.38,
        lock.minRangePx || weapon.minRadius,
      );
    }
    if (
      finiteNumber(lock.rawX) &&
      finiteNumber(lock.rawY) &&
      Math.hypot(lock.rawX - lock.x, lock.rawY - lock.y) > 1
    ) {
      g.lineStyle(1.5, 0xffd15c, 0.48);
      dashedLine(g, lock.rawX, lock.rawY, lock.x, lock.y, 8, 6);
    }
    drawLockedArtilleryTarget(g, lock.x, lock.y, radiusPx, targetColor, preview.ability);
  }
  return true;
}

function artilleryFieldOfFireProfile(tileSize) {
  return {
    minRadius: ARTILLERY_MIN_RANGE_TILES * tileSize,
    maxRadius: ARTILLERY_MAX_RANGE_TILES * tileSize,
    arc: ARTILLERY_FIELD_OF_FIRE_RAD,
  };
}

function drawLockedArtilleryTarget(g, x, y, radiusPx, color, ability) {
  const markerRadius = ability === ABILITY.BLANKET_FIRE
    ? Math.max(18, radiusPx)
    : Math.max(18, radiusPx || 24);
  g.lineStyle(2, color, 0.95);
  drawDashedCircle(g, x, y, markerRadius, ability === ABILITY.BLANKET_FIRE ? 36 : 18);
  g.beginFill(color, 0.14);
  g.drawCircle(x, y, ability === ABILITY.BLANKET_FIRE ? 7 : Math.min(18, markerRadius));
  g.endFill();
  g.lineStyle(2, color, 0.85);
  const arm = ability === ABILITY.BLANKET_FIRE ? 13 : Math.min(18, markerRadius * 0.45);
  g.moveTo(x - arm, y);
  g.lineTo(x + arm, y);
  g.moveTo(x, y - arm);
  g.lineTo(x, y + arm);
}

function drawDashedCircle(g, x, y, radius, segments) {
  if (!(radius > 0)) return;
  const count = Math.max(8, segments || 16);
  for (let i = 0; i < count; i += 1) {
    const a0 = (i / count) * Math.PI * 2;
    const a1 = ((i + 0.5) / count) * Math.PI * 2;
    g.moveTo(x + Math.cos(a0) * radius, y + Math.sin(a0) * radius);
    g.lineTo(x + Math.cos(a1) * radius, y + Math.sin(a1) * radius);
  }
}
