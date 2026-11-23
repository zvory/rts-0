import { KIND, SETUP } from "../protocol.js";

// Click tolerance is independent of simulation bodies, marquee selection and art scale.
function radiusScale(entity) {
  switch (entity.kind) {
    case KIND.WORKER:
    case KIND.RIFLEMAN:
    case KIND.PANZERFAUST:
    case KIND.MACHINE_GUNNER: return 1.5;
    case KIND.WARRIOR: return 0.75;
    case KIND.LIGHTNESS_ADEPT:
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
    if (scale !== 1) {
      clickFootprint = Object.freeze({ kind: "circle", radiusPx: footprint.radiusPx * scale });
      if (scale < 1) directClickFootprint = clickFootprint;
    }
  } else if (entity.kind === KIND.ANTI_TANK_GUN) {
    // Outward-rounded visible atlas bounds, excluding shadows. The articulated
    // trails widen on deployment; transitions use the union of both end poses.
    const packed = entity.setupState === SETUP.PACKED;
    const halfWidth = packed ? 14 : 23;
    const facing = Number.isFinite(entity.weaponFacing) ? entity.weaponFacing : entity.facing;
    clickFootprint = orientedBounds(entity, -44, -halfWidth, 23, halfWidth, facing);
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
