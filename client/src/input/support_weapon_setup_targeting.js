import { KIND } from "../protocol.js";

export const SETUP_CONVERGENCE_END_TILES = 14;
export const SETUP_PARALLEL_DISTANCE_TILES = 20;
export const SETUP_FULL_FAN_DISTANCE_TILES = 25;
export const SETUP_FULL_FAN_HALF_ANGLE_RAD = Math.PI / 4;
const SETUP_RAY_LENGTH_TILES = 1000;

export function supportWeaponSetupTargets(weapons, rawTarget, tileSize = 32) {
  const list = Array.isArray(weapons) ? weapons : [];
  if (!finitePoint(rawTarget) || !(tileSize > 0)) return literalTargets(list, rawTarget);
  const guns = list.filter((weapon) =>
    weapon?.kind === KIND.ANTI_TANK_GUN &&
    Number.isFinite(weapon.id) &&
    finitePoint(weapon));
  if (guns.length === 0) return literalTargets(list, rawTarget);

  const centroid = guns.reduce((sum, gun) => ({
    x: sum.x + gun.x,
    y: sum.y + gun.y,
  }), { x: 0, y: 0 });
  centroid.x /= guns.length;
  centroid.y /= guns.length;

  const dx = rawTarget.x - centroid.x;
  const dy = rawTarget.y - centroid.y;
  const distance = Math.hypot(dx, dy);
  const convergeEnd = SETUP_CONVERGENCE_END_TILES * tileSize;
  if (!(distance > convergeEnd)) return literalTargets(list, rawTarget);

  const forwardFacing = Math.atan2(dy, dx);
  const convergence = smoothstep(remap01(
    distance,
    convergeEnd,
    SETUP_PARALLEL_DISTANCE_TILES * tileSize,
  ));
  const fan = smoothstep(remap01(
    distance,
    SETUP_PARALLEL_DISTANCE_TILES * tileSize,
    SETUP_FULL_FAN_DISTANCE_TILES * tileSize,
  ));
  const fanAngleById = rankedFanAngles(guns, forwardFacing);
  const rayLength = SETUP_RAY_LENGTH_TILES * tileSize;

  return list.map((weapon) => {
    if (weapon?.kind !== KIND.ANTI_TANK_GUN || !finitePoint(weapon)) {
      return { id: weapon?.id, x: rawTarget.x, y: rawTarget.y };
    }
    const literalFacing = Math.atan2(rawTarget.y - weapon.y, rawTarget.x - weapon.x);
    const parallelFacing = lerpAngle(literalFacing, forwardFacing, convergence);
    const facing = parallelFacing + (fanAngleById.get(weapon.id) || 0) * fan;
    return {
      id: weapon.id,
      x: weapon.x + Math.cos(facing) * rayLength,
      y: weapon.y + Math.sin(facing) * rayLength,
      facing,
    };
  });
}

export function supportWeaponSetupTargetGroups(targets) {
  const groups = new Map();
  for (const target of Array.isArray(targets) ? targets : []) {
    if (!Number.isFinite(target?.id) || !finitePoint(target)) continue;
    const key = `${target.x},${target.y}`;
    let group = groups.get(key);
    if (!group) {
      group = { units: [], x: target.x, y: target.y };
      groups.set(key, group);
    }
    group.units.push(target.id);
  }
  return [...groups.values()];
}

export function supportWeaponsWithSetupTargets(weapons, rawTarget, tileSize = 32) {
  const targets = supportWeaponSetupTargets(weapons, rawTarget, tileSize);
  const byId = new Map(targets.map((target) => [target.id, target]));
  return (Array.isArray(weapons) ? weapons : []).map((weapon) => {
    const target = byId.get(weapon?.id);
    return target ? { ...weapon, setupAimX: target.x, setupAimY: target.y } : weapon;
  });
}

function rankedFanAngles(guns, forwardFacing) {
  const rightX = -Math.sin(forwardFacing);
  const rightY = Math.cos(forwardFacing);
  const ranked = guns.slice().sort((left, right) => {
    const leftProjection = left.x * rightX + left.y * rightY;
    const rightProjection = right.x * rightX + right.y * rightY;
    return leftProjection - rightProjection || left.id - right.id;
  });
  const angles = new Map();
  for (let index = 0; index < ranked.length; index += 1) {
    const normalized = ranked.length === 1 ? 0 : (index / (ranked.length - 1)) * 2 - 1;
    angles.set(ranked[index].id, normalized * SETUP_FULL_FAN_HALF_ANGLE_RAD);
  }
  return angles;
}

function literalTargets(weapons, rawTarget) {
  return weapons.map((weapon) => ({ id: weapon?.id, x: rawTarget?.x, y: rawTarget?.y }));
}

function remap01(value, start, end) {
  if (!(end > start)) return value >= end ? 1 : 0;
  return Math.max(0, Math.min(1, (value - start) / (end - start)));
}

function smoothstep(value) {
  return value * value * (3 - 2 * value);
}

function lerpAngle(from, to, amount) {
  const delta = Math.atan2(Math.sin(to - from), Math.cos(to - from));
  return from + delta * amount;
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}
