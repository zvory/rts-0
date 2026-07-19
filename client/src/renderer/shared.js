import { COLORS, TANK_BODY } from "../config.js";
import { KIND } from "../protocol.js";
import { WEAPON_RECOIL_PX, ZERO_OFFSET } from "./palette.js";

export function rendererVisualNow(renderer) {
  return renderer?.visualNow?.() ?? performance.now();
}

// Shared pure renderer helpers.

/** Per-attacker muzzle-flash radius in world px. 0 means no flash for this kind. */
export function muzzleFlashRadius(kind) {
  if (kind === KIND.ARTILLERY) return 11;
  if (kind === KIND.TANK) return 18;
  if (kind === KIND.ANTI_TANK_GUN) return 15;
  if (kind === KIND.SCOUT_CAR || kind === KIND.COMMAND_CAR) return 9;
  if (kind === KIND.MACHINE_GUNNER) return 9;
  if (kind === KIND.RIFLEMAN || kind === KIND.PANZERFAUST) return 7;
  return 0;
}

export function weaponRecoilOffset(kind, progress) {
  return (WEAPON_RECOIL_PX[kind] || 0) * clamp01(progress);
}

/** Clamp a number to [0,1]. */
export function clamp01(v) {
  if (v == null || Number.isNaN(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

/** Smoothstep easing on [0,1]. */
export function smoothstep01(v) {
  const t = clamp01(v);
  return t * t * (3 - 2 * t);
}

export function lerp(a, b, t) {
  return a + (b - a) * t;
}

export function dashedLine(g, x1, y1, x2, y2, dash, gap) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy);
  if (len <= 0.001) return;
  const ux = dx / len;
  const uy = dy / len;
  let cursor = 0;
  while (cursor < len) {
    const end = Math.min(cursor + dash, len);
    g.moveTo(x1 + ux * cursor, y1 + uy * cursor);
    g.lineTo(x1 + ux * end, y1 + uy * end);
    cursor = end + gap;
  }
}

export function rectEdgePointTowardCenter(fromX, fromY, centerX, centerY, halfW, halfH) {
  const dx = centerX - fromX;
  const dy = centerY - fromY;
  if (Math.hypot(dx, dy) <= 0.001) return { x: centerX, y: centerY };

  const minX = centerX - halfW;
  const maxX = centerX + halfW;
  const minY = centerY - halfH;
  const maxY = centerY + halfH;
  let tEnter = 0;
  let tExit = 1;

  if (Math.abs(dx) > 0.001) {
    const tx1 = (minX - fromX) / dx;
    const tx2 = (maxX - fromX) / dx;
    tEnter = Math.max(tEnter, Math.min(tx1, tx2));
    tExit = Math.min(tExit, Math.max(tx1, tx2));
  } else if (fromX < minX || fromX > maxX) {
    return { x: centerX, y: centerY };
  }

  if (Math.abs(dy) > 0.001) {
    const ty1 = (minY - fromY) / dy;
    const ty2 = (maxY - fromY) / dy;
    tEnter = Math.max(tEnter, Math.min(ty1, ty2));
    tExit = Math.min(tExit, Math.max(ty1, ty2));
  } else if (fromY < minY || fromY > maxY) {
    return { x: centerX, y: centerY };
  }

  if (tEnter <= 0.001 || tEnter > tExit || tEnter > 1) return { x: centerX, y: centerY };
  return { x: fromX + dx * tEnter, y: fromY + dy * tEnter };
}

/** Point at angle `a` (radians) and distance `d` from the origin. */
export function polar(a, d) {
  return { x: Math.cos(a) * d, y: Math.sin(a) * d };
}

export function recoilVector(a, d) {
  return d > 0 ? polar(a + Math.PI, d) : ZERO_OFFSET;
}

export function offsetPoint(p, offset) {
  return { x: p.x + offset.x, y: p.y + offset.y };
}

export function rotatePoint(x, y, a) {
  const c = Math.cos(a);
  const s = Math.sin(a);
  return { x: x * c - y * s, y: x * s + y * c };
}

export function rotatedPolygon(points, a) {
  const out = [];
  for (let i = 0; i < points.length; i += 2) {
    const p = rotatePoint(points[i], points[i + 1], a);
    out.push(p.x, p.y);
  }
  return out;
}

export function drawRotatedRect(g, cx, cy, w, h, a) {
  drawRotatedRectOffset(g, cx, cy, w, h, a, ZERO_OFFSET);
}

export function drawFreeRotatedRect(g, cx, cy, w, h, a) {
  const hw = w / 2;
  const hh = h / 2;
  const corners = [
    [-hw, -hh],
    [hw, -hh],
    [hw, hh],
    [-hw, hh],
  ];
  const polygon = [];
  for (const [x, y] of corners) {
    const p = rotatePoint(x, y, a);
    polygon.push(cx + p.x, cy + p.y);
  }
  g.drawPolygon(polygon);
}

export function drawRotatedRectOffset(g, cx, cy, w, h, a, offset) {
  const hw = w / 2;
  const hh = h / 2;
  const corners = [
    [cx - hw, cy - hh],
    [cx + hw, cy - hh],
    [cx + hw, cy + hh],
    [cx - hw, cy + hh],
  ];
  const polygon = [];
  for (const [x, y] of corners) {
    const p = rotatePoint(x, y, a);
    polygon.push(p.x + offset.x, p.y + offset.y);
  }
  g.drawPolygon(polygon);
}

export function tankBodyVisual(stat = {}) {
  const body = stat.body || TANK_BODY;
  const halfLen = body.length * 0.5;
  const halfWidth = body.width * 0.5;
  const clearance = body.clearance || 0;
  return {
    halfLen,
    halfWidth,
    clearance,
    shadowRadius: Math.hypot(halfLen + clearance, halfWidth + clearance),
  };
}

export function isVehicleBodyKind(kind) {
  return kind === KIND.ANTI_TANK_GUN ||
    kind === KIND.ARTILLERY ||
    kind === KIND.TANK ||
    kind === KIND.SCOUT_CAR ||
    kind === KIND.COMMAND_CAR;
}

export function drawTankTracks(g, body, facing, motion) {
  const trackW = 5;
  const trackY = body.halfWidth - trackW * 0.5;
  const trackLen = body.halfLen * 2;
  g.lineStyle(1.5, 0x100d0a, 0.95);
  g.beginFill(0x15120f, 0.96);
  drawRotatedRect(g, 0, -trackY, trackLen, trackW, facing);
  drawRotatedRect(g, 0, trackY, trackLen, trackW, facing);
  g.endFill();

  drawTrackTreads(g, body, facing, -trackY, motion.leftPhase, motion.leftDir, motion.activity);
  drawTrackTreads(g, body, facing, trackY, motion.rightPhase, motion.rightDir, motion.activity);
}

export function drawTrackTreads(g, body, facing, y, phase, dir, activity) {
  const spacing = 6;
  const treadW = 2.4;
  const treadH = 4.4;
  const alpha = lerp(0.35, 0.82, clamp01(activity));
  const offset = positiveMod(phase * 0.85, spacing);
  g.beginFill(dir < 0 ? 0x8f7f5e : 0xd8d0b0, alpha);
  for (let x = -body.halfLen - spacing; x <= body.halfLen + spacing; x += spacing) {
    const treadX = x + offset;
    if (treadX < -body.halfLen || treadX > body.halfLen) continue;
    drawRotatedRect(g, treadX, y, treadW, treadH, facing);
  }
  g.endFill();
}

export function drawTankHull(g, body, tint, facing) {
  const inset = 2;
  g.beginFill(tint);
  g.drawPolygon(rotatedPolygon([
    -body.halfLen + inset, -body.halfWidth + 3,
    body.halfLen - 6, -body.halfWidth + 3,
    body.halfLen, -body.halfWidth + 7,
    body.halfLen, body.halfWidth - 7,
    body.halfLen - 6, body.halfWidth - 3,
    -body.halfLen + inset, body.halfWidth - 3,
    -body.halfLen, body.halfWidth - 7,
    -body.halfLen, -body.halfWidth + 7,
  ], facing));
  g.endFill();

  g.beginFill(0x1a1712, 0.24);
  drawRotatedRect(g, -2, 0, body.halfLen * 1.15, body.halfWidth * 0.82, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.06), 0.95);
  drawRotatedRect(g, body.halfLen - 7, 0, 7, body.halfWidth * 1.35, facing);
  g.endFill();

  g.beginFill(0x1a1712, 0.22);
  drawRotatedRect(g, body.halfLen - 3, 0, 3, body.halfWidth * 1.2, facing);
  g.endFill();
}

