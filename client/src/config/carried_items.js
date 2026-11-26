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

// Short authored HUD copy. Costs come from the ability affordance, never this table.
export const CARRIED_ITEMS = Object.freeze([
  { ability: ABILITY.REGEN, tooltipName: "HP Regen", summary: "+3 HP per second", qualifier: "Permanent", label: "Regen", shortLabel: "RGN", color: 0x54e878, shape: "cross" },
  { ability: ABILITY.HASTE, tooltipName: "Haste", summary: "+15% move speed · +25% primary weapon attack speed", qualifier: "Permanent · Attack while moving", label: "Haste", shortLabel: "HST", color: 0xffd15e, shape: "bolt" },
  { ability: ABILITY.DOUBLE_DAMAGE, tooltipName: "Double Damage", summary: "2× primary weapon damage", qualifier: "Permanent", label: "Double Damage", shortLabel: "DD", color: 0xff825d, shape: "sword" },
  { ability: ABILITY.ARMOR_TREASURE, tooltipName: "Armor Treasure", summary: "+50 percentage points of armor", qualifier: "Permanent · 100% cap before armor piercing", label: "Armor Treasure", shortLabel: "ARM", color: 0x73d8ff, shape: "shield" },
  { ability: ABILITY.VITALITY_TREASURE, tooltipName: "Vitality Treasure", summary: "+150 max and current HP", qualifier: "Permanent", label: "Vitality Treasure", shortLabel: "VIT", color: 0x87edaa, shape: "heart" },
  { ability: ABILITY.ESCAPE_TREASURE, tooltipName: "Escape Treasure", summary: "Lethal damage returns unit to Shop at 1 HP", qualifier: "One use · 60s purchase cooldown", label: "Escape Treasure", shortLabel: "ESC", color: 0xdfa5ff, shape: "return" },
  { ability: ABILITY.DOUBLE_XP, tooltipName: "2X XP", summary: "2× Insight gain", qualifier: "Permanent", label: "2X XP", shortLabel: "2X", color: 0x68a9ff, shape: "xp" },
  { ability: ABILITY.BREAKTHROUGH_TREASURE, tooltipName: "Breakthrough", summary: "+25% move speed within 3 tiles", qualifier: "Permanent · Auras do not stack", label: "Breakthrough", shortLabel: "BT", color: 0xafffff, shape: "bolt" },
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
