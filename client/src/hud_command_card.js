import { ABILITY, KIND, SETUP, STATE, isBuilding, isUnit } from "./protocol.js";
import {
  ABILITIES,
  STATS,
  UPGRADES,
  WORKER_BUILDABLE,
} from "./config.js";

// Command-card hotkeys follow the keyboard grid (3 columns):
//   Q W E
//   A S D
//   Z X C
export const GRID_HOTKEYS = Object.freeze(["Q", "W", "E", "A", "S", "D", "Z", "X", "C"]);

export function buildCommandCardDescriptors(ctx) {
  if (ctx?.spectator) return { kind: "spectator", signature: "spectator", slots: [] };

  const selection = ctx?.selection || [];
  const primary = commandSubject(ctx, selection);
  if (!primary) return { kind: "empty", signature: "empty", slots: new Array(9).fill(null) };

  if (ctx.commandCardMode === "workerBuild" && workerOnlySelection(ctx, selection)) {
    return buildWorkerBuildCard(ctx);
  }
  if (selectedOwnUnits(ctx, selection).length > 0) {
    return buildUnitCard(ctx, selection);
  }
  return buildTrainCard(ctx, primary);
}

export function commandSubject(ctx, selection) {
  for (const e of selection || []) {
    if (!isOwn(ctx, e)) continue;
    if (isUnit(e.kind)) return e;
    if (isBuilding(e.kind) && (trainsOf(e.kind).length > 0 || researchesOf(e.kind).length > 0)) return e;
  }
  return null;
}

export function buildWorkerBuildCard(ctx) {
  const resources = resourcesOf(ctx);
  const slots = [];
  const sigParts = [];
  let idx = 0;
  for (const kind of WORKER_BUILDABLE) {
    const st = STATS[kind];
    if (!st) continue;
    const availability = buildAvailability(ctx, kind, resources);
    sigParts.push(`${kind}:${availability}`);
    slots.push({
      id: `build:${kind}`,
      kind: "button",
      action: "build",
      intent: { type: "beginPlacement", building: kind },
      icon: st.icon,
      label: st.label,
      hotkey: GRID_HOTKEYS[idx++],
      cost: st.cost,
      enabled: availability === "ready",
      unaffordable: availability === "unaffordable",
      title: buildDisabledReason(ctx, kind, resources),
      tooltipKind: kind,
    });
  }
  while (slots.length < 8) slots.push(null);
  slots.push({
    id: "worker:return",
    kind: "button",
    action: "returnWorker",
    intent: { type: "closeCommandCardMenu" },
    icon: "RTN",
    label: "Worker",
    hotkey: GRID_HOTKEYS[8],
    enabled: true,
    title: "Return to worker commands",
  });
  return {
    kind: "workerBuild",
    signature: `build|${sigParts.join(",")}`,
    slots,
  };
}

