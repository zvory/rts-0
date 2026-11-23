import { ABILITY } from "./protocol.js";
import { abilityAutocastEnabled } from "./hud_ability_affordance.js";

export function placeItemDescriptor(slots, units, factionId) {
  if (factionId !== "cultivators") return;
  const carriers = units.filter((e) => e.escapeTreasure);
  if (!carriers.length) return;
  const unitIds = carriers.map((e) => e.id);
  const auto = carriers.some((e) => abilityAutocastEnabled(e, ABILITY.ESCAPE_TREASURE));
  slots[9] = {
    id: "item:escape",
    commandId: `${factionId}.item.escape`,
    kind: "button", action: "ability", icon: "E", label: "Escape",
    title: "Return to the supplying Shop; automatically escapes lethal damage. Right-click or Alt+R toggles autocast.",
    enabled: true,
    intent: { type: "ability", ability: ABILITY.ESCAPE_TREASURE, targetMode: "self", readyIds: unitIds },
    cls: auto ? "autocast-enabled" : "",
    contextIntent: { type: "setAutocast", ability: ABILITY.ESCAPE_TREASURE, unitIds, enabled: !auto },
    contextHotkeyModifiers: ["alt"],
  };
}

export function itemSlotSignature(units) {
  return units.filter((e) => e.escapeTreasure).map((e) =>
    `${e.id}:${abilityAutocastEnabled(e, ABILITY.ESCAPE_TREASURE)}`).join(",");
}
