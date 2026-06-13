const RESOURCE_ICON_FALLBACKS = Object.freeze({
  steel: "▰",
  oil: "⬤",
  supply: "▲",
});

export function resourceIconHtml(kind) {
  return `<span class="res-icon ${kind}">${RESOURCE_ICON_FALLBACKS[kind] || ""}</span>`;
}

export function resourceValueElement(kind, value, className) {
  const el = document.createElement("span");
  el.className = className;
  el.title = resourceTitle(kind);

  const icon = document.createElement("span");
  icon.className = `res-icon ${kind}`;
  icon.textContent = RESOURCE_ICON_FALLBACKS[kind] || "";
  icon.setAttribute("aria-hidden", "true");
  const number = document.createElement("span");
  number.className = "resource-value-number";
  number.textContent = formatResourceValue(value);
  el.append(icon, number);
  return el;
}

function resourceTitle(kind) {
  if (kind === "steel") return "Steel value";
  if (kind === "oil") return "Oil value";
  if (kind === "supply") return "Supply";
  return "Resource value";
}

function formatResourceValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}
