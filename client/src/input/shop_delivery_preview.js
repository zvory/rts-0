import { KIND, ABILITY, isUnit } from "../protocol.js";
import { ownOwner, friendlyOwner } from "./commands.js";
import { carriedItemSlotsFull } from "./shop_full_cursor.js";

export function updateShopDeliveryPreview(input) {
  const intent = input._intent();
  intent?.updateShopDeliveryPreview(null);
  if (!intent || input._drag || intent.placement || intent.activeLabTool
      || input.inputRouter?.activePreviewSurface?.()) return;
  if (intent.commandCardMode === "shopBuy") {
    const selected = input.state.selectedEntities().filter((e) =>
      [KIND.WARRIOR, KIND.WIND_ADEPT, KIND.WAR_ADEPT, KIND.MAGIC_ADEPT].includes(e.kind)
      && e.hp > 0 && !e.visionOnly && ownOwner(input.state, e.owner, input.controlPolicy));
    const owner = selected[0]?.owner;
    const shop = input.state.entitiesInterpolated(1).filter((e) => e.owner === owner
      && e.kind === KIND.SHOP && e.hp > 0 && e.buildProgress == null).sort((a, b) => a.id - b.id)[0];
    if (shop) intent.updateShopDeliveryPreview({ lines: selected.map((target) =>
      ({ x: shop.x, y: shop.y, targetX: target.x, targetY: target.y })) });
    return;
  }
  if (!input.mouse) return;
  const mode = intent.commandTarget;
  if (mode && !(mode.kind === "ability" && [ABILITY.REGEN,
    ABILITY.ESCAPE_TREASURE, ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE,
    ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE].includes(mode.ability))) return;
  const shop = input.state.selectedEntities().find((e) => e.kind === KIND.SHOP
    && e.hp > 0 && e.buildProgress == null && ownOwner(input.state, e.owner, input.controlPolicy));
  if (!shop) return;
  const target = input._entityAtScreen(input.mouse, false, (e) => isUnit(e.kind)
    && friendlyOwner(input.state, e.owner, input.controlPolicy));
  if (!target || target.hp <= 0 || target.visionOnly || carriedItemSlotsFull(target)) return;
  if (mode?.ability === ABILITY.ESCAPE_TREASURE && target.escapeTreasure) return;
  if (mode && target.abilities?.some((entry) => entry.ability === mode.ability)) return;
  intent.updateShopDeliveryPreview({ x: shop.x, y: shop.y, targetX: target.x, targetY: target.y });
}
