// GameState — the single client-side model the renderer, HUD, minimap and
// input all read from. See docs/design/client-ui.md §4.1.
//
// It holds the two most recent server snapshots (for interpolation), the
// latest resources/events, the local selection set, and the local build
// placement preview. Selection, command-card menus, and placement are client-only concepts; the
// server never sees them directly (only the resulting commands).

import { RESOURCE_AMOUNTS } from "./config.js";
import { CommandComposer } from "./command_composer.js";
import { KIND, PASSABLE, STATE, isBuilding, isResource, isUnit } from "./protocol.js";

const TWO_PI = Math.PI * 2;
const SHOT_REVEAL_MS = 1500;
const WEAPON_RECOIL_MS = Object.freeze({
  [KIND.RIFLEMAN]: 420,
  [KIND.MACHINE_GUNNER]: 160,
  [KIND.AT_TEAM]: 820,
  [KIND.MORTAR_TEAM]: 520,
  [KIND.ARTILLERY]: 980,
  [KIND.SCOUT_CAR]: 160,
  [KIND.TANK]: 650,
});

function normalizeAngle(a) {
  let out = (a + Math.PI) % TWO_PI;
  if (out < 0) out += TWO_PI;
  return out - Math.PI;
}

function shortestAngleDelta(from, to) {
  return normalizeAngle(to - from);
}

function lerpAngle(from, to, t) {
  return normalizeAngle(from + shortestAngleDelta(from, to) * t);
}

