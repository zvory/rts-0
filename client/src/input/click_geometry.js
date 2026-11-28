import { KIND, SETUP } from "../protocol.js";

// Click tolerance is independent of simulation bodies, marquee selection and art scale.
function radiusScale(entity) {
  switch (entity.kind) {
    case KIND.WORKER:
    case KIND.RIFLEMAN:
    case KIND.PANZERFAUST:
    case KIND.MACHINE_GUNNER: return 1.5;
    case KIND.BOW_NOVICE:
    case KIND.WARRIOR: return 0.75;
    case KIND.WIND_ADEPT:
    case KIND.MAGIC_ADEPT: return entity.daoMaster ? 1 : 0.75;
    case KIND.WAR_ADEPT: return entity.daoMaster ? 1.5 : 1.25;
    default: return 1;
  }
}

export function clickGeometry(entity, footprint) {
  let clickFootprint = footprint;
  let directClickFootprint = footprint;
  let scale = 1;
  if (footprint.kind === "circle") {
    scale = radiusScale(entity);
    if (entity.kind === KIND.WARRIOR || entity.kind === KIND.BOW_NOVICE) {
      // Preserve the reviewed art-based click area independently of collision size.
      clickFootprint = Object.freeze({ kind: "circle", radiusPx: 19.5 });
      directClickFootprint = clickFootprint;
    } else if (scale !== 1) {
      clickFootprint = Object.freeze({ kind: "circle", radiusPx: footprint.radiusPx * scale });
      if (scale < 1) directClickFootprint = clickFootprint;
    }
  } else if (entity.kind === KIND.ANTI_TANK_GUN) {
    // Outward-rounded visible atlas bounds, excluding shadows. The articulated
    // trails widen on deployment; transitions cover the full facing sweep.
    const transitional = entity.setupState === SETUP.SETTING_UP || entity.setupState === SETUP.TEARING_DOWN;
    const packed = entity.setupState !== SETUP.DEPLOYED && !transitional;
    const halfWidth = packed ? 14 : 23;
    const facing = Number.isFinite(entity.facing) ? entity.facing : 0;
    const weaponFacing = Number.isFinite(entity.weaponFacing) ? entity.weaponFacing : facing;
    clickFootprint = transitional
      ? sweptBounds(entity, facing, weaponFacing)
      : orientedBounds(entity, -44, -halfWidth, 23, halfWidth,
        entity.setupState === SETUP.DEPLOYED ? weaponFacing : facing);
    directClickFootprint = clickFootprint;
  } else if (entity.kind === KIND.TANK || entity.kind === KIND.ROCKET_LAUNCHER) {
    // Scale only the local forward coordinate, preserving lateral precision.
    const facing = Number.isFinite(entity.facing) ? entity.facing : 0;
    const cos = Math.cos(facing), sin = Math.sin(facing);
    clickFootprint = Object.freeze({
      kind: "polygon",
      points: Object.freeze(footprint.points.map((point) => {
        const dx = point.x - entity.x, dy = point.y - entity.y;
        const forward = (dx * cos + dy * sin) * 1.1;
        const side = -dx * sin + dy * cos;
        return Object.freeze({ x: entity.x + forward * cos - side * sin,
          y: entity.y + forward * sin + side * cos });
      })),
    });
  }
  return {
    clickFootprint,
    directClickFootprint,
    clickMinScreenRadiusCssPx: 6 * scale,
    directClickMinScreenRadiusCssPx: 6 * Math.min(1, scale),
  };
}

function sweptBounds(entity, facing, weaponFacing) {
  // Setup animation runs on the renderer's clock. Conservatively cover every
  // intermediate shortest-arc orientation without consulting mutable rig state.
  let delta = (weaponFacing - facing) % (Math.PI * 2);
  if (delta > Math.PI) delta -= Math.PI * 2;
  if (delta < -Math.PI) delta += Math.PI * 2;
  const low = Math.min(0, delta), high = Math.max(0, delta);
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const [x, y] of [[-44, -23], [23, -23], [23, 23], [-44, 23]]) {
    const angles = [low, high];
    const phase = Math.atan2(y, x);
    // Coordinate extrema occur at multiples of a quarter turn.
    const quarterTurn = Math.PI / 2;
    for (let k = Math.ceil((low + phase) / quarterTurn);
      k <= Math.floor((high + phase) / quarterTurn); k += 1) {
      angles.push(k * quarterTurn - phase);
    }
    for (const angle of angles) {
      const px = x * Math.cos(angle) - y * Math.sin(angle);
      const py = x * Math.sin(angle) + y * Math.cos(angle);
      minX = Math.min(minX, px); maxX = Math.max(maxX, px);
      minY = Math.min(minY, py); maxY = Math.max(maxY, py);
    }
  }
  return orientedBounds(entity, minX, minY, maxX, maxY, facing);
}

function orientedBounds(entity, minX, minY, maxX, maxY, facingValue) {
  const facing = Number.isFinite(facingValue) ? facingValue : 0;
  const cos = Math.cos(facing), sin = Math.sin(facing);
  return Object.freeze({
    kind: "polygon",
    points: Object.freeze([[minX, minY], [maxX, minY], [maxX, maxY], [minX, maxY]]
      .map(([x, y]) => Object.freeze({
        x: entity.x + x * cos - y * sin, y: entity.y + x * sin + y * cos,
      }))),
  });
}
