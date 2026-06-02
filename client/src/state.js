// GameState — the single client-side model the renderer, HUD, minimap and
// input all read from. See DESIGN.md §4.1.
//
// It holds the two most recent server snapshots (for interpolation), the
// latest resources/events, the local selection set, and the local build
// placement preview. Selection and placement are client-only concepts; the
// server never sees them directly (only the resulting commands).

import { PASSABLE } from "./protocol.js";

export class GameState {
  /**
   * @param {object} startInfo the §2.3 `start` payload.
   */
  constructor(startInfo) {
    /** @type {number} our player id (repeat of welcome, for convenience). */
    this.playerId = startInfo.playerId;
    /** @type {object} the full §2.3 start payload, kept for reference. */
    this.startInfo = startInfo;
    /** @type {{width:number,height:number,tileSize:number,terrain:number[]}} */
    this.map = startInfo.map;
    /** @type {Array<{id:number,name:string,color:string,startTileX:number,startTileY:number}>} */
    this.players = startInfo.players || [];

    // --- snapshot buffering for interpolation ---
    /** @type {object|null} previous snapshot (older of the two we keep). */
    this._prev = null;
    /** @type {object|null} current snapshot (most recent received). */
    this._cur = null;
    /** performance.now() stamp when `_prev` arrived. */
    this._prevRecvTime = 0;
    /** performance.now() stamp when `_cur` arrived. */
    this._curRecvTime = 0;
    /** @type {Map<number, object>} id -> entity for the current snapshot. */
    this._curById = new Map();
    /** @type {Map<number, object>} id -> entity for the previous snapshot. */
    this._prevById = new Map();

    // --- derived latest state ---
    /** @type {{steel:number,oil:number,supplyUsed:number,supplyCap:number}} */
    this.resources = { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
    /** @type {Array<object>} latest snapshot's transient events. */
    this.events = [];

    // --- selection (client-only) ---
    /** @type {Set<number>} */
    this.selection = new Set();

    // --- build placement preview (client-only) ---
    /** @type {null | {building:string, tileX:number, tileY:number, valid:boolean}} */
    this.placement = null;

    // --- command targeting / feedback (client-only) ---
    /** @type {null | "move" | "attack"} */
    this.commandTarget = null;
    /** @type {Array<{kind:string,x:number,y:number,createdAt:number}>} */
    this.commandFeedback = [];

  /** @type {Array<{from:number,to:number,createdAt:number}>} */
  this.muzzleFlashes = [];
  }

  /** Maximum number of entities the local selection may contain. */
  static get MAX_SELECTION_SIZE() {
    return 12;
  }

  /** World pixels per tile. */
  get tileSize() {
    return this.map.tileSize;
  }

  /**
   * Receive time of the previous snapshot, or null when no prior snapshot
   * exists yet. main.js computeAlpha() reads this to interpolate.
   * @returns {number|null}
   */
  get prevRecvTime() {
    return this._prev ? this._prevRecvTime : null;
  }

  /**
   * Receive time of the current snapshot, or null when none received yet.
   * @returns {number|null}
   */
  get currRecvTime() {
    return this._cur ? this._curRecvTime : null;
  }

  // --- snapshots ----------------------------------------------------------

  /**
   * Ingest a new snapshot. The current snapshot becomes the previous one and
   * the new message becomes current, each stamped with the receive time so
   * entitiesInterpolated() can position entities between them. Resources,
   * events and the id->entity index are refreshed from the latest snapshot.
   * @param {object} msg a §2.4 snapshot payload.
   */
  applySnapshot(msg) {
    const now = performance.now();

    this._prev = this._cur;
    this._prevRecvTime = this._curRecvTime;
    this._prevById = this._curById;

    this._cur = msg;
    this._curRecvTime = now;
    this._curById = new Map();
    const entities = msg.entities || [];
    for (const e of entities) this._curById.set(e.id, e);

    this.resources = {
      steel: msg.steel | 0,
      oil: msg.oil | 0,
      supplyUsed: msg.supplyUsed | 0,
      supplyCap: msg.supplyCap | 0,
    };
    this.events = msg.events || [];
    this._pruneSelection();

    for (const ev of this.events) {
      if (ev && ev.e === "attack" && typeof ev.from === "number" && typeof ev.to === "number") {
        this.muzzleFlashes.push({ from: ev.from, to: ev.to, createdAt: now });
      }
    }
    if (this.muzzleFlashes.length > 256) {
      this.muzzleFlashes.splice(0, this.muzzleFlashes.length - 256);
    }
  }

  /**
   * Return live muzzle-flash records, pruning expired ones.
   * @param {number} now
   * @returns {Array<{from:number,to:number,createdAt:number}>}
   */
  liveMuzzleFlashes(now) {
    const ttlMs = 240;
    this.muzzleFlashes = this.muzzleFlashes.filter((f) => now - f.createdAt <= ttlMs);
    return this.muzzleFlashes;
  }

  /**
   * Entities of the current snapshot with x,y linearly interpolated toward
   * their current positions from where they were in the previous snapshot.
   * All other fields are carried through unchanged. Entities with no prior
   * sample (newly visible) use their current position.
   * @param {number} alpha blend factor in [0,1]; 0 = previous, 1 = current.
   * @returns {Array<object>}
   */
  entitiesInterpolated(alpha) {
    if (!this._cur) return [];
    const t = alpha < 0 ? 0 : alpha > 1 ? 1 : alpha;
    const out = [];
    for (const e of this._cur.entities || []) {
      const prior = this._prevById.get(e.id);
      if (prior) {
        out.push({
          ...e,
          x: prior.x + (e.x - prior.x) * t,
          y: prior.y + (e.y - prior.y) * t,
        });
      } else {
        // No previous sample: render at the current position (a shallow copy
        // keeps callers from mutating the live snapshot entity).
        out.push({ ...e });
      }
    }
    return out;
  }

  /**
   * Look up an entity by id in the current snapshot.
   * @param {number} id
   * @returns {object|undefined}
   */
  entityById(id) {
    return this._curById.get(id);
  }

  // --- selection (client-only) -------------------------------------------

  /**
   * Resolve the current selection to live entities, dropping ids that no
   * longer exist in the current snapshot (e.g. units that died).
   * @returns {Array<object>}
   */
  selectedEntities() {
    this._pruneSelection();
    const out = [];
    for (const id of this.selection) {
      const e = this._curById.get(id);
      if (e) out.push(e);
    }
    return out;
  }

  /**
   * Replace the selection with the given ids.
   * @param {Iterable<number>} ids
   */
  setSelection(ids) {
    this.selection = new Set();
    for (const id of ids) {
      this.selection.add(id);
      if (this.selection.size >= GameState.MAX_SELECTION_SIZE) break;
    }
  }

  /**
   * Add ids to the existing selection.
   * @param {Iterable<number>} ids
   */
  addToSelection(ids) {
    this._pruneSelection();
    for (const id of ids) {
      if (this.selection.size >= GameState.MAX_SELECTION_SIZE) break;
      this.selection.add(id);
    }
  }

  /** Clear the selection. */
  clearSelection() {
    this.selection.clear();
  }

  /** Drop selected ids that are no longer present in the latest snapshot. */
  _pruneSelection() {
    if (!this.selection || this.selection.size === 0) return;
    let changed = false;
    const live = new Set();
    for (const id of this.selection) {
      if (this._curById.has(id)) {
        live.add(id);
      } else {
        changed = true;
      }
    }
    if (changed) this.selection = live;
  }

  // --- build placement (client-only) -------------------------------------

  /**
   * Start previewing placement of a building. Position/validity are filled in
   * by updatePlacement as the cursor moves.
   * @param {string} buildingKind a building EntityKind.
   */
  beginPlacement(buildingKind) {
    this.commandTarget = null;
    this.placement = { building: buildingKind, tileX: 0, tileY: 0, valid: false };
  }

  /**
   * Update the placement preview's tile and validity. No-op if no placement
   * is in progress.
   * @param {number} tileX
   * @param {number} tileY
   * @param {boolean} valid
   */
  updatePlacement(tileX, tileY, valid) {
    if (!this.placement) return;
    this.placement.tileX = tileX;
    this.placement.tileY = tileY;
    this.placement.valid = !!valid;
  }

  /** Stop previewing placement. */
  endPlacement() {
    this.placement = null;
  }

  // --- command targeting / feedback (client-only) ------------------------

  /**
   * Arm a one-click command target mode from the HUD.
   * @param {"move"|"attack"} kind
   */
  beginCommandTarget(kind) {
    this.placement = null;
    this.commandTarget = kind;
  }

  /** Clear any armed command target mode. */
  endCommandTarget() {
    this.commandTarget = null;
  }

  /**
   * Add a short-lived local command marker at a world point.
   * @param {"move"|"attack"} kind
   * @param {number} x
   * @param {number} y
   */
  addCommandFeedback(kind, x, y) {
    this.commandFeedback.push({ kind, x, y, createdAt: performance.now() });
    if (this.commandFeedback.length > 12) {
      this.commandFeedback.splice(0, this.commandFeedback.length - 12);
    }
  }

  /**
   * Return live command feedback markers, pruning expired ones.
   * @param {number} now
   * @returns {Array<{kind:string,x:number,y:number,createdAt:number}>}
   */
  liveCommandFeedback(now) {
    const ttlMs = 650;
    this.commandFeedback = this.commandFeedback.filter((f) => now - f.createdAt <= ttlMs);
    return this.commandFeedback;
  }

  // --- map helpers --------------------------------------------------------

  /**
   * Whether a world point lies inside the map bounds.
   * @param {number} wx world x in pixels
   * @param {number} wy world y in pixels
   * @returns {boolean}
   */
  worldInBounds(wx, wy) {
    return (
      wx >= 0 &&
      wy >= 0 &&
      wx < this.map.width * this.map.tileSize &&
      wy < this.map.height * this.map.tileSize
    );
  }

  /**
   * Terrain code at a tile, or null if out of bounds.
   * @param {number} tileX
   * @param {number} tileY
   * @returns {number|null} a TERRAIN code, or null.
   */
  terrainAt(tileX, tileY) {
    if (tileX < 0 || tileY < 0 || tileX >= this.map.width || tileY >= this.map.height) {
      return null;
    }
    return this.map.terrain[tileY * this.map.width + tileX];
  }

  /**
   * Whether a tile is passable per the PASSABLE table (useful to input.js for
   * placement validity). Out-of-bounds tiles are impassable.
   * @param {number} tileX
   * @param {number} tileY
   * @returns {boolean}
   */
  isPassable(tileX, tileY) {
    const terrain = this.terrainAt(tileX, tileY);
    if (terrain == null) return false;
    return !!PASSABLE[terrain];
  }
}