export class GameState {
  /**
   * @param {object} startInfo the §2.3 `start` payload.
   */
  constructor(startInfo) {
    /** @type {number} our player id (repeat of welcome, for convenience). */
    this.playerId = startInfo.playerId;
    /** @type {boolean} true when this client is observing instead of playing. */
    this.spectator = !!startInfo.spectator;
    /** @type {object} the full §2.3 start payload, kept for reference. */
    this.startInfo = startInfo;
    /** @type {{width:number,height:number,tileSize:number,terrain:number[]}} */
    this.map = startInfo.map;
    this.map.resources = (this.map.resources || []).map((node, index) =>
      this._normalizeResource(node, index),
    );
    /** @type {Map<number, object>} id -> static resource node with last-known remaining. */
    this.resourceById = new Map();
    for (const node of this.map.resources) this.resourceById.set(node.id, node);
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
    /** @type {Array<{id:number,steel:number,oil:number,supplyUsed:number,supplyCap:number}>} */
    this.playerResources = [];
    /** @type {Array<object>} latest snapshot's transient events. */
    this.events = [];
    /** @type {string[]} upgrades completed for this player. */
    this.upgrades = [];

    // --- selection (client-only) ---
    /** @type {Set<number>} */
    this.selection = new Set();
    /** @type {boolean} true when the server says movement path diagnostics are available. */
    this.debugPathOverlaysAvailable = !!startInfo.debugMode;
    /** @type {boolean} local gear-menu preference for drawing movement path diagnostics. */
    this.debugPathOverlaysEnabled = !!startInfo.debugMode;
    /** @type {boolean} true for dev viewers that should show all server debug paths. */
    this.showAllDebugPathOverlays = false;
    /** @type {Array<Array<number>>} ten local control groups, slot 9 is key 0. */
    this.controlGroups = Array.from({ length: 10 }, () => []);

    // --- build placement preview (client-only) ---
    /** @type {null | {building:string, tileX:number, tileY:number, valid:boolean}} */
    this.placement = null;

    // --- command card submenu (client-only) ---
    /** @type {null | "workerBuild"} */
    this.commandCardMode = null;

    // --- command targeting / feedback (client-only) ---
    /** @type {null | "move" | "attack" | "setupAtGuns"} */
    this.commandTarget = null;
    this.commandComposer = new CommandComposer();
    /** @type {null | {quickCast:boolean,target:string|object,queued:boolean}} */
    this.lastCommandTargetArm = null;
    /** @type {Array<{kind:string,x:number,y:number,append:boolean,createdAt:number}>} */
    this.commandFeedback = [];
    /** @type {null | {resourceId:number, resourceX:number, resourceY:number, ccId:number, ccX:number, ccY:number, inRange:boolean}} */
    this.resourceMiningPreview = null;
    /** @type {null | {mouseX:number, mouseY:number, guns:Array<object>}} */
    this.atGunSetupPreview = null;
    /** @type {null | {ability:string, mouseX:number, mouseY:number, carriers:Array<object>, rangePx:number, hoverInRange:boolean, hoverInsideMinRange?:boolean}} */
    this.abilityTargetPreview = null;
    /** @type {Array<{id:number,x:number,y:number,radiusTiles:number,expiresIn:number}>} */
    this.smokes = [];
    /** @type {Array<{id:number,owner:number,kind:string,x:number,y:number,footprint:Array<[number,number]>,observedTick:number}>} */
    this.rememberedBuildings = [];
    /** @type {Array<{fromX:number,fromY:number,toX:number,toY:number,durationMs:number,createdAt:number}>} */
    this.smokeCanisters = [];
    /** @type {number[]|Uint8Array} row-major current server-authoritative visibility. */
    this.visibleTiles = [];

    /** @type {Array<{from:number,to:number,createdAt:number}>} */
    this.muzzleFlashes = [];
    /** @type {Array<{x:number,y:number,createdAt:number}>} */
    this.mortarLaunches = [];
    /** @type {Array<{fromX:number,fromY:number,toX:number,toY:number,radiusTiles:number,durationMs:number,seed:number,createdAt:number}>} */
    this.mortarShells = [];
    /** @type {Array<{fromX:number,fromY:number,x:number,y:number,radiusTiles:number,durationMs:number,seed:number,createdAt:number}>} */
    this.mortarTargets = [];
    /** @type {Array<{x:number,y:number,radiusTiles:number,seed:number,createdAt:number}>} */
    this.mortarImpacts = [];
    /** @type {Array<{x:number,y:number,radiusTiles:number,delayTicks:number,seed:number,createdAt:number}>} */
    this.artilleryTargets = [];
    /** @type {Array<{x:number,y:number,facing:number,seed:number,createdAt:number}>} */
    this.artilleryLaunches = [];
    /** @type {Array<{x:number,y:number,radiusTiles:number,seed:number,createdAt:number}>} */
    this.artilleryImpacts = [];
    /** @type {Map<number, number>} attacker id -> latest shot receive time. */
    this.weaponRecoilById = new Map();
    /** @type {Array<{x:number,y:number,createdAt:number}>} */
    this.pendingMortarTargets = [];
    /** @type {Map<number, object>} attacker id -> temporary fog reveal entity. */
    this.shotRevealsById = new Map();
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
    // Snapshots can arrive batched in a single event-loop turn (a throttled or
    // backgrounded tab drains its socket buffer at once) and performance.now()
    // is clamped to a coarse resolution, so two consecutive snapshots could
    // otherwise share a receive time. Force the receive clock strictly forward
    // so the interpolation window (curr - prev) is always positive and never
    // collapses to a degenerate, alpha-pinned-to-1 span. Real time reasserts
    // itself via Math.max as soon as performance.now() passes the floor.
    const now = Math.max(performance.now(), this._curRecvTime + 1);

    this._prev = this._cur;
    this._prevRecvTime = this._curRecvTime;
    this._prevById = this._curById;

    const events = msg.events || [];
    this._applyResourceDeltas(msg.resourceDeltas || []);
    this._applyResourceDeaths(events);
    const wireEntities = (msg.entities || []).filter((e) => !isResource(e.kind));
    this._applyAttackReveals(events, now);
    const visibleIds = new Set(wireEntities.map((e) => e.id));
    const entities = wireEntities
      .concat(this._resourceEntityViews())
      .concat(this._shotRevealEntityViews(now, visibleIds));

    this._cur = { ...msg, entities };
    this._curRecvTime = now;
    this._curById = new Map();
    for (const e of entities) this._curById.set(e.id, e);
    for (const id of this.weaponRecoilById.keys()) {
      if (!this._curById.has(id)) this.weaponRecoilById.delete(id);
    }

    this.resources = {
      steel: msg.steel | 0,
      oil: msg.oil | 0,
      supplyUsed: msg.supplyUsed | 0,
      supplyCap: msg.supplyCap | 0,
    };
    this.playerResources = msg.playerResources || [];
    this.upgrades = Array.isArray(msg.upgrades) ? msg.upgrades : [];
    this.smokes = Array.isArray(msg.smokes) ? msg.smokes : [];
    this.rememberedBuildings = Array.isArray(msg.rememberedBuildings)
      ? msg.rememberedBuildings
      : [];
    this.visibleTiles = Array.isArray(msg.visibleTiles) || msg.visibleTiles instanceof Uint8Array
      ? msg.visibleTiles
      : [];
    this.events = events;
    this._pruneSelection();
    this._pruneControlGroups();

    for (const ev of this.events) {
      if (ev && ev.e === "attack" && typeof ev.from === "number" && typeof ev.to === "number") {
        const targetPos = Array.isArray(ev.toPos) && ev.toPos.length === 2
          ? { x: ev.toPos[0], y: ev.toPos[1] }
          : null;
        this.muzzleFlashes.push({ from: ev.from, to: ev.to, targetPos, createdAt: now });
        this.weaponRecoilById.set(ev.from, now);
      } else if (ev && ev.e === "smokeLaunch") {
        this.addSmokeCanister(ev, now);
      } else if (ev && ev.e === "mortarLaunch") {
        this.addMortarLaunch(ev, now);
      } else if (ev && ev.e === "mortarImpact") {
        this.addMortarImpact(ev, now);
      } else if (ev && ev.e === "artilleryTarget") {
        this.addArtilleryTarget(ev, now);
      } else if (ev && ev.e === "artilleryImpact") {
        this.addArtilleryImpact(ev, now);
      }
    }
    if (this.muzzleFlashes.length > 256) {
      this.muzzleFlashes.splice(0, this.muzzleFlashes.length - 256);
    }
    if (this.smokeCanisters.length > 64) {
      this.smokeCanisters.splice(0, this.smokeCanisters.length - 64);
    }
    if (this.mortarLaunches.length > 32) {
      this.mortarLaunches.splice(0, this.mortarLaunches.length - 32);
    }
    if (this.mortarShells.length > 32) {
      this.mortarShells.splice(0, this.mortarShells.length - 32);
    }
    if (this.mortarTargets.length > 32) {
      this.mortarTargets.splice(0, this.mortarTargets.length - 32);
    }
    if (this.mortarImpacts.length > 32) {
      this.mortarImpacts.splice(0, this.mortarImpacts.length - 32);
    }
    if (this.artilleryTargets.length > 48) {
      this.artilleryTargets.splice(0, this.artilleryTargets.length - 48);
    }
    if (this.artilleryLaunches.length > 32) {
      this.artilleryLaunches.splice(0, this.artilleryLaunches.length - 32);
    }
    if (this.artilleryImpacts.length > 32) {
      this.artilleryImpacts.splice(0, this.artilleryImpacts.length - 32);
    }
  }

