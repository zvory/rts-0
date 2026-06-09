import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
  STATS,
  PLAYER_PALETTE,
  RESOURCE_AMOUNTS,
  AT_GUN_DEPLOYED_RANGE_TILES,
  AT_GUN_FIELD_OF_FIRE_RAD,
  isProducerBuilding,
} from "../config.js";
import { KIND, SETUP, STATE, isBuilding, isResource } from "../protocol.js";
import {
  DEPLOYED_WEAPON_ANIM_MS,
  SWEEP_EVICT_FRAMES,
  WEAPON_RECOIL_PX,
  ZERO_OFFSET,
} from "./palette.js";
import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAtGun,
  drawFacingWedge,
  drawImpassableEdge,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawRotatedRect,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hexToInt,
  isImpassableAt,
  isVehicleBodyKind,
  lightenColor,
  muzzleFlashRadius,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  smoothstep01,
  tankBodyVisual,
  terrainColor,
  terrainOverlayColor,
  weaponRecoilOffset,
} from "./shared.js";

export function _deployedWeaponSetupVisual(e) {
  const now = performance.now();
  const setupState = e.setupState || SETUP.PACKED;
  const prev = this._setupVisuals.get(e.id);
  if (!prev || prev.state !== setupState) {
    this._setupVisuals.set(e.id, { state: setupState, changedAt: now });
  }
  const rec = this._setupVisuals.get(e.id);
  const elapsed = now - rec.changedAt;
  const t = smoothstep01(elapsed / DEPLOYED_WEAPON_ANIM_MS);

  if (setupState === SETUP.SETTING_UP) {
    return { prongFactor: t, barrel: false };
  }
  if (setupState === SETUP.TEARING_DOWN) {
    return { prongFactor: 1 - t, barrel: false };
  }
  if (setupState === SETUP.DEPLOYED) {
    return { prongFactor: 1, barrel: e.state !== STATE.MOVE };
  }
  return { prongFactor: 0, barrel: false };
}

export function _sweepSetupVisuals(liveIds) {
  for (const id of [...this._setupVisuals.keys()]) {
    if (!liveIds.has(id)) this._setupVisuals.delete(id);
  }
}

export function _sweepTankMotion(liveIds) {
  for (const id of [...this._tankMotion.keys()]) {
    if (!liveIds.has(id)) this._tankMotion.delete(id);
  }
}

export function _tankMotionVisual(e, facing, state, body) {
  const prev = this._tankMotion.get(e.id);
  let leftPhase = prev ? prev.leftPhase : 0;
  let rightPhase = prev ? prev.rightPhase : 0;
  let leftDir = 0;
  let rightDir = 0;
  let activity = 0;

  if (prev) {
    const dx = e.x - prev.x;
    const dy = e.y - prev.y;
    const dist = Math.hypot(dx, dy);
    const turn = angleDelta(prev.facing, facing);
    const avgFacing = prev.facing + turn * 0.5;
    const forward = Math.cos(avgFacing);
    const forwardY = Math.sin(avgFacing);
    const forwardMove = dx * forward + dy * forwardY;
    const lateralMove = -dx * forwardY + dy * forward;
    const drive = Math.abs(forwardMove) >= Math.abs(lateralMove) * 0.5
      ? forwardMove
      : Math.sign(forwardMove || 1) * dist;
    const turnTravel = turn * body.halfWidth;
    const leftDelta = drive - turnTravel;
    const rightDelta = drive + turnTravel;
    leftPhase += leftDelta;
    rightPhase += rightDelta;
    leftDir = Math.sign(leftDelta);
    rightDir = Math.sign(rightDelta);
    activity = clamp01((Math.abs(leftDelta) + Math.abs(rightDelta)) / 4);
  }

  const ownTank = e.owner === state.playerId;
  const oil = state.resources ? state.resources.oil : null;
  const oilStarved = ownTank && oil === 0 && (e.state === STATE.MOVE || e.state === STATE.ATTACK);
  const lowOil = ownTank && typeof oil === "number" && oil > 0 && oil <= 5;
  const next = { x: e.x, y: e.y, facing, leftPhase, rightPhase };
  this._tankMotion.set(e.id, next);
  return { leftPhase, rightPhase, leftDir, rightDir, activity, lowOil, oilStarved };
}

function workerIsBusy(e) {
  return e.kind === KIND.WORKER && (e.latchedNode || e.state === STATE.BUILD);
}

function drawWorkerBusyIndicator(g, r) {
  g.lineStyle(2, 0xf2d16b, 0.95);
  g.moveTo(-r * 0.55, -r * 1.15);
  g.lineTo(-r * 0.2, -r * 1.45);
  g.lineTo(r * 0.2, -r * 1.45);
  g.lineTo(r * 0.55, -r * 1.15);
}

