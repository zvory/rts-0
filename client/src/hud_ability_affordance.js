import { ABILITY } from "./protocol.js";
import { TICK_HZ, VECTOR_FIELD_MASTER_COOLDOWN_TICKS } from "./config.js";

function abilityStatus(entity, ability) {
  return Array.isArray(entity.abilities)
    ? entity.abilities.find((entry) => entry.ability === ability)
    : null;
}

export function abilityCooldownLeft(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.cooldownLeft === "number" ? projected.cooldownLeft : 0;
}

export function abilityCooldownClocks(carriers, definition, groupCooldownClocks) {
  const cooldownGroups = new Map();
  for (const entity of carriers) {
    const cooldown = abilityCooldownLeft(entity, definition.ability);
    let total = definition.cooldownTicks;
    if (definition.ability === ABILITY.THROW_SWORD && !entity.daoMaster) {
      total *= 2;
    } else if (definition.ability === ABILITY.VECTOR_FIELD && entity.daoMaster) {
      total = VECTOR_FIELD_MASTER_COOLDOWN_TICKS;
    }
    // Promotion preserves the Adept cooldown. The clock helper clamps values
    // above the current rank's duration while still grouping nearby cooldowns.
    if (!cooldownGroups.has(total)) cooldownGroups.set(total, []);
    cooldownGroups.get(total).push(cooldown);
  }
  const chargeRecharges = carriers.map((e) =>
    abilityChargeRechargeLeft(e, definition.ability),
  );
  return [
    ...Array.from(cooldownGroups, ([total, cooldowns]) =>
      groupCooldownClocks(cooldowns, total)).flat(),
    ...groupCooldownClocks(chargeRecharges, definition.chargeRechargeTicks),
  ];
}

/** Seconds until the next displayed cooldown/recharge ends, independent of clock grouping. */
export function abilityCooldownSeconds(carriers, definition) {
  let next = Infinity;
  for (const entity of carriers) {
    const cooldown = abilityCooldownLeft(entity, definition.ability);
    const recharge = abilityChargeRechargeLeft(entity, definition.ability);
    if (definition.cooldownTicks > 0 && cooldown > 0) next = Math.min(next, cooldown);
    if (definition.chargeRechargeTicks > 0 && recharge > 0) next = Math.min(next, recharge);
  }
  return Number.isFinite(next) ? Math.ceil(next / TICK_HZ) : 0;
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
  return definition.ability !== "charge" &&
    (!definition.ability.startsWith("evolve") || (!entity.dao && entity.insight >= 60)) &&
    abilityCooldownLeft(entity, definition.ability) === 0 &&
    (definition.ability === "barrage" || abilityRemainingUses(entity, definition.ability) !== 0) &&
    !abilityLockoutActive(entity, definition.ability);
}

export function abilityUnitQueueAdmissible(entity, definition) {
  if (definition.queuePolicy === "notQueueable") return false;
  if (definition.queuePolicy !== "waitUntilReady") return abilityUnitReady(entity, definition);
  return (definition.ability === "barrage" || abilityRemainingUses(entity, definition.ability) !== 0) &&
    !abilityLockoutActive(entity, definition.ability);
}

function abilityLockoutActive(entity, ability) {
  const projected = abilityStatus(entity, ability);
  return projected && typeof projected.lockoutUntilTick === "number";
}

function intentReadyIds(definition, affordance) {
  if (definition.targetMode !== "self" || affordance.readyUnits.length <= 1) {
    return affordance.readyIds;
  }
  const center = averagePosition(affordance.readyUnits);
  let best = affordance.readyUnits[0];
  let bestDist = distanceSqToCenter(best, center);
  for (const unit of affordance.readyUnits.slice(1)) {
    const dist = distanceSqToCenter(unit, center);
    if (dist < bestDist || (dist === bestDist && unit.id < best.id)) {
      best = unit;
      bestDist = dist;
    }
  }
  return [best.id];
}

export function intentAbilityIds(definition, affordance) {
  if (definition.ability.startsWith("evolve")) return affordance.carrierIds;
  if (definition.queuePolicy === "waitUntilReady") {
    return affordance.queueAdmissibleIds;
  }
  return intentReadyIds(definition, affordance);
}

function averagePosition(units) {
  let x = 0;
  let y = 0;
  let count = 0;
  for (const unit of units) {
    if (!Number.isFinite(unit.x) || !Number.isFinite(unit.y)) continue;
    x += unit.x;
    y += unit.y;
    count++;
  }
  return count > 0 ? { x: x / count, y: y / count } : null;
}

function distanceSqToCenter(unit, center) {
  if (!center || !Number.isFinite(unit.x) || !Number.isFinite(unit.y)) return 0;
  const dx = unit.x - center.x;
  const dy = unit.y - center.y;
  return dx * dx + dy * dy;
}
