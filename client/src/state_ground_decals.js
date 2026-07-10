import { STATS } from "./config.js";
import { EVENT, KIND, isBuilding } from "./protocol.js";

export const GROUND_DECAL_CLASS = Object.freeze({
  NONE: "none",
  INFANTRY: "infantry",
  SCORCH: "scorch",
  BUILDING_SCORCH: "buildingScorch",
});

const INFANTRY_DECAL_KINDS = new Set([
  KIND.WORKER,
  KIND.RIFLEMAN,
  KIND.MACHINE_GUNNER,
  KIND.MORTAR_TEAM,
  KIND.EKAT,
]);

const SCORCH_DECAL_KINDS = new Set([
  KIND.SCOUT_CAR,
  KIND.TANK,
  KIND.COMMAND_CAR,
  KIND.ANTI_TANK_GUN,
  KIND.ARTILLERY,
]);

const TWO_PI = Math.PI * 2;
const NEUTRAL_DECAL_COLOR = "#9aa0a8";

export function groundDecalClassForKind(kind) {
  if (INFANTRY_DECAL_KINDS.has(kind)) return GROUND_DECAL_CLASS.INFANTRY;
  if (SCORCH_DECAL_KINDS.has(kind)) return GROUND_DECAL_CLASS.SCORCH;
  if (isBuilding(kind)) return GROUND_DECAL_CLASS.BUILDING_SCORCH;
  return GROUND_DECAL_CLASS.NONE;
}

export class GroundDecalBuffer {
  constructor() {
    this.paintedDeathIds = new Set();
    this._pending = [];
  }

  applySnapshotEvents(events, context = {}) {
    if (!Array.isArray(events) || events.length === 0) return 0;
    let queued = 0;
    for (const ev of events) {
      if (!ev || ev.e !== EVENT.DEATH || typeof ev.id !== "number") continue;
      if (this.paintedDeathIds.has(ev.id)) continue;
      const decal = normalizeGroundDecalEvent(ev, context);
      if (!decal) continue;
      this.paintedDeathIds.add(ev.id);
      this._pending.push(decal);
      queued += 1;
    }
    return queued;
  }

  consumePending() {
    if (this._pending.length === 0) return [];
    const out = this._pending;
    this._pending = [];
    return out;
  }

  get pendingCount() {
    return this._pending.length;
  }

  clear() {
    this.paintedDeathIds.clear();
    this._pending = [];
  }
}

export function normalizeGroundDecalEvent(ev, {
  prevById = null,
  curById = null,
  players = [],
  tick = 0,
  tileSize = 32,
} = {}) {
  if (!ev || ev.e !== EVENT.DEATH || typeof ev.id !== "number") return null;
  if (!Number.isFinite(ev.x) || !Number.isFinite(ev.y)) return null;
  const decalClass = groundDecalClassForKind(ev.kind);
  if (decalClass === GROUND_DECAL_CLASS.NONE) return null;

  const seed = groundDecalSeed(ev, tick);
  const source = lookupEntity(prevById, ev.id) || lookupEntity(curById, ev.id);
  const owner = Number.isFinite(source?.owner) ? source.owner : 0;
  const fallbackFacing = angleFromSeed(seed);
  const facing = normalizeAngle(
    Number.isFinite(source?.facing)
      ? source.facing
      : Number.isFinite(source?.weaponFacing)
        ? source.weaponFacing
        : fallbackFacing,
  );
  const weaponFacing = normalizeAngle(
    Number.isFinite(source?.weaponFacing) ? source.weaponFacing : facing,
  );
  const footprint = decalClass === GROUND_DECAL_CLASS.BUILDING_SCORCH
    ? buildingFootprintPixels(ev.kind, tileSize)
    : null;

  return {
    id: ev.id,
    kind: ev.kind,
    decalClass,
    x: ev.x,
    y: ev.y,
    owner,
    color: playerColor(players, owner),
    facing,
    weaponFacing,
    seed,
    variant: seed % 4,
    ...(footprint || {}),
  };
}

export function groundDecalSeed(ev, tick = 0) {
  const qx = Math.round((Number.isFinite(ev?.x) ? ev.x : 0) * 4);
  const qy = Math.round((Number.isFinite(ev?.y) ? ev.y : 0) * 4);
  let hash = 0x811c9dc5;
  hash = hashMix(hash, ev?.id ?? 0);
  hash = hashMix(hash, tick ?? 0);
  hash = hashMix(hash, qx);
  hash = hashMix(hash, qy);
  const kind = String(ev?.kind || "");
  for (let i = 0; i < kind.length; i += 1) hash = hashMix(hash, kind.charCodeAt(i));
  return hash >>> 0;
}

function lookupEntity(map, id) {
  return map && typeof map.get === "function" ? map.get(id) : null;
}

function playerColor(players, owner) {
  const player = Array.isArray(players) ? players.find((p) => p?.id === owner) : null;
  const color = player?.color;
  return /^#[0-9a-fA-F]{6}$/.test(color || "") ? color : NEUTRAL_DECAL_COLOR;
}

function buildingFootprintPixels(kind, tileSize) {
  const stat = STATS[kind] || {};
  const safeTileSize = Number.isFinite(tileSize) && tileSize > 0 ? tileSize : 32;
  const footW = Number.isFinite(stat.footW) && stat.footW > 0 ? stat.footW : 1;
  const footH = Number.isFinite(stat.footH) && stat.footH > 0 ? stat.footH : 1;
  return {
    footprintWidth: footW * safeTileSize,
    footprintHeight: footH * safeTileSize,
  };
}

function angleFromSeed(seed) {
  return ((seed >>> 0) / 0xffffffff) * TWO_PI - Math.PI;
}

function normalizeAngle(angle) {
  let out = (angle + Math.PI) % TWO_PI;
  if (out < 0) out += TWO_PI;
  return out - Math.PI;
}

function hashMix(hash, value) {
  let v = Number.isFinite(value) ? value | 0 : 0;
  hash ^= v & 0xff;
  hash = Math.imul(hash, 0x01000193);
  hash ^= (v >>> 8) & 0xff;
  hash = Math.imul(hash, 0x01000193);
  hash ^= (v >>> 16) & 0xff;
  hash = Math.imul(hash, 0x01000193);
  hash ^= (v >>> 24) & 0xff;
  return Math.imul(hash, 0x01000193);
}