  addMortarLaunch(ev, now = performance.now()) {
    if (
      !Number.isFinite(ev.fromX) ||
      !Number.isFinite(ev.fromY) ||
      !Number.isFinite(ev.toX) ||
      !Number.isFinite(ev.toY)
    ) {
      return;
    }
    const delayTicks = Number.isFinite(ev.delayTicks) ? Math.max(0, ev.delayTicks) : 0;
    const durationMs = Math.max(1, (delayTicks / 30) * 1000);
    const radiusTiles = Number.isFinite(ev.radiusTiles) ? ev.radiusTiles : 1.5;
    const seed = Math.floor(ev.toX * 13 + ev.toY * 7 + now) >>> 0;
    if (typeof ev.from === "number") {
      this.weaponRecoilById.set(ev.from, now);
    }
    this.pendingMortarTargets = this.pendingMortarTargets.filter(
      (p) => Math.hypot(p.x - ev.toX, p.y - ev.toY) > 2,
    );
    this.mortarLaunches.push({ x: ev.fromX, y: ev.fromY, createdAt: now });
    this.mortarShells.push({
      fromX: ev.fromX,
      fromY: ev.fromY,
      toX: ev.toX,
      toY: ev.toY,
      radiusTiles,
      durationMs,
      seed,
      createdAt: now,
    });
    this.mortarTargets.push({
      fromX: ev.fromX,
      fromY: ev.fromY,
      x: ev.toX,
      y: ev.toY,
      radiusTiles,
      durationMs,
      seed,
      createdAt: now,
    });
  }

