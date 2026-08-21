import { playerAnalysisRows } from "./observer_analysis_rows.js";
import { resourceIconHtml, resourceValueElement } from "./resource_icons.js";
import { HARVEST_TICKS, OIL_LOAD, TICK_HZ } from "./config.js";

const SAMPLE_INTERVAL_TICKS = 30;
const SVG_NS = "http://www.w3.org/2000/svg";
export const RESOURCE_COLLECTION_WINDOW_SECONDS = 8;
const COLLECTION_WINDOW_TICKS = TICK_HZ * RESOURCE_COLLECTION_WINDOW_SECONDS;
const STEEL_LOAD = 2;
const STEEL_PATCHES_PER_BASE = 12;
const OIL_PATCHES_PER_BASE = 3;
const HARVESTS_PER_WINDOW = COLLECTION_WINDOW_TICKS / HARVEST_TICKS;
export const RESOURCE_ADVANTAGE_MIN_EXTENT = Object.freeze({
  steel: STEEL_PATCHES_PER_BASE * STEEL_LOAD * HARVESTS_PER_WINDOW,
  oil: OIL_PATCHES_PER_BASE * OIL_LOAD * HARVESTS_PER_WINDOW,
});

export class ResourceCollectionHistory {
  constructor() {
    this.samples = [];
  }

  record(analysis) {
    const tick = Math.max(0, Math.trunc(Number(analysis?.tick) || 0));
    const players = Array.isArray(analysis?.players) ? analysis.players : [];
    if (players.length !== 2) {
      this.samples = [];
      return;
    }

    const ids = players.map((player) => player.id).sort((a, b) => a - b);
    const last = this.samples[this.samples.length - 1];
    if (last && (last.playerIds[0] !== ids[0] || last.playerIds[1] !== ids[1])) {
      this.samples = [];
    } else if (last && tick < last.tick) {
      this.samples = this.samples.filter((sample) => sample.tick <= tick);
    }

    const current = this.samples[this.samples.length - 1];
    if (current && tick - current.tick < SAMPLE_INTERVAL_TICKS) return;
    const byId = new Map(players.map((player) => [player.id, player]));
    const first = byId.get(ids[0]);
    const second = byId.get(ids[1]);
    if (!first || !second) return;

    const cumulativeSteel = (first.resources?.lifetime?.steel || 0) - (second.resources?.lifetime?.steel || 0);
    const cumulativeOil = (first.resources?.lifetime?.oil || 0) - (second.resources?.lifetime?.oil || 0);
    const targetTick = tick - COLLECTION_WINDOW_TICKS;
    const baseline = [...this.samples].reverse().find((sample) => sample.tick <= targetTick);
    const hasWindowBaseline = baseline && targetTick - baseline.tick <= SAMPLE_INTERVAL_TICKS;
    this.samples.push({
      tick,
      playerIds: ids,
      cumulativeSteel,
      cumulativeOil,
      steel: hasWindowBaseline ? cumulativeSteel - baseline.cumulativeSteel : 0,
      oil: hasWindowBaseline ? cumulativeOil - baseline.cumulativeOil : 0,
    });
  }
}

export function normalizeResourceWindows(resources) {
  return {
    lifetime: normalizeResourceTotals(resources?.lifetime),
    last5s: normalizeResourceTotals(resources?.last5s),
    lastMinute: normalizeResourceTotals(resources?.lastMinute),
  };
}

export function renderResourcesMetric({ analysis, players, collectionHistory = [] }) {
  const wrap = document.createElement("div");
  wrap.className = "replay-analysis-metric replay-resources";
  const rows = playerAnalysisRows({ analysis, players });
  if (!analysis) {
    wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
    return wrap;
  }
  if (!rows.length) {
    wrap.appendChild(renderEmptyMetric("No players"));
    return wrap;
  }

  wrap.appendChild(renderCollectionAdvantage({
    rows,
    samples: collectionHistory,
    currentTick: analysis.tick,
  }));

  for (const window of RESOURCE_WINDOWS) {
    wrap.appendChild(renderResourceWindowGroup({
      label: window.label,
      resourceKey: window.resourceKey,
      players: rows,
    }));
  }
  return wrap;
}

export function collectionAdvantageAreaPoints(
  samples,
  resource,
  width = 420,
  height = 92,
  minExtent = 0,
  currentTick = null,
) {
  if (!Array.isArray(samples) || samples.length === 0) return [];
  const mid = height / 2;
  const extent = Math.max(
    1,
    Number(minExtent) || 0,
    ...samples.map((sample) => Math.abs(Number(sample?.[resource]) || 0)),
  );
  const lastSampleTick = Math.max(0, Number(samples[samples.length - 1]?.tick) || 0);
  const lastTick = Math.max(lastSampleTick, Number(currentTick) || 0);
  const tickSpan = Math.max(1, lastTick);
  return samples.map((sample) => ({
    x: Math.max(0, Math.min(1, (Number(sample?.tick) || 0) / tickSpan)) * width,
    y: mid - ((Number(sample?.[resource]) || 0) / extent) * (mid - 5),
  }));
}