export function buildUnitCard(ctx, selection) {
  const ownUnits = selectedOwnUnits(ctx, selection);
  const unitIds = ownUnits.map((e) => e.id);
  const setupGunIds = ownUnits
    .filter((e) => e.kind === KIND.AT_TEAM || e.kind === KIND.ARTILLERY)
    .map((e) => e.id);
  const abilityAffordances = selectedAbilityAffordances(ctx, selection);
  const hasArmyUnit = ownUnits.some((e) => e.kind !== KIND.WORKER);
  const workerSelected = !hasArmyUnit && ownUnits.some((e) => e.kind === KIND.WORKER);
  const signature =
    `units|${unitIds.join(".")}|target:${commandTargetSig(ctx.commandTarget)}|` +
    `|setup:${setupGunIds.join(".")}|` +
    `|abilities:${abilityAffordances.map((affordance) =>
      `${affordance.definition.ability}:${affordance.unlocked ? 1 : 0}:${affordance.affordable ? 1 : 0}:` +
      `${affordance.depletedCount}:${affordance.setupBlockedCount}:` +
      `${affordance.readyIds.join(".")}:` +
      `${affordance.autocastEnabledIds.join(".")}:` +
      `${affordance.cooldownClocks.map((group) => group.count).join(",")}`,
    ).join("|")}|` +
    (workerSelected ? "worker-main" : "no-build");

  if (workerSelected) {
    return {
      kind: "unit",
      signature,
      abilityAffordances,
      slots: [
        moveDescriptor(ctx, unitIds, 0),
        null,
        null,
        attackDescriptor(ctx, unitIds, 3),
        holdDescriptor(unitIds, 4),
        null,
        {
          id: "worker:build-menu",
          kind: "button",
          action: "openWorkerBuildMenu",
          intent: { type: "openWorkerBuildMenu" },
          icon: "BLD",
          label: "Build",
          title: "Open worker build menu",
          hotkey: GRID_HOTKEYS[6],
          enabled: unitIds.length > 0,
        },
        null,
        null,
      ],
    };
  }

  const slots = new Array(9).fill(null);
  slots[0] = moveDescriptor(ctx, unitIds, 0);
  slots[3] = attackDescriptor(ctx, unitIds, 3);
  slots[4] = holdDescriptor(unitIds, 4);

  let sequentialSlot = 6;
  const claimSlot = (preferred) => {
    if (preferred != null && preferred >= 0 && preferred < 9 && slots[preferred] == null) {
      return preferred;
    }
    while (sequentialSlot < 9 && slots[sequentialSlot] != null) sequentialSlot++;
    return sequentialSlot < 9 ? sequentialSlot++ : -1;
  };

  for (const affordance of abilityAffordances) {
    if (!affordance.unlocked) continue;
    const definition = affordance.definition;
    const readyCount = affordance.readyIds.length;
    const showReadyCount = readyCount < affordance.carrierIds.length;
    const preferred = definition.hotkey ? GRID_HOTKEYS.indexOf(definition.hotkey) : -1;
    const slot = claimSlot(preferred);
    if (slot < 0) continue;
    slots[slot] = {
      id: `ability:${definition.ability}`,
      kind: "button",
      action: "ability",
      intent: {
        type: "ability",
        ability: definition.ability,
        targetMode: definition.targetMode,
        readyIds: affordance.readyIds,
      },
      icon: definition.icon,
      label: definition.label,
      title: abilityDisabledReason(ctx, affordance),
      ability: definition.ability,
      hotkey: GRID_HOTKEYS[slot],
      enabled: readyCount > 0 && affordance.affordable,
      unaffordable: readyCount > 0 && !affordance.affordable,
      countBadge: showReadyCount ? `${readyCount}` : "",
      cooldownClocks: affordance.cooldownClocks,
      cost: definition.cost,
      cls: [
        abilityTargetActive(ctx.commandTarget, definition.ability) ? "active" : "",
        affordance.autocastEnabledIds.length > 0 ? "autocast-enabled" : "",
      ].filter(Boolean).join(" "),
      onUnavailableIntent: { type: "playNotEnough", cost: definition.cost },
      contextIntent: definition.ability === ABILITY.MORTAR_FIRE
        ? {
            type: "setAutocast",
            ability: definition.ability,
            unitIds: affordance.carrierIds,
            enabled: false,
          }
        : null,
    };
  }

  if (setupGunIds.length > 0) {
    const setupSlot = claimSlot(null);
    if (setupSlot >= 0) {
      slots[setupSlot] = {
        id: "unit:setup",
        kind: "button",
        action: "setupAtGuns",
        intent: { type: "beginCommandTarget", target: "setupAtGuns" },
        icon: "SET",
        label: "Set Up",
        title: "Set up selected support weapons toward a target point",
        hotkey: GRID_HOTKEYS[setupSlot],
        enabled: true,
        cls: ctx.commandTarget === "setupAtGuns" ? "active" : "",
      };
    }
  }

  return { kind: "unit", signature, abilityAffordances, slots };
}

