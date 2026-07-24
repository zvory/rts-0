import { gfxStrokePaths } from "./native_graphics.js";
import {
  ABILITIES,
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  MORTAR_FIELD_OF_FIRE_RAD,
  MORTAR_MIN_RANGE_TILES,
  MORTAR_RANGE_TILES,
  STATS,
} from "../config.js";
import { ABILITY, KIND, SETUP, isUnit } from "../protocol.js";
import { feedbackOwner } from "./feedback_ownership.js";
import { drawFacingWedge, finiteNumber } from "./shared.js";

const UNIT_RANGE_COLOR = 0x8eb7ff;
const UNIT_RANGE_MIN_COLOR = 0x9f3a34;
const UNIT_RANGE_DOT_SPACING_PX = 15;
const UNIT_RANGE_LINE_ALPHA = 0.68;
const UNIT_RANGE_MIN_LINE_ALPHA = 0.56;
const UNIT_FIELD_OF_FIRE_FILL_ALPHA = 0.07;
const UNIT_FIELD_OF_FIRE_LINE_ALPHA = 0.36;
const ENEMY_AT_THREAT_COLOR = 0xffb000;
const ENEMY_AT_THREAT_DARK_COLOR = 0x3d2b00;
const ENEMY_AT_MEMORY_COLOR = 0xffdce5;
const ENEMY_AT_MEMORY_DARK_COLOR = 0x65424c;
const ENEMY_AT_THREAT_HATCH_SPACING_PX = 60;
const ENEMY_AT_THREAT_HATCH_ANGLE = Math.PI / 4;
const ENEMY_AT_THREAT_ARC_STEP_PX = 42;
const ENEMY_AT_THREAT_OUTLINE_WIDTH = 1.95;

export function _drawSelectedMortarRanges(state) {
  return _drawSelectedUnitRanges.call(this, state);
}

export function _drawSelectedUnitRanges(state) {
  if (!state) return;
  drawEnemyAntiTankGunThreats(this._feedbackGfx, state, (this._map && this._map.tileSize) || 32);
  if (typeof state.selectedEntities !== "function") return;
  const drawAllRanges = !!state.showUnitRangesEnabled;
  const drawSelectedFieldOfFire = !!state.showSelectedFieldOfFireEnabled;
  if (!drawAllRanges && !drawSelectedFieldOfFire) return;
  const g = this._feedbackGfx;
  const tileSize = (this._map && this._map.tileSize) || 32;

  for (const e of state.selectedEntities()) {
    if (!finiteNumber(e.x) || !finiteNumber(e.y)) continue;
    if (!feedbackOwner(state, e.owner) || !isUnit(e.kind)) continue;
    const profile = selectedUnitRangeProfile(e, tileSize);
    if (!profile) continue;
    if (!drawAllRanges && profile.kind !== "fieldOfFire") continue;
    if (profile.kind === "fieldOfFire") {
      drawFacingWedge(
        g,
        e.x,
        e.y,
        profile.maxRadius,
        profile.facing,
        profile.arc,
        UNIT_RANGE_COLOR,
        UNIT_FIELD_OF_FIRE_FILL_ALPHA,
        UNIT_FIELD_OF_FIRE_LINE_ALPHA,
        profile.minRadius,
      );
    } else {
      dottedCircle(g, e.x, e.y, profile.maxRadius, UNIT_RANGE_DOT_SPACING_PX,
        UNIT_RANGE_COLOR, UNIT_RANGE_LINE_ALPHA);
      if (profile.minRadius > 0) {
        dottedCircle(g, e.x, e.y, profile.minRadius, UNIT_RANGE_DOT_SPACING_PX * 0.8,
          UNIT_RANGE_MIN_COLOR, UNIT_RANGE_MIN_LINE_ALPHA);
      }
    }
  }
}