export function renderAliveResourcesMetric({ analysis, players }) {
  const wrap = renderAnalysisMetric("replay-alive-resources", "Lifetime resources still alive");
  const note = document.createElement("div");
  note.className = "replay-analysis-note";
  note.textContent = "Lifetime mined resources minus destroyed unit and building value.";
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

function renderCollectionAdvantage({ rows, samples, currentTick }) {
  const section = document.createElement("section");
  section.className = "replay-resource-advantage";

  if (rows.length !== 2) {
    section.appendChild(renderEmptyMetric("Collection graphs are shown for 1v1 replays."));
    return section;
  }

  if (!Array.isArray(samples) || samples.length < 2) {
    section.appendChild(renderEmptyMetric("Play the replay to build its collection timeline."));
    return section;
  }

  section.append(
    renderAdvantageChart({ resource: "steel", label: "Steel", rows, samples, currentTick, serial: 0 }),
    renderAdvantageChart({ resource: "oil", label: "Oil", rows, samples, currentTick, serial: 1 }),
  );
  return section;
}

function renderAdvantageChart({ resource, label, rows, samples, currentTick, serial }) {
  const width = 420;
  const chartHeight = 92;
  const labelHeight = 18;
  const plotLeft = 100;
  const mid = chartHeight / 2;
  const points = collectionAdvantageAreaPoints(
    samples,
    resource,
    width - plotLeft,
    chartHeight,
    RESOURCE_ADVANTAGE_MIN_EXTENT[resource],
    currentTick,
  )
    .map((point) => ({ ...point, x: point.x + plotLeft }));
  const curve = points.map((point) => `${point.x.toFixed(1)},${point.y.toFixed(1)}`).join(" L ");
  const area = `M ${plotLeft},${mid} L ${curve} L ${width},${mid} Z`;
  const clipTopId = `resource-advantage-top-${resource}-${serial}`;
  const clipBottomId = `resource-advantage-bottom-${resource}-${serial}`;

  const figure = document.createElement("figure");
  figure.className = "replay-resource-advantage-chart";
  const caption = document.createElement("figcaption");
  caption.className = "replay-resource-advantage-chart-heading";
  caption.innerHTML = `${resourceIconHtml(resource)}<span>${label}</span>`;
  figure.appendChild(caption);

  const svg = svgElement("svg", {
    viewBox: `0 0 ${width} ${chartHeight + labelHeight}`,
    role: "img",
    "aria-label": `${label} collection advantage over replay time. ${rows[0].name} is above the center line and ${rows[1].name} is below.`,
  });
  const defs = svgElement("defs");
  const topClip = svgElement("clipPath", { id: clipTopId });
  topClip.appendChild(svgElement("rect", { x: 0, y: 0, width, height: mid }));
  const bottomClip = svgElement("clipPath", { id: clipBottomId });
  bottomClip.appendChild(svgElement("rect", { x: 0, y: mid, width, height: mid }));
  defs.append(topClip, bottomClip);
  svg.appendChild(defs);
  svg.appendChild(svgElement("path", {
    d: area,
    fill: safeCssColor(rows[0].color),
    opacity: "0.72",
    "clip-path": `url(#${clipTopId})`,
  }));
  svg.appendChild(svgElement("path", {
    d: area,
    fill: safeCssColor(rows[1].color),
    opacity: "0.72",
    "clip-path": `url(#${clipBottomId})`,
  }));
  svg.appendChild(svgElement("path", {
    d: `M 0,${mid} H ${width}`,
    class: "replay-resource-advantage-zero",
  }));
  svg.appendChild(svgElement("path", {
    d: `M ${curve}`,
    class: "replay-resource-advantage-outline",
  }));
  appendPlayerLabel(svg, rows[0], 8, mid - 10);
  appendPlayerLabel(svg, rows[1], 8, mid + 16);
  const lastSampleTick = Math.max(0, Number(samples[samples.length - 1]?.tick) || 0);
  const lastTick = Math.max(lastSampleTick, Number(currentTick) || 0);
  appendTimeLabel(svg, plotLeft, chartHeight + 13, formatReplayTick(0), "start");
  appendTimeLabel(
    svg,
    plotLeft + ((width - plotLeft) / 2),
    chartHeight + 13,
    formatReplayTick(lastTick / 2),
    "middle",
  );
  appendTimeLabel(svg, width, chartHeight + 13, formatReplayTick(lastTick), "end");
  figure.appendChild(svg);
  return figure;
}

function appendPlayerLabel(svg, player, x, y) {
  const label = svgElement("text", {
    x,
    y,
    class: "replay-resource-advantage-player-label",
  });
  label.textContent = player.name;
  svg.appendChild(label);
}

function svgElement(tag, attributes = {}) {
  const element = document.createElementNS?.(SVG_NS, tag) || document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, String(value));
  return element;
}

function appendTimeLabel(svg, x, y, value, anchor) {
  const label = svgElement("text", { x, y, "text-anchor": anchor });
  label.textContent = value;
  svg.appendChild(label);
}

function formatReplayTick(tick) {
  const seconds = Math.max(0, Math.floor((Number(tick) || 0) / TICK_HZ));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
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
