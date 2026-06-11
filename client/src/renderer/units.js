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
  angleLerp,
  clamp01,
  dashedLine,
  drawAtGun,
  drawFacingWedge,
  drawFreeRotatedRect,
  drawGunTire,
  drawImpassableEdge,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawRotatedRect,
  drawRotatedRectOffset,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hexToInt,
  isImpassableAt,
  isVehicleBodyKind,
  lerp,
  lightenColor,
  muzzleFlashRadius,
  normRect,
  offsetPoint,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  rotatePoint,
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

function drawMortarTeam(g, r, tint, facing, weaponFacing, setup, recoil) {
  const deploy = clamp01(setup.prongFactor);
  const travelA = facing;
  const fireA = weaponFacing - Math.PI * 0.22 + Math.PI / 4;
  const a = angleLerp(travelA, fireA, smoothstep01(deploy));
  const kick = recoilVector(a, recoil);
  const carriageKick = recoilVector(a, recoil * 0.18);
  const wheelX = lerp(-r * 0.42, -r * 0.28, deploy);
  const wheelY = r * 0.52;
  const tireLength = r * 0.46;
  const tireWidth = r * 0.26;

  const axleL = offsetPoint(rotatePoint(wheelX, -wheelY, a), carriageKick);
  const axleR = offsetPoint(rotatePoint(wheelX, wheelY, a), carriageKick);
  g.lineStyle(2, 0x1a1712, 0.9);
  g.moveTo(axleL.x, axleL.y);
  g.lineTo(axleR.x, axleR.y);
  drawGunTire(g, axleL.x, axleL.y, tireLength, tireWidth, a);
  drawGunTire(g, axleR.x, axleR.y, tireLength, tireWidth, a);

  const base = offsetPoint(rotatePoint(-r * 0.16, 0, a), carriageKick);
  const tow = offsetPoint(rotatePoint(lerp(-r * 1.2, -r * 0.72, deploy), 0, a), carriageKick);
  const bipodRoot = offsetPoint(rotatePoint(r * 0.22, 0, a), carriageKick);
  const footSpread = lerp(r * 0.12, r * 0.46, deploy);
  const footForward = lerp(r * 0.52, r * 0.82, deploy);

  g.lineStyle(2, tint, 0.9);
  g.moveTo(tow.x, tow.y);
  g.lineTo(base.x, base.y);
  const footL = offsetPoint(rotatePoint(footForward, -footSpread, a), carriageKick);
  const footR = offsetPoint(rotatePoint(footForward, footSpread, a), carriageKick);
  g.lineStyle(2, 0x3f5f32, 0.9);
  g.moveTo(bipodRoot.x, bipodRoot.y);
  g.lineTo(footL.x, footL.y);
  g.moveTo(bipodRoot.x, bipodRoot.y);
  g.lineTo(footR.x, footR.y);

  g.beginFill(tint, 0.95);
  drawRotatedRectOffset(g, -r * 0.08, 0, r * 0.58, r * 0.42, a, carriageKick);
  g.endFill();
  g.beginFill(tint, 0.92);
  drawFreeRotatedRect(g, base.x, base.y, r * 0.34, r * 0.5, a);
  g.endFill();

  const tubeRear = offsetPoint(rotatePoint(-r * 0.14, 0, a), kick);
  const muzzleDist = lerp(r * 1.02, r * 0.74, deploy);
  const muzzle = offsetPoint(rotatePoint(muzzleDist, 0, a), kick);
  g.lineStyle(r * 0.22, 0x263f22, 0.98);
  g.moveTo(tubeRear.x, tubeRear.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.lineStyle(r * 0.08, 0x58734c, 0.66);
  g.moveTo(tubeRear.x + Math.sin(a) * r * 0.08, tubeRear.y - Math.cos(a) * r * 0.08);
  g.lineTo(muzzle.x + Math.sin(a) * r * 0.08, muzzle.y - Math.cos(a) * r * 0.08);
  g.beginFill(0x1c2c19, 0.98);
  drawFreeRotatedRect(g, muzzle.x, muzzle.y, r * 0.16, r * 0.28, a);
  g.endFill();
}

function drawArtillery(g, body, tint, facing, weaponFacing, setup, recoil, motion) {
  const deploy = clamp01(setup.prongFactor);
  drawTankTracks(g, body, facing, motion);

  g.beginFill(tint, 0.97);
  g.drawPolygon(rotatedArtilleryHull(body, facing));
  g.endFill();
  g.beginFill(0x1a1712, 0.2);
  drawRotatedRect(g, -body.halfLen * 0.08, 0, body.halfLen * 1.34, body.halfWidth * 0.92, facing);
  g.endFill();

  const trailSpread = body.halfWidth * (0.44 + deploy * 0.46);
  const trailRear = -body.halfLen * (0.34 + deploy * 0.28);
  const trailRoot = rotatePoint(-body.halfLen * 0.08, 0, weaponFacing);
  const trailL = rotatePoint(trailRear, -trailSpread, weaponFacing);
  const trailR = rotatePoint(trailRear, trailSpread, weaponFacing);
  g.lineStyle(4, 0x2a2119, 0.92 * deploy);
  g.moveTo(trailRoot.x, trailRoot.y);
  g.lineTo(trailL.x, trailL.y);
  g.moveTo(trailRoot.x, trailRoot.y);
  g.lineTo(trailR.x, trailR.y);

  const tireLength = body.halfWidth * 0.58;
  const tireWidth = body.halfWidth * 0.25;
  for (const y of [-body.halfWidth * 0.74, body.halfWidth * 0.74]) {
    const wheel = rotatePoint(-body.halfLen * 0.15, y, facing);
    drawGunTire(g, wheel.x, wheel.y, tireLength, tireWidth, facing);
  }

  g.beginFill(0x1a1712, 0.3);
  drawRotatedRect(g, body.halfLen * 0.1, 0, body.halfLen * 0.92, body.halfWidth * 0.74, weaponFacing);
  g.endFill();
  g.beginFill(tint, 0.98);
  drawRotatedRect(g, body.halfLen * 0.07, 0, body.halfLen * 0.68, body.halfWidth * 0.64, weaponFacing);
  g.endFill();

  const kick = recoilVector(weaponFacing, recoil * 0.8);
  const breech = offsetPoint(rotatePoint(body.halfLen * 0.18, 0, weaponFacing), kick);
  const muzzle = offsetPoint(rotatePoint(body.halfLen * 1.52, 0, weaponFacing), kick);
  g.lineStyle(8, 0x241d17, 0.98);
  g.moveTo(breech.x, breech.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.lineStyle(2.5, 0xd8d0b0, 0.58);
  g.moveTo(breech.x + Math.sin(weaponFacing) * 3, breech.y - Math.cos(weaponFacing) * 3);
  g.lineTo(muzzle.x + Math.sin(weaponFacing) * 3, muzzle.y - Math.cos(weaponFacing) * 3);
  g.beginFill(0x3d3528, 0.98);
  drawFreeRotatedRect(g, breech.x, breech.y, body.halfLen * 0.34, body.halfWidth * 0.44, weaponFacing);
  g.endFill();
}

function rotatedArtilleryHull(body, facing) {
  const nose = body.halfLen;
  const rear = -body.halfLen;
  const w = body.halfWidth;
  return [
    rotatePoint(rear + 5, -w + 5, facing),
    rotatePoint(nose - 8, -w + 4, facing),
    rotatePoint(nose, -w * 0.48, facing),
    rotatePoint(nose, w * 0.48, facing),
    rotatePoint(nose - 8, w - 4, facing),
    rotatePoint(rear + 5, w - 5, facing),
    rotatePoint(rear, w * 0.64, facing),
    rotatePoint(rear, -w * 0.64, facing),
  ].flatMap((p) => [p.x, p.y]);
}

function unitVehicleBody(kind, stat) {
  if (kind === KIND.ARTILLERY) return tankBodyVisual(stat);
  return isVehicleBodyKind(kind) ? tankBodyVisual(stat) : null;
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
    : e.kind === KIND.ARTILLERY
      ? recoilVector(weaponFacing, recoil * 0.65)
    : e.kind === KIND.AT_TEAM
      ? recoilVector(weaponFacing, recoil * 0.42)
      : e.kind === KIND.MORTAR_TEAM
        ? recoilVector(weaponFacing, recoil * 0.28)
      : ZERO_OFFSET;
  const vehicleBody = unitVehicleBody(e.kind, stat);

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
  } else if (e.kind === KIND.MORTAR_TEAM) {
    drawMortarTeam(g, r, tint, facing, weaponFacing, this._deployedWeaponSetupVisual(e), recoil);
  } else if (e.kind === KIND.ARTILLERY) {
    const body = vehicleBody;
    const motion = this._tankMotionVisual(e, facing, state, body);
    drawArtillery(g, body, tint, facing, weaponFacing, this._deployedWeaponSetupVisual(e), recoil, motion);
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
    e.kind !== KIND.MORTAR_TEAM &&
    e.kind !== KIND.ARTILLERY &&
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
