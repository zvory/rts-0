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

  const total = rows.reduce((acc, player) => addResourceWindows(acc, player.resources), emptyResourceWindows());
  wrap.appendChild(renderResourceWindowGroup({
    className: "replay-resources-group is-total",
    name: "Total",
    color: "#e7dfc5",
    resources: total,
  }));

  for (const player of rows) {
    wrap.appendChild(renderResourceWindowGroup({
      className: "replay-resources-group",
      name: player.name,
      color: player.color,
      resources: player.resources,
    }));
  }
  return wrap;
}

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

function renderResourceWindowGroup({ className, name, color, resources }) {
  const group = document.createElement("div");
  group.className = className;

  const heading = document.createElement("div");
  heading.className = "replay-resources-heading";

  const swatch = document.createElement("span");
  swatch.className = "replay-analysis-player-swatch";
  swatch.setAttribute("style", `background:${safeCssColor(color)};`);
  swatch.setAttribute("aria-hidden", "true");

  const nameEl = document.createElement("span");
  nameEl.className = "replay-resources-name";
  nameEl.textContent = name;
  heading.append(swatch, nameEl);
  group.appendChild(heading);

  group.appendChild(renderResourceWindowRow("Lifetime", resources?.lifetime));
  group.appendChild(renderResourceWindowRow("Last 5s", resources?.last5s));
  group.appendChild(renderResourceWindowRow("Last 1m", resources?.lastMinute));
  return group;
}

function renderResourceWindowRow(label, totals) {
  const row = document.createElement("div");
  row.className = "replay-resources-row";

  const labelEl = document.createElement("span");
  labelEl.className = "replay-resources-window";
  labelEl.textContent = label;

  const steelEl = resourceValueElement("steel", totals?.steel || 0, "replay-resources-steel");
  const oilEl = resourceValueElement("oil", totals?.oil || 0, "replay-resources-oil");

  row.append(labelEl, steelEl, oilEl);
  return row;
}

function emptyResourceWindows() {
  return {
    lifetime: { steel: 0, oil: 0 },
    last5s: { steel: 0, oil: 0 },
    lastMinute: { steel: 0, oil: 0 },
  };
}

function addResourceWindows(acc, resources) {
  for (const key of ["lifetime", "last5s", "lastMinute"]) {
    acc[key].steel += Math.max(0, Math.trunc(Number(resources?.[key]?.steel) || 0));
    acc[key].oil += Math.max(0, Math.trunc(Number(resources?.[key]?.oil) || 0));
  }
  return acc;
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}