  addMortarImpact(ev, now = performance.now()) {
    if (!Number.isFinite(ev.x) || !Number.isFinite(ev.y)) return;
    this.mortarTargets = this.mortarTargets.filter(
      (target) => Math.hypot(target.x - ev.x, target.y - ev.y) > 2,
    );
    this.mortarShells = this.mortarShells.filter(
      (shell) => Math.hypot(shell.toX - ev.x, shell.toY - ev.y) > 2,
    );
    this.mortarImpacts.push({
      x: ev.x,
      y: ev.y,
      radiusTiles: Number.isFinite(ev.radiusTiles) ? ev.radiusTiles : 1.5,
      seed: Math.floor(ev.x * 13 + ev.y * 7 + now) >>> 0,
      createdAt: now,
    });
  }

  addArtilleryTarget(ev, now = performance.now()) {
    if (!Number.isFinite(ev.x) || !Number.isFinite(ev.y)) return;
    if (typeof ev.from === "number") {
      this.weaponRecoilById.set(ev.from, now);
      const shooter = this.entityById(ev.from);
      if (shooter && Number.isFinite(shooter.x) && Number.isFinite(shooter.y)) {
        const facing = Number.isFinite(shooter.weaponFacing)
          ? shooter.weaponFacing
          : Number.isFinite(shooter.facing)
            ? shooter.facing
            : 0;
        this.artilleryLaunches.push({
          x: shooter.x,
          y: shooter.y,
          facing,
          seed: Math.floor(shooter.x * 23 + shooter.y * 29 + now) >>> 0,
          createdAt: now,
        });
      }
    }
    this.artilleryTargets.push({
      x: ev.x,
      y: ev.y,
      radiusTiles: Number.isFinite(ev.radiusTiles) ? ev.radiusTiles : 3,
      delayTicks: Number.isFinite(ev.delayTicks) ? Math.max(0, ev.delayTicks) : 0,
      seed: Math.floor(ev.x * 17 + ev.y * 11 + now) >>> 0,
      createdAt: now,
    });
  }

  addArtilleryImpact(ev, now = performance.now()) {
    if (!Number.isFinite(ev.x) || !Number.isFinite(ev.y)) return;
    this.artilleryImpacts.push({
      x: ev.x,
      y: ev.y,
      radiusTiles: Number.isFinite(ev.radiusTiles) ? ev.radiusTiles : 3,
      seed: Math.floor(ev.x * 19 + ev.y * 23 + now) >>> 0,
      createdAt: now,
    });
  }

  addSmokeCanister(ev, now = performance.now()) {
    if (
      !Number.isFinite(ev.fromX) ||
      !Number.isFinite(ev.fromY) ||
      !Number.isFinite(ev.toX) ||
      !Number.isFinite(ev.toY)
    ) {
      return;
    }
    const delayTicks = Number.isFinite(ev.delayTicks) ? Math.max(0, ev.delayTicks) : 0;
    const durationMs = (delayTicks / 30) * 1000;
    if (durationMs <= 0) return;
    this.smokeCanisters.push({
      fromX: ev.fromX,
      fromY: ev.fromY,
      toX: ev.toX,
      toY: ev.toY,
      durationMs,
      createdAt: now,
    });
  }

  /**
   * Return live smoke canister launch visuals, pruning landed ones.
   * @param {number} now
   * @returns {Array<{fromX:number,fromY:number,toX:number,toY:number,durationMs:number,createdAt:number}>}
   */
  liveSmokeCanisters(now) {
    this.smokeCanisters = this.smokeCanisters.filter((f) => now - f.createdAt <= f.durationMs);
    return this.smokeCanisters;
  }

  /**
   * Return live mortar launch dust puffs, pruning expired ones.
   * @param {number} now
   * @returns {Array<{x:number,y:number,createdAt:number}>}
   */
  liveMortarLaunches(now) {
    const ttlMs = 360;
    this.mortarLaunches = this.mortarLaunches.filter((f) => now - f.createdAt <= ttlMs);
    return this.mortarLaunches;
  }

