import { ABILITY } from "./protocol.js";
import { abilityAutocastEnabled } from "./hud_ability_affordance.js";

const ITEMS = Object.freeze([
  { ability: ABILITY.ARMOR_TREASURE, slot: 10, category: "defense", label: "Armor", icon: "AR", effect: "50% additional armor" },
  { ability: ABILITY.VITALITY_TREASURE, slot: 10, category: "defense", label: "Vitality", icon: "+HP", effect: "+150 maximum and current HP" },
  { ability: ABILITY.HASTE, slot: 11, category: "attack", label: "Haste", icon: "H", effect: "Double primary weapon attack speed" },
  { ability: ABILITY.DOUBLE_DAMAGE, slot: 11, category: "attack", label: "Double Damage", icon: "2X", effect: "Double primary weapon damage" },
]);

function status(unit, ability) {
  return unit.abilities?.find((entry) => entry.ability === ability);
}

function shownItems(units) {
  const available = ITEMS.filter((item) => units.some((unit) => status(unit, item.ability)));
  return available.filter((item, index, all) => all.findIndex((other) => other.slot === item.slot) === index);
}

export function placeItemDescriptor(slots, units) {
  // Allied deliveries may equip another faction; carried commands keep one stable identity.
  const carriers = units.filter((e) => e.escapeTreasure);
  if (carriers.length) {
    const unitIds = carriers.map((e) => e.id);
    const auto = carriers.some((e) => abilityAutocastEnabled(e, ABILITY.ESCAPE_TREASURE));
    slots[9] = {
      id: "item:escape", commandId: `cultivators.item.escape`,
      kind: "button", action: "ability", icon: "E", label: "Escape",
      title: "Movement item: return to the supplying Shop; automatically escapes lethal damage. Right-click or Alt+R toggles autocast.",
      enabled: true,
      intent: { type: "ability", ability: ABILITY.ESCAPE_TREASURE, targetMode: "self", readyIds: unitIds },
      cls: auto ? "autocast-enabled" : "",
      contextIntent: { type: "setAutocast", ability: ABILITY.ESCAPE_TREASURE, unitIds, enabled: !auto },
      contextHotkeyModifiers: ["alt"],
    };
  }
  for (const item of shownItems(units)) {
    const carriers = units.filter((unit) => status(unit, item.ability));
    slots[item.slot] = {
      id: `item:${item.ability}`, commandId: `cultivators.item.${item.ability}`,
      kind: "button", action: null, ability: item.ability,
      icon: item.icon, label: item.label,
      title: `Permanent passive ${item.category} item: ${item.effect}, applied on arrival.`,
      passive: true, enabled: false, intent: null, contextIntent: null,
      contextHotkeyModifiers: [], cooldownClocks: [],
      countBadge: carriers.length > 1 ? `${carriers.length}` : "",
    };
  }
}

export function itemSlotSignature(units) {
  return units.map((unit) => `${unit.id}:${!!unit.escapeTreasure}:${abilityAutocastEnabled(unit, ABILITY.ESCAPE_TREASURE)}:` +
    ITEMS.map((item) => {
      const entry = status(unit, item.ability);
      return entry ? `${item.ability}:${entry.remainingUses}:${entry.cooldownLeft > 0}` : "";
    }).join(",")).join("|");
}
