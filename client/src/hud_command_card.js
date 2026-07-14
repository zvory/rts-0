import { ABILITY, DEFAULT_FACTION_ID, KIND, SETUP, UPGRADE, isBuilding, isUnit } from "./protocol.js";
import {
  STATS,
  UPGRADES,
  WORKER_BUILD_CARD_SLOTS,
  commandCardAbilitiesForFaction,
  researchableUpgradesForFaction,
  trainableUnitsForFaction,
  workerBuildablesForFaction,
} from "./config.js";
import {
  abilityActiveObjectId,
  abilityAutocastEnabled,
  abilityCooldownLeft,
  abilityRemainingUses,
  abilityUnitQueueAdmissible,
  abilityUnitReady,
} from "./hud_ability_affordance.js";
import {
  attackDescriptor,
  holdDescriptor,
  moveDescriptor,
  stopDescriptor,
} from "./hud_unit_commands.js";
import {
  firstOpenCommandSlot,
  researchAvailability,
  researchDisabledReason,
  researchSlotForUpgrade,
  selectedProducerBuildingsForUnit,
  selectedProducingBuildingsForKind,
  trainAvailability,
  trainDisabledReason,
  trainLimitSignature,
  trainSlotForUnit,
} from "./hud_train_card_helpers.js";

// Command-card hotkeys follow the keyboard grid (3 columns):
//   Q W E
//   A S D
//   Z X C
export const GRID_HOTKEYS = Object.freeze(["Q", "W", "E", "A", "S", "D", "Z", "X", "C"]);

export function gridHotkeyForSlot(slotIndex) {
  return GRID_HOTKEYS[slotIndex] || "";
}

export function factionCommandId(factionId, family, subject) {
  return `${normalizedFactionId(factionId)}.${family}.${subject}`;
}