export function _drawUnit(e, colorByOwner, state, pools = {}) {
  const shadowPool = pools.shadow || "unitShadows";
  const unitPool = pools.unit || "units";
  const stat = STATS[e.kind] || {};
  const r = stat.size || 9;
  const tint = this._tintFor(e.owner, colorByOwner);
  const facing = typeof e.facing === "number" ? e.facing : 0;
  const weaponFacing = typeof e.weaponFacing === "number" ? e.weaponFacing : facing;
  const recoilProgress = typeof state.weaponRecoil === "function"
    ? state.weaponRecoil(e.id, e.kind, performance.now())
    : 0;
  const recoil = weaponRecoilOffset(e.kind, recoilProgress);
  const heavyKick = e.kind === KIND.TANK
    ? recoilVector(weaponFacing, recoil * 0.85)
    : e.kind === KIND.AT_TEAM
      ? recoilVector(weaponFacing, recoil * 0.42)
      : ZERO_OFFSET;
  const vehicleBody = isVehicleBodyKind(e.kind) ? tankBodyVisual(stat) : null;

  // Shadow on its own layer (under all units).
  const sh = this._slot(shadowPool, e.id);
  sh.position.set(e.x + heavyKick.x, e.y + heavyKick.y);
  if (vehicleBody) {
    this._vehicleShadow(sh, 0, 0, vehicleBody, facing);
  } else {
    this._shadow(sh, 0, 0, r);
  }

  // Body on the unit layer.
  const g = this._slot(unitPool, e.id);
  g.position.set(e.x + heavyKick.x, e.y + heavyKick.y);
  g.lineStyle(2, 0x1a1712, 0.95);

  if (e.kind === KIND.RIFLEMAN || e.kind === KIND.MACHINE_GUNNER) {
    drawInfantryBase(g, r, tint, facing);
    if (e.kind === KIND.RIFLEMAN) {
      drawInfantryRifle(g, r, facing, recoil);
    } else {
      drawInfantryMachineGun(g, r, facing, weaponFacing, this._deployedWeaponSetupVisual(e), recoil);
    }
  } else if (e.kind === KIND.AT_TEAM) {
    drawAtGun(g, r, tint, facing, weaponFacing, this._deployedWeaponSetupVisual(e), recoil);
  } else if (e.kind === KIND.SCOUT_CAR) {
    // Scout cars currently use the tank-like vehicle movement model server-side.
    // Replace with truck/wheeled movement semantics once that model exists.
    const body = vehicleBody;
    const motion = this._tankMotionVisual(e, facing, state, body);
    drawScoutCar(g, body, tint, facing, weaponFacing, motion, recoil);
  } else if (e.kind === KIND.TANK) {
    // Hull follows movement facing; turret/barrel follow weapon facing.
    const body = vehicleBody;
    const motion = this._tankMotionVisual(e, facing, state, body);
    drawTankTracks(g, body, facing, motion);
    drawTankHull(g, body, tint, facing);

    const barrel = polar(weaponFacing, Math.max(body.halfLen * 0.8, body.halfLen + 8 - recoil));
    g.lineStyle(5, 0x241d17, 0.95);
    g.moveTo(0, 0);
    g.lineTo(barrel.x, barrel.y);

    g.lineStyle(2, 0x1a1712, 0.95);
    g.beginFill(lightenColor(tint, 0.12));
    drawRotatedRect(g, 1, 0, body.halfLen * 0.72, body.halfWidth * 0.9, weaponFacing);
    g.endFill();

    const nose = polar(facing, body.halfLen - 2);
    g.lineStyle(2, 0xd8d0b0, 0.75);
    g.moveTo(nose.x - Math.cos(facing) * 5, nose.y - Math.sin(facing) * 5);
    g.lineTo(nose.x, nose.y);
    drawTankFuelCue(g, body, facing, motion);
  } else {
    // Engineer (and any other unit kind): compact tool-carrying block.
    g.beginFill(tint);
    g.drawPolygon([
      0, -r,
      r * 0.85, -r * 0.25,
      r * 0.55, r * 0.9,
      -r * 0.55, r * 0.9,
      -r * 0.85, -r * 0.25,
    ]);
    g.endFill();
    if (workerIsBusy(e)) drawWorkerBusyIndicator(g, r);
  }

  // Facing indicator: a short pale tick from center outward.
  if (
    e.kind !== KIND.RIFLEMAN &&
    e.kind !== KIND.MACHINE_GUNNER &&
    e.kind !== KIND.AT_TEAM &&
    e.kind !== KIND.SCOUT_CAR &&
    e.kind !== KIND.TANK
  ) {
    const fp = polar(facing, r + 3);
    g.lineStyle(2, 0xd8d0b0, 0.85);
    g.moveTo(0, 0);
    g.lineTo(fp.x, fp.y);
  }
}

export function _drawShotRevealUnit(e, colorByOwner, state) {
  const now = performance.now();
  const age = Math.max(0, now - (e.shotRevealCreatedAt || now));
  const ttl = Math.max(1, (e.shotRevealExpiresAt || now + 1) - (e.shotRevealCreatedAt || now));
  const t = clamp01(age / ttl);
  const alpha = 0.82 * (1 - smoothstep01(Math.max(0, t - 0.62) / 0.38));
  this._drawUnit(e, colorByOwner, state, {
    shadow: "shotRevealShadows",
    unit: "shotReveals",
  });
  const sh = this._pools.shotRevealShadows.get(e.id);
  const g = this._pools.shotReveals.get(e.id);
  if (sh) sh.alpha = alpha * 0.9;
  if (g) g.alpha = alpha;
}
