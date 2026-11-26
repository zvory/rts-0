import { KIND } from "../protocol.js";
import { CULTIVATOR_NOVICE_ITEM_CAPACITY, CULTIVATOR_ITEM_CAPACITY, CULTIVATOR_MASTER_ITEM_CAPACITY } from "../config.js";

const FIGHTERS = new Set([KIND.WARRIOR, KIND.WIND_ADEPT, KIND.WAR_ADEPT, KIND.MAGIC_ADEPT]);

export function carriedItemSlotsFull(entity) {
  if (!FIGHTERS.has(entity?.kind)) return false;
  const capacity = entity.daoMaster ? CULTIVATOR_MASTER_ITEM_CAPACITY
    : entity.kind === KIND.WARRIOR ? CULTIVATOR_NOVICE_ITEM_CAPACITY : CULTIVATOR_ITEM_CAPACITY;
  return Number.isInteger(entity.itemRadianceTier) && entity.itemRadianceTier >= capacity;
}

/** Viewport-local text follows both the ordinary and locked gameplay cursor. */
export class ShopFullCursor {
  constructor(dom) {
    this.dom = dom;
    this.label = document.createElement("div");
    this.label.className = "shop-full-cursor";
    this.label.textContent = "Full";
    Object.assign(this.label.style, {
      position: "absolute", left: "0", top: "0", zIndex: "30", pointerEvents: "none",
      color: "#fff", font: "22px Arial, sans-serif", whiteSpace: "nowrap",
      webkitTextStroke: "2px #000", paintOrder: "stroke fill",
    });
    this.label.hidden = true;
    dom.appendChild(this.label);
  }

  update(input) {
    const visible = !!input.mouse && !!input._shopFullCursorTarget();
    this.label.hidden = !visible;
    if (!visible) return;
    const x = Math.max(0, Math.min(input.mouse.x + 16, this.dom.clientWidth - 64));
    const y = Math.max(0, Math.min(input.mouse.y + 18, this.dom.clientHeight - 32));
    this.label.style.transform = `translate(${x}px, ${y}px)`;
  }

  destroy() {
    this.label.remove();
  }
}