export function buildTrainCard(ctx, building) {
  const resources = resourcesOf(ctx);
  const trains = trainsOf(building.kind);
  const researches = availableResearchesOf(ctx, building.kind);
  const producingBuildings = selectedProducingBuildingsForKind(ctx, building.kind);
  const cancelSlot = 8;
  const signature =
    `train|${building.id}|` +
    trains.map((unit) => {
      const producerIds = selectedProducerBuildingsForUnit(ctx, unit).map((e) => e.id).join(".");
      return `${unit}:${trainAvailability(ctx, unit, resources)}:${producerIds}`;
    }).join(",") +
    `|research:` +
    researches.map((upgrade) => `${upgrade}:${researchAvailability(ctx, upgrade, resources)}`).join(",") +
    `|cancel:${producingBuildings.map((e) => e.id).join(".")}`;

  const slots = new Array(9).fill(null);
  for (const unit of trains) {
    const st = STATS[unit];
    if (!st) continue;
    const slot = slots.findIndex((entry, idx) => entry == null && idx !== cancelSlot);
    if (slot < 0) break;
    const availability = trainAvailability(ctx, unit, resources);
    slots[slot] = {
      id: `train:${unit}`,
      kind: "button",
      action: "train",
      intent: { type: "train", unit },
      icon: st.icon,
      label: st.label,
      hotkey: GRID_HOTKEYS[slot],
      cost: st.cost,
      enabled: availability === "ready",
      unaffordable: availability === "unaffordable",
      title: trainDisabledReason(ctx, unit, resources),
      tooltipKind: unit,
      repeatable: true,
      onUnavailableIntent: { type: "playNotEnough", cost: st.cost },
    };
  }
  for (const upgrade of researches) {
    const def = UPGRADES[upgrade];
    if (!def) continue;
    const preferredSlot = researchSlotForUpgrade(building.kind, upgrade, trains);
    const slot = firstOpenCommandSlot(slots, preferredSlot, cancelSlot);
    if (slot < 0) continue;
    const availability = researchAvailability(ctx, upgrade, resources);
    slots[slot] = {
      id: `research:${upgrade}`,
      kind: "button",
      action: "research",
      intent: { type: "research", upgrade },
      icon: def.icon,
      label: def.label,
      hotkey: GRID_HOTKEYS[slot],
      cost: def.cost,
      enabled: availability === "ready",
      unaffordable: availability === "unaffordable",
      title: researchDisabledReason(ctx, upgrade, resources),
      tooltipUpgrade: upgrade,
      repeatable: false,
      onUnavailableIntent: { type: "playNotEnough", cost: def.cost },
    };
  }

  if (producingBuildings.length > 0) {
    slots[cancelSlot] = {
      id: `cancel:${building.kind}`,
      kind: "button",
      action: "cancel",
      intent: { type: "cancelProduction", buildingKind: building.kind },
      icon: "CNCL",
      label: "Cancel",
      hotkey: GRID_HOTKEYS[cancelSlot],
      enabled: true,
      cls: "cancel",
      title: "Cancel latest queued production",
      repeatable: true,
    };
  }

  return { kind: "train", signature, slots };
}

function moveDescriptor(ctx, unitIds, slot) {
  return {
    id: "unit:move",
    kind: "button",
    action: "move",
    intent: { type: "beginCommandTarget", target: "move" },
    icon: "MV",
    label: "Move",
    title: "Move to a target point",
    hotkey: GRID_HOTKEYS[slot],
    enabled: unitIds.length > 0,
    cls: ctx.commandTarget === "move" ? "active" : "",
  };
}

function attackDescriptor(ctx, unitIds, slot) {
  return {
    id: "unit:attack",
    kind: "button",
    action: "attack",
    intent: { type: "beginCommandTarget", target: "attack" },
    icon: "AT",
    label: "Attack",
    title: "Attack a target or attack-move to a point",
    hotkey: GRID_HOTKEYS[slot],
    enabled: unitIds.length > 0,
    cls: ctx.commandTarget === "attack" ? "active" : "",
  };
}

function holdDescriptor(unitIds, slot) {
  return {
    id: "unit:hold",
    kind: "button",
    action: "stop",
    intent: { type: "stop", unitIds },
    icon: "ST",
    label: "Stop",
    title: "Stop selected units",
    hotkey: GRID_HOTKEYS[slot],
    enabled: unitIds.length > 0,
  };
}

export function selectedAbilityAffordances(ctx, selection) {
  const ownUnits = selectedOwnUnits(ctx, selection);
  const resources = resourcesOf(ctx);
  return Object.values(ABILITIES)
    .map((definition) => {
      const carriers = ownUnits.filter((e) => definition.carriers.includes(e.kind));
      if (carriers.length === 0) return null;
      const unlocked = abilityUnlocked(ctx, definition);
      const canAfford = affordable(definition.cost, resources);
      const readyUnits = carriers.filter((e) => abilityUnitReady(e, definition));
      const cooldowns = carriers.map((e) =>
        abilityCooldownLeft(e, definition.ability),
      );
      const depletedCount = carriers.filter(
        (e) => abilityRemainingUses(e, definition.ability) === 0,
      ).length;
      const setupBlockedCount = carriers.filter((e) =>
        abilityRequiresSetup(e, definition),
      ).length;
      const autocastEnabledIds = carriers
        .filter((e) => abilityAutocastEnabled(e, definition.ability))
        .map((e) => e.id);
      return {
        definition,
        unlocked,
        affordable: canAfford,
        depletedCount,
        setupBlockedCount,
        carrierIds: carriers.map((e) => e.id),
        readyIds: readyUnits.map((e) => e.id),
        autocastEnabledIds,
        cooldownClocks: ctx.groupCooldownClocks(cooldowns, definition.cooldownTicks),
      };
    })
    .filter(Boolean);
}

