import { CARRIED_ITEMS } from "./config.js";

const escapeHtml = (value) => String(value).replace(/[&<>"']/g, (c) =>
  ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

// Content for the existing command-card tooltip, shared by Shop and Buy menu.
export function shopItemTooltipHtml(ability, cost = {}, status = "") {
  const item = CARRIED_ITEMS.find((entry) => entry.ability === ability);
  if (!item) return "";
  const costs = [cost?.steel ? `${cost.steel} steel` : "", cost?.oil ? `${cost.oil} oil` : ""]
    .filter(Boolean).join(" · ") || "Free";
  return `<span class="cmd-tooltip-title">${escapeHtml(item.tooltipName)}</span>` +
    `<span class="cmd-shop-effect">${escapeHtml(item.summary)}` +
      `<span class="cmd-shop-qualifier"> · ${escapeHtml(item.qualifier)}</span></span>` +
    `<span class="cmd-tooltip-costs">${escapeHtml(costs)}</span>` +
    (status ? `<span class="cmd-shop-status">${escapeHtml(status)}</span>` : "");
}
