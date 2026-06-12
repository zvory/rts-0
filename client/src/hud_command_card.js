import { ABILITY, KIND, ORDER_STAGE, SETUP, STATE, UPGRADE, isBuilding, isUnit } from "./protocol.js";
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

export function gridHotkeyForSlot(slotIndex) {
  return GRID_HOTKEYS[slotIndex] || "";
}

function card(kind, signature, slots, extras = {}) {
  return {
    kind,
    signature,
    ...extras,
    slots: slots.map((slot, slotIndex) => slot ? renderedDescriptor(slot, slotIndex) : null),
  };
}

function renderedDescriptor(descriptor, slotIndex) {
  const commandId = descriptor.commandId || descriptor.id;
  return {
    ...descriptor,
    commandId,
    slotIndex,
    hotkey: gridHotkeyForSlot(slotIndex),
  };
}

export function commandCardActivationCandidates(renderedCard, commandId) {
  return (renderedCard?.slots || [])
    .filter((slot) => slot?.commandId === commandId)
    .map((slot) => ({
      commandId: slot.commandId,
      slotIndex: slot.slotIndex,
      hotkey: slot.hotkey,
      label: slot.label,
      enabled: !!slot.enabled,
    }));
}

export function duplicateCommandIdsForCard(renderedCard) {
  const firstByCommandId = new Map();
  const duplicates = [];
  for (const slot of renderedCard?.slots || []) {
    if (!slot?.commandId) continue;
    const first = firstByCommandId.get(slot.commandId);
    if (first) {
      duplicates.push({
        commandId: slot.commandId,
        firstSlotIndex: first.slotIndex,
        duplicateSlotIndex: slot.slotIndex,
      });
      continue;
    }
    firstByCommandId.set(slot.commandId, slot);
  }
  return duplicates;
}

