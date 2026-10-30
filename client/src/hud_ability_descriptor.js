import { ABILITY } from "./protocol.js";

export function abilityButtonDescriptor({
  affordance,
  commandId,
  disabledReason,
  readyIds,
  active,
}) {
  const definition = affordance.definition;
  const recastActive = affordance.recastTargetObjectId != null;
  const readyCount = recastActive ? affordance.recastReadyIds.length : affordance.readyIds.length;
  const commandableCount = recastActive
    ? affordance.recastReadyIds.length
    : affordance.queueAdmissibleIds.length;
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
    tooltipHtml: !affordance.unlocked
      ? `<span class="cmd-tooltip-title">${definition.label}</span>` +
        `<span class="cmd-tooltip-desc">${disabledReason}</span>`
      : "",
    ability: definition.ability,
    enabled: affordance.unlocked && commandableCount > 0 && affordance.affordable,
    unaffordable: affordance.unlocked && commandableCount > 0 && !affordance.affordable,
    countBadge: showChargeCount
      ? `${affordance.remainingUsesTotal}`
      : (showReadyCount ? `${readyCount}` : ""),
    cooldownClocks: affordance.cooldownClocks,
    cost: affordance.cost,
    cls: [
      active ? "active" : "",
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
