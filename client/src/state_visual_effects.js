import { ARTILLERY_OUTER_RADIUS_TILES } from "./config.js";
import { EVENT, KIND, STATE, WEAPON_KIND, isUnit } from "./protocol.js";

const SHOT_REVEAL_MS = 1500;
const MISS_TOAST_MS = 760;
const WEAPON_RECOIL_MS = Object.freeze({
  [KIND.RIFLEMAN]: 420,
  [KIND.PANZERFAUST]: 420,
  [KIND.MACHINE_GUNNER]: 160,
  [KIND.ANTI_TANK_GUN]: 820,
  [KIND.MORTAR_TEAM]: 520,
  [KIND.ARTILLERY]: 980,
  [KIND.SCOUT_CAR]: 160,
  [KIND.TANK]: 650,
});
const WEAPON_RECOIL_MS_BY_WEAPON_KIND = Object.freeze({
  [WEAPON_KIND.RIFLEMAN_RIFLE]: WEAPON_RECOIL_MS[KIND.RIFLEMAN],
  [WEAPON_KIND.MACHINE_GUNNER_MG]: WEAPON_RECOIL_MS[KIND.MACHINE_GUNNER],
  [WEAPON_KIND.ANTI_TANK_GUN]: WEAPON_RECOIL_MS[KIND.ANTI_TANK_GUN],
  [WEAPON_KIND.MORTAR_TEAM_MORTAR]: WEAPON_RECOIL_MS[KIND.MORTAR_TEAM],
  [WEAPON_KIND.ARTILLERY_GUN]: WEAPON_RECOIL_MS[KIND.ARTILLERY],
  [WEAPON_KIND.SCOUT_CAR_MG]: WEAPON_RECOIL_MS[KIND.SCOUT_CAR],
  [WEAPON_KIND.PANZERFAUST_LOADED_SHOT]: 620,
  [WEAPON_KIND.TANK_CANNON]: WEAPON_RECOIL_MS[KIND.TANK],
});
const WEAPON_KINDS = new Set(Object.values(WEAPON_KIND));

export class VisualEffectBuffers {
  constructor() {
    /** @type {Array<{fromX:number,fromY:number,toX:number,toY:number,durationMs:number,createdAt:number}>} */
    this.smokeCanisters = [];
    /** @type {Array<{from:number,to:number,targetPos?:object,weaponKind?:string,createdAt:number}>} */
    this.muzzleFlashes = [];
    /** @type {Array<{id:number,to:number,createdAt:number}>} */
    this.missToasts = [];
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
    /** @type {Array<{from?:number,fromX:number,fromY:number,toX:number,toY:number,durationMs:number,seed:number,createdAt:number}>} */
    this.panzerfaustShots = [];
    /** @type {Array<{x:number,y:number,seed:number,createdAt:number}>} */
    this.panzerfaustImpacts = [];
    /** @type {Map<number, number|{startedAt:number,weaponKind?:string}>} attacker id -> latest shot receive time. */
    this.weaponRecoilById = new Map();
    /** @type {Array<{x:number,y:number,createdAt:number}>} */
    this.pendingMortarTargets = [];
    /** @type {Map<number, object>} attacker id -> temporary fog reveal entity. */
    this.shotRevealsById = new Map();
    this._nextMissToastId = 1;
  }

  pruneRecoilForSnapshot(curById) {
    for (const id of this.weaponRecoilById.keys()) {
      if (!curById.has(id)) this.weaponRecoilById.delete(id);
    }
  }

  applyAttackReveals(events, now) {
    for (const ev of events) {
      if (
        !ev ||
        (ev.e !== EVENT.ATTACK && ev.e !== EVENT.MORTAR_IMPACT) ||
        typeof ev.from !== "number"
      ) {
        continue;
      }
      const reveal = this._normalizeAttackReveal(ev, now);
      if (!reveal) continue;
      this.shotRevealsById.set(ev.from, reveal);
    }
  }

