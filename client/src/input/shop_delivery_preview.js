import { KIND, ABILITY, isUnit } from "../protocol.js";
import { ownOwner, friendlyOwner } from "./commands.js";
import { carriedItemSlotsFull } from "./shop_full_cursor.js";

export function updateShopDeliveryPreview(input) {
  const intent = input._intent();
  intent?.updateShopDeliveryPreview(null);
  if (!intent || !input.mouse || input._drag || intent.placement || intent.activeLabTool
      || input.inputRouter?.activePreviewSurface?.()) return;
  const mode = intent.commandTarget;
  if (mode && !(mode.kind === "ability" && [ABILITY.HEALING_POTION,
    ABILITY.ESCAPE_TREASURE, ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE,
    ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE].includes(mode.ability))) return;
  const shop = input.state.selectedEntities().find((e) => e.kind === KIND.SHOP
    && e.hp > 0 && e.buildProgress == null && ownOwner(input.state, e.owner, input.controlPolicy));
  if (!shop) return;
  const target = input._entityAtScreen(input.mouse, false, (e) => isUnit(e.kind)
    && friendlyOwner(input.state, e.owner, input.controlPolicy));
  if (!target || target.hp <= 0 || target.visionOnly || (mode?.ability !== ABILITY.HEALING_POTION && carriedItemSlotsFull(target))) return;
  if (mode?.ability === ABILITY.ESCAPE_TREASURE && target.escapeTreasure) return;
  if (mode && target.abilities?.some((entry) => entry.ability === mode.ability)) return;
  intent.updateShopDeliveryPreview({ x: shop.x, y: shop.y, targetX: target.x, targetY: target.y });
}
