import { ABILITIES, carriedItemMask, carriedItemsForEntity, statsForFaction } from "./config.js";

let summarySerial = 0;

export function carriedItemDetails(entity, players = []) {
  const items = carriedItemsForEntity(entity);
  const faction = players.find((player) => player.id === entity.owner)?.factionId;
  const label = statsForFaction(entity.kind, faction, entity.dao, entity.daoMaster)?.label || entity.kind;
  const oil = items.reduce((sum, item) => sum + (ABILITIES[item.ability]?.cost?.oil || 0), 0);
  return `${label} · ${items.length} carried ${items.length === 1 ? "item" : "items"} · ${oil} oil value\n` +
    (items.length ? items.map((item) => item.label).join(" · ") : "No carried items");
}

// Owns only selection-inspection listeners; it never issues gameplay commands.
export class HudCarriedItems {
  constructor(panel) {
    this.panel = panel;
    this.entities = [];
    this.players = [];
    this.focusId = null;
    this.summary = null;
    this.summaryId = `carried-items-${++summarySerial}`;
    this.onFocus = (event) => {
      const node = event.target?.closest?.("[data-selection-entity-id]");
      if (!node || (this.panel.contains && !this.panel.contains(node))) return;
      const id = Number(node.getAttribute("data-selection-entity-id"));
      const entity = this.entities.find((entry) => entry.id === id);
      if (!entity || !this.summary) return;
      this.focusId = id;
      this.summary.textContent = carriedItemDetails(entity, this.players);
    };
    panel?.addEventListener?.("pointerover", this.onFocus);
    panel?.addEventListener?.("focusin", this.onFocus);
  }

  setSelection(entities, players = []) {
    this.entities = entities;
    if (!entities.length) this.summary = null;
    this.players = players;
    if (!entities.some((entity) => entity.id === this.focusId)) {
      const equipped = entities.filter((entity) => carriedItemMask(entity));
      this.focusId = equipped.sort((a, b) =>
        carriedItemsForEntity(b).length - carriedItemsForEntity(a).length || a.id - b.id)[0]?.id ?? null;
    }
  }

  createSummary() {
    this.summary = null;
    if (!this.entities.some((entity) => carriedItemMask(entity))) return null;
    const entity = this.entities.find((entry) => entry.id === this.focusId) || this.entities[0];
    const summary = document.createElement("div");
    summary.className = "sel-item-summary";
    summary.id = this.summaryId;
    summary.setAttribute("role", "status");
    summary.setAttribute("aria-live", "polite");
    summary.textContent = carriedItemDetails(entity, this.players);
    this.summary = summary;
    return summary;
  }

  decoratePortrait(node, entity) {
    const items = carriedItemsForEntity(entity);
    if (!items.length) return;
    const badge = document.createElement("span");
    badge.className = "sel-item-count";
    badge.textContent = String(items.length);
    badge.setAttribute("aria-hidden", "true");
    node.appendChild(badge);
    if (entity.escapeTreasure === true) node.className += " has-carried-escape";
    node.title += `\n${carriedItemDetails(entity, this.players)}`;
    node.setAttribute("aria-label", `${node.getAttribute("aria-label")} · ${items.length} carried items`);
    node.setAttribute("aria-describedby", this.summaryId);
  }

  destroy() {
    this.panel?.removeEventListener?.("pointerover", this.onFocus);
    this.panel?.removeEventListener?.("focusin", this.onFocus);
    this.entities = [];
    this.players = [];
    this.summary = null;
    this.panel = null;
  }
}
