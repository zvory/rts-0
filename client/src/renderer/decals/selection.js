import { GROUND_DECAL_ASSET_MANIFEST } from "./manifest.js";

const DECAL_CLASS_INFANTRY = "infantry";
const DECAL_CLASS_SCORCH = "scorch";
const DECAL_CLASS_MORTAR_BLAST = "mortarBlast";
const DECAL_CLASS_ARTILLERY_BLAST = "artilleryBlast";
const NEUTRAL_COLOR = "#9aa0a8";
const TWO_PI = Math.PI * 2;
const MORTAR_BLAST_RADIUS_WORLD = 48;
// The unchanged artillery mask was authored for the former three-tile (96 px)
// footprint. Keep that calibration independent of the current gameplay radius
// so radiusWorld scales the asset instead of redefining its native size.
const ARTILLERY_BLAST_AUTHORED_RADIUS_WORLD = 96;

export const GROUND_DECAL_ASSET_COUNTS = Object.freeze({
  infantry: GROUND_DECAL_ASSET_MANIFEST.infantry.length,
  vehicleScorch: GROUND_DECAL_ASSET_MANIFEST.vehicleScorch.length,
  vehiclePaint: GROUND_DECAL_ASSET_MANIFEST.vehiclePaint.length,
  mortarBlast: GROUND_DECAL_ASSET_MANIFEST.mortarBlast.length,
  artilleryBlast: GROUND_DECAL_ASSET_MANIFEST.artilleryBlast.length,
});

export function createGroundDecalStampPlan(decal, {
  assetCounts = GROUND_DECAL_ASSET_COUNTS,
} = {}) {
  if (!decal || !isSupportedDecalClass(decal.decalClass)) {
    return null;
  }
  const seed = decal.seed || decal.id || 1;
  const rng = mulberry32(seed);
  const color = normalizeColorNumber(decal.color);
  if (decal.decalClass === DECAL_CLASS_INFANTRY) {
    return {
      decalClass: DECAL_CLASS_INFANTRY,
      color,
      variantIndex: pickIndex(seed, assetCounts.infantry),
      rotation: seededAngle(rng),
      scale: 0.86 + rng() * 0.28,
      flipX: rng() < 0.5 ? -1 : 1,
      flipY: rng() < 0.18 ? -1 : 1,
      opacity: 0.54 + rng() * 0.16,
      shadowOpacity: 0.14 + rng() * 0.06,
      offsetWorldX: (rng() - 0.5) * 5,
      offsetWorldY: (rng() - 0.5) * 5,
    };
  }

  if (decal.decalClass === DECAL_CLASS_MORTAR_BLAST || decal.decalClass === DECAL_CLASS_ARTILLERY_BLAST) {
    const artillery = decal.decalClass === DECAL_CLASS_ARTILLERY_BLAST;
    const baseRadius = artillery ? ARTILLERY_BLAST_AUTHORED_RADIUS_WORLD : MORTAR_BLAST_RADIUS_WORLD;
    const radiusWorld = Number.isFinite(decal.radiusWorld) && decal.radiusWorld > 0
      ? decal.radiusWorld
      : baseRadius;
    return {
      decalClass: decal.decalClass,
      variantIndex: pickIndex(seed, artillery ? assetCounts.artilleryBlast : assetCounts.mortarBlast),
      rotation: seededAngle(rng),
      scale: clamp(radiusWorld / baseRadius, 0.64, 1.55) * (0.95 + rng() * 0.1),
      flipX: rng() < 0.5 ? -1 : 1,
      flipY: rng() < 0.5 ? -1 : 1,
      soilOpacity: artillery ? 0.48 + rng() * 0.08 : 0.44 + rng() * 0.08,
      charOpacity: artillery ? 0.48 + rng() * 0.08 : 0.44 + rng() * 0.08,
      charScale: artillery ? 0.87 + rng() * 0.04 : 0.78 + rng() * 0.04,
      offsetWorldX: (rng() - 0.5) * (artillery ? 5 : 3),
      offsetWorldY: (rng() - 0.5) * (artillery ? 5 : 3),
    };
  }

  const facing = Number.isFinite(decal.facing) ? decal.facing : seededAngle(rng);
  return {
    decalClass: DECAL_CLASS_SCORCH,
    color,
    variantIndex: pickIndex(seed, assetCounts.vehicleScorch),
    paintVariantIndex: pickIndex(seed >>> 7, assetCounts.vehiclePaint),
    rotation: normalizeAngle(facing + (rng() - 0.5) * 0.24),
    scale: 0.94 + rng() * 0.14,
    flipX: 1,
    flipY: rng() < 0.5 ? -1 : 1,
    scorchOpacity: 0.48 + rng() * 0.12,
    ashOpacity: 0.06 + rng() * 0.05,
    paintOpacity: 0.13 + rng() * 0.07,
    offsetWorldX: (rng() - 0.5) * 3,
    offsetWorldY: (rng() - 0.5) * 3,
  };
}

function isSupportedDecalClass(decalClass) {
  return decalClass === DECAL_CLASS_INFANTRY ||
    decalClass === DECAL_CLASS_SCORCH ||
    decalClass === DECAL_CLASS_MORTAR_BLAST ||
    decalClass === DECAL_CLASS_ARTILLERY_BLAST;
}

export function normalizeColorNumber(color) {
  if (typeof color === "number" && Number.isFinite(color)) return color >>> 0;
  const match = /^#?([0-9a-fA-F]{6})$/.exec(String(color || NEUTRAL_COLOR));
  return match ? Number.parseInt(match[1], 16) : Number.parseInt(NEUTRAL_COLOR.slice(1), 16);
}

export function rgba(color, alpha) {
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return `rgba(${r},${g},${b},${alpha})`;
}

export function mulberry32(seed) {
  let value = seed >>> 0;
  return () => {
    value = (value + 0x6d2b79f5) >>> 0;
    let t = value;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function seededAngle(rng) {
  return (rng() * 2 - 1) * Math.PI;
}

function pickIndex(seed, count) {
  const safeCount = Math.max(0, count | 0);
  if (safeCount <= 0) return -1;
  return (seed >>> 0) % safeCount;
}

function normalizeAngle(angle) {
  let out = (angle + Math.PI) % TWO_PI;
  if (out < 0) out += TWO_PI;
  return out - Math.PI;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
