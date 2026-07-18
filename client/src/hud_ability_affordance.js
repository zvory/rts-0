function abilityStatus(entity, ability) {
  return Array.isArray(entity.abilities)
    ? entity.abilities.find((entry) => entry.ability === ability)
    : null;
}

export function abilityCooldownLeft(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.cooldownLeft === "number" ? projected.cooldownLeft : 0;
}

export function abilityRemainingUses(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.remainingUses === "number"
    ? projected.remainingUses
    : null;
}

export function abilityChargeRechargeLeft(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.chargeRechargeLeft === "number"
    ? projected.chargeRechargeLeft
    : 0;
}

export function abilityAutocastEnabled(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.autocastEnabled === "boolean"
    ? projected.autocastEnabled
    : false;
}

export function abilityActiveObjectId(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.activeObjectId === "number"
    ? projected.activeObjectId
    : null;
}

export function abilityUnitReady(entity, definition) {
  return abilityCooldownLeft(entity, definition.ability) === 0 &&
    abilityRemainingUses(entity, definition.ability) !== 0 &&
    !abilityLockoutActive(entity, definition.ability);
}

export function abilityUnitQueueAdmissible(entity, definition) {
  if (definition.queuePolicy === "notQueueable") return false;
  if (definition.queuePolicy !== "waitUntilReady") return abilityUnitReady(entity, definition);
  return abilityRemainingUses(entity, definition.ability) !== 0 &&
    !abilityLockoutActive(entity, definition.ability);
}

function abilityLockoutActive(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.lockoutUntilTick === "number";
}