  /**
   * Return live mortar shell projectiles, pruning after expected impact.
   * @param {number} now
   * @returns {Array<{fromX:number,fromY:number,toX:number,toY:number,radiusTiles:number,durationMs:number,seed:number,createdAt:number}>}
   */
  liveMortarShells(now) {
    this.mortarShells = this.mortarShells.filter((f) => now - f.createdAt <= f.durationMs + 120);
    return this.mortarShells;
  }

  /**
   * Return live mortar target warnings, pruning after expected impact.
   * @param {number} now
   * @returns {Array<{fromX:number,fromY:number,x:number,y:number,radiusTiles:number,durationMs:number,seed:number,createdAt:number}>}
   */
  liveMortarTargets(now) {
    this.mortarTargets = this.mortarTargets.filter((f) => now - f.createdAt <= f.durationMs + 120);
    return this.mortarTargets;
  }

  /**
   * Return live mortar impact explosions, pruning expired ones.
   * @param {number} now
   * @returns {Array<{x:number,y:number,radiusTiles:number,seed:number,createdAt:number}>}
   */
  liveMortarImpacts(now) {
    const ttlMs = 1000;
    this.mortarImpacts = this.mortarImpacts.filter((f) => now - f.createdAt <= ttlMs);
    return this.mortarImpacts;
  }

  /**
   * Return live owner-only artillery target markers, pruning after the shell lands.
   * @param {number} now
   * @returns {Array<{x:number,y:number,radiusTiles:number,delayTicks:number,seed:number,createdAt:number}>}
   */
  liveArtilleryTargets(now) {
    this.artilleryTargets = this.artilleryTargets.filter((f) => {
      const ttlMs = Math.max(900, ((f.delayTicks || 0) / 30) * 1000 + 350);
      return now - f.createdAt <= ttlMs;
    });
    return this.artilleryTargets;
  }

  /**
   * Return live owner-only artillery launch dust puffs, pruning expired ones.
   * @param {number} now
   * @returns {Array<{x:number,y:number,facing:number,seed:number,createdAt:number}>}
   */
  liveArtilleryLaunches(now) {
    const ttlMs = 820;
    this.artilleryLaunches = this.artilleryLaunches.filter((f) => now - f.createdAt <= ttlMs);
    return this.artilleryLaunches;
  }

