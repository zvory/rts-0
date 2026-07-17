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

export function _drawSelectedMortarRanges(state) {
  return _drawSelectedUnitRanges.call(this, state);
}

export function _drawSelectedUnitRanges(state) {
  if (!state || typeof state.selectedEntities !== "function") return;
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
      g.lineStyle(1, UNIT_RANGE_COLOR, UNIT_RANGE_LINE_ALPHA);
      dottedCircle(g, e.x, e.y, profile.maxRadius);
      if (profile.minRadius > 0) {
        g.lineStyle(1, UNIT_RANGE_MIN_COLOR, UNIT_RANGE_MIN_LINE_ALPHA);
        dottedCircle(g, e.x, e.y, profile.minRadius, UNIT_RANGE_DOT_SPACING_PX * 0.8);
      }
    }
  }
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
    const facing = firstFinite(e.setupFacing, e.weaponFacing, e.facing);
    if (!finiteNumber(facing)) return null;
    const weapon = fieldOfFireProfile(e.kind, tileSize);
    return weapon
      ? {
        kind: "fieldOfFire",
        minRadius: weapon.minRadius,
        maxRadius: weapon.maxRadius,
        arc: weapon.arc,
        facing,
      }
      : null;
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

function dottedCircle(g, cx, cy, radius, dotSpacing = UNIT_RANGE_DOT_SPACING_PX) {
  if (!(radius > 0)) return;
  const circumference = Math.PI * 2 * radius;
  const count = Math.max(18, Math.ceil(circumference / Math.max(6, dotSpacing)));
  const dotArc = Math.min(0.035, 2.2 / Math.max(1, radius));
  for (let i = 0; i < count; i += 1) {
    const a = (i / count) * Math.PI * 2;
    const a0 = a - dotArc;
    const a1 = a + dotArc;
    g.moveTo(cx + Math.cos(a0) * radius, cy + Math.sin(a0) * radius);
    g.lineTo(cx + Math.cos(a1) * radius, cy + Math.sin(a1) * radius);
  }
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
