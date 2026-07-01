import { KIND, STATE, WEAPON_KIND, isUnit } from "./protocol.js";

const SHOT_REVEAL_MS = 1500;
const WEAPON_RECOIL_MS = Object.freeze({
  [KIND.RIFLEMAN]: 420,
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
  [WEAPON_KIND.TANK_CANNON]: WEAPON_RECOIL_MS[KIND.TANK],
  [WEAPON_KIND.TANK_COAX]: WEAPON_RECOIL_MS[KIND.TANK],
});

export class VisualEffectBuffers {
  constructor() {
    /** @type {Array<{fromX:number,fromY:number,toX:number,toY:number,durationMs:number,createdAt:number}>} */
    this.smokeCanisters = [];
    /** @type {Array<{from:number,to:number,targetPos?:object,weaponKind?:string,createdAt:number}>} */
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
    /** @type {Map<number, number|{startedAt:number,weaponKind?:string}>} attacker id -> latest shot receive time. */
    this.weaponRecoilById = new Map();
    /** @type {Array<{x:number,y:number,createdAt:number}>} */
    this.pendingMortarTargets = [];
    /** @type {Map<number, object>} attacker id -> temporary fog reveal entity. */
    this.shotRevealsById = new Map();
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

  applySnapshotEvents(events, now, entityById) {
    for (const ev of events) {
      if (ev && ev.e === "attack" && typeof ev.from === "number" && typeof ev.to === "number") {
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
        this.weaponRecoilById.set(ev.from, recoilRecord(now, ev.weaponKind));
      } else if (ev && ev.e === "smokeLaunch") {
        this.addSmokeCanister(ev, now);
      } else if (ev && ev.e === "mortarLaunch") {
        this.addMortarLaunch(ev, now);
      } else if (ev && ev.e === "mortarImpact") {
        this.addMortarImpact(ev, now);
      } else if (ev && ev.e === "artilleryTarget") {
        this.addArtilleryTarget(ev, now, entityById);
      } else if (ev && ev.e === "artilleryImpact") {
        this.addArtilleryImpact(ev, now);
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

  liveMuzzleFlashes(now) {
    const ttlMs = 240;
    this.muzzleFlashes = this.muzzleFlashes.filter((f) => now - f.createdAt <= ttlMs);
    return this.muzzleFlashes;
  }

  weaponRecoil(id, kind, now, weaponKind) {
    if (typeof now !== "number") {
      now = kind;
      kind = undefined;
      weaponKind = undefined;
    }
    const record = this.weaponRecoilById.get(id);
    const startedAt = typeof record === "number" ? record : record?.startedAt;
    if (typeof startedAt !== "number") return 0;
    const recoilWeaponKind = normalizedWeaponKind(weaponKind) || normalizedWeaponKind(record?.weaponKind);
    const ttlMs = recoilMsFor(kind, recoilWeaponKind);
    const age = now - startedAt;
    if (age < 0) return 1;
    if (age > ttlMs) {
      this.weaponRecoilById.delete(id);
      return 0;
    }
    const t = age / ttlMs;
    return recoilCurve(t);
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
    trimHead(this.smokeCanisters, 64);
    trimHead(this.mortarLaunches, 32);
    trimHead(this.mortarShells, 32);
    trimHead(this.mortarTargets, 32);
    trimHead(this.mortarImpacts, 32);
    trimHead(this.artilleryTargets, 48);
    trimHead(this.artilleryLaunches, 32);
    trimHead(this.artilleryImpacts, 32);
  }
}

function recoilRecord(startedAt, weaponKind) {
  const normalized = normalizedWeaponKind(weaponKind);
  return normalized ? { startedAt, weaponKind: normalized } : startedAt;
}

function normalizedWeaponKind(weaponKind) {
  return Object.values(WEAPON_KIND).includes(weaponKind) ? weaponKind : undefined;
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