  applySnapshotEvents(events, now, entityById) {
    for (const ev of events) {
      if (ev && ev.e === EVENT.ATTACK && typeof ev.from === "number" && typeof ev.to === "number") {
        const targetPos = eventTargetPos(ev);
        if (ev.from !== ev.to) {
          this.muzzleFlashes.push({
            from: ev.from,
            to: ev.to,
            targetPos,
            weaponKind: normalizedWeaponKind(ev.weaponKind),
            createdAt: now,
          });
        }
        if (ev.weaponKind !== WEAPON_KIND.TANK_COAX) {
          this.weaponRecoilById.set(ev.from, recoilRecord(now, ev.weaponKind));
        }
      } else if (ev && ev.e === EVENT.MISS && typeof ev.to === "number") {
        this.missToasts.push({
          id: this._nextMissToastId++,
          to: ev.to,
          createdAt: now,
        });
      } else if (ev && ev.e === EVENT.SMOKE_LAUNCH) {
        this.addSmokeCanister(ev, now);
      } else if (ev && ev.e === EVENT.MORTAR_LAUNCH) {
        this.addMortarLaunch(ev, now);
      } else if (ev && ev.e === EVENT.MORTAR_IMPACT) {
        this.addMortarImpact(ev, now);
      } else if (ev && ev.e === EVENT.ARTILLERY_TARGET) {
        this.addArtilleryTarget(ev, now, entityById);
      } else if (ev && ev.e === EVENT.ARTILLERY_IMPACT) {
        this.addArtilleryImpact(ev, now);
      } else if (ev && ev.e === EVENT.PANZERFAUST_LAUNCH) {
        this.addPanzerfaustShot(ev, now);
      } else if (ev && ev.e === EVENT.PANZERFAUST_IMPACT) {
        this.addPanzerfaustImpact(ev, now);
      }
    }
    this._trimQueues();
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
      this.weaponRecoilById.set(ev.from, recoilRecord(now, WEAPON_KIND.MORTAR_TEAM_MORTAR));
    }
    this.pendingMortarTargets = this.pendingMortarTargets.filter(
      (p) => Math.hypot(p.x - ev.toX, p.y - ev.toY) > 2,
    );
    this.mortarLaunches.push({
      x: ev.fromX,
      y: ev.fromY,
      toX: ev.toX,
      toY: ev.toY,
      seed,
      createdAt: now,
    });
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