export function drawTankFuelCue(g, body, facing, motion) {
  if (!motion.lowOil && !motion.oilStarved) return;
  const x = -body.halfLen + 6;
  const y = -body.halfWidth - 4;
  const color = motion.oilStarved ? 0xd47a5f : 0xc9b56a;
  g.lineStyle(2, color, motion.oilStarved ? 0.95 : 0.75);
  drawRotatedRectOutline(g, x, y, 8, 5, facing);
  if (motion.oilStarved) {
    const a = facing;
    const p1 = rotatePoint(x - 3, y - 1.5, a);
    const p2 = rotatePoint(x + 3, y + 1.5, a);
    const p3 = rotatePoint(x + 3, y - 1.5, a);
    const p4 = rotatePoint(x - 3, y + 1.5, a);
    g.moveTo(p1.x, p1.y);
    g.lineTo(p2.x, p2.y);
    g.moveTo(p3.x, p3.y);
    g.lineTo(p4.x, p4.y);
  }
}

export function drawScoutCar(g, body, tint, facing, weaponFacing, motion, recoil) {
  const sideAlpha = lerp(0.62, 0.88, motion.activity);

  // Single blocky truck hull with enclosed side running gear; nothing protrudes past the body.
  const noseShoulderX = body.halfLen * 0.4;
  const noseHalfWidth = body.halfWidth * 0.62;
  g.beginFill(tint);
  g.drawPolygon(rotatedPolygon([
    -body.halfLen, -body.halfWidth,
    noseShoulderX, -body.halfWidth,
    body.halfLen, -noseHalfWidth,
    body.halfLen, noseHalfWidth,
    noseShoulderX, body.halfWidth,
    -body.halfLen, body.halfWidth,
  ], facing));
  g.endFill();

  g.beginFill(0x15120f, sideAlpha);
  drawRotatedRect(g, -body.halfLen * 0.08, -body.halfWidth * 0.78, body.halfLen * 1.58, body.halfWidth * 0.22, facing);
  drawRotatedRect(g, -body.halfLen * 0.08, body.halfWidth * 0.78, body.halfLen * 1.58, body.halfWidth * 0.22, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.08), 0.96);
  drawRotatedRect(g, -body.halfLen * 0.32, 0, body.halfLen * 0.96, body.halfWidth * 1.44, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.14), 0.95);
  g.drawPolygon(rotatedPolygon([
    body.halfLen * 0.1, -body.halfWidth * 0.68,
    body.halfLen * 0.58, -body.halfWidth * 0.56,
    body.halfLen * 0.9, -body.halfWidth * 0.4,
    body.halfLen * 0.9, body.halfWidth * 0.4,
    body.halfLen * 0.58, body.halfWidth * 0.56,
    body.halfLen * 0.1, body.halfWidth * 0.68,
  ], facing));
  g.endFill();

  g.beginFill(0x211b14, 0.82);
  drawRotatedRect(g, body.halfLen * 0.68, 0, body.halfLen * 0.2, body.halfWidth * 0.88, facing);
  drawRotatedRect(g, body.halfLen * 0.24, -body.halfWidth * 0.36, body.halfLen * 0.18, body.halfWidth * 0.34, facing);
  drawRotatedRect(g, body.halfLen * 0.24, body.halfWidth * 0.36, body.halfLen * 0.18, body.halfWidth * 0.34, facing);
  g.endFill();

  g.lineStyle(2, 0xd8d0b0, 0.6);
  const hoodA = rotatePoint(body.halfLen * 0.48, -body.halfWidth * 0.45, facing);
  const hoodB = rotatePoint(body.halfLen * 0.48, body.halfWidth * 0.45, facing);
  g.moveTo(hoodA.x, hoodA.y);
  g.lineTo(hoodB.x, hoodB.y);

  const gunnerAnchor = rotatePoint(-body.halfLen * 0.42, 0, facing);
  const gunner = offsetPoint(gunnerAnchor, recoilVector(weaponFacing, recoil));
  const mount = rotatePoint(-body.halfLen * 0.32, 0, facing);
  g.beginFill(0x1a1712, 0.9);
  g.drawCircle(mount.x, mount.y, body.halfWidth * 0.32);
  g.endFill();

  const a = weaponFacing;
  const gunnerTorso = offsetPoint(gunner, {
    x: Math.cos(a + Math.PI) * body.halfWidth * 0.1,
    y: Math.sin(a + Math.PI) * body.halfWidth * 0.1,
  });
  g.beginFill(lightenColor(tint, 0.14), 0.98);
  drawFreeRotatedRect(g, gunnerTorso.x, gunnerTorso.y, body.halfWidth * 0.5, body.halfWidth * 0.64, a);
  g.endFill();

  const gunnerHead = {
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.2,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.2,
  };
  g.beginFill(lightenColor(tint, 0.24), 0.98);
  g.drawCircle(gunnerHead.x, gunnerHead.y, body.halfWidth * 0.18);
  g.endFill();

  const handSpan = body.halfWidth * 0.32;
  const grip = {
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.2,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.2,
  };
  g.lineStyle(2, 0xd8d0b0, 0.86);
  g.moveTo(gunner.x - Math.sin(a) * handSpan, gunner.y + Math.cos(a) * handSpan);
  g.lineTo(grip.x, grip.y);
  g.moveTo(gunner.x + Math.sin(a) * handSpan, gunner.y - Math.cos(a) * handSpan);
  g.lineTo(grip.x, grip.y);

  const stock = {
    x: gunner.x + Math.cos(a + Math.PI) * body.halfWidth * 0.34,
    y: gunner.y + Math.sin(a + Math.PI) * body.halfWidth * 0.34,
  };
  const muzzle = {
    x: gunner.x + Math.cos(a) * (body.halfLen * 0.78),
    y: gunner.y + Math.sin(a) * (body.halfLen * 0.78),
  };
  g.lineStyle(3, 0x17130f, 0.98);
  g.moveTo(stock.x, stock.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.beginFill(0x32291f, 0.98);
  const receiver = {
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.42,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.42,
  };
  drawFreeRotatedRect(g, receiver.x, receiver.y, body.halfWidth * 0.58, body.halfWidth * 0.3, a);
  g.endFill();

  const shroud = {
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.9,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.9,
  };
  g.beginFill(0x241d17, 0.98);
  drawFreeRotatedRect(g, shroud.x, shroud.y, body.halfWidth * 0.82, body.halfWidth * 0.18, a);
  g.endFill();

  const nose = polar(facing, body.halfLen - 2);
  g.lineStyle(2, 0xd8d0b0, 0.72);
  g.moveTo(nose.x - Math.cos(facing) * 4, nose.y - Math.sin(facing) * 4);
  g.lineTo(nose.x, nose.y);
}

