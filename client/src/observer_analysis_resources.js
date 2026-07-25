import { playerAnalysisRows } from "./observer_analysis_rows.js";
import { resourceValueElement } from "./resource_icons.js";

export function normalizeResourceWindows(resources) {
  return {
    lifetime: normalizeResourceTotals(resources?.lifetime),
    last5s: normalizeResourceTotals(resources?.last5s),
    lastMinute: normalizeResourceTotals(resources?.lastMinute),
  };
}

export function renderResourcesMetric({ analysis, players }) {
  const wrap = renderAnalysisMetric("replay-resources", "Mined resources");
  const rows = playerAnalysisRows({ analysis, players });
  if (!analysis) {
    wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
    return wrap;
  }
  if (!rows.length) {
    wrap.appendChild(renderEmptyMetric("No players"));
    return wrap;
  }

  for (const window of RESOURCE_WINDOWS) {
    wrap.appendChild(renderResourceWindowGroup({
      label: window.label,
      resourceKey: window.resourceKey,
      players: rows,
    }));
  }
  return wrap;
}

export function renderAliveResourcesMetric({ analysis, players }) {
  const wrap = renderAnalysisMetric("replay-alive-resources", "Lifetime resources still alive");
  const note = document.createElement("div");
  note.className = "replay-analysis-note";
  note.textContent = "Lifetime mined resources minus dead unit value.";
  wrap.appendChild(note);

  const rows = playerAnalysisRows({ analysis, players });
  if (!analysis) {
    wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
    return wrap;
  }
  if (!rows.length) {
    wrap.appendChild(renderEmptyMetric("No players"));
    return wrap;
  }

  for (const player of rows) {
    wrap.appendChild(renderAliveResourcesRow(player));
  }
  return wrap;
}

const RESOURCE_WINDOWS = [
  { label: "Last 5s", resourceKey: "last5s" },
  { label: "Last 1m", resourceKey: "lastMinute" },
  { label: "Lifetime", resourceKey: "lifetime" },
];

function normalizeResourceTotals(totals) {
  return {
    steel: Math.max(0, Math.trunc(Number(totals?.steel) || 0)),
    oil: Math.max(0, Math.trunc(Number(totals?.oil) || 0)),
  };
}

function renderAnalysisMetric(className, headingText) {
  const wrap = document.createElement("div");
  wrap.className = `replay-analysis-metric ${className}`;
  const heading = document.createElement("div");
  heading.className = "replay-analysis-metric-heading";
  heading.textContent = headingText;
  wrap.appendChild(heading);
  return wrap;
}

function renderEmptyMetric(text) {
  const empty = document.createElement("div");
  empty.className = "replay-analysis-empty";
  empty.textContent = text;
  return empty;
}

function renderResourceWindowGroup({ label, resourceKey, players }) {
  const group = document.createElement("div");
  group.className = "replay-resources-group";

  const heading = document.createElement("div");
  heading.className = "replay-resources-window";
  heading.textContent = label;
  group.appendChild(heading);

  for (const player of players) {
    group.appendChild(renderResourcePlayerRow({
      name: player.name,
      color: player.color,
      totals: player.resources?.[resourceKey],
    }));
  }
  return group;
}

function renderResourcePlayerRow({ name, color, totals }) {
  const row = document.createElement("div");
  row.className = "replay-resources-row";

  const player = document.createElement("span");
  player.className = "replay-resources-player";

  const swatch = document.createElement("span");
  swatch.className = "replay-analysis-player-swatch";
  swatch.setAttribute("style", `background:${safeCssColor(color)};`);
  swatch.setAttribute("aria-hidden", "true");

  const nameEl = document.createElement("span");
  nameEl.className = "replay-resources-name";
  nameEl.textContent = name;
  player.append(swatch, nameEl);

  const steelEl = resourceValueElement("steel", totals?.steel || 0, "replay-resources-steel");
  const oilEl = resourceValueElement("oil", totals?.oil || 0, "replay-resources-oil");

  row.append(player, steelEl, oilEl);
  return row;
}

function renderAliveResourcesRow(player) {
  const row = document.createElement("div");
  row.className = "replay-resources-lost-row replay-alive-resources-row";

  const swatch = document.createElement("span");
  swatch.className = "replay-analysis-player-swatch";
  swatch.setAttribute("style", `background:${safeCssColor(player.color)};`);
  swatch.setAttribute("aria-hidden", "true");

  const name = document.createElement("span");
  name.className = "replay-resources-lost-name";
  name.textContent = player.name;

  const steel = player.resources.lifetime.steel - player.resourcesLost.steel;
  const oil = player.resources.lifetime.oil - player.resourcesLost.oil;
  row.append(
    swatch,
    name,
    resourceValueElement("steel", steel, "replay-resources-lost-steel", { allowNegative: true }),
    resourceValueElement("oil", oil, "replay-resources-lost-oil", { allowNegative: true }),
  );
  return row;
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}
