import { KIND, UPGRADE } from "./protocol.js";
import { UPGRADES, DAO_TRAINING_COST_OIL, MAX_DAO_TRAINING_QUEUE } from "./config.js";

const CHOICES = [UPGRADE.DAO_OF_LIGHTNESS, UPGRADE.DAO_OF_HEAVINESS, UPGRADE.DAO_OF_MAGIC];

export function buildDaoTrainingCard(ctx, building) {
  const queue = building.daoTrainingQueue ?? [];
  const slots = new Array(9).fill(null);
  for (const [slot, upgrade] of CHOICES.entries()) {
    const def = UPGRADES[upgrade];
    const researched = (ctx.upgrades ?? []).includes(upgrade);
    const enabled = researched && queue.length < MAX_DAO_TRAINING_QUEUE;
    slots[slot] = {
      id: `dao-training:${upgrade}`,
      commandId: `cultivators.dao-training.${upgrade}`,
      kind: "button", action: "research",
      intent: { type: "research", upgrade, buildingId: building.id },
      icon: def.icon, label: def.label,
      cost: { steel: 0, oil: DAO_TRAINING_COST_OIL },
      enabled, repeatable: enabled,
      countBadge: `${queue.filter((dao) => dao === upgrade).length}`,
      title: !researched ? `Requires ${def.label} research in the Three Dao Pavilion`
        : queue.length >= MAX_DAO_TRAINING_QUEUE ? "Training queue is full"
        : "Queue one Disciple specialization. Send a base Disciple inside; costs 100 oil on entry and takes 10 seconds. Portal rally supported.",
      cls: `dao-choice${enabled ? " dao-ready" : ""}`,
    };
  }
  if (queue.length > (building.prodKind ? 1 : 0)) {
    slots[8] = {
      id: "cancel:dao-training", commandId: `production.cancel.${KIND.PROVEN_GROUNDS}`,
      kind: "button", action: "cancel", icon: "CNCL", label: "Cancel",
      intent: { type: "cancelProduction", buildingKind: KIND.PROVEN_GROUNDS },
      enabled: true, repeatable: true, cls: "cancel",
      title: "Remove the latest waiting training order. Active training cannot be canceled.",
    };
  }
  return { kind: "train", signature: `dao-training|${building.id}|${queue.join(",")}|${(ctx.upgrades ?? []).join(",")}|${building.prodKind ?? ""}`, slots };
}