export function drawCommandCar(g, body, tint, facing, motion) {
  const sideAlpha = lerp(0.58, 0.82, motion.activity);
  const noseShoulderX = body.halfLen * 0.2;
  const noseHalfWidth = body.halfWidth * 0.58;

  g.beginFill(tint);
  g.drawPolygon(rotatedPolygon([
    -body.halfLen, -body.halfWidth * 0.82,
    noseShoulderX, -body.halfWidth * 0.82,
    body.halfLen, -noseHalfWidth,
    body.halfLen, noseHalfWidth,
    noseShoulderX, body.halfWidth * 0.82,
    -body.halfLen, body.halfWidth * 0.82,
  ], facing));
  g.endFill();

  g.beginFill(0x15120f, sideAlpha);
  drawRotatedRect(g, -body.halfLen * 0.08, -body.halfWidth * 0.78, body.halfLen * 1.58, body.halfWidth * 0.18, facing);
  drawRotatedRect(g, -body.halfLen * 0.08, body.halfWidth * 0.78, body.halfLen * 1.58, body.halfWidth * 0.18, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.1), 0.98);
  drawRotatedRect(g, -body.halfLen * 0.25, 0, body.halfLen * 0.72, body.halfWidth * 1.18, facing);
  g.endFill();

  g.beginFill(0x211b14, 0.78);
  drawRotatedRect(g, body.halfLen * 0.52, 0, body.halfLen * 0.24, body.halfWidth * 0.78, facing);
  drawRotatedRect(g, -body.halfLen * 0.34, -body.halfWidth * 0.28, body.halfLen * 0.2, body.halfWidth * 0.26, facing);
  drawRotatedRect(g, -body.halfLen * 0.34, body.halfWidth * 0.28, body.halfLen * 0.2, body.halfWidth * 0.26, facing);
  g.endFill();

  g.lineStyle(2, 0xd8d0b0, 0.62);
  const windshieldA = rotatePoint(body.halfLen * 0.16, -body.halfWidth * 0.48, facing);
  const windshieldB = rotatePoint(body.halfLen * 0.16, body.halfWidth * 0.48, facing);
  g.moveTo(windshieldA.x, windshieldA.y);
  g.lineTo(windshieldB.x, windshieldB.y);

  const nose = polar(facing, body.halfLen - 1.5);
  g.lineStyle(2, 0xd8d0b0, 0.74);
  g.moveTo(nose.x - Math.cos(facing) * 3.5, nose.y - Math.sin(facing) * 3.5);
  g.lineTo(nose.x, nose.y);
}

