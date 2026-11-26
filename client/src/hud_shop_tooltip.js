import { CARRIED_ITEMS } from "./config.js";
import { resourceIconHtml } from "./resource_icons.js";

const escapeHtml = (value) => String(value).replace(/[&<>"']/g, (c) =>
  ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

// The same title/cost/description markup and styling as training tooltips.
export function shopItemTooltipHtml(ability, cost = {}, status = "") {
  const item = CARRIED_ITEMS.find((entry) => entry.ability === ability);
  if (!item) return "";
  const costs = ["steel", "oil"].filter((kind) => cost?.[kind]).map((kind) =>
    `<span class="cmd-tooltip-cost" aria-label="${escapeHtml(`${cost[kind]} ${kind}`)}">` +
      `${resourceIconHtml(kind)}<span>${escapeHtml(cost[kind])}</span></span>`).join("") || "Free";
  return `<span class="cmd-tooltip-title">${escapeHtml(item.tooltipName)}</span>` +
    `<span class="cmd-tooltip-costs">${costs}</span>` +
    `<span class="cmd-tooltip-desc">${escapeHtml(`${item.summary} · ${item.qualifier}`)}</span>` +
    (status ? `<span class="cmd-tooltip-desc">${escapeHtml(status)}</span>` : "");
}
