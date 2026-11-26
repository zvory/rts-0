// Presentation of server-projected carried items. Never infer inventory from public radiance.
import { ABILITY, isBuilding } from "../protocol.js";

export const ITEM_ICON_SHAPES = Object.freeze({
  xp: [-5, -3, -3, -5, 0, -2, 3, -5, 5, -3, 2, 0, 5, 3, 3, 5, 0, 2, -3, 5, -5, 3, -2, 0],
  cross: [-1.5, -5, 1.5, -5, 1.5, -1.5, 5, -1.5, 5, 1.5, 1.5, 1.5, 1.5, 5, -1.5, 5, -1.5, 1.5, -5, 1.5, -5, -1.5, -1.5, -1.5],
  bolt: [-1, -5, -4.5, 1, 0, 1, -1, 5, 4.5, -1, 1, -1],
  sword: [0, -5, 3, -2, 1, 1, 3, 1, 3, 2, 1, 2, 1, 5, -1, 5, -1, 2, -3, 2, -3, 1, -1, 1, -3, -2],
  shield: [-4.5, -5, 4.5, -5, 4, 1, 2, 3, 0, 5, -2, 3, -4, 1],
  heart: [-4.5, -2, -3, -4.5, -1, -4.5, 0, -2.5, 1, -4.5, 3, -4.5, 4.5, -2, 4, 1, 0, 5, -4, 1],
  return: [0, -5, 5, 0, 0, 5, -5, 0],
});

export const CARRIED_ITEMS = Object.freeze([
  { ability: ABILITY.REGEN, label: "Regen", shortLabel: "RGN", color: 0x54e878, shape: "cross" },
  { ability: ABILITY.HASTE, label: "Haste", shortLabel: "HST", color: 0xffd15e, shape: "bolt" },
  { ability: ABILITY.DOUBLE_DAMAGE, label: "Double Damage", shortLabel: "DD", color: 0xff825d, shape: "sword" },
  { ability: ABILITY.ARMOR_TREASURE, label: "Armor Treasure", shortLabel: "ARM", color: 0x73d8ff, shape: "shield" },
  { ability: ABILITY.VITALITY_TREASURE, label: "Vitality Treasure", shortLabel: "VIT", color: 0x87edaa, shape: "heart" },
  { ability: ABILITY.ESCAPE_TREASURE, label: "Escape Treasure", shortLabel: "ESC", color: 0xdfa5ff, shape: "return" },
  { ability: ABILITY.DOUBLE_XP, label: "2X XP", shortLabel: "2X", color: 0x68a9ff, shape: "xp" },
  { ability: ABILITY.BREAKTHROUGH_TREASURE, label: "Breakthrough", shortLabel: "BT", color: 0xafffff, shape: "bolt" },
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