function isOwn(ctx, e) {
  return e && e.owner === ctx.playerId;
}

function selectedOwnUnits(ctx, selection) {
  return (selection || []).filter((e) => isOwn(ctx, e) && isUnit(e.kind));
}

function workerOnlySelection(ctx, selection) {
  const ownUnits = selectedOwnUnits(ctx, selection);
  return ownUnits.length > 0 && ownUnits.every((e) => e.kind === KIND.WORKER);
}

function trainsOf(kind) {
  const st = STATS[kind];
  return (st && st.trains) || [];
}

function researchesOf(kind) {
  const st = STATS[kind];
  return (st && st.researches) || [];
}

function requirementsOf(definition) {
  if (!definition || !definition.requires) return [];
  return Array.isArray(definition.requires) ? definition.requires : [definition.requires];
}

function resourcesOf(ctx) {
  return ctx.resources || { steel: 0, oil: 0 };
}

function playerHasCompleteKind(ctx, kind) {
  return typeof ctx.playerHasCompleteKind === "function" && ctx.playerHasCompleteKind(kind);
}

function abilityUnlocked(ctx, definition) {
  return requirementsOf(definition).every((req) => playerHasCompleteKind(ctx, req));
}

function abilityTargetActive(commandTarget, ability) {
  return commandTarget?.kind === "ability" && commandTarget.ability === ability;
}

function commandTargetSig(commandTarget) {
  if (!commandTarget) return "";
  if (typeof commandTarget === "string") return commandTarget;
  return `${commandTarget.kind || ""}:${commandTarget.ability || ""}`;
}

function abilityCooldownLeft(entity, ability) {
  const projected = Array.isArray(entity.abilities)
    ? entity.abilities.find((entry) => entry.ability === ability)
    : null;
  if (projected && typeof projected.cooldownLeft === "number") return projected.cooldownLeft;
  if (ability === ABILITY.CHARGE) return entity.chargeCooldownLeft || 0;
  return 0;
}

function abilityRemainingUses(entity, ability) {
  const projected = Array.isArray(entity.abilities)
    ? entity.abilities.find((entry) => entry.ability === ability)
    : null;
  return projected && typeof projected.remainingUses === "number"
    ? projected.remainingUses
    : null;
}

function abilityAutocastEnabled(entity, ability) {
  const projected = Array.isArray(entity.abilities)
    ? entity.abilities.find((entry) => entry.ability === ability)
    : null;
  return projected && typeof projected.autocastEnabled === "boolean"
    ? projected.autocastEnabled
    : false;
}

function abilityUnitReady(entity, definition) {
  return abilityCooldownLeft(entity, definition.ability) === 0 &&
    abilityRemainingUses(entity, definition.ability) !== 0 &&
    !abilityRequiresSetup(entity, definition);
}

function abilityRequiresSetup(entity, definition) {
  return definition.ability === ABILITY.POINT_FIRE && entity.setupState !== SETUP.DEPLOYED;
}

function affordable(cost, resources) {
  if (!cost) return true;
  const steel = resources.steel ?? 0;
  const oil = resources.oil ?? 0;
  return steel >= (cost.steel ?? 0) && oil >= (cost.oil ?? 0);
}

function buildAvailability(ctx, kind, resources) {
  const st = STATS[kind];
  if (!st) return "locked";
  if (requirementsOf(st).some((req) => !playerHasCompleteKind(ctx, req))) return "locked";
  return affordable(st.cost, resources) ? "ready" : "unaffordable";
}

function buildDisabledReason(ctx, kind, resources) {
  const st = STATS[kind];
  if (!st) return "";
  const missing = requirementsOf(st).find((req) => !playerHasCompleteKind(ctx, req));
  if (missing) return `Requires ${STATS[missing]?.label || missing}`;
  if (!affordable(st.cost, resources)) return "Not enough resources";
  return "";
}

function trainAvailability(ctx, unit, resources) {
  const st = STATS[unit];
  if (!st) return "locked";
  if (requirementsOf(st).some((req) => !playerHasCompleteKind(ctx, req))) return "locked";
  if (st.upgradeRequires && !(ctx.upgrades || []).includes(st.upgradeRequires)) return "locked";
  return affordable(st.cost, resources) ? "ready" : "unaffordable";
}

