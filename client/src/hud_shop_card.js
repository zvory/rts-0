import { shopItemTooltipHtml } from "./hud_shop_tooltip.js";
import { KIND } from "./protocol.js";
import { gridHotkeyForSlot } from "./hud_command_layout.js";

export function isShopBuyer(entity) {
  return [KIND.WARRIOR, KIND.WIND_ADEPT, KIND.WAR_ADEPT, KIND.MAGIC_ADEPT].includes(entity.kind);
}

export function shopBuyMenuDescriptor() {
  return {
    id: "shop:buy-menu", commandId: "cultivators.item.buyMenu", kind: "button", action: "openShopBuyMenu",
    intent: { type: "openShopBuyMenu" }, icon: "BUY", label: "Buy menu", enabled: true,
    title: "Buy items for selected cultivators: Master, Adept, Novice; then insight and carried items",
  };
}

export function buildShopBuyCard(ctx, isOwn, buildTrainCard) {
  if (ctx.commandCardMode !== "shopBuy") return null;
  const buyers = (ctx.selection || []).filter((e) => isOwn(ctx, e) && isShopBuyer(e));
  if (!buyers.length) return null;
  const shops = (ctx.currentEntities || ctx.entities || []).filter((e) =>
    isOwn(ctx, e) && e.kind === KIND.SHOP && e.hp > 0 && e.buildProgress == null)
    .sort((a, b) => a.id - b.id);
  const result = buildTrainCard(ctx, shops[0] || { id: 0, owner: ctx.playerId, kind: KIND.SHOP });
  for (const slot of result.slots) {
    if (slot?.action !== "ability") continue;
    slot.intent = { type: "buyShopItem", ability: slot.ability };
    slot.enabled &&= shops.length > 0;
    slot.unaffordable &&= shops.length > 0;
    if (!shops.length) {
      slot.title = "Requires a completed Shop";
      slot.tooltipHtml = shopItemTooltipHtml(slot.ability, slot.cost, slot.title);
    }
  }
  result.slots[11] = {
    id: "shop:return", commandId: "cultivators.item.returnBuyMenu", kind: "button", action: "return",
    intent: { type: "closeCommandCardMenu" }, icon: "RTN", label: "Return", enabled: true,
    slotIndex: 11, hotkey: gridHotkeyForSlot(11), hotkeyCode: "KeyV",
  };
  result.signature += `|buy:${buyers.map((e) => e.id).join(".")}|shops:${shops.map((e) => e.id).join(".")}`;
  return result;
}

export function shopCardExtras(building, underConstruction, abilityAffordances) {
  return building.kind === KIND.SHOP && !underConstruction ? { rowLabels: ["Util", "Defense", "Offense"], abilityAffordances } : {};
}
