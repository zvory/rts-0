import { ARTILLERY_OUTER_RADIUS_TILES, STATS } from "./config.js";

export const GROUND_DECAL_CLASS = Object.freeze({
  NONE: "none",
  INFANTRY: "infantry",
  SCORCH: "scorch",
  BUILDING_SCORCH: "buildingScorch",
  MORTAR_BLAST: "mortarBlast",
  ARTILLERY_BLAST: "artilleryBlast",
  TANK_TREADS: "tankTreads",
});

const TWO_PI = Math.PI * 2;
const NEUTRAL_DECAL_COLOR = "#9aa0a8";
const DEFAULT_TILE_SIZE = 32;
const AUTHORITATIVE_DECAL_CLASSES = new Set(Object.values(GROUND_DECAL_CLASS));

export class GroundDecalBuffer {
  constructor() {
    this._pending = [];
    this._reconciled = null;
    this._reconciledRevision = 0;
    this._nextRevision = 1;
    this.authoritativeRevision = 0;
    this.authoritativeDecals = new Map();
  }

  applyAuthoritativeBatch({ revision, decals, tankTrails } = {}, context = {}) {
    if (!Number.isInteger(revision) || revision < 0 || revision > 0xffffffff) {
      return { accepted: false, queued: 0 };
    }
    if (revision < this.authoritativeRevision) return { accepted: false, queued: 0 };

    // Reliable responses can contain the entire durable history after a reconnect or perspective
    // reset. They repair the permanent surface, but are not evidence that an infantry death just
    // happened on this presentation timeline.
    const queued = this._queueMarks(decals, tankTrails, context, { animateInfantryDeath: false });
    this.authoritativeRevision = revision;
    return { accepted: true, queued };
  }

  applySnapshotDelta(
    { revision, afterRevision, decals, tankTrails } = {},
    context = {},
    { animateInfantryDeath = false } = {},
  ) {
    if (!Number.isInteger(revision) || revision < 0 || revision > 0xffffffff) {
      return { accepted: false, complete: false, queued: 0 };
    }
    if (!Number.isInteger(afterRevision) || afterRevision < 0 || afterRevision > revision) {
      return { accepted: false, complete: false, queued: 0 };
    }
    if (revision < this.authoritativeRevision) {
      return { accepted: false, complete: false, queued: 0 };
    }

    // Inline snapshot deltas are the live event path, so newly learned infantry deaths may play
    // their short-lived authored sprite.
    const queued = this._queueMarks(decals, tankTrails, context, { animateInfantryDeath });
    const complete = afterRevision <= this.authoritativeRevision;
    if (complete) this.authoritativeRevision = revision;
    return { accepted: true, complete, queued };
  }

  _queueMarks(records, tankTrails, context, { animateInfantryDeath = false } = {}) {
    let queued = 0;
    for (const record of Array.isArray(records) ? records : []) {
      const decal = normalizeAuthoritativeGroundDecal(record, context);
      const key = decal?.id;
      if (!decal || this.authoritativeDecals.has(key)) continue;
      this.authoritativeDecals.set(key, decal);
      const animate = typeof animateInfantryDeath === "function"
        ? animateInfantryDeath(decal)
        : !!animateInfantryDeath;
      this._pending.push({ ...decal, animateInfantryDeath: animate });
      queued += 1;
    }
    for (const record of Array.isArray(tankTrails) ? tankTrails : []) {
      const trail = normalizeAuthoritativeTankTrail(record);
      const key = trail && `tread:${trail.id}`;
      if (!trail || this.authoritativeDecals.has(key)) continue;
      this.authoritativeDecals.set(key, trail);
      this._pending.push(trail);
      queued += 1;
    }
    return queued;
  }

  consumePending() {
    const reconciled = this._reconciled || [];
    if (reconciled.length === 0 && this._pending.length === 0) return [];
    const out = reconciled.concat(this._pending);
    this._reconciled = null;
    this._reconciledRevision = 0;
    this._pending = [];
    return out;
  }

  reconcilePending() {
    if (this._reconciled === null || (this._reconciled.length === 0 && this._pending.length > 0)) {
      this._reconciled = this._pending;
      this._pending = [];
      this._reconciledRevision = this._reconciled.length > 0 ? this._nextRevision++ : 0;
    }
    return this._reconciled;
  }

  reconcileBatch() {
    const decals = this.reconcilePending();
    return Object.freeze({ revision: this._reconciledRevision, decals });
  }

  acknowledgeReconciled(revision) {
    if (!Number.isSafeInteger(revision) || revision <= 0 || revision !== this._reconciledRevision) return 0;
    const count = this._reconciled?.length || 0;
    this._reconciled = null;
    this._reconciledRevision = 0;
    return count;
  }