export function drawRotatedRectOutline(g, cx, cy, w, h, a) {
  const hw = w / 2;
  const hh = h / 2;
  const corners = [
    rotatePoint(cx - hw, cy - hh, a),
    rotatePoint(cx + hw, cy - hh, a),
    rotatePoint(cx + hw, cy + hh, a),
    rotatePoint(cx - hw, cy + hh, a),
  ];
  g.moveTo(corners[0].x, corners[0].y);
  for (let i = 1; i < corners.length; i += 1) g.lineTo(corners[i].x, corners[i].y);
  g.lineTo(corners[0].x, corners[0].y);
}

export function drawInfantryBase(g, r, tint, facing) {
  // Shared combat-infantry body: same soldier, different oversized weapon.
  g.lineStyle(2, 0x1a1712, 0.95);
  g.beginFill(tint);
  g.drawPolygon(rotatedPolygon([
    r * 0.72, 0,
    r * 0.22, -r * 0.62,
    -r * 0.58, -r * 0.48,
    -r * 0.78, 0,
    -r * 0.58, r * 0.48,
    r * 0.22, r * 0.62,
  ], facing));
  g.endFill();

  const head = polar(facing, r * 0.72);
  g.beginFill(lightenColor(tint, 0.16));
  g.drawCircle(head.x, head.y, r * 0.34);
  g.endFill();

  const rear = polar(facing + Math.PI, r * 0.68);
  const shoulderL = polar(facing - 1.42, r * 0.48);
  const shoulderR = polar(facing + 1.42, r * 0.48);
  g.lineStyle(2, 0x1a1712, 0.5);
  g.moveTo(rear.x, rear.y);
  g.lineTo(shoulderL.x, shoulderL.y);
  g.moveTo(rear.x, rear.y);
  g.lineTo(shoulderR.x, shoulderR.y);
}

