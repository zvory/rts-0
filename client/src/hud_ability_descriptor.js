import { ABILITY } from "./protocol.js";
import { shopItemTooltipHtml } from "./hud_shop_tooltip.js";

export function abilityButtonDescriptor({
  affordance,
  commandId,
  disabledReason,
  readyIds,
  active,
}) {
  const definition = affordance.definition;
  if (definition.ability === ABILITY.CHARGE) {
    return {
      id: `ability:${definition.ability}`, commandId, kind: "button", action: "ability",
      ability: definition.ability, icon: definition.icon, label: definition.label,
      title: affordance.unlocked ? definition.title : `${disabledReason} at Technique Library`,
      passive: true, enabled: false, intent: null, contextIntent: null, onUnavailableIntent: null,
      contextHotkeyModifiers: [], cls: "autocast-enabled",
      cooldownClocks: affordance.cooldownClocks,
      cooldownSeconds: affordance.cooldownSeconds,
    };
  }
  const shopTooltip = shopItemTooltipHtml(definition.ability, affordance.cost,
    disabledReason !== definition.title ? disabledReason : "");
  const evolution = definition.ability.startsWith("evolve");
  const recastActive = affordance.recastTargetObjectId != null;
  const readyCount = recastActive ? affordance.recastReadyIds.length : affordance.readyIds.length;
  const commandableCount = evolution ? affordance.carrierIds.length : recastActive
    ? affordance.recastReadyIds.length
    : definition.queuePolicy === "waitUntilReady"
      ? affordance.queueAdmissibleIds.length
      : affordance.readyIds.length;
  const chosenCount = evolution && affordance.chosenIds.length
    ? `${affordance.chosenIds.length}/${affordance.carrierIds.length}` : "";
  const showReadyCount = readyCount < affordance.carrierIds.length;
  const showChargeCount = definition.ability !== ABILITY.BARRAGE &&
    definition.charges != null && affordance.remainingUsesTotal != null;
  return {
    id: `ability:${definition.ability}`,
    commandId,
    kind: "button",
    action: "ability",
    intent: {
      type: "ability",
      ability: definition.ability,
      targetMode: recastActive ? "recast" : definition.targetMode,
      readyIds: recastActive ? affordance.recastReadyIds : readyIds,
      targetObjectId: recastActive ? affordance.recastTargetObjectId : null,
    },
    icon: definition.icon,
    label: definition.label,
    title: disabledReason,
    tooltipHtml: shopTooltip || (!affordance.unlocked
      ? `<span class="cmd-tooltip-title">${definition.label}</span>` +
        `<span class="cmd-tooltip-desc">${disabledReason}</span>`
      : ""),
    ability: definition.ability,
    enabled: (evolution || affordance.unlocked) && commandableCount > 0 && affordance.affordable,
    unaffordable: affordance.unlocked && commandableCount > 0 && !affordance.affordable,
    countBadge: evolution ? chosenCount : showChargeCount
      ? `${affordance.remainingUsesTotal}`
      : (showReadyCount ? `${readyCount}` : ""),
    cooldownClocks: affordance.cooldownClocks,
    cooldownSeconds: affordance.cooldownSeconds,
    cost: affordance.cost,
    cls: [
      active ? "active" : "",
      definition.ability.startsWith("evolve") ? "dao-choice" : "",
      evolution && !affordance.unlocked ? "dao-awaiting-research" : "",
      definition.ability.startsWith("evolve") && affordance.unlocked && readyCount > 0 && affordance.affordable ? "dao-ready" : "",
      affordance.autocastEnabledIds.length > 0 ? "autocast-enabled" : "",
    ].filter(Boolean).join(" "),
    onUnavailableIntent: { type: "playNotEnough", cost: affordance.cost },
    contextIntent: definition.autocast
      ? {
          type: "setAutocast",
          ability: definition.ability,
          unitIds: affordance.carrierIds,
          enabled: affordance.autocastEnabledIds.length === 0,
        }
      : null,
    contextHotkeyModifiers: definition.autocast ? ["alt"] : [],
  };
}