  peekPending() {
    return [...(this._reconciled || []), ...this._pending];
  }

  get pendingCount() {
    return this._pending.length + (this._reconciled?.length || 0);
  }

  clear() {
    this._pending = [];
    this._reconciled = null;
    this._reconciledRevision = 0;
    this._nextRevision = 1;
    this.authoritativeRevision = 0;
    this.authoritativeDecals.clear();
  }

  requeueAuthoritative() {
    this._pending = [...this.authoritativeDecals.values()].map((decal) => ({
      ...decal,
      animateInfantryDeath: false,
    }));
    this._reconciled = null;
    this._reconciledRevision = 0;
    return this._pending.length;
  }

}

export function normalizeAuthoritativeTankTrail(record) {
  if (!record || !Number.isSafeInteger(record.id) || record.id <= 0 ||
      !Array.isArray(record.poses) || record.poses.length < 2 || record.poses.length > 64) {
    return null;
  }
  const poses = [];
  for (const pose of record.poses) {
    if (!Array.isArray(pose) || pose.length !== 3 ||
        !Number.isInteger(pose[0]) || pose[0] < 0 || pose[0] > 0xffff ||
        !Number.isInteger(pose[1]) || pose[1] < 0 || pose[1] > 0xffff ||
        !Number.isInteger(pose[2]) || pose[2] < -0x8000 || pose[2] > 0x7fff) return null;
    poses.push([pose[0], pose[1], pose[2]]);
  }
  return { id: record.id, decalClass: GROUND_DECAL_CLASS.TANK_TREADS, poses };
}

export function normalizeAuthoritativeGroundDecal(record, {
  players = [],
  tileSize = DEFAULT_TILE_SIZE,
} = {}) {
  if (!record || !Number.isSafeInteger(record.id) || record.id < 0) return null;
  if (!Number.isFinite(record.x) || !Number.isFinite(record.y)) return null;
  const decalClass = AUTHORITATIVE_DECAL_CLASSES.has(record.decalClass)
    && record.decalClass !== GROUND_DECAL_CLASS.NONE
    ? record.decalClass
    : GROUND_DECAL_CLASS.NONE;
  if (decalClass === GROUND_DECAL_CLASS.NONE) return null;

  const kind = typeof record.sourceKind === "string" ? record.sourceKind : record.kind;
  if (typeof kind !== "string" || kind.length === 0) return null;
  const seed = Number.isInteger(record.seed) ? record.seed >>> 0 : record.id >>> 0;
  const owner = Number.isInteger(record.owner) && record.owner >= 0 ? record.owner : 0;
  const fallbackFacing = angleFromSeed(seed);
  const facing = normalizeAngle(Number.isFinite(record.facing) ? record.facing : fallbackFacing);
  const weaponFacing = normalizeAngle(
    Number.isFinite(record.weaponFacing) ? record.weaponFacing : facing,
  );
  const safeTileSize = Number.isFinite(tileSize) && tileSize > 0 ? tileSize : DEFAULT_TILE_SIZE;
  const radiusTiles = Number.isFinite(record.radiusTiles) && record.radiusTiles > 0
    ? record.radiusTiles
    : decalClass === GROUND_DECAL_CLASS.MORTAR_BLAST
      ? 1.5
      : decalClass === GROUND_DECAL_CLASS.ARTILLERY_BLAST
        ? ARTILLERY_OUTER_RADIUS_TILES
        : null;
  const footprint = decalClass === GROUND_DECAL_CLASS.BUILDING_SCORCH
    ? buildingFootprintPixels(kind, safeTileSize)
    : null;

  return {
    id: record.id,
    kind,
    decalClass,
    x: record.x,
    y: record.y,
    owner,
    color: playerColor(players, owner),
    facing,
    weaponFacing,
    seed,
    variant: seed % 4,
    ...(radiusTiles == null ? {} : {
      radiusTiles,
      radiusWorld: radiusTiles * safeTileSize,
    }),
    ...(footprint || {}),
  };
}

function playerColor(players, owner) {
  const player = Array.isArray(players) ? players.find((p) => p?.id === owner) : null;
  const color = player?.color;
  return /^#[0-9a-fA-F]{6}$/.test(color || "") ? color : NEUTRAL_DECAL_COLOR;
}

function buildingFootprintPixels(kind, tileSize) {
  const stat = STATS[kind] || {};
  const safeTileSize = Number.isFinite(tileSize) && tileSize > 0 ? tileSize : DEFAULT_TILE_SIZE;
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