export function drawInfantryRifle(g, r, facing, recoil) {
  const a = facing - 0.2;
  const kick = recoilVector(a, recoil);
  const stock = offsetPoint(polar(a + Math.PI, r * 0.18), kick);
  const muzzle = offsetPoint(polar(a, r * 1.82), kick);
  const hand = offsetPoint(polar(a, r * 0.55), kick);

  g.lineStyle(3, 0x2a2119, 0.96);
  g.moveTo(stock.x, stock.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.lineStyle(2, 0xd8d0b0, 0.85);
  g.moveTo(hand.x - Math.sin(a) * r * 0.32, hand.y + Math.cos(a) * r * 0.32);
  g.lineTo(hand.x + Math.sin(a) * r * 0.32, hand.y - Math.cos(a) * r * 0.32);
}

export function drawInfantryMachineGun(g, r, facing, weaponFacing, setup, recoil) {
  const deploy = clamp01(setup.prongFactor);
  const carryA = facing + 0.86;
  const aimA = weaponFacing;
  const a = angleLerp(carryA, aimA, smoothstep01(deploy));
  const kick = recoilVector(a, recoil);
  const stockRearDist = lerp(r * 0.76, r * 0.58, deploy);
  const muzzleDist = lerp(r * 1.36, r * 2.46, deploy);
  const stockRear = offsetPoint(polar(a + Math.PI, stockRearDist), kick);
  const muzzle = offsetPoint(polar(a, muzzleDist), kick);

  // MG42-inspired profile: shoulder stock, box receiver, long perforated shroud, no rotary barrels.
  g.lineStyle(3, 0x17130f, 0.98);
  g.moveTo(stockRear.x, stockRear.y);
  g.lineTo(muzzle.x, muzzle.y);

  const stockCenterX = lerp(-r * 0.42, -r * 0.28, deploy);
  g.beginFill(0x4a3420, 0.96);
  drawRotatedRectOffset(g, stockCenterX, 0, r * 0.62, r * 0.38, a, kick);
  g.endFill();

  const receiverX = lerp(r * 0.04, r * 0.2, deploy);
  g.beginFill(0x32291f, 0.98);
  drawRotatedRectOffset(g, receiverX, 0, r * 0.72, r * 0.48, a, kick);
  g.endFill();

  g.beginFill(0xd8d0b0, 0.82);
  drawRotatedRectOffset(g, receiverX + r * 0.08, -r * 0.2, r * 0.56, r * 0.12, a, kick);
  g.endFill();

  const shroudX = lerp(r * 0.62, r * 1.08, deploy);
  const shroudW = lerp(r * 0.72, r * 1.22, deploy);
  g.beginFill(0x241d17, 0.98);
  drawRotatedRectOffset(g, shroudX, 0, shroudW, r * 0.24, a, kick);
  g.endFill();

  g.beginFill(0xd8d0b0, 0.72);
  const slotCount = deploy > 0.55 ? 4 : 3;
  for (let i = 0; i < slotCount; i += 1) {
    const t = slotCount === 1 ? 0.5 : i / (slotCount - 1);
    drawRotatedRectOffset(
      g,
      shroudX - shroudW * 0.3 + shroudW * 0.6 * t,
      0,
      r * 0.09,
      r * 0.14,
      a,
      kick,
    );
  }
  g.endFill();

  const muzzleBase = offsetPoint(polar(a, muzzleDist - r * 0.18), kick);
  g.lineStyle(2, 0xd8d0b0, 0.78);
  g.moveTo(muzzleBase.x - Math.sin(a) * r * 0.22, muzzleBase.y + Math.cos(a) * r * 0.22);
  g.lineTo(muzzleBase.x + Math.sin(a) * r * 0.22, muzzleBase.y - Math.cos(a) * r * 0.22);

  const grip = offsetPoint(polar(a + Math.PI, r * 0.02), kick);
  g.lineStyle(3, 0xd8d0b0, 0.86);
  g.moveTo(grip.x - Math.sin(a) * r * 0.34, grip.y + Math.cos(a) * r * 0.34);
  g.lineTo(grip.x + Math.sin(a) * r * 0.34, grip.y - Math.cos(a) * r * 0.34);

  if (deploy > 0.02) {
    const bipodRoot = offsetPoint(polar(a, lerp(r * 0.9, r * 1.72, deploy)), kick);
    const legLen = r * lerp(0.38, 1.0, deploy);
    const spread = lerp(0.32, 0.72, deploy);
    const left = {
      x: bipodRoot.x + Math.cos(a + spread) * legLen,
      y: bipodRoot.y + Math.sin(a + spread) * legLen,
    };
    const right = {
      x: bipodRoot.x + Math.cos(a - spread) * legLen,
      y: bipodRoot.y + Math.sin(a - spread) * legLen,
    };
    g.lineStyle(3, 0xd8d0b0, 0.9);
    g.moveTo(bipodRoot.x, bipodRoot.y);
    g.lineTo(left.x, left.y);
    g.moveTo(bipodRoot.x, bipodRoot.y);
    g.lineTo(right.x, right.y);
  }

  if (setup.barrel || deploy > 0.75) {
    g.beginFill(0x241d17, 0.96);
    drawRotatedRectOffset(g, muzzleDist, 0, r * 0.22, r * 0.16, a, kick);
    g.endFill();
  }
}

export function drawAntiTankGun(g, r, tint, facing, weaponFacing, setup, recoil) {
  const deploy = clamp01(setup.prongFactor);
  const a = angleLerp(facing, weaponFacing, smoothstep01(deploy));
  const barrelKick = recoilVector(a, recoil);
  const carriageKick = recoilVector(a, recoil * 0.12);
  const tireLength = r * 0.68;
  const tireWidth = r * 0.34;
  const wheelY = r * 0.42;
  const axleX = -r * 0.16;
  const trailRear = lerp(-r * 0.45, -r * 1.55, deploy);
  const trailSpread = lerp(r * 0.18, r * 0.72, deploy);
  const trailRootX = -r * 0.14;
  const muzzleX = r * 1.9;
  const breechX = -r * 0.28;

  g.lineStyle(4, 0x1a1712, 0.9);
  const axleL = rotatePoint(axleX, -wheelY, a);
  const axleR = rotatePoint(axleX, wheelY, a);
  g.moveTo(axleL.x + carriageKick.x, axleL.y + carriageKick.y);
  g.lineTo(axleR.x + carriageKick.x, axleR.y + carriageKick.y);

  const leftWheel = rotatePoint(axleX, -wheelY, a);
  const rightWheel = rotatePoint(axleX, wheelY, a);
  drawGunTire(g, leftWheel.x + carriageKick.x, leftWheel.y + carriageKick.y, tireLength, tireWidth, a);
  drawGunTire(g, rightWheel.x + carriageKick.x, rightWheel.y + carriageKick.y, tireLength, tireWidth, a);

  const shield = rotatePoint(r * 0.12, 0, a);
  g.beginFill(tint, 0.96);
  drawRotatedRectOffset(g, shield.x, shield.y, r * 0.46, r * 1.18, a, carriageKick);
  g.endFill();
  g.beginFill(0x1a1712, 0.28);
  drawRotatedRectOffset(
    g,
    shield.x + Math.cos(a) * r * 0.1,
    shield.y + Math.sin(a) * r * 0.1,
    r * 0.12,
    r,
    a,
    carriageKick,
  );
  g.endFill();

  const trailRoot = offsetPoint(rotatePoint(trailRootX, 0, a), carriageKick);
  const trailL = offsetPoint(rotatePoint(trailRear, -trailSpread, a), carriageKick);
  const trailR = offsetPoint(rotatePoint(trailRear, trailSpread, a), carriageKick);
  g.lineStyle(4, 0xd8d0b0, 0.9);
  g.moveTo(trailRoot.x, trailRoot.y);
  g.lineTo(trailL.x, trailL.y);
  g.moveTo(trailRoot.x, trailRoot.y);
  g.lineTo(trailR.x, trailR.y);
  if (deploy > 0.05) {
    const braceL = offsetPoint(
      rotatePoint(lerp(-r * 0.2, -r * 0.95, deploy), -trailSpread * 0.72, a),
      carriageKick,
    );
    const braceR = offsetPoint(
      rotatePoint(lerp(-r * 0.2, -r * 0.95, deploy), trailSpread * 0.72, a),
      carriageKick,
    );
    g.lineStyle(3, 0x2a2119, 0.96);
    g.moveTo(trailRoot.x, trailRoot.y);
    g.lineTo(braceL.x, braceL.y);
    g.moveTo(trailRoot.x, trailRoot.y);
    g.lineTo(braceR.x, braceR.y);
  }

  const breech = offsetPoint(rotatePoint(breechX, 0, a), barrelKick);
  const muzzle = offsetPoint(rotatePoint(muzzleX, 0, a), barrelKick);
  g.lineStyle(r * 0.22, 0x241d17, 0.98);
  g.moveTo(breech.x, breech.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.lineStyle(r * 0.07, 0xd8d0b0, 0.58);
  g.moveTo(breech.x + Math.sin(a) * r * 0.07, breech.y - Math.cos(a) * r * 0.07);
  g.lineTo(muzzle.x + Math.sin(a) * r * 0.07, muzzle.y - Math.cos(a) * r * 0.07);
  g.lineStyle(r * 0.1, 0xd8d0b0, 0.75);
  g.moveTo(muzzle.x - Math.cos(a) * r * 0.32, muzzle.y - Math.sin(a) * r * 0.32);
  g.lineTo(muzzle.x, muzzle.y);
  g.beginFill(0x3d3528, 0.98);
  drawRotatedRectOffset(g, breechX - r * 0.1, 0, r * 0.52, r * 0.42, a, barrelKick);
  g.endFill();
}

export function drawGunTire(g, cx, cy, length, width, a) {
  g.lineStyle(2.4, 0x17130f, 0.98);
  g.beginFill(0x26221b, 0.98);
  g.drawPolygon(orientedCapsulePolygon(cx, cy, length, width, a));
  g.endFill();

  g.lineStyle(1.5, 0xd8d0b0, 0.5);
  const treadOffset = width * 0.32;
  const treadInset = length * 0.26;
  const sideTreadLength = length - treadInset * 2;
  drawRotatedLine(g, cx, cy, -sideTreadLength / 2, -treadOffset, sideTreadLength / 2, -treadOffset, a);
  drawRotatedLine(g, cx, cy, -sideTreadLength / 2, treadOffset, sideTreadLength / 2, treadOffset, a);

  g.lineStyle(1.2, 0x4a4031, 0.9);
  for (let i = -1; i <= 1; i += 1) {
    const x = i * length * 0.2;
    drawRotatedLine(g, cx, cy, x, -width * 0.42, x, width * 0.42, a);
  }

  g.lineStyle(1.4, 0x17130f, 0.9);
  g.beginFill(0xd8d0b0, 0.76);
  g.drawCircle(cx, cy, width * 0.32);
  g.endFill();
}

export function orientedCapsulePolygon(cx, cy, length, width, a) {
  const radius = width / 2;
  const halfStraight = Math.max(0, length / 2 - radius);
  const points = [];
  const steps = 8;
  for (let i = 0; i <= steps; i += 1) {
    const t = -Math.PI / 2 + (Math.PI * i) / steps;
    const p = rotatePoint(halfStraight + Math.cos(t) * radius, Math.sin(t) * radius, a);
    points.push(cx + p.x, cy + p.y);
  }
  for (let i = 0; i <= steps; i += 1) {
    const t = Math.PI / 2 + (Math.PI * i) / steps;
    const p = rotatePoint(-halfStraight + Math.cos(t) * radius, Math.sin(t) * radius, a);
    points.push(cx + p.x, cy + p.y);
  }
  return points;
}

export function drawRotatedLine(g, cx, cy, x1, y1, x2, y2, a) {
  const p1 = rotatePoint(x1, y1, a);
  const p2 = rotatePoint(x2, y2, a);
  g.moveTo(cx + p1.x, cy + p1.y);
  g.lineTo(cx + p2.x, cy + p2.y);
}

export function drawFacingWedge(g, x, y, radius, facing, width, color, fillAlpha, lineAlpha, innerRadius = 0) {
  const half = width / 2;
  const start = facing - half;
  const end = facing + half;
  const inner = Math.max(0, Math.min(innerRadius || 0, radius));

  if (width >= Math.PI * 2) {
    g.lineStyle(1.5, color, lineAlpha);
    g.beginFill(color, fillAlpha);
    g.drawCircle(x, y, radius);
    if (inner > 0) {
      g.beginHole();
      g.drawCircle(x, y, inner);
      g.endHole();
    }
    g.endFill();
    return;
  }

  const sx = x + Math.cos(start) * radius;
  const sy = y + Math.sin(start) * radius;

  g.lineStyle(1.5, color, lineAlpha);
  g.beginFill(color, fillAlpha);
  if (inner > 0) {
    const exInner = x + Math.cos(end) * inner;
    const eyInner = y + Math.sin(end) * inner;
    g.moveTo(sx, sy);
    g.arc(x, y, radius, start, end);
    g.lineTo(exInner, eyInner);
    g.arc(x, y, inner, end, start, true);
    g.lineTo(sx, sy);
  } else {
    g.moveTo(x, y);
    g.lineTo(sx, sy);
    g.arc(x, y, radius, start, end);
    g.lineTo(x, y);
  }
  g.endFill();
}

export function finiteNumber(value) {
  return typeof value === "number" && Number.isFinite(value);
}

export function angleLerp(a, b, t) {
  let d = (b - a) % (Math.PI * 2);
  if (d > Math.PI) d -= Math.PI * 2;
  if (d < -Math.PI) d += Math.PI * 2;
  return a + d * clamp01(t);
}

export function angleDelta(from, to) {
  let d = (to - from) % (Math.PI * 2);
  if (d > Math.PI) d -= Math.PI * 2;
  if (d < -Math.PI) d += Math.PI * 2;
  return d;
}

export function positiveMod(value, modulus) {
  return ((value % modulus) + modulus) % modulus;
}

export function lightenColor(color, amount) {
  const r = Math.min(255, ((color >> 16) & 0xff) + Math.round(255 * amount));
  const g = Math.min(255, ((color >> 8) & 0xff) + Math.round(255 * amount));
  const b = Math.min(255, (color & 0xff) + Math.round(255 * amount));
  return (r << 16) | (g << 8) | b;
}

/** Normalize a possibly-negative-size rect to positive width/height. */
export function normRect(r) {
  const x = Math.min(r.x, r.x + r.w);
  const y = Math.min(r.y, r.y + r.h);
  return { x, y, w: Math.abs(r.w), h: Math.abs(r.h) };
}

/** Deterministic 0..1 noise for terrain dithering. */
export function hash2(x, y) {
  let n = (x * 374761393 + y * 668265263) | 0;
  n = (n ^ (n >>> 13)) | 0;
  n = Math.imul(n, 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
}

/**
 * Parse a CSS color string ("#rrggbb" or "#rgb") to a 0xRRGGBB int. Already-numeric
 * inputs pass through. Falls back to a neutral grey on anything unexpected.
 */
export function hexToInt(c) {
  if (typeof c === "number") return c;
  if (typeof c !== "string") return 0x9aa0a8;
  let s = c.trim().replace(/^#/, "");
  if (s.length === 3) s = s.split("").map((ch) => ch + ch).join("");
  const n = parseInt(s, 16);
  return Number.isNaN(n) ? 0x9aa0a8 : n;
}