export function buildCommandCardContextCatalog() {
  const playerId = 1;
  const baseEntities = [
    { id: 1, owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { id: 2, owner: playerId, kind: KIND.BARRACKS, buildProgress: null },
    { id: 3, owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: null },
    { id: 4, owner: playerId, kind: KIND.RESEARCH_COMPLEX, buildProgress: null },
    { id: 5, owner: playerId, kind: KIND.FACTORY, buildProgress: null },
    { id: 6, owner: playerId, kind: KIND.STEELWORKS, buildProgress: null },
  ];
  const worker = { id: 10, owner: playerId, kind: KIND.WORKER };
  const rifleman = {
    id: 11,
    owner: playerId,
    kind: KIND.RIFLEMAN,
    abilities: [{ ability: ABILITY.CHARGE, cooldownLeft: 0, remainingUses: null }],
  };
  const scoutCar = {
    id: 12,
    owner: playerId,
    kind: KIND.SCOUT_CAR,
    abilities: [{ ability: ABILITY.SMOKE, cooldownLeft: 0, remainingUses: 1 }],
  };
  const mortar = {
    id: 13,
    owner: playerId,
    kind: KIND.MORTAR_TEAM,
    abilities: [{ ability: ABILITY.MORTAR_FIRE, cooldownLeft: 0, remainingUses: null }],
  };
  const artillery = {
    id: 14,
    owner: playerId,
    kind: KIND.ARTILLERY,
    setupState: SETUP.DEPLOYED,
    abilities: [{ ability: ABILITY.POINT_FIRE, cooldownLeft: 0, remainingUses: null }],
  };
  const commandCar = {
    id: 15,
    owner: playerId,
    kind: KIND.COMMAND_CAR,
    abilities: [{ ability: ABILITY.BREAKTHROUGH, cooldownLeft: 0, remainingUses: null }],
  };
  const allEntities = [...baseEntities, worker, rifleman, scoutCar, mortar, artillery, commandCar];
  const ctx = (selection, overrides = {}) => ({
    playerId,
    selection,
    resources: { steel: 1000, oil: 1000 },
    upgrades: [
      UPGRADE.METHAMPHETAMINES,
      UPGRADE.AT_GUN_UNLOCK,
      UPGRADE.ARTILLERY_UNLOCK,
      UPGRADE.TANK_UNLOCK,
      UPGRADE.COMMAND_CAR_UNLOCK,
    ],
    groupCooldownClocks: () => [],
    playerHasCompleteKind: (kind) => allEntities.some((e) =>
      e.owner === playerId && e.kind === kind && e.buildProgress == null
    ),
    ...overrides,
  });
  return [
    { id: "empty", card: buildCommandCardDescriptors(ctx([])) },
    { id: "worker-main", card: buildCommandCardDescriptors(ctx([worker])) },
    { id: "worker-build", card: buildCommandCardDescriptors(ctx([worker], { commandCardMode: "workerBuild" })) },
    { id: "mixed-army-support", card: buildCommandCardDescriptors(ctx([rifleman, scoutCar, mortar, artillery, commandCar])) },
    { id: "city-centre-train", card: buildCommandCardDescriptors(ctx([baseEntities[0]])) },
    { id: "factory-train", card: buildCommandCardDescriptors(ctx([baseEntities[4]])) },
    { id: "gun-works-train", card: buildCommandCardDescriptors(ctx([baseEntities[5]])) },
    { id: "research-complex", card: buildCommandCardDescriptors(ctx([baseEntities[3]], { upgrades: [] })) },
  ];
}

export function buildCommandCardDescriptors(ctx) {
  if (ctx?.spectator) return { kind: "spectator", signature: "spectator", slots: [] };

  const selection = ctx?.selection || [];
  const primary = commandSubject(ctx, selection);
  if (!primary) return card("empty", "empty", new Array(9).fill(null));

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
      commandId: `build.${kind}`,
      kind: "button",
      action: "build",
      intent: { type: "beginPlacement", building: kind },
      icon: st.icon,
      label: st.label,
      cost: st.cost,
      enabled: availability === "ready",
      unaffordable: availability === "unaffordable",
      title: buildDisabledReason(ctx, kind, resources),
      tooltipKind: kind,
      onUnavailableIntent: { type: "playNotEnough", cost: st.cost },
    });
    idx++;
  }
  while (slots.length < 8) slots.push(null);
  slots.push({
    id: "worker:return",
    commandId: "worker.return",
    kind: "button",
    action: "returnWorker",
    intent: { type: "closeCommandCardMenu" },
    icon: "RTN",
    label: "Worker",
    enabled: true,
    title: "Return to worker commands",
  });
  return card("workerBuild", `build|${sigParts.join(",")}`, slots);
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
    return card("unit", signature, [
        moveDescriptor(ctx, unitIds, 0),
        null,
        null,
        attackDescriptor(ctx, unitIds, 3),
        holdDescriptor(unitIds, 4),
        null,
        {
          id: "worker:build-menu",
          commandId: "worker.buildMenu",
          kind: "button",
          action: "openWorkerBuildMenu",
          intent: { type: "openWorkerBuildMenu" },
          icon: "BLD",
          label: "Build",
          title: "Open worker build menu",
          enabled: unitIds.length > 0,
        },
        null,
        null,
      ], { abilityAffordances });
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
    const abilityReadyIds = intentReadyIds(definition, affordance);
    const showReadyCount = readyCount < affordance.carrierIds.length;
    const preferred = definition.hotkey ? GRID_HOTKEYS.indexOf(definition.hotkey) : -1;
    const slot = claimSlot(preferred);
    if (slot < 0) continue;
    slots[slot] = {
      id: `ability:${definition.ability}`,
      commandId: `ability.${definition.ability}`,
      kind: "button",
      action: "ability",
      intent: {
        type: "ability",
        ability: definition.ability,
        targetMode: definition.targetMode,
        readyIds: abilityReadyIds,
      },
      icon: definition.icon,
      label: definition.label,
      title: abilityDisabledReason(ctx, affordance),
      ability: definition.ability,
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
        commandId: "unit.setupSupportWeapon",
        kind: "button",
        action: "setupAtGuns",
        intent: { type: "beginCommandTarget", target: "setupAtGuns" },
        icon: "SET",
        label: "Set Up",
        title: "Set up selected support weapons toward a target point",
        enabled: true,
        cls: ctx.commandTarget === "setupAtGuns" ? "active" : "",
      };
    }
  }

  return card("unit", signature, slots, { abilityAffordances });
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
      commandId: `train.${unit}`,
      kind: "button",
      action: "train",
      intent: { type: "train", unit },
      icon: st.icon,
      label: st.label,
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
      commandId: `research.${upgrade}`,
      kind: "button",
      action: "research",
      intent: { type: "research", upgrade },
      icon: def.icon,
      label: def.label,
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
      commandId: `production.cancel.${building.kind}`,
      kind: "button",
      action: "cancel",
      intent: { type: "cancelProduction", buildingKind: building.kind },
      icon: "CNCL",
      label: "Cancel",
      enabled: true,
      cls: "cancel",
      title: "Cancel latest queued production",
      repeatable: true,
    };
  }

  return card("train", signature, slots);
}

function moveDescriptor(ctx, unitIds, slot) {
  return {
    id: "unit:move",
    commandId: "unit.move",
    kind: "button",
    action: "move",
    intent: { type: "beginCommandTarget", target: "move" },
    icon: "MV",
    label: "Move",
    title: "Move to a target point",
    enabled: unitIds.length > 0,
    cls: ctx.commandTarget === "move" ? "active" : "",
  };
}

function attackDescriptor(ctx, unitIds, slot) {
  return {
    id: "unit:attack",
    commandId: "unit.attack",
    kind: "button",
    action: "attack",
    intent: { type: "beginCommandTarget", target: "attack" },
    icon: "AT",
    label: "Attack",
    title: "Attack a target or attack-move to a point",
    enabled: unitIds.length > 0,
    cls: ctx.commandTarget === "attack" ? "active" : "",
  };
}

function holdDescriptor(unitIds, slot) {
  return {
    id: "unit:hold",
    commandId: "unit.stop",
    kind: "button",
    action: "stop",
    intent: { type: "stop", unitIds },
    icon: "ST",
    label: "Stop",
    title: "Stop selected units",
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
        readyUnits,
        autocastEnabledIds,
        cooldownClocks: ctx.groupCooldownClocks(cooldowns, definition.cooldownTicks),
      };
    })
    .filter(Boolean);
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
  return definition.ability === ABILITY.POINT_FIRE &&
    entity.setupState !== SETUP.DEPLOYED &&
    !hasPointFireOrder(entity);
}

function hasPointFireOrder(entity) {
  return Array.isArray(entity?.orderPlan) &&
    entity.orderPlan.some((marker) => marker?.kind === ORDER_STAGE.POINT_FIRE);
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