  addArtilleryTarget(ev, now = performance.now(), entityById = null) {
    if (!Number.isFinite(ev.x) || !Number.isFinite(ev.y)) return;
    if (typeof ev.from === "number") {
      this.weaponRecoilById.set(ev.from, recoilRecord(now, WEAPON_KIND.ARTILLERY_GUN));
      const shooter = typeof entityById === "function" ? entityById(ev.from) : null;
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
      radiusTiles: Number.isFinite(ev.radiusTiles)
        ? ev.radiusTiles
        : ARTILLERY_OUTER_RADIUS_TILES,
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
      radiusTiles: Number.isFinite(ev.radiusTiles)
        ? ev.radiusTiles
        : ARTILLERY_OUTER_RADIUS_TILES,
      seed: Math.floor(ev.x * 19 + ev.y * 23 + now) >>> 0,
      createdAt: now,
    });
  }

  addPanzerfaustShot(ev, now = performance.now()) {
    if (
      !Number.isFinite(ev.fromX) ||
      !Number.isFinite(ev.fromY) ||
      !Number.isFinite(ev.toX) ||
      !Number.isFinite(ev.toY)
    ) {
      return;
    }
    const delayTicks = Number.isFinite(ev.delayTicks) ? Math.max(0, ev.delayTicks) : 15;
    const durationMs = Math.max(80, (delayTicks / 30) * 1000);
    if (typeof ev.from === "number") {
      this.weaponRecoilById.set(ev.from, recoilRecord(now, WEAPON_KIND.PANZERFAUST_LOADED_SHOT));
    }
    this.panzerfaustShots.push({
      from: typeof ev.from === "number" ? ev.from : undefined,
      fromX: ev.fromX,
      fromY: ev.fromY,
      toX: ev.toX,
      toY: ev.toY,
      durationMs,
      seed: Math.floor(ev.fromX * 17 + ev.toY * 19 + now) >>> 0,
      createdAt: now,
    });
  }

  addPanzerfaustImpact(ev, now = performance.now()) {
    if (!Number.isFinite(ev.x) || !Number.isFinite(ev.y)) return;
    this.panzerfaustShots = this.panzerfaustShots.filter(
      (shot) => Math.hypot(shot.toX - ev.x, shot.toY - ev.y) > 2,
    );
    this.panzerfaustImpacts.push({
      x: ev.x,
      y: ev.y,
      seed: Math.floor(ev.x * 29 + ev.y * 31 + now) >>> 0,
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

  liveSmokeCanisters(now) {
    this.smokeCanisters = this.smokeCanisters.filter((f) => now - f.createdAt <= f.durationMs);
    return this.smokeCanisters;
  }

  liveMortarLaunches(now) {
    const ttlMs = 360;
    this.mortarLaunches = this.mortarLaunches.filter((f) => now - f.createdAt <= ttlMs);
    return this.mortarLaunches;
  }

  liveMortarShells(now) {
    this.mortarShells = this.mortarShells.filter((f) => now - f.createdAt <= f.durationMs + 120);
    return this.mortarShells;
  }

  liveMortarTargets(now) {
    this.mortarTargets = this.mortarTargets.filter((f) => now - f.createdAt <= f.durationMs + 120);
    return this.mortarTargets;
  }

  liveMortarImpacts(now) {
    const ttlMs = 1000;
    this.mortarImpacts = this.mortarImpacts.filter((f) => now - f.createdAt <= ttlMs);
    return this.mortarImpacts;
  }

  liveArtilleryTargets(now) {
    this.artilleryTargets = this.artilleryTargets.filter((f) => {
      const ttlMs = Math.max(900, ((f.delayTicks || 0) / 30) * 1000 + 350);
      return now - f.createdAt <= ttlMs;
    });
    return this.artilleryTargets;
  }

  liveArtilleryLaunches(now) {
    const ttlMs = 820;
    this.artilleryLaunches = this.artilleryLaunches.filter((f) => now - f.createdAt <= ttlMs);
    return this.artilleryLaunches;
  }

  liveArtilleryImpacts(now) {
    const ttlMs = 850;
    this.artilleryImpacts = this.artilleryImpacts.filter((f) => now - f.createdAt <= ttlMs);
    return this.artilleryImpacts;
  }

  livePanzerfaustShots(now) {
    this.panzerfaustShots = this.panzerfaustShots.filter(
      (f) => now - f.createdAt <= f.durationMs + 160,
    );
    return this.panzerfaustShots;
  }

  livePanzerfaustImpacts(now) {
    const ttlMs = 720;
    this.panzerfaustImpacts = this.panzerfaustImpacts.filter((f) => now - f.createdAt <= ttlMs);
    return this.panzerfaustImpacts;
  }

  liveMuzzleFlashes(now) {
    const ttlMs = 240;
    this.muzzleFlashes = this.muzzleFlashes.filter((f) => now - f.createdAt <= ttlMs);
    return this.muzzleFlashes;
  }

  liveMissToasts(now) {
    this.missToasts = this.missToasts.filter((f) => now - f.createdAt <= MISS_TOAST_MS);
    return this.missToasts;
  }

  weaponRecoil(id, kind, now, weaponKind) {
    const sample = this._weaponRecoilSample(id, kind, now, weaponKind);
    if (!sample) return 0;
    if (sample.age < 0) return 1;
    return recoilCurve(sample.age / sample.ttlMs);
  }

  weaponRecoilPhase(id, kind, now, weaponKind) {
    const sample = this._weaponRecoilSample(id, kind, now, weaponKind);
    if (!sample || sample.age < 0) return 0;
    return clamp01(sample.age / sample.ttlMs);
  }

  weaponRecoilKind(id) {
    return normalizedWeaponKind(this.weaponRecoilById.get(id)?.weaponKind);
  }

  _weaponRecoilSample(id, kind, now, weaponKind) {
    if (typeof now !== "number") {
      now = kind;
      kind = undefined;
      weaponKind = undefined;
    }
    const record = this.weaponRecoilById.get(id);
    const startedAt = typeof record === "number" ? record : record?.startedAt;
    if (typeof startedAt !== "number") return null;
    const recoilWeaponKind = normalizedWeaponKind(weaponKind) || normalizedWeaponKind(record?.weaponKind);
    if (recoilWeaponKind === WEAPON_KIND.TANK_COAX) return null;
    const ttlMs = recoilMsFor(kind, recoilWeaponKind);
    const age = now - startedAt;
    if (age > ttlMs) {
      this.weaponRecoilById.delete(id);
      return null;
    }
    return { age, ttlMs };
  }

  shotRevealEntityViews(now, visibleIds) {
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

  _normalizeAttackReveal(ev, now) {
    const r = ev.reveal;
    if (!r || !isUnit(r.kind)) return null;
    if (!Number.isFinite(r.x) || !Number.isFinite(r.y)) return null;
    const targetPos = eventTargetPos(ev)
      ?? (Number.isFinite(ev.x) && Number.isFinite(ev.y) ? { x: ev.x, y: ev.y } : null);
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

  _trimQueues() {
    trimHead(this.muzzleFlashes, 256);
    trimHead(this.missToasts, 96);
    trimHead(this.smokeCanisters, 64);
    trimHead(this.mortarLaunches, 32);
    trimHead(this.mortarShells, 32);
    trimHead(this.mortarTargets, 32);
    trimHead(this.mortarImpacts, 32);
    trimHead(this.artilleryTargets, 48);
    trimHead(this.artilleryLaunches, 32);
    trimHead(this.artilleryImpacts, 32);
    trimHead(this.panzerfaustShots, 48);
    trimHead(this.panzerfaustImpacts, 48);
  }
}

// GameState inherits this facade so render/HUD callers do not reach into buffer internals.
export class VisualEffectBackedState {
  constructor() {
    this.visualEffects = new VisualEffectBuffers();
  }

  get smokeCanisters() { return this.visualEffects.smokeCanisters; }
  set smokeCanisters(value) { this.visualEffects.smokeCanisters = Array.isArray(value) ? value : []; }
  get muzzleFlashes() { return this.visualEffects.muzzleFlashes; }
  set muzzleFlashes(value) { this.visualEffects.muzzleFlashes = Array.isArray(value) ? value : []; }
  get missToasts() { return this.visualEffects.missToasts; }
  set missToasts(value) { this.visualEffects.missToasts = Array.isArray(value) ? value : []; }
  get mortarLaunches() { return this.visualEffects.mortarLaunches; }
  set mortarLaunches(value) { this.visualEffects.mortarLaunches = Array.isArray(value) ? value : []; }
  get mortarShells() { return this.visualEffects.mortarShells; }
  set mortarShells(value) { this.visualEffects.mortarShells = Array.isArray(value) ? value : []; }
  get mortarTargets() { return this.visualEffects.mortarTargets; }
  set mortarTargets(value) { this.visualEffects.mortarTargets = Array.isArray(value) ? value : []; }
  get mortarImpacts() { return this.visualEffects.mortarImpacts; }
  set mortarImpacts(value) { this.visualEffects.mortarImpacts = Array.isArray(value) ? value : []; }
  get artilleryTargets() { return this.visualEffects.artilleryTargets; }
  set artilleryTargets(value) { this.visualEffects.artilleryTargets = Array.isArray(value) ? value : []; }
  get artilleryLaunches() { return this.visualEffects.artilleryLaunches; }
  set artilleryLaunches(value) { this.visualEffects.artilleryLaunches = Array.isArray(value) ? value : []; }
  get artilleryImpacts() { return this.visualEffects.artilleryImpacts; }
  set artilleryImpacts(value) { this.visualEffects.artilleryImpacts = Array.isArray(value) ? value : []; }
  get panzerfaustShots() { return this.visualEffects.panzerfaustShots; }
  set panzerfaustShots(value) { this.visualEffects.panzerfaustShots = Array.isArray(value) ? value : []; }
  get panzerfaustImpacts() { return this.visualEffects.panzerfaustImpacts; }
  set panzerfaustImpacts(value) { this.visualEffects.panzerfaustImpacts = Array.isArray(value) ? value : []; }
  get weaponRecoilById() { return this.visualEffects.weaponRecoilById; }
  set weaponRecoilById(value) { this.visualEffects.weaponRecoilById = value instanceof Map ? value : new Map(); }
  get pendingMortarTargets() { return this.visualEffects.pendingMortarTargets; }
  set pendingMortarTargets(value) { this.visualEffects.pendingMortarTargets = Array.isArray(value) ? value : []; }
  get shotRevealsById() { return this.visualEffects.shotRevealsById; }
  set shotRevealsById(value) { this.visualEffects.shotRevealsById = value instanceof Map ? value : new Map(); }

  addMortarLaunch(ev, now = performance.now()) { this.visualEffects.addMortarLaunch(ev, now); }
  addMortarImpact(ev, now = performance.now()) { this.visualEffects.addMortarImpact(ev, now); }
  addArtilleryTarget(ev, now = performance.now()) {
    this.visualEffects.addArtilleryTarget(ev, now, (id) => this.entityById(id));
  }
  addArtilleryImpact(ev, now = performance.now()) { this.visualEffects.addArtilleryImpact(ev, now); }
  addPanzerfaustShot(ev, now = performance.now()) { this.visualEffects.addPanzerfaustShot(ev, now); }
  addPanzerfaustImpact(ev, now = performance.now()) { this.visualEffects.addPanzerfaustImpact(ev, now); }
  addSmokeCanister(ev, now = performance.now()) { this.visualEffects.addSmokeCanister(ev, now); }

  liveSmokeCanisters(now) { return this.visualEffects.liveSmokeCanisters(now); }
  liveMortarLaunches(now) { return this.visualEffects.liveMortarLaunches(now); }
  liveMortarShells(now) { return this.visualEffects.liveMortarShells(now); }
  liveMortarTargets(now) { return this.visualEffects.liveMortarTargets(now); }
  liveMortarImpacts(now) { return this.visualEffects.liveMortarImpacts(now); }
  liveArtilleryTargets(now) { return this.visualEffects.liveArtilleryTargets(now); }
  liveArtilleryLaunches(now) { return this.visualEffects.liveArtilleryLaunches(now); }
  liveArtilleryImpacts(now) { return this.visualEffects.liveArtilleryImpacts(now); }
  livePanzerfaustShots(now) { return this.visualEffects.livePanzerfaustShots(now); }
  livePanzerfaustImpacts(now) { return this.visualEffects.livePanzerfaustImpacts(now); }
  liveMuzzleFlashes(now) { return this.visualEffects.liveMuzzleFlashes(now); }
  liveMissToasts(now) { return this.visualEffects.liveMissToasts(now); }
  weaponRecoil(id, kind, now, weaponKind) {
    return this.visualEffects.weaponRecoil(id, kind, now, weaponKind);
  }
  weaponRecoilPhase(id, kind, now, weaponKind) {
    return this.visualEffects.weaponRecoilPhase(id, kind, now, weaponKind);
  }
  weaponRecoilKind(id) {
    return this.visualEffects.weaponRecoilKind(id);
  }
}

function recoilRecord(startedAt, weaponKind) {
  const normalized = normalizedWeaponKind(weaponKind);
  return normalized ? { startedAt, weaponKind: normalized } : startedAt;
}

function normalizedWeaponKind(weaponKind) {
  return WEAPON_KINDS.has(weaponKind) ? weaponKind : undefined;
}

function recoilMsFor(kind, weaponKind) {
  return WEAPON_RECOIL_MS_BY_WEAPON_KIND[weaponKind] || WEAPON_RECOIL_MS[kind] || 300;
}

function eventTargetPos(ev) {
  return Array.isArray(ev.toPos) && ev.toPos.length === 2
    ? { x: ev.toPos[0], y: ev.toPos[1] }
    : null;
}

function trimHead(items, limit) {
  if (items.length > limit) {
    items.splice(0, items.length - limit);
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

function clamp01(value) {
  return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0));
}