function drawEnemyAntiTankGunThreats(g, state, tileSize) {
  if (typeof state.enemyAntiTankGunThreats !== "function") return;
  const weapon = fieldOfFireProfile(KIND.ANTI_TANK_GUN, tileSize);
  if (!weapon) return;

  for (const entity of state.enemyAntiTankGunThreats()) {
    const facing = firstFinite(entity?.setupFacing, entity?.weaponFacing, entity?.facing);
    if (!finiteNumber(entity?.x) || !finiteNumber(entity?.y) || !finiteNumber(facing)) continue;
    const paths = hatchedWedgePaths(
      entity.x,
      entity.y,
      weapon.maxRadius,
      facing,
      weapon.arc,
      ENEMY_AT_THREAT_HATCH_SPACING_PX,
      ENEMY_AT_THREAT_HATCH_ANGLE,
    );
    const remembered = entity?.threatMemory === true;
    const color = remembered ? ENEMY_AT_MEMORY_COLOR : ENEMY_AT_THREAT_COLOR;
    const darkColor = remembered ? ENEMY_AT_MEMORY_DARK_COLOR : ENEMY_AT_THREAT_DARK_COLOR;
    const darkAlpha = remembered ? 0.1 : 0.26;
    const hatchAlpha = remembered ? 0.32 : 0.78;
    const hatchWidth = remembered ? 0.8 : 1.3;
    const keylineWidth = remembered ? 1.8 : 2.8;
    const outlineAlpha = remembered ? 0.26 : 0.68;
    // Luminance and stroke weight distinguish live threats from stale intel even
    // without red/green hue perception. The dark keyline remains legible on snow.
    gfxStrokePaths(g, paths, keylineWidth, darkColor, darkAlpha);
    gfxStrokePaths(g, paths, hatchWidth, color, hatchAlpha);
    drawFacingWedge(
      g,
      entity.x,
      entity.y,
      weapon.maxRadius,
      facing,
      weapon.arc,
      color,
      0,
      outlineAlpha,
      0,
      ENEMY_AT_THREAT_OUTLINE_WIDTH,
    );
  }
}

function hatchedWedgePaths(cx, cy, radius, facing, arc, spacing, hatchAngle) {
  if (!(radius > 0) || !(arc > 0) || !(spacing > 0)) return [];
  const polygon = wedgePolygon(cx, cy, radius, facing, arc);
  const direction = { x: Math.cos(hatchAngle), y: Math.sin(hatchAngle) };
  const normal = { x: -direction.y, y: direction.x };
  const reach = radius * 2;
  const paths = [];
  for (let offset = -radius; offset <= radius; offset += spacing) {
    const midX = cx + normal.x * offset;
    const midY = cy + normal.y * offset;
    const clipped = clipSegmentToConvexPolygon(
      [midX - direction.x * reach, midY - direction.y * reach],
      [midX + direction.x * reach, midY + direction.y * reach],
      polygon,
    );
    if (clipped) paths.push(clipped);
  }
  return paths;
}

function wedgePolygon(cx, cy, radius, facing, arc) {
  const start = facing - arc / 2;
  const steps = Math.max(8, Math.ceil((radius * arc) / ENEMY_AT_THREAT_ARC_STEP_PX));
  const points = [[cx, cy]];
  for (let i = 0; i <= steps; i += 1) {
    const angle = start + (arc * i) / steps;
    points.push([cx + Math.cos(angle) * radius, cy + Math.sin(angle) * radius]);
  }
  return points;
}

function clipSegmentToConvexPolygon(start, end, polygon) {
  const winding = polygonSignedArea(polygon) >= 0 ? 1 : -1;
  const dx = end[0] - start[0];
  const dy = end[1] - start[1];
  let enter = 0;
  let exit = 1;

  for (let i = 0; i < polygon.length; i += 1) {
    const a = polygon[i];
    const b = polygon[(i + 1) % polygon.length];
    const edgeX = b[0] - a[0];
    const edgeY = b[1] - a[1];
    const startSide = winding * (edgeX * (start[1] - a[1]) - edgeY * (start[0] - a[0]));
    const deltaSide = winding * (edgeX * dy - edgeY * dx);
    if (Math.abs(deltaSide) < 1e-9) {
      if (startSide < 0) return null;
      continue;
    }
    const crossing = -startSide / deltaSide;
    if (deltaSide > 0) enter = Math.max(enter, crossing);
    else exit = Math.min(exit, crossing);
    if (enter > exit) return null;
  }

  if (exit < 0 || enter > 1) return null;
  const t0 = Math.max(0, enter);
  const t1 = Math.min(1, exit);
  if (t1 - t0 <= 1e-6) return null;
  return [
    [start[0] + dx * t0, start[1] + dy * t0],
    [start[0] + dx * t1, start[1] + dy * t1],
  ];
}

function polygonSignedArea(points) {
  let twiceArea = 0;
  for (let i = 0; i < points.length; i += 1) {
    const a = points[i];
    const b = points[(i + 1) % points.length];
    twiceArea += a[0] * b[1] - b[0] * a[1];
  }
  return twiceArea / 2;
}

function selectedUnitRangeProfile(e, tileSize) {
  if (e?.kind === KIND.WORKER) return null;
  const dynamic = dynamicUnitRangeProfile(e, tileSize);
  if (dynamic !== undefined) return dynamic;
  return staticUnitRangeProfile(e, tileSize);
}