export function parsedFactionCommandId(commandId) {
  const match = /^([a-z0-9_]+)\.(build|train|research|ability)\.([A-Za-z0-9_]+)$/.exec(String(commandId || ""));
  return match ? { factionId: match[1], family: match[2], subject: match[3] } : null;
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
    abilities: [
      { ability: ABILITY.POINT_FIRE, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.BLANKET_FIRE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  const commandCar = {
    id: 15,
    owner: playerId,
    kind: KIND.COMMAND_CAR,
    abilities: [
      { ability: ABILITY.BREAKTHROUGH, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.SCOUT_PLANE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  const scoutPlane = {
    id: 16,
    owner: playerId,
    kind: KIND.SCOUT_PLANE,
    scoutPlane: {
      orbitCenter: [256, 256],
    },
  };
  const allEntities = [...baseEntities, worker, rifleman, scoutCar, mortar, artillery, commandCar, scoutPlane];
  const ctx = (selection, overrides = {}) => ({
    playerId,
    factionId: DEFAULT_FACTION_ID,
    selection,
    entities: allEntities,
    resources: { steel: 1000, oil: 1000 },
    upgrades: [
      UPGRADE.METHAMPHETAMINES,
      UPGRADE.ANTI_TANK_GUN_UNLOCK,
      UPGRADE.ARTILLERY_UNLOCK,
      UPGRADE.BALLISTIC_TABLES,
      UPGRADE.TANK_UNLOCK,
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
    { id: "command-car", card: buildCommandCardDescriptors(ctx([commandCar], {
      entities: allEntities.filter((e) => e.id !== scoutPlane.id),
    })) },
    { id: "city-centre-train", card: buildCommandCardDescriptors(ctx([baseEntities[0]])) },
    { id: "barracks-train", card: buildCommandCardDescriptors(ctx([baseEntities[1]])) },
    { id: "factory-train", card: buildCommandCardDescriptors(ctx([baseEntities[4]])) },
    { id: "gun-works-train", card: buildCommandCardDescriptors(ctx([baseEntities[5]])) },
    { id: "research-complex", card: buildCommandCardDescriptors(ctx([baseEntities[3]], { upgrades: [] })) },
    {
      id: "research-complex-medium-guns",
      card: buildCommandCardDescriptors(ctx([baseEntities[3]], {
        upgrades: [UPGRADE.ANTI_TANK_GUN_UNLOCK],
      })),
    },
  ];
}

export function buildCommandCardDescriptors(ctx) {
  if (!commandSurfaceEnabled(ctx)) return { kind: "spectator", signature: "spectator", slots: [] };

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

export function commandSurfaceEnabled(ctx) {
  if (typeof ctx?.commandSurfaceEnabled === "boolean") return ctx.commandSurfaceEnabled;
  return !ctx?.spectator;
}

export function commandSubject(ctx, selection) {
  for (const e of selection || []) {
    if (!isOwn(ctx, e)) continue;
    if (isUnit(e.kind)) return e;
    if (isBuilding(e.kind) && (factionTrainsOf(ctx, e.kind).length > 0 || factionResearchesOf(ctx, e.kind).length > 0)) return e;
  }
  return null;
}

export function buildWorkerBuildCard(ctx) {
  const resources = resourcesOf(ctx);
  const factionId = commandFactionId(ctx);
  const slots = [];
  const sigParts = [];
  const buildables = new Set(workerBuildablesForFaction(factionId));
  for (const kind of WORKER_BUILD_CARD_SLOTS) {
    if (!kind || !buildables.has(kind)) {
      slots.push(null);
      continue;
    }
    const st = STATS[kind];
    if (!st) continue;
    const availability = buildAvailability(ctx, kind, resources);
    sigParts.push(`${kind}:${availability}`);
    slots.push({
      id: `build:${kind}`,
      commandId: factionCommandId(factionId, "build", kind),
      kind: "button",
      action: "build",
      intent: { type: "beginPlacement", building: kind },
      icon: st.icon,
      label: st.label,
      cost: st.cost,
      enabled: availability !== "locked",
      unaffordable: availability === "unaffordable",
      title: buildDisabledReason(ctx, kind, resources),
      tooltipKind: kind,
      onUnavailableIntent: { type: "playNotEnough", cost: st.cost },
    });
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
  const factionId = commandFactionId(ctx);
  const ownUnits = selectedOwnUnits(ctx, selection);
  const unitIds = ownUnits.map((e) => e.id);
  const setupGunIds = ownUnits
    .filter((e) => e.kind === KIND.ANTI_TANK_GUN || e.kind === KIND.ARTILLERY)
    .map((e) => e.id);
  const abilityAffordances = selectedAbilityAffordances(ctx, ownUnits);
  const hasArmyUnit = ownUnits.some((e) => e.kind !== KIND.WORKER);
  const workerSelected = !hasArmyUnit && ownUnits.some((e) => e.kind === KIND.WORKER);
  const signature =
    `units|${unitIds.join(".")}|target:${commandTargetSig(ctx.commandTarget)}|` +
    `|setup:${setupGunIds.join(".")}|` +
    `|abilities:${abilityAffordances.map((affordance) =>
      `${affordance.definition.ability}:${affordance.unlocked ? 1 : 0}:${affordance.affordable ? 1 : 0}:` +
      `${affordance.activeBlock ? 1 : 0}:` +
      `${affordance.depletedCount}:` +
      `${affordance.readyIds.join(".")}:` +
      `${affordance.queueAdmissibleIds.join(".")}:` +
      `${affordance.autocastEnabledIds.join(".")}:` +
      `${affordance.cooldownClocks.map((group) => group.count).join(",")}`,
    ).join("|")}|` +
    (workerSelected ? "worker-main" : "no-build");

  if (workerSelected) {
    return card("unit", signature, [
        moveDescriptor(ctx, unitIds),
        holdDescriptor(unitIds),
        null,
        attackDescriptor(ctx, unitIds),
        stopDescriptor(unitIds),
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
  slots[0] = moveDescriptor(ctx, unitIds);
  if (unitIds.length > 0) {
    slots[1] = holdDescriptor(unitIds);
    slots[3] = attackDescriptor(ctx, unitIds);
    slots[4] = stopDescriptor(unitIds);
  }

  let sequentialSlot = 6;
  const claimSlot = (preferred) => {
    if (preferred != null && preferred >= 0 && preferred < 9 && slots[preferred] == null) {
      return preferred;
    }
    while (sequentialSlot < 9 && slots[sequentialSlot] != null) sequentialSlot++;
    return sequentialSlot < 9 ? sequentialSlot++ : -1;
  };

  for (const affordance of abilityAffordances) {
    const definition = affordance.definition;
    const recastActive = affordance.recastTargetObjectId != null;
    const readyCount = recastActive ? affordance.recastReadyIds.length : affordance.readyIds.length;
    const commandableCount = recastActive
      ? affordance.recastReadyIds.length
      : affordance.queueAdmissibleIds.length;
    const abilityReadyIds = recastActive
      ? affordance.recastReadyIds
      : intentAbilityIds(definition, affordance);
    const showReadyCount = readyCount < affordance.carrierIds.length;
    const preferred = definition.hotkey ? GRID_HOTKEYS.indexOf(definition.hotkey) : -1;
    const slot = claimSlot(preferred);
    if (slot < 0) continue;
    slots[slot] = {
      id: `ability:${definition.ability}`,
      commandId: factionCommandId(factionId, "ability", definition.ability),
      kind: "button",
      action: "ability",
      intent: {
        type: "ability",
        ability: definition.ability,
        targetMode: recastActive ? "recast" : definition.targetMode,
        readyIds: abilityReadyIds,
        targetObjectId: recastActive ? affordance.recastTargetObjectId : null,
      },
      icon: definition.icon,
      label: definition.label,
      title: abilityDisabledReason(ctx, affordance),
      ability: definition.ability,
      enabled: affordance.unlocked && commandableCount > 0 && affordance.affordable,
      unaffordable: affordance.unlocked && commandableCount > 0 && !affordance.affordable,
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
            enabled: affordance.autocastEnabledIds.length === 0,
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
        action: "setupAntiTankGuns",
        intent: { type: "beginCommandTarget", target: "setupAntiTankGuns" },
        icon: "SET",
        label: "Set Up",
        title: "Set up selected support weapons toward a target point",
        enabled: true,
        cls: ctx.commandTarget === "setupAntiTankGuns" ? "active" : "",
      };
    }
  }

  return card("unit", signature, slots, { abilityAffordances });
}

export function buildTrainCard(ctx, building) {
  const resources = trainResourcesOf(ctx);
  const factionId = commandFactionId(ctx);
  const trains = factionTrainsOf(ctx, building.kind);
  const researches = availableResearchesOf(ctx, building.kind);
  const producingBuildings = selectedProducingBuildingsForKind(ctx, building.kind, isOwn);
  const cancelSlot = 8;
  const signature =
    `train|${building.id}|` +
    trains.map((unit) => {
      const producers = selectedProducerBuildingsForUnit(ctx, unit, isOwn, factionTrainsOf);
      const producerIds = producers.map((e) => e.id).join(".");
      const repeatingIds = producers
        .filter((producer) => producer.prodRepeatKinds?.includes(unit))
        .map((producer) => producer.id)
        .join(".");
      return `${unit}:${trainAvailability(ctx, unit, resources, isOwn)}:${trainLimitSignature(ctx, unit, isOwn)}:${producerIds}:repeat:${repeatingIds}`;
    }).join(",") +
    `|research:` +
    researches.map((upgrade) => `${upgrade}:${researchAvailability(ctx, upgrade, resources, isOwn)}`).join(",") +
    `|cancel:${producingBuildings.map((e) => e.id).join(".")}`;

  const slots = new Array(9).fill(null);
  for (const unit of trains) {
    const st = STATS[unit];
    if (!st) continue;
    const slot = firstOpenCommandSlot(slots, trainSlotForUnit(building.kind, unit, trains), cancelSlot);
    if (slot < 0) continue;
    const availability = trainAvailability(ctx, unit, resources, isOwn);
    const producerIds = selectedProducerBuildingsForUnit(ctx, unit, isOwn, factionTrainsOf)
      .map((producer) => producer.id);
    const repeatingIds = selectedProducerBuildingsForUnit(ctx, unit, isOwn, factionTrainsOf)
      .filter((producer) => producer.prodRepeatKinds?.includes(unit))
      .map((producer) => producer.id);
    const disabledReason = trainDisabledReason(ctx, unit, resources, isOwn);
    const repeatHelp = "Alt-click or Alt+hotkey adds one auto-build; hold Shift to remove one";
    slots[slot] = {
      id: `train:${unit}`,
      commandId: factionCommandId(factionId, "train", unit),
      kind: "button",
      action: "train",
      intent: { type: "train", unit },
      icon: st.icon,
      label: st.label,
      cost: st.cost,
      enabled: availability !== "locked",
      unaffordable: availability === "unaffordable",
      title: disabledReason ? `${disabledReason}. ${repeatHelp}` : repeatHelp,
      tooltipKind: unit,
      repeatable: availability === "ready",
      countBadge: `${repeatingIds.length}/${producerIds.length}`,
      autobuildIndicatorCount: repeatingIds.length,
      cls: repeatingIds.length > 0 ? "autocast-enabled production-repeat-enabled" : "",
      contextIntent: {
        type: "adjustProductionRepeat",
        buildingIds: producerIds,
        unit,
      },
      onUnavailableIntent: { type: "playNotEnough", cost: st.cost, supply: st.supply },
    };
  }
  for (const upgrade of researches) {
    const def = UPGRADES[upgrade];
    if (!def) continue;
    if (def.replacesUpgrade && !(ctx.upgrades || []).includes(def.replacesUpgrade)) continue;
    const preferredSlot = researchSlotForUpgrade(building.kind, upgrade, trains);
    const slot = firstOpenCommandSlot(slots, preferredSlot, cancelSlot);
    if (slot < 0) continue;
    const availability = researchAvailability(ctx, upgrade, resources, isOwn);
    slots[slot] = {
      id: `research:${upgrade}`,
      commandId: factionCommandId(factionId, "research", upgrade),
      kind: "button",
      action: "research",
      intent: { type: "research", upgrade },
      icon: def.icon,
      label: def.label,
      cost: def.cost,
      enabled: availability !== "locked",
      unaffordable: availability === "unaffordable",
      title: researchDisabledReason(ctx, upgrade, resources, isOwn),
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

export function selectedAbilityAffordances(ctx, selection) {
  const ownUnits = selectedOwnUnits(ctx, selection);
  const resources = resourcesOf(ctx);
  return commandCardAbilitiesForFaction(commandFactionId(ctx))
    .map((definition) => {
      const carriers = ownUnits.filter((e) => definition.carriers.includes(e.kind));
      if (carriers.length === 0) return null;
      const unlocked = abilityUnlocked(ctx, definition);
      const canAfford = affordable(definition.cost, resources);
      const activeScoutPlaneSources = definition.ability === ABILITY.SCOUT_PLANE
        ? activeOwnScoutPlaneSourceIds(ctx)
        : null;
      const activeBlockedUnits = activeScoutPlaneSources
        ? carriers.filter((e) => activeScoutPlaneSources.has(e.id))
        : [];
      const activeBlock = activeBlockedUnits.length === carriers.length;
      const readyUnits = carriers.filter((e) =>
        !activeScoutPlaneSources?.has(e.id) && abilityUnitReady(e, definition));
      const queueAdmissibleUnits = carriers.filter((e) =>
        !activeScoutPlaneSources?.has(e.id) && abilityUnitQueueAdmissible(e, definition));
      const recastUnits = carriers.filter((e) => abilityActiveObjectId(e, definition.ability) != null);
      const cooldowns = carriers.map((e) =>
        abilityCooldownLeft(e, definition.ability),
      );
      const depletedCount = carriers.filter(
        (e) => abilityRemainingUses(e, definition.ability) === 0,
      ).length;
      const autocastEnabledIds = carriers
        .filter((e) => abilityAutocastEnabled(e, definition.ability))
        .map((e) => e.id);
      return {
        definition,
        unlocked,
        affordable: canAfford,
        activeBlock,
        depletedCount,
        carrierIds: carriers.map((e) => e.id),
        readyIds: readyUnits.map((e) => e.id),
        queueAdmissibleIds: queueAdmissibleUnits.map((e) => e.id),
        readyUnits,
        recastReadyIds: recastUnits.map((e) => e.id),
        recastTargetObjectId: recastUnits.length > 0
          ? abilityActiveObjectId(recastUnits[0], definition.ability)
          : null,
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

function intentAbilityIds(definition, affordance) {
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

function isOwn(ctx, e) {
  const commandOwner = Number(ctx?.commandOwner);
  if (Number.isInteger(commandOwner) && commandOwner > 0) return e && Number(e.owner) === commandOwner;
  if (ctx?.controlPolicy?.kind === "lab") {
    if (typeof ctx.controlPolicy.isCommandOwner === "function") {
      return ctx.controlPolicy.isCommandOwner(e?.owner, ctx.state);
    }
    return ctx.controlPolicy.canControlOwner(e?.owner, ctx.state);
  }
  return e && e.owner === ctx.playerId;
}

function selectedOwnUnits(ctx, selection) {
  return (selection || []).filter((e) =>
    isOwn(ctx, e) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE);
}

function workerOnlySelection(ctx, selection) {
  const ownUnits = selectedOwnUnits(ctx, selection);
  return ownUnits.length > 0 && ownUnits.every((e) => e.kind === KIND.WORKER);
}

function researchesOf(kind) {
  const st = STATS[kind];
  return (st && st.researches) || [];
}

function factionTrainsOf(ctx, kind) {
  return trainableUnitsForFaction(commandFactionId(ctx), kind);
}

function factionResearchesOf(ctx, kind) {
  return researchableUpgradesForFaction(commandFactionId(ctx), kind);
}

function requirementsOf(definition) {
  if (!definition || !definition.requires) return [];
  return Array.isArray(definition.requires) ? definition.requires : [definition.requires];
}

function resourcesOf(ctx) {
  return ctx.resources || { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
}

function trainResourcesOf(ctx) {
  const base = resourcesOf(ctx);
  const resources = {
    steel: base.steel ?? 0,
    oil: base.oil ?? 0,
    supplyUsed: Number.isFinite(base.supplyUsed) ? base.supplyUsed : 0,
    supplyCap: Number.isFinite(base.supplyCap) ? base.supplyCap : null,
  };
  for (const entry of ctx.optimisticProduction || []) {
    const st = STATS[entry?.unit];
    if (!st) continue;
    const cost = st.cost || {};
    resources.steel -= cost.steel ?? 0;
    resources.oil -= cost.oil ?? 0;
    const supply = st.supply ?? 0;
    if (Number.isFinite(supply) && supply > 0) resources.supplyUsed += supply;
  }
  return resources;
}

function commandFactionId(ctx) {
  return normalizedFactionId(ctx?.factionId);
}

function normalizedFactionId(factionId) {
  return typeof factionId === "string" && /^[a-z0-9_]+$/.test(factionId)
    ? factionId
    : DEFAULT_FACTION_ID;
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
  if (!affordable(st.cost, resources)) return "Place now; construction waits for resources";
  return "";
}

function availableResearchesOf(ctx, kind) {
  const completed = ctx.upgrades || [];
  return factionResearchesOf(ctx, kind).filter((upgrade) => !completed.includes(upgrade));
}

function abilityDisabledReason(ctx, affordance) {
  if (!affordance.unlocked) {
    const missing = requirementsOf(affordance.definition)
      .find((req) => !playerHasCompleteKind(ctx, req));
    if (missing) return `Requires ${STATS[missing]?.label || missing}`;
  }
  if (affordance.activeBlock) return "Scout Plane already active";
  if (affordance.depletedCount === affordance.carrierIds.length) return "Depleted";
  if (!affordance.affordable) return "Not enough resources";
  if (
    affordance.readyIds.length === 0 &&
    affordance.definition.queuePolicy !== "waitUntilReady"
  ) return "On cooldown";
  return affordance.definition.title || "";
}

function currentEntitiesOf(ctx) {
  if (Array.isArray(ctx?.currentEntities)) return ctx.currentEntities;
  if (Array.isArray(ctx?.entities)) return ctx.entities;
  if (typeof ctx?.state?.entitiesInterpolated === "function") {
    return ctx.state.entitiesInterpolated(1) || [];
  }
  return ctx?.selection || [];
}

function activeOwnScoutPlaneSourceIds(ctx) {
  return new Set(currentEntitiesOf(ctx)
    .filter((e) => isOwn(ctx, e) && e.kind === KIND.SCOUT_PLANE && e.hp !== 0)
    .map((e) => e.scoutPlane?.sourceCommandCar)
    .filter((id) => Number.isInteger(id) && id > 0));
}
