import { STATE, isBuilding } from "./protocol.js";
import { STATS, UPGRADES } from "./config.js";

export function selectedProducerBuildingsForUnit(ctx, unit, isOwn, factionTrainsOf) {
  return (ctx.selection || []).filter(
    (e) =>
      isOwn(ctx, e) &&
      isBuilding(e.kind) &&
      e.buildProgress == null &&
      factionTrainsOf(ctx, e.kind).includes(unit),
  );
}

export function selectedProducingBuildingsForKind(ctx, kind, isOwn) {
  return (ctx.selection || []).filter(
    (e) =>
      isOwn(ctx, e) &&
      e.kind === kind &&
      isBuilding(e.kind) &&
      e.buildProgress == null &&
      ((e.prodQueue ?? 0) > 0 || e.state === STATE.TRAIN),
  );
}

export function trainAvailability(ctx, unit, resources, isOwn) {
  const st = STATS[unit];
  if (!st) return "locked";
  if (requirementsOf(st).some((req) => !playerHasCompleteKind(ctx, req))) return "locked";
  if (!playerHasAnyCompleteKind(ctx, requirementsAnyOf(st))) return "locked";
  if (st.upgradeRequires && !(ctx.upgrades || []).includes(st.upgradeRequires)) return "locked";
  return affordable(st.cost, resources) && hasSupplyFor(st, resources) ? "ready" : "unaffordable";
}

export function trainDisabledReason(ctx, unit, resources, isOwn) {
  const st = STATS[unit];
  if (!st) return "";
  const missing = requirementsOf(st).find((req) => !playerHasCompleteKind(ctx, req));
  if (missing) return `Requires ${STATS[missing]?.label || missing}`;
  if (!playerHasAnyCompleteKind(ctx, requirementsAnyOf(st))) {
    return st.requiresAnyText || `Requires ${requirementsAnyOf(st)
      .map((req) => STATS[req]?.label || req)
      .join(" or ")}`;
  }
  if (st.upgradeRequires && !(ctx.upgrades || []).includes(st.upgradeRequires)) {
    return st.upgradeRequiresText ||
      `Requires ${UPGRADES[st.upgradeRequires]?.label || st.upgradeRequires}`;
  }
  if (!affordable(st.cost, resources)) return "Queue now; production waits for resources";
  if (!hasSupplyFor(st, resources)) return "Queue now; production waits for supply";
  return "";
}

export function researchAvailability(ctx, upgrade, resources, isOwn) {
  const def = UPGRADES[upgrade];
  if (!def) return "locked";
  if (def.replacesUpgrade && !(ctx.upgrades || []).includes(def.replacesUpgrade)) return "locked";
  if ((ctx.upgrades || []).includes(upgrade)) return "locked";
  if (selectedProducingBuildingsForKind(ctx, def.researchedAt, isOwn)
    .some((e) => e.prodUpgrade === upgrade)) return "locked";
  if (def.requiresUpgrade && !(ctx.upgrades || []).includes(def.requiresUpgrade)) return "locked";
  return affordable(def.cost, resources) ? "ready" : "unaffordable";
}

export function researchDisabledReason(ctx, upgrade, resources, isOwn) {
  const def = UPGRADES[upgrade];
  if (!def) return "";
  if (def.replacesUpgrade && !(ctx.upgrades || []).includes(def.replacesUpgrade)) {
    return def.requiresText || `Requires ${UPGRADES[def.replacesUpgrade]?.label || def.replacesUpgrade}`;
  }
  if ((ctx.upgrades || []).includes(upgrade)) return "Researched";
  if (selectedProducingBuildingsForKind(ctx, def.researchedAt, isOwn)
    .some((e) => e.prodUpgrade === upgrade)) return "Researching";
  if (def.requiresUpgrade && !(ctx.upgrades || []).includes(def.requiresUpgrade)) {
    return def.requiresText || `Requires ${UPGRADES[def.requiresUpgrade]?.label || def.requiresUpgrade}`;
  }
  if (!affordable(def.cost, resources)) return "Queue now; research waits for resources";
  return "";
}

export function trainLimitSignature(ctx, unit, isOwn) {
  return "";
}

export function researchSlotForUpgrade(buildingKind, upgrade, trains) {
  const replacedUpgrade = UPGRADES[upgrade]?.replacesUpgrade;
  if (replacedUpgrade) return researchSlotForUpgrade(buildingKind, replacedUpgrade, trains);
  const unitIndex = trains.findIndex((unit) => STATS[unit]?.upgradeRequires === upgrade);
  if (unitIndex >= 0) return unitIndex + 3;
  const researchIndex = researchesOf(buildingKind).indexOf(upgrade);
  if (researchIndex >= 0) return researchIndex;
  const afterTrainSlot = trains.findIndex((unit) => STATS[unit] == null);
  return afterTrainSlot >= 0 ? afterTrainSlot : trains.length;
}

export function trainSlotForUnit(buildingKind, unit, trains) {
  const slot = STATS[unit]?.trainSlot;
  if (Number.isInteger(slot)) return slot;
  const slotsByProducer = STATS[unit]?.trainSlots;
  if (slotsByProducer && Number.isInteger(slotsByProducer[buildingKind])) {
    return slotsByProducer[buildingKind];
  }
  return trains.indexOf(unit);
}

export function firstOpenCommandSlot(slots, preferredSlot, reservedSlot = -1) {
  const trySlot = (slot) =>
    slot >= 0 && slot < slots.length && slot !== reservedSlot && slots[slot] == null;
  if (trySlot(preferredSlot)) return preferredSlot;
  for (let slot = 0; slot < slots.length; slot++) {
    if (trySlot(slot)) return slot;
  }
  return -1;
}

function requirementsOf(definition) {
  if (!definition || !definition.requires) return [];
  return Array.isArray(definition.requires) ? definition.requires : [definition.requires];
}

function requirementsAnyOf(definition) {
  if (!definition || !definition.requiresAny) return [];
  return Array.isArray(definition.requiresAny) ? definition.requiresAny : [definition.requiresAny];
}

function playerHasCompleteKind(ctx, kind) {
  return typeof ctx.playerHasCompleteKind === "function" && ctx.playerHasCompleteKind(kind);
}

function playerHasAnyCompleteKind(ctx, kinds) {
  return kinds.length === 0 || kinds.some((kind) => playerHasCompleteKind(ctx, kind));
}

function affordable(cost, resources) {
  if (!cost) return true;
  const steel = resources.steel ?? 0;
  const oil = resources.oil ?? 0;
  return steel >= (cost.steel ?? 0) && oil >= (cost.oil ?? 0);
}

function hasSupplyFor(st, resources) {
  const supply = st?.supply ?? 0;
  if (!Number.isFinite(supply) || supply <= 0) return true;
  if (!Number.isFinite(resources.supplyCap)) return true;
  const used = resources.supplyUsed ?? 0;
  return used + supply <= resources.supplyCap;
}

function researchesOf(kind) {
  const st = STATS[kind];
  return (st && st.researches) || [];
}