  /**
   * Return live visual-only artillery impact explosions, pruning expired ones.
   * @param {number} now
   * @returns {Array<{x:number,y:number,radiusTiles:number,seed:number,createdAt:number}>}
   */
  liveArtilleryImpacts(now) {
    const ttlMs = 850;
    this.artilleryImpacts = this.artilleryImpacts.filter((f) => now - f.createdAt <= ttlMs);
    return this.artilleryImpacts;
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
   * Recoil progress for an entity that fired recently. Returns 0 when idle.
   * @param {number} id
   * @param {string=} kind
   * @param {number} now
   * @returns {number}
   */
  weaponRecoil(id, kind, now) {
    if (typeof now !== "number") {
      now = kind;
      kind = undefined;
    }
    const startedAt = this.weaponRecoilById.get(id);
    if (typeof startedAt !== "number") return 0;
    const ttlMs = WEAPON_RECOIL_MS[kind] || 300;
    const age = now - startedAt;
    if (age < 0) return 1;
    if (age > ttlMs) {
      this.weaponRecoilById.delete(id);
      return 0;
    }
    const t = age / ttlMs;
    return recoilCurve(t);
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
        const next = {
          ...e,
          x: prior.x + (e.x - prior.x) * t,
          y: prior.y + (e.y - prior.y) * t,
        };
        if (typeof prior.facing === "number" && typeof e.facing === "number") {
          next.facing = lerpAngle(prior.facing, e.facing, t);
        }
        if (typeof prior.weaponFacing === "number" && typeof e.weaponFacing === "number") {
          next.weaponFacing = lerpAngle(prior.weaponFacing, e.weaponFacing, t);
        }
        out.push(next);
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

  _normalizeResource(node, index) {
    const kind = node.kind === KIND.OIL ? KIND.OIL : KIND.STEEL;
    return {
      id: typeof node.id === "number" ? node.id : -(index + 1),
      owner: 0,
      kind,
      x: node.x,
      y: node.y,
      hp: 1,
      maxHp: 1,
      state: "idle",
      remaining: node.remaining ?? RESOURCE_AMOUNTS[kind] ?? 0,
    };
  }

  _applyResourceDeltas(deltas) {
    for (const delta of deltas) {
      if (!delta || typeof delta.id !== "number") continue;
      const node = this.resourceById.get(delta.id);
      if (!node || typeof delta.remaining !== "number") continue;
      node.remaining = delta.remaining;
    }
  }

  _applyResourceDeaths(events) {
    for (const ev of events) {
      if (!ev || ev.e !== "death" || typeof ev.id !== "number") continue;
      const node = this.resourceById.get(ev.id);
      if (node) node.remaining = 0;
    }
  }

  _applyAttackReveals(events, now) {
    for (const ev of events) {
      if (
        !ev ||
        (ev.e !== "attack" && ev.e !== "mortarImpact") ||
        typeof ev.from !== "number"
      ) {
        continue;
      }
      const reveal = this._normalizeAttackReveal(ev, now);
      if (!reveal) continue;
      this.shotRevealsById.set(ev.from, reveal);
    }
  }

  _normalizeAttackReveal(ev, now) {
    const r = ev.reveal;
    if (!r || !isUnit(r.kind)) return null;
    if (!Number.isFinite(r.x) || !Number.isFinite(r.y)) return null;
    const targetPos = Array.isArray(ev.toPos) && ev.toPos.length === 2
      ? { x: ev.toPos[0], y: ev.toPos[1] }
      : Number.isFinite(ev.x) && Number.isFinite(ev.y)
        ? { x: ev.x, y: ev.y }
      : null;
    const targetAngle = targetPos && Number.isFinite(targetPos.x) && Number.isFinite(targetPos.y)
      ? Math.atan2(targetPos.y - r.y, targetPos.x - r.x)
      : null;
    const facing = Number.isFinite(r.facing) ? r.facing : (targetAngle ?? 0);
    const weaponFacing = Number.isFinite(r.weaponFacing) ? r.weaponFacing : facing;
    return {
      id: ev.from,
      owner: typeof r.owner === "number" ? r.owner : 0,
      kind: r.kind,
      x: r.x,
      y: r.y,
      hp: 1,
      maxHp: 1,
      state: STATE.ATTACK,
      facing,
      weaponFacing,
      setupState: r.setupState,
      shotReveal: true,
      shotRevealCreatedAt: now,
      shotRevealExpiresAt: now + SHOT_REVEAL_MS,
    };
  }

  _shotRevealEntityViews(now, visibleIds) {
    const out = [];
    for (const [id, reveal] of this.shotRevealsById) {
      if (visibleIds.has(id) || now > reveal.shotRevealExpiresAt) {
        this.shotRevealsById.delete(id);
        continue;
      }
      out.push({ ...reveal });
    }
    return out;
  }

  _resourceEntityViews() {
    return (this.map.resources || []).map((node) => ({ ...node }));
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
      if (e && !e.shotReveal && !e.visionOnly) out.push(e);
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
    this.closeCommandCardMenu();
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
    this.closeCommandCardMenu();
  }

  /**
   * Remove ids from the existing selection.
   * @param {Iterable<number>} ids
   */
  removeFromSelection(ids) {
    this._pruneSelection();
    for (const id of ids) {
      this.selection.delete(id);
    }
    this.closeCommandCardMenu();
  }

  /** Clear the selection. */
  clearSelection() {
    this.selection.clear();
    this.closeCommandCardMenu();
  }

  /** Drop selected ids that are no longer present in the latest snapshot. */
  _pruneSelection() {
    if (!this.selection || this.selection.size === 0) return;
    let changed = false;
    const live = new Set();
    for (const id of this.selection) {
      const entity = this._curById.get(id);
      if (entity && !entity.shotReveal && !entity.visionOnly) {
        live.add(id);
      } else {
        changed = true;
      }
    }
    if (changed) this.selection = live;
  }

  /**
   * Replace a control group with currently-live own units/buildings.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @param {Iterable<number>} ids selected ids to store.
   * @returns {Array<number>} stored ids.
   */
  setControlGroup(slot, ids) {
    if (!this._validControlGroupSlot(slot)) return [];
    const next = this._ownControllableIds(ids, GameState.MAX_SELECTION_SIZE);
    this.controlGroups[slot] = next;
    return next.slice();
  }

  /**
   * Add currently-live own units/buildings to a control group, ignoring overflow.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @param {Iterable<number>} ids selected ids to add.
   * @returns {Array<number>} stored ids after the add.
   */
  addToControlGroup(slot, ids) {
    if (!this._validControlGroupSlot(slot)) return [];
    const merged = this.controlGroups[slot] ? this.controlGroups[slot].slice() : [];
    const seen = new Set(merged);
    const additions = this._ownControllableIds(ids, GameState.MAX_SELECTION_SIZE);
    for (const id of additions) {
      if (merged.length >= GameState.MAX_SELECTION_SIZE) break;
      if (seen.has(id)) continue;
      merged.push(id);
      seen.add(id);
    }
    this.controlGroups[slot] = merged;
    return merged.slice();
  }

  /**
   * Resolve a control group to live entities, pruning dead/stale ids first.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @returns {Array<object>}
   */
  controlGroupEntities(slot) {
    if (!this._validControlGroupSlot(slot)) return [];
    this._pruneControlGroup(slot);
    const out = [];
    for (const id of this.controlGroups[slot]) {
      const e = this._curById.get(id);
      if (e) out.push(e);
    }
    return out;
  }

  /**
   * Select a control group if it has live members.
   * @param {number} slot 0-based control-group slot; slot 9 maps to key 0.
   * @returns {Array<number>} selected ids.
   */
  selectControlGroup(slot) {
    if (!this._validControlGroupSlot(slot)) return [];
    this._pruneControlGroup(slot);
    const ids = this.controlGroups[slot] || [];
    if (ids.length > 0) this.setSelection(ids);
    return ids.slice();
  }

  _validControlGroupSlot(slot) {
    return Number.isInteger(slot) && slot >= 0 && slot < this.controlGroups.length;
  }

  _ownControllableIds(ids, limit) {
    const out = [];
    const seen = new Set();
    for (const id of ids || []) {
      if (out.length >= limit) break;
      if (seen.has(id)) continue;
      const e = this._curById.get(id);
      if (!e || e.owner !== this.playerId) continue;
      if (!isUnit(e.kind) && !isBuilding(e.kind)) continue;
      out.push(id);
      seen.add(id);
    }
    return out;
  }

  _pruneControlGroups() {
    for (let i = 0; i < this.controlGroups.length; i++) this._pruneControlGroup(i);
  }

  _pruneControlGroup(slot) {
    const group = this.controlGroups[slot];
    if (!group || group.length === 0) return;
    const live = [];
    let changed = false;
    const seen = new Set();
    for (const id of group) {
      const e = this._curById.get(id);
      if (
        e &&
        e.owner === this.playerId &&
        (isUnit(e.kind) || isBuilding(e.kind)) &&
        !seen.has(id) &&
        live.length < GameState.MAX_SELECTION_SIZE
      ) {
        live.push(id);
        seen.add(id);
      } else {
        changed = true;
      }
    }
    if (changed) this.controlGroups[slot] = live;
  }

  // --- build placement (client-only) -------------------------------------

  /** Open the worker build command-card submenu. */
  openWorkerBuildMenu() {
    this.placement = null;
    this.commandTarget = null;
    this.lastCommandTargetArm = null;
    this.atGunSetupPreview = null;
    this.commandCardMode = "workerBuild";
  }

  /**
   * Close any command-card submenu.
   * @returns {boolean} true if a submenu was open.
   */
  closeCommandCardMenu() {
    const hadMenu = this.commandCardMode != null;
    this.commandCardMode = null;
    return hadMenu;
  }

  /**
   * Start previewing placement of a building. Position/validity are filled in
   * by updatePlacement as the cursor moves.
   * @param {string} buildingKind a building EntityKind.
   */
  beginPlacement(buildingKind) {
    this.commandTarget = null;
    this.lastCommandTargetArm = null;
    this.closeCommandCardMenu();
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
   * @param {"move"|"attack"|"setupAtGuns"|{kind:"ability",ability:string}} kind
   */
  beginCommandTarget(kind, options = {}) {
    this.placement = null;
    this.closeCommandCardMenu();
    const armed = this.commandComposer.arm(kind, options);
    this.lastCommandTargetArm = armed;
    this._syncCommandTargetFromComposer();
    return armed;
  }

  /** Clear any armed command target mode. */
  endCommandTarget() {
    this.commandComposer.cancel();
    this.lastCommandTargetArm = null;
    this._syncCommandTargetFromComposer();
  }

  /** Mark a physical key as holding the current command target alive. */
  holdCommandTarget(kind, key, shiftKey = false) {
    this.commandComposer.hold(kind, key, { shiftKey });
    this._syncCommandTargetFromComposer();
  }

  /**
   * Record a click issue and return whether the target remains armed.
   * @param {{shiftKey?: boolean}} ev
   * @returns {{target:null|string|object,queued:boolean,keepArmed:boolean}}
   */
  issueCommandTarget(ev = {}) {
    const issued = this.commandComposer.issue(ev);
    this._syncCommandTargetFromComposer();
    return issued;
  }

  /** Release a physical command key. */
  releaseCommandTargetKey(key, shiftKey = false) {
    this.commandComposer.releaseKey(key, { shiftKey });
    this._syncCommandTargetFromComposer();
  }

  /** Release Shift preservation for a tapped command. */
  releaseCommandTargetShift() {
    this.commandComposer.releaseShift();
    this._syncCommandTargetFromComposer();
  }

  _syncCommandTargetFromComposer() {
    this.commandTarget = this.commandComposer.target;
    this.atGunSetupPreview = null;
    this.abilityTargetPreview = null;
  }

  /**
   * Add a short-lived local command marker at a world point.
   * @param {"move"|"attack"} kind
   * @param {number} x
   * @param {number} y
   * @param {boolean=} append
   */
  addCommandFeedback(kind, x, y, append = false, radiusTiles = null) {
    if (kind === "mortar" && Number.isFinite(x) && Number.isFinite(y)) {
      this.pendingMortarTargets.push({ x, y, createdAt: performance.now() });
      this.pendingMortarTargets = this.pendingMortarTargets.filter(
        (p) => performance.now() - p.createdAt <= 700,
      );
    }
    this.commandFeedback.push({
      kind,
      x,
      y,
      append: !!append,
      radiusTiles,
      createdAt: performance.now(),
    });
    if (this.commandFeedback.length > 12) {
      this.commandFeedback.splice(0, this.commandFeedback.length - 12);
    }
  }

  /**
   * Return live command feedback markers, pruning expired ones.
   * @param {number} now
   * @returns {Array<{kind:string,x:number,y:number,append:boolean,createdAt:number}>}
   */
  liveCommandFeedback(now) {
    const ttlMs = 650;
    this.commandFeedback = this.commandFeedback.filter((f) => now - f.createdAt <= ttlMs);
    return this.commandFeedback;
  }

  /**
   * Set or clear the hovered resource-to-City-Centre mining preview.
   * @param {null | {resourceId:number, resourceX:number, resourceY:number, ccId:number, ccX:number, ccY:number, inRange:boolean}} preview
   */
  updateResourceMiningPreview(preview) {
    this.resourceMiningPreview = preview;
  }

  /**
   * Set or clear the AT gun manual setup cone preview.
   * @param {null | {mouseX:number, mouseY:number, guns:Array<object>}} preview
   */
  updateAtGunSetupPreview(preview) {
    this.atGunSetupPreview = preview;
  }

  /**
   * Set or clear the armed-ability targeting preview (range circles + hover validity).
   * @param {null | {ability:string, mouseX:number, mouseY:number, carriers:Array<object>, rangePx:number, hoverInRange:boolean, hoverInsideMinRange?:boolean}} preview
   */
  updateAbilityTargetPreview(preview) {
    this.abilityTargetPreview = preview;
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
   * Whether a tile is passable per the PASSABLE table (useful to input/index.js for
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

function recoilCurve(t) {
  const progress = t < 0 ? 0 : t > 1 ? 1 : t;
  if (progress < 0.18) {
    return 1 - progress * 0.12;
  }
  const settle = (progress - 0.18) / 0.82;
  return Math.cos(settle * Math.PI * 0.5) * 0.88;
}