function dynamicUnitRangeProfile(e, tileSize) {
  const source = firstObject(e?.weaponRangeProfile, e?.firingRangeProfile, e?.attackRangeProfile);
  const hasDynamicRangeFields = !!source || firstFinite(
    e?.weaponRangePx,
    e?.firingRangePx,
    e?.attackRangePx,
    e?.weaponRangeTiles,
    e?.firingRangeTiles,
    e?.attackRangeTiles,
  ) !== null;
  if (!hasDynamicRangeFields) return undefined;
  if (source?.active === false || source?.available === false) return null;
  const maxRadius = firstFinite(
    source?.maxPx,
    source?.rangePx,
    e?.weaponRangePx,
    e?.firingRangePx,
    e?.attackRangePx,
  ) ?? tilesToPx(firstFinite(
    source?.maxTiles,
    source?.rangeTiles,
    e?.weaponRangeTiles,
    e?.firingRangeTiles,
    e?.attackRangeTiles,
  ), tileSize);
  if (!(maxRadius > 0)) return null;

  const minRadius = Math.max(0, firstFinite(
    source?.minPx,
    e?.weaponMinRangePx,
    e?.firingMinRangePx,
    e?.attackMinRangePx,
  ) ?? tilesToPx(firstFinite(
    source?.minTiles,
    e?.weaponMinRangeTiles,
    e?.firingMinRangeTiles,
    e?.attackMinRangeTiles,
  ), tileSize) ?? 0);
  const arc = firstFinite(source?.arcRad, source?.arc, e?.weaponArcRad, e?.firingArcRad, e?.attackArcRad);
  const facing = firstFinite(source?.facing, e?.setupFacing, e?.weaponFacing, e?.facing);
  if (arc > 0 && arc < Math.PI * 2 && finiteNumber(facing)) {
    return {
      kind: "fieldOfFire",
      minRadius,
      maxRadius,
      arc,
      facing,
    };
  }
  return { kind: "circle", minRadius, maxRadius };
}

function staticUnitRangeProfile(e, tileSize) {
  if (
    e.kind === KIND.ARTILLERY ||
    e.kind === KIND.ANTI_TANK_GUN ||
    (e.kind === KIND.MORTAR_TEAM && e.setupState === SETUP.DEPLOYED)
  ) {
    if (e.setupState !== SETUP.DEPLOYED) return null;
    const weapon = fieldOfFireProfile(e.kind, tileSize);
    if (!weapon) return null;
    const facing = firstFinite(e.setupFacing, e.weaponFacing, e.facing);
    if (!finiteNumber(facing) && weapon.arc < Math.PI * 2) return null;
    return {
      kind: "fieldOfFire",
      minRadius: weapon.minRadius,
      maxRadius: weapon.maxRadius,
      arc: weapon.arc,
      facing: finiteNumber(facing) ? facing : 0,
    };
  }

  const stat = STATS[e.kind] || {};
  const rangeTiles = e.kind === KIND.MORTAR_TEAM
    ? ABILITIES[ABILITY.MORTAR_FIRE]?.rangeTiles || stat.rangeTiles || 0
    : stat.rangeTiles || 0;
  const maxRadius = rangeTiles * tileSize;
  if (!(maxRadius > 0)) return null;
  const minRadius = Math.max(0, (stat.minRangeTiles || 0) * tileSize);
  return { kind: "circle", minRadius, maxRadius };
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

function dottedCircle(g, cx, cy, radius, dotSpacing, color, alpha) {
  if (!(radius > 0)) return;
  const circumference = Math.PI * 2 * radius;
  const count = Math.max(18, Math.ceil(circumference / Math.max(6, dotSpacing)));
  const dotArc = Math.min(0.035, 2.2 / Math.max(1, radius));
  const paths = [];
  for (let i = 0; i < count; i += 1) {
    const a = (i / count) * Math.PI * 2;
    const a0 = a - dotArc;
    const a1 = a + dotArc;
    paths.push([
      [cx + Math.cos(a0) * radius, cy + Math.sin(a0) * radius],
      [cx + Math.cos(a1) * radius, cy + Math.sin(a1) * radius],
    ]);
  }
  gfxStrokePaths(g, paths, 1, color, alpha);
}

function firstFinite(...values) {
  for (const value of values) {
    if (finiteNumber(value)) return value;
  }
  return null;
}

function firstObject(...values) {
  for (const value of values) {
    if (value && typeof value === "object") return value;
  }
  return null;
}

function tilesToPx(value, tileSize) {
  return finiteNumber(value) ? value * tileSize : null;
}
