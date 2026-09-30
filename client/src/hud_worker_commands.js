import { workerBuildCardSlotsForFaction, workerBuildablesForFaction } from "./config.js";
import { attackDescriptor, holdDescriptor, moveDescriptor, stopDescriptor } from "./hud_unit_commands.js";

export function workerCommandSlots(ctx, unitIds, factionId) {
  return [
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
    workerBuildCardSlotsForFaction(factionId, true).some((kind) =>
      workerBuildablesForFaction(factionId).includes(kind)) ? {
      id: "worker:advanced-build-menu",
      commandId: "worker.advancedBuildMenu",
      kind: "button",
      action: "openWorkerBuildMenu",
      intent: { type: "openWorkerBuildMenu", advanced: true },
      icon: "ADV",
      label: "Advanced Build",
      title: "Open advanced worker build menu",
      enabled: unitIds.length > 0,
    } : null,
    null,
  ];
}
