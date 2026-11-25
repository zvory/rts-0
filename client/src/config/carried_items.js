// Presentation of server-projected carried items. Never infer inventory from public radiance.
import { ABILITY, isBuilding } from "../protocol.js";

export const CARRIED_ITEMS = Object.freeze([
  { ability: ABILITY.HASTE, label: "Haste", shortLabel: "HST", color: 0xffd15e, shape: "bolt" },
  { ability: ABILITY.DOUBLE_DAMAGE, label: "Double Damage", shortLabel: "DD", color: 0xff825d, shape: "sword" },
  { ability: ABILITY.ARMOR_TREASURE, label: "Armor Treasure", shortLabel: "ARM", color: 0x73d8ff, shape: "shield" },
  { ability: ABILITY.VITALITY_TREASURE, label: "Vitality Treasure", shortLabel: "VIT", color: 0x87edaa, shape: "heart" },
  { ability: ABILITY.ESCAPE_TREASURE, label: "Escape Treasure", shortLabel: "ESC", color: 0xdfa5ff, shape: "return" },
].map(Object.freeze));

export function carriedItemMask(entity) {
  if (!entity || isBuilding(entity.kind)) return 0;
  let mask = 0;
  for (let i = 0; i < CARRIED_ITEMS.length; i++) {
    const { ability } = CARRIED_ITEMS[i];
    const equipped = ability === ABILITY.ESCAPE_TREASURE
      ? entity.escapeTreasure === true
      : entity.abilities?.some((entry) => entry.ability === ability && entry.remainingUses > 0);
    if (equipped) mask |= 1 << i;
  }
  return mask;
}

export function carriedItemsForEntity(entity) {
  const mask = carriedItemMask(entity);
  return CARRIED_ITEMS.filter((_, index) => mask & (1 << index));
}