function trainDisabledReason(ctx, unit, resources) {
  const st = STATS[unit];
  if (!st) return "";
  const missing = requirementsOf(st).find((req) => !playerHasCompleteKind(ctx, req));
  if (missing) return `Requires ${STATS[missing]?.label || missing}`;
  if (st.upgradeRequires && !(ctx.upgrades || []).includes(st.upgradeRequires)) {
    return st.upgradeRequiresText ||
      `Requires ${UPGRADES[st.upgradeRequires]?.label || st.upgradeRequires}`;
  }
  if (!affordable(st.cost, resources)) return "Not enough resources";
  return "";
}

function availableResearchesOf(ctx, kind) {
  const completed = ctx.upgrades || [];
  return researchesOf(kind).filter((upgrade) => !completed.includes(upgrade));
}

function researchAvailability(ctx, upgrade, resources) {
  const def = UPGRADES[upgrade];
  if (!def) return "locked";
  if ((ctx.upgrades || []).includes(upgrade)) return "locked";
  if (selectedProducingBuildingsForKind(ctx, def.researchedAt)
    .some((e) => e.prodUpgrade === upgrade)) return "locked";
  if (def.requiresUpgrade && !(ctx.upgrades || []).includes(def.requiresUpgrade)) return "locked";
  return affordable(def.cost, resources) ? "ready" : "unaffordable";
}

function researchDisabledReason(ctx, upgrade, resources) {
  const def = UPGRADES[upgrade];
  if (!def) return "";
  if ((ctx.upgrades || []).includes(upgrade)) return "Researched";
  if (selectedProducingBuildingsForKind(ctx, def.researchedAt)
    .some((e) => e.prodUpgrade === upgrade)) return "Researching";
  if (def.requiresUpgrade && !(ctx.upgrades || []).includes(def.requiresUpgrade)) {
    return def.requiresText || `Requires ${UPGRADES[def.requiresUpgrade]?.label || def.requiresUpgrade}`;
  }
  if (!affordable(def.cost, resources)) return "Not enough resources";
  return "";
}

function abilityDisabledReason(ctx, affordance) {
  if (!affordance.unlocked) {
    const missing = requirementsOf(affordance.definition)
      .find((req) => !playerHasCompleteKind(ctx, req));
    if (missing) return `Requires ${STATS[missing]?.label || missing}`;
  }
  if (affordance.depletedCount === affordance.carrierIds.length) return "Depleted";
  if (affordance.setupBlockedCount === affordance.carrierIds.length) {
    return "Set up artillery before using Point Fire";
  }
  if (!affordance.affordable) return "Not enough resources";
  if (affordance.readyIds.length === 0) return "On cooldown";
  return affordance.definition.title || "";
}

function selectedProducerBuildingsForUnit(ctx, unit) {
  return (ctx.selection || []).filter(
    (e) =>
      isOwn(ctx, e) &&
      isBuilding(e.kind) &&
      e.buildProgress == null &&
      trainsOf(e.kind).includes(unit),
  );
}

function selectedProducingBuildingsForKind(ctx, kind) {
  return (ctx.selection || []).filter(
    (e) =>
      isOwn(ctx, e) &&
      e.kind === kind &&
      isBuilding(e.kind) &&
      e.buildProgress == null &&
      ((e.prodQueue ?? 0) > 0 || e.state === STATE.TRAIN),
  );
}

function researchSlotForUpgrade(buildingKind, upgrade, trains) {
  const unitIndex = trains.findIndex((unit) => STATS[unit]?.upgradeRequires === upgrade);
  if (unitIndex >= 0) return unitIndex + 3;
  const researchIndex = researchesOf(buildingKind).indexOf(upgrade);
  if (researchIndex >= 0) return researchIndex;
  const afterTrainSlot = trains.findIndex((unit) => STATS[unit] == null);
  return afterTrainSlot >= 0 ? afterTrainSlot : trains.length;
}

function firstOpenCommandSlot(slots, preferredSlot, reservedSlot = -1) {
  const trySlot = (slot) =>
    slot >= 0 && slot < slots.length && slot !== reservedSlot && slots[slot] == null;
  if (trySlot(preferredSlot)) return preferredSlot;
  for (let slot = 0; slot < slots.length; slot++) {
    if (trySlot(slot)) return slot;
  }
  return -1;
}
