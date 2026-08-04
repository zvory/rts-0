import { ABILITY, ORDER_STAGE, UPGRADE, isResource } from "./protocol.js";

export function ownOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    if (typeof controlPolicy.isCommandOwner === "function") {
      return controlPolicy.isCommandOwner(owner, state);
    }
    return controlPolicy.canControlOwner(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

export function allyOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    return typeof controlPolicy.isCommandAllyOwner === "function"
      ? controlPolicy.isCommandAllyOwner(owner, state)
      : false;
  }
  return typeof state?.isAllyOwner === "function" && state.isAllyOwner(owner);
}

export function commandFeedbackOwner(state, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    const owner = typeof controlPolicy.feedbackOwner === "function"
      ? controlPolicy.feedbackOwner(state)
      : typeof controlPolicy.issueAsOwnerForSelection === "function"
        ? controlPolicy.issueAsOwnerForSelection(state.selectedEntities?.() || [])
        : null;
    const ownerId = Number(owner);
    return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
  }
  const ownerId = Number(state?.playerId);
  return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
}

export function abilityTargetRadiusTiles(definition, ability, state, controlPolicy = null) {
  const baseRadius = definition?.radiusTiles || 0;
  if (ability === ABILITY.SMOKE && commandUpgrades(state, controlPolicy).includes(UPGRADE.SMOKE_PLUS)) {
    return definition?.upgradedRadiusTiles || baseRadius;
  }
  return baseRadius;
}

export function commandUpgrades(state, controlPolicy = null) {
  if (typeof controlPolicy?.commandUpgrades === "function") {
    const upgrades = controlPolicy.commandUpgrades(state);
    return Array.isArray(upgrades) ? upgrades : [];
  }
  return Array.isArray(state?.upgrades) ? state.upgrades : [];
}

export function commandTargetsMatch(left, right) {
  if (left === right) return true;
  if (!left || !right || typeof left !== "object" || typeof right !== "object") return false;
  return left.kind === right.kind && left.ability === right.ability;
}

export function resourceRallyTargetAt(map, x, y) {
  const radius = Math.max(0, Number(map?.tileSize) || 0) * 0.5;
  const radius2 = radius * radius;
  let best = null;
  let bestDist2 = Infinity;
  for (const node of map?.resources || []) {
    if (node?.remaining === 0 || !isResource(node?.kind)) continue;
    const dx = Number(node.x) - x;
    const dy = Number(node.y) - y;
    const dist2 = dx * dx + dy * dy;
    if (!Number.isFinite(dist2) || dist2 > radius2) continue;
    if (dist2 < bestDist2 || (dist2 === bestDist2 && node.id < best?.id)) {
      best = node;
      bestDist2 = dist2;
    }
  }
  return best;
}

export function supportWeaponSetupPreviewEntity(entity) {
  const origin = latestMovementOrderPlanPoint(entity);
  return origin ? { ...entity, x: origin.x, y: origin.y } : entity;
}

export function plannedEntityForIntent(intent, entity) {
  return typeof intent?.entityWithPlannedOrder === "function"
    ? intent.entityWithPlannedOrder(entity)
    : entity;
}

function latestMovementOrderPlanPoint(entity) {
  if (!Array.isArray(entity?.orderPlan)) return null;
  let origin = null;
  for (const marker of entity.orderPlan) {
    if (
      (marker?.kind === ORDER_STAGE.MOVE || marker?.kind === ORDER_STAGE.ATTACK_MOVE) &&
      Number.isFinite(marker.x) &&
      Number.isFinite(marker.y)
    ) {
      origin = { x: marker.x, y: marker.y };
    }
  }
  return origin;
}
