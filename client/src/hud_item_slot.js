import { ABILITY } from "./protocol.js";
import { abilityAutocastEnabled } from "./hud_ability_affordance.js";

const ITEMS = Object.freeze([
  { ability: ABILITY.ARMOR_TREASURE, slot: 10, category: "defense", label: "Armor", icon: "AR", effect: "50% additional armor" },
  { ability: ABILITY.HASTE, slot: 11, category: "attack", label: "Haste", icon: "H", effect: "Double primary weapon attack speed" },
  { ability: ABILITY.DOUBLE_DAMAGE, slot: 11, category: "attack", label: "Double Damage", icon: "2X", effect: "Double primary weapon damage" },
]);

function status(unit, ability) {
  return unit.abilities?.find((entry) => entry.ability === ability);
}

function shownItems(units) {
  const available = ITEMS.filter((item) => units.some((unit) => status(unit, item.ability)));
  // A mixed selection exposes a ready item before an active buff. Once used,
  // the other attack item becomes the actionable tile in this shared category.
  return available.sort((a, b) => Number(!units.some((u) => status(u, a.ability)?.remainingUses > 0))
    - Number(!units.some((u) => status(u, b.ability)?.remainingUses > 0)))
    .filter((item, index, all) => all.findIndex((other) => other.slot === item.slot) === index);
}

export function itemCooldownAffordances(units, groupCooldownClocks = () => []) {
  return shownItems(units).map((item) => ({
    definition: { ability: item.ability },
    cooldownClocks: groupCooldownClocks(units.map((u) => status(u, item.ability)?.cooldownLeft || 0), 900),
  }));
}

export function placeItemDescriptor(slots, units, factionId, groupCooldownClocks = () => []) {
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
    const readyIds = carriers.filter((unit) => status(unit, item.ability).remainingUses > 0).map((unit) => unit.id);
    const active = carriers.some((unit) => status(unit, item.ability).cooldownLeft > 0);
    slots[item.slot] = {
      id: `item:${item.ability}`, commandId: `cultivators.item.${item.ability}`,
      kind: "button", action: "ability", ability: item.ability,
      icon: item.icon, label: item.label,
      title: `${item.category === "defense" ? "Defense" : "Attack"} item: ${item.effect} for 30 seconds. ${readyIds.length ? "Activate to consume." : "Active; expires when the clock completes."}`,
      enabled: readyIds.length > 0,
      intent: { type: "ability", ability: item.ability, targetMode: "self", readyIds },
      cls: active ? "active" : "", countBadge: carriers.length > 1 ? `${readyIds.length}` : "",
      cooldownClocks: groupCooldownClocks(carriers.map((unit) => status(unit, item.ability).cooldownLeft || 0), 900),
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
