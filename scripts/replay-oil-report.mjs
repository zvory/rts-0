#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

const KINDS = ["scout_car", "command_car", "tank"];
const LABELS = { scout_car: "Scout cars", command_car: "Command cars", tank: "Tanks" };
const COLORS = { scout_car: "#2672b8", command_car: "#d17822", tank: "#48834a" };

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) throw new Error("expected --analyses DIR --manifest FILE --out DIR");
    options[flag.slice(2)] = value;
  }
  if (!options.analyses || !options.manifest || !options.out) throw new Error("expected --analyses DIR --manifest FILE --out DIR");
  return options;
}

function csvCell(value) {
  if (value === null || value === undefined) return "";
  const text = String(value);
  return /[",\n\r]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function csv(headers, rows) {
  return `${headers.join(",")}\n${rows.map((row) => headers.map((header) => csvCell(row[header])).join(",")).join("\n")}\n`;
}

function nearestRank(sorted, quantile) {
  if (!sorted.length) return null;
  return sorted[Math.max(0, Math.ceil(quantile * sorted.length) - 1)];
}

function summarize(rows, kind) {
  const all = rows.filter((row) => row.unit_kind === kind);
  const values = all.filter((row) => row.moved === 1).map((row) => row.lifetime_oil_spend).sort((a, b) => a - b);
  const mean = values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : null;
  return {
    unit_kind: kind,
    total_observed: all.length,
    moved_count: values.length,
    unmoved_count: all.length - values.length,
    mean,
    p50: nearestRank(values, 0.50),
    p75: nearestRank(values, 0.75),
    p95: nearestRank(values, 0.95),
    p99: nearestRank(values, 0.99),
    max: values.length ? values.at(-1) : null,
    percentile_method: "nearest-rank: sorted[ceil(q*n)-1]",
  };
}

function fmt(value, digits = 2) {
  return value === null ? "—" : Number(value).toFixed(digits);
}

function escapeXml(value) {
  return String(value).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function percentileSvg(summary) {
  const width = 980;
  const height = 500;
  const metrics = ["mean", "p50", "p75", "p95", "p99", "max"];
  const globalMax = Math.max(1, ...summary.flatMap((row) => metrics.map((key) => row[key] ?? 0)));
  const plotLeft = 160;
  const plotWidth = 750;
  const lines = [];
  lines.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">`);
  lines.push(`<rect width="100%" height="100%" fill="#fbfaf7"/><text x="32" y="40" font-family="system-ui" font-size="24" font-weight="700">Moved-unit lifetime oil: mean and tail</text>`);
  for (let tick = 0; tick <= 5; tick++) {
    const value = (globalMax * tick) / 5;
    const x = plotLeft + (plotWidth * tick) / 5;
    lines.push(`<line x1="${x}" y1="70" x2="${x}" y2="445" stroke="#dedbd3"/><text x="${x}" y="468" text-anchor="middle" font-family="system-ui" font-size="12" fill="#555">${fmt(value, 0)}</text>`);
  }
  summary.forEach((row, rowIndex) => {
    const top = 95 + rowIndex * 115;
    lines.push(`<text x="32" y="${top + 38}" font-family="system-ui" font-size="17" font-weight="650" fill="${COLORS[row.unit_kind]}">${LABELS[row.unit_kind]}</text>`);
    metrics.forEach((metric, metricIndex) => {
      const value = row[metric];
      if (value === null) return;
      const y = top + metricIndex * 14;
      const x = plotLeft + (value / globalMax) * plotWidth;
      const labelOnLeft = x > plotLeft + plotWidth - 105;
      const labelX = labelOnLeft ? x - 7 : x + 7;
      const labelAnchor = labelOnLeft ? "end" : "start";
      lines.push(`<line x1="${plotLeft}" y1="${y}" x2="${x}" y2="${y}" stroke="${COLORS[row.unit_kind]}" stroke-width="3" opacity="0.75"/>`);
      lines.push(`<circle cx="${x}" cy="${y}" r="4" fill="${COLORS[row.unit_kind]}"/><text x="${labelX}" y="${y + 4}" text-anchor="${labelAnchor}" font-family="system-ui" font-size="11">${metric.toUpperCase()} ${fmt(value)}</text>`);
    });
  });
  lines.push(`<text x="535" y="492" text-anchor="middle" font-family="system-ui" font-size="13" fill="#555">modeled fractional oil</text></svg>`);
  return lines.join("");
}

function ecdfSvg(rows) {
  const width = 980;
  const panelWidth = 285;
  const panelHeight = 250;
  const lines = [`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="350" viewBox="0 0 ${width} 350">`, `<rect width="100%" height="100%" fill="#fbfaf7"/>`, `<text x="32" y="38" font-family="system-ui" font-size="24" font-weight="700">Lifetime movement-oil distribution (moved units)</text>`];
  KINDS.forEach((kind, panelIndex) => {
    const values = rows.filter((row) => row.unit_kind === kind && row.moved === 1).map((row) => row.lifetime_oil_spend).sort((a, b) => a - b);
    const left = 48 + panelIndex * 315;
    const top = 72;
    const max = Math.max(1, values.at(-1) ?? 1);
    lines.push(`<text x="${left}" y="${top - 10}" font-family="system-ui" font-size="17" font-weight="650" fill="${COLORS[kind]}">${LABELS[kind]} (n=${values.length})</text>`);
    lines.push(`<line x1="${left}" y1="${top}" x2="${left}" y2="${top + panelHeight}" stroke="#555"/><line x1="${left}" y1="${top + panelHeight}" x2="${left + panelWidth}" y2="${top + panelHeight}" stroke="#555"/>`);
    if (values.length) {
      const points = values.map((value, index) => `${left + (value / max) * panelWidth},${top + panelHeight - ((index + 1) / values.length) * panelHeight}`).join(" ");
      lines.push(`<polyline points="${points}" fill="none" stroke="${COLORS[kind]}" stroke-width="3"/>`);
      for (const value of values) {
        const x = left + (value / max) * panelWidth;
        lines.push(`<line x1="${x}" y1="${top + panelHeight - 6}" x2="${x}" y2="${top + panelHeight}" stroke="${COLORS[kind]}" opacity="0.35"/>`);
      }
    }
    lines.push(`<text x="${left - 8}" y="${top + 5}" text-anchor="end" font-family="system-ui" font-size="11">100%</text><text x="${left}" y="${top + panelHeight + 18}" font-family="system-ui" font-size="11">0</text><text x="${left + panelWidth}" y="${top + panelHeight + 18}" text-anchor="end" font-family="system-ui" font-size="11">${fmt(max, 1)} oil</text>`);
  });
  lines.push(`</svg>`);
  return lines.join("");
}

function inclusionSvg(summary) {
  const width = 760;
  const height = 330;
  const max = Math.max(1, ...summary.map((row) => row.total_observed));
  const lines = [`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">`, `<rect width="100%" height="100%" fill="#fbfaf7"/>`, `<text x="28" y="38" font-family="system-ui" font-size="24" font-weight="700">Observed vs moved units</text>`];
  summary.forEach((row, index) => {
    const y = 85 + index * 75;
    const totalWidth = (row.total_observed / max) * 520;
    const movedWidth = (row.moved_count / max) * 520;
    lines.push(`<text x="28" y="${y + 20}" font-family="system-ui" font-size="16">${LABELS[row.unit_kind]}</text><rect x="175" y="${y}" width="${totalWidth}" height="28" fill="#d7d3c9"/><rect x="175" y="${y}" width="${movedWidth}" height="28" fill="${COLORS[row.unit_kind]}"/><text x="${180 + totalWidth}" y="${y + 20}" font-family="system-ui" font-size="13">${row.moved_count} moved / ${row.total_observed} observed</text>`);
  });
  lines.push(`</svg>`);
  return lines.join("");
}

function writeDatabase(filename, matches, rows, summary, metadata) {
  const db = new DatabaseSync(filename);
  db.exec("PRAGMA foreign_keys=ON; CREATE TABLE analysis_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE matches(match_id INTEGER PRIMARY KEY,replay_number INTEGER NOT NULL,started_at TEXT NOT NULL,map_name TEXT NOT NULL,duration_ticks INTEGER NOT NULL,server_build_sha TEXT NOT NULL,analysis_build_sha TEXT NOT NULL,participants_json TEXT NOT NULL); CREATE TABLE unit_oil_spend(match_id INTEGER NOT NULL,entity_id INTEGER NOT NULL,owner_id INTEGER NOT NULL,owner_name TEXT NOT NULL,unit_kind TEXT NOT NULL CHECK(unit_kind IN ('scout_car','command_car','tank')),first_seen_tick INTEGER NOT NULL,last_seen_tick INTEGER NOT NULL,first_moved_tick INTEGER,last_moved_tick INTEGER,survived_to_end INTEGER NOT NULL CHECK(survived_to_end IN(0,1)),moved INTEGER NOT NULL CHECK(moved IN(0,1)),lifetime_oil_spend REAL NOT NULL CHECK(lifetime_oil_spend>=0),PRIMARY KEY(match_id,entity_id),FOREIGN KEY(match_id) REFERENCES matches(match_id)); CREATE TABLE unit_oil_summary(unit_kind TEXT PRIMARY KEY,total_observed INTEGER NOT NULL,moved_count INTEGER NOT NULL,unmoved_count INTEGER NOT NULL,mean REAL,p50 REAL,p75 REAL,p95 REAL,p99 REAL,max REAL,percentile_method TEXT NOT NULL); CREATE INDEX unit_oil_distribution ON unit_oil_spend(unit_kind,moved,lifetime_oil_spend); CREATE INDEX unit_oil_owner ON unit_oil_spend(owner_name,unit_kind);");
  const insertMeta = db.prepare("INSERT INTO analysis_metadata VALUES (?,?)");
  const insertMatch = db.prepare("INSERT INTO matches VALUES (?,?,?,?,?,?,?,?)");
  const insertUnit = db.prepare("INSERT INTO unit_oil_spend VALUES (?,?,?,?,?,?,?,?,?,?,?,?)");
  const insertSummary = db.prepare("INSERT INTO unit_oil_summary VALUES (?,?,?,?,?,?,?,?,?,?,?)");
  db.exec("BEGIN");
  for (const [key, value] of Object.entries(metadata)) insertMeta.run(key, String(value));
  for (const match of matches) insertMatch.run(match.match_id, match.match_id, match.started_at, match.map_name, match.duration_ticks, match.server_build_sha, match.analysis_build_sha, JSON.stringify(match.participants));
  for (const row of rows) insertUnit.run(row.match_id, row.entity_id, row.owner_id, row.owner_name, row.unit_kind, row.first_seen_tick, row.last_seen_tick, row.first_moved_tick, row.last_moved_tick, row.survived_to_end, row.moved, row.lifetime_oil_spend);
  for (const row of summary) insertSummary.run(...["unit_kind","total_observed","moved_count","unmoved_count","mean","p50","p75","p95","p99","max","percentile_method"].map((key) => row[key]));
  db.exec("COMMIT");
  db.close();
}

const options = parseArgs(process.argv.slice(2));
const manifest = JSON.parse(fs.readFileSync(options.manifest, "utf8"));
const manifestById = new Map(manifest.matches.map((match) => [Number(match.id), match]));
const analyses = fs.readdirSync(options.analyses).filter((name) => name.endsWith(".json")).sort((a, b) => Number.parseInt(a) - Number.parseInt(b)).map((name) => JSON.parse(fs.readFileSync(path.join(options.analyses, name), "utf8")));
if (analyses.length !== manifest.matches.length) throw new Error(`expected ${manifest.matches.length} analyses, found ${analyses.length}`);
const matches = [];
const rows = [];
for (const analysis of analyses) {
  const match = manifestById.get(analysis.matchId);
  if (!match) throw new Error(`analysis ${analysis.matchId} absent from manifest`);
  if (analysis.analysisBuildSha !== match.build_sha) throw new Error(`build mismatch for ${analysis.matchId}: ${analysis.analysisBuildSha} != ${match.build_sha}`);
  const names = new Map(analysis.replay.players.map((player) => [player.id, player.name]));
  matches.push({ match_id: analysis.matchId, started_at: match.started_at, map_name: match.map_name, duration_ticks: match.duration_ticks, server_build_sha: match.build_sha, analysis_build_sha: analysis.analysisBuildSha, participants: match.participants });
  for (const vehicle of analysis.vehicles) rows.push({ match_id: analysis.matchId, replay_number: analysis.matchId, started_at: match.started_at, map_name: match.map_name, duration_ticks: match.duration_ticks, entity_id: vehicle.entityId, owner_id: vehicle.ownerId, owner_name: names.get(vehicle.ownerId) ?? "", unit_kind: vehicle.unitKind, first_seen_tick: vehicle.firstSeenTick, last_seen_tick: vehicle.lastSeenTick, first_moved_tick: vehicle.firstMovedTick, last_moved_tick: vehicle.lastMovedTick, survived_to_end: Number(vehicle.survivedToEnd), moved: Number(vehicle.lifetimeOilSpend > 0), lifetime_oil_spend: vehicle.lifetimeOilSpend });
}
rows.sort((a, b) => a.match_id - b.match_id || a.entity_id - b.entity_id);
const summary = KINDS.map((kind) => summarize(rows, kind));
fs.mkdirSync(options.out, { recursive: true });
const unitHeaders = ["match_id","replay_number","started_at","map_name","duration_ticks","entity_id","owner_id","owner_name","unit_kind","first_seen_tick","last_seen_tick","first_moved_tick","last_moved_tick","survived_to_end","moved","lifetime_oil_spend"];
const summaryHeaders = ["unit_kind","total_observed","moved_count","unmoved_count","mean","p50","p75","p95","p99","max","percentile_method"];
fs.writeFileSync(path.join(options.out, "unit_oil_spend.csv"), csv(unitHeaders, rows));
fs.writeFileSync(path.join(options.out, "moved_unit_oil_spend.csv"), csv(unitHeaders, rows.filter((row) => row.moved === 1)));
fs.writeFileSync(path.join(options.out, "unit_oil_summary.csv"), csv(summaryHeaders, summary));
const percentileChart = percentileSvg(summary);
const ecdfChart = ecdfSvg(rows);
const inclusionChart = inclusionSvg(summary);
fs.writeFileSync(path.join(options.out, "oil-spend-percentiles.svg"), percentileChart);
fs.writeFileSync(path.join(options.out, "oil-spend-ecdf.svg"), ecdfChart);
fs.writeFileSync(path.join(options.out, "oil-spend-inclusion.svg"), inclusionChart);
const generatedAt = new Date().toISOString();
const metadata = { generated_at: generatedAt, selection: manifest.selection, aliases: manifest.aliases.join(","), match_count: matches.length, percentile_method: "nearest-rank: sorted[ceil(q*n)-1]", moved_filter: "lifetime_oil_spend > 0", measurement: "modeled fractional movement oil accumulated by the recorded-build simulation; may exceed literal stockpile deductions under oil starvation" };
writeDatabase(path.join(options.out, "replay_oil_analysis.sqlite"), matches, rows, summary, metadata);
const summaryRows = summary.map((row) => `<tr><th>${escapeXml(LABELS[row.unit_kind])}</th><td>${row.total_observed}</td><td>${row.moved_count}</td><td>${fmt(row.mean)}</td><td>${fmt(row.p50)}</td><td>${fmt(row.p75)}</td><td>${fmt(row.p95)}</td><td>${fmt(row.p99)}</td><td>${fmt(row.max)}</td></tr>`).join("");
const html = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Replay movement-oil analysis</title><style>body{font-family:system-ui;margin:0;background:#f2f0ea;color:#202020}main{max-width:1040px;margin:auto;padding:32px}section{background:#fbfaf7;border:1px solid #d8d4ca;border-radius:12px;padding:18px;margin:20px 0;overflow:auto}table{border-collapse:collapse;width:100%}th,td{padding:9px;border-bottom:1px solid #ddd;text-align:right}th:first-child{text-align:left}code{background:#ece9e1;padding:2px 5px;border-radius:4px}.note{color:#555;line-height:1.5}</style><main><h1>Lifetime vehicle movement-oil analysis</h1><p class="note">${matches.length} newest qualifying human 1v1 replays involving exact case-insensitive aliases ${escapeXml(manifest.aliases.join(", "))}. Statistics include only units with <code>lifetime_oil_spend &gt; 0</code>. Values are modeled fractional movement liability from each replay's recorded build, not necessarily literal whole oil deducted when a player was starved.</p><section><table><thead><tr><th>Unit</th><th>Observed</th><th>Moved</th><th>Mean</th><th>P50</th><th>P75</th><th>P95</th><th>P99</th><th>Max</th></tr></thead><tbody>${summaryRows}</tbody></table><p class="note">Percentiles use nearest rank: sorted[ceil(q×n)−1].</p></section><section>${percentileChart}</section><section>${ecdfChart}</section><section>${inclusionChart}</section></main>`;
fs.writeFileSync(path.join(options.out, "oil-spend-report.html"), html);
console.log(JSON.stringify({ matches: matches.length, units: rows.length, movedUnits: rows.filter((row) => row.moved).length, summary }, null, 2));
