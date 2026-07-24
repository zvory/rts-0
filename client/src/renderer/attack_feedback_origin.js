import { STATS } from "../config.js";
import { KIND, WEAPON_KIND, isBuilding } from "../protocol.js";
import { isVehicleBodyKind } from "./shared.js";
import { transformedRigAnchorPoint } from "./rigs/animation.js";
import { liveRigDefinitionFor } from "./rigs/live_routing.js";

export function attackFeedbackKindForWeapon(attackerKind, weaponKind) {
  switch (weaponKind) {
    case WEAPON_KIND.WORKER_TOOLS:
      return KIND.WORKER;
    case WEAPON_KIND.GOLEM_FISTS:
      return KIND.GOLEM;
    case WEAPON_KIND.RIFLEMAN_RIFLE:
      return KIND.RIFLEMAN;
    case WEAPON_KIND.MACHINE_GUNNER_MG:
      return KIND.MACHINE_GUNNER;
    case WEAPON_KIND.SCOUT_CAR_MG:
      return KIND.SCOUT_CAR;
    case WEAPON_KIND.ANTI_TANK_GUN:
      return KIND.ANTI_TANK_GUN;
    case WEAPON_KIND.MORTAR_TEAM_MORTAR:
      return KIND.MORTAR_TEAM;
    case WEAPON_KIND.ARTILLERY_GUN:
      return KIND.ARTILLERY;
    case WEAPON_KIND.TANK_CANNON:
      return KIND.TANK;
    case WEAPON_KIND.TANK_COAX:
      return KIND.MACHINE_GUNNER;
    default:
      return attackerKind;
  }
}

export function attackFeedbackOriginForWeapon({
  definitionsByKind,
  attacker,
  weaponKind,
  targetPos,
  state,
  now,
  map,
  stat,
}) {
  const originKind = attackFeedbackOriginKindForWeapon(attacker.kind, weaponKind);
  const facing = attackFeedbackFacing(attacker, targetPos);
  const fallbackOrigin = fallbackAttackFeedbackOrigin(attacker, originKind, stat, facing, map);
  const rigOrigin = attackFeedbackRigOrigin(definitionsByKind, attacker, weaponKind, state, now, map);
  return rigOrigin ?? fallbackOrigin;
}

function attackFeedbackOriginKindForWeapon(attackerKind, weaponKind) {
  if (weaponKind === WEAPON_KIND.TANK_COAX) return attackerKind;
  return attackFeedbackKindForWeapon(attackerKind, weaponKind);
}

function attackFeedbackFacing(attacker, targetPos) {
  if (isVehicleBodyKind(attacker.kind) && typeof attacker.weaponFacing === "number") {
    return attacker.weaponFacing;
  }
  if (typeof attacker.facing === "number") return attacker.facing;
  if (targetPos) return Math.atan2(targetPos.y - attacker.y, targetPos.x - attacker.x);
  return 0;
}

function attackFeedbackRigOrigin(definitionsByKind, attacker, weaponKind, state, now, map) {
  const anchorName = attackFeedbackAnchorNameForWeapon(attacker.kind, weaponKind);
  if (!anchorName) return null;
  const definition = liveRigDefinitionFor(definitionsByKind, attacker.kind);
  return transformedRigAnchorPoint(definition, attacker, anchorName, { state, now, map });
}

function attackFeedbackAnchorNameForWeapon(attackerKind, weaponKind) {
  if (attackerKind === KIND.TANK) {
    return weaponKind === WEAPON_KIND.TANK_COAX ? "coaxMuzzle" : "muzzle";
  }
  if (attackerKind === KIND.ARTILLERY && weaponKind === WEAPON_KIND.ARTILLERY_GUN) {
    return "muzzle";
  }
  return null;
}

function fallbackAttackFeedbackOrigin(attacker, originKind, stat, facing, map) {
  const originStat = STATS[originKind] || STATS[attacker.kind] || stat;
  const reach = isBuilding(originKind)
    ? Math.max(originStat.footW || 2, originStat.footH || 2) * ((map && map.tileSize) || 32) * 0.5
    : originKind === KIND.ANTI_TANK_GUN
      ? (originStat.size || 9) * 1.9
    : (originStat.size || 9) * 1.1;
  return {
    x: attacker.x + Math.cos(facing) * reach,
    y: attacker.y + Math.sin(facing) * reach,
  };
}
