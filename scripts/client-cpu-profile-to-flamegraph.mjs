#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function analyzeCpuProfile(profile) {
  if (!profile || !Array.isArray(profile.nodes) || profile.nodes.length === 0) {
    throw new Error("CPU profile has no call-tree nodes");
  }
  const nodes = new Map(profile.nodes.map((node) => [node.id, {
    ...node,
    parentId: null,
    selfUs: 0,
    totalUs: 0,
  }]));
  if (nodes.size !== profile.nodes.length) throw new Error("CPU profile has duplicate node ids");
  for (const node of nodes.values()) {
    for (const childId of node.children || []) {
      const child = nodes.get(childId);
      if (!child) throw new Error(`CPU profile node ${node.id} references missing child ${childId}`);
      if (child.parentId != null && child.parentId !== node.id) {
        throw new Error(`CPU profile node ${childId} has multiple parents`);
      }
      child.parentId = node.id;
    }
  }

  const samples = profile.samples;
  const deltas = profile.timeDeltas;
  if (!Array.isArray(samples) || samples.length === 0) {
    throw new Error("CPU profile has no samples");
  }
  if (!Array.isArray(deltas) || deltas.length !== samples.length) {
    throw new Error("CPU profile samples and time deltas differ in length");
  }
  let sampledUs = 0;
  for (let index = 0; index < samples.length; index += 1) {
    const delta = Number(deltas[index]);
    if (!Number.isFinite(delta) || delta < 0) {
      throw new Error(`CPU profile has invalid time delta at sample ${index}`);
    }
    const node = nodes.get(samples[index]);
    if (!node) throw new Error(`CPU profile sample ${index} references missing node ${samples[index]}`);
    node.selfUs += delta;
    sampledUs += delta;
  }

  const roots = [...nodes.values()].filter((node) => node.parentId == null);
  if (roots.length !== 1) throw new Error(`CPU profile call tree has ${roots.length} roots; expected one`);
  const calculateTotal = (node, active = new Set()) => {
    if (active.has(node.id)) return node.selfUs;
    active.add(node.id);
    node.totalUs = node.selfUs;
    for (const childId of node.children || []) {
      const child = nodes.get(childId);
      if (child) node.totalUs += calculateTotal(child, active);
    }
    active.delete(node.id);
    return node.totalUs;
  };
  for (const root of roots) calculateTotal(root);

  const rows = [...nodes.values()].map((node) => describeNode(node, sampledUs));
  const aggregated = aggregateRows(rows, sampledUs);
  const summary = {
    schemaVersion: 1,
    sampleCount: samples.length,
    sampledUs,
    wallDurationUs: Math.max(0, Number(profile.endTime || 0) - Number(profile.startTime || 0)),
    samplingIntervalMeanUs: samples.length > 0 ? sampledUs / samples.length : 0,
    topSelf: rows.filter((row) => row.selfUs > 0).sort((a, b) => b.selfUs - a.selfUs).slice(0, 50),
    topInclusive: rows.filter((row) => row.totalUs > 0).sort((a, b) => b.totalUs - a.totalUs).slice(0, 50),
    topSelfByFunction: aggregated.sort((a, b) => b.selfUs - a.selfUs).slice(0, 50),
  };
  return { nodes, roots, summary };
}

function aggregateRows(rows, sampledUs) {
  const grouped = new Map();
  for (const row of rows) {
    if (row.selfUs <= 0) continue;
    const key = `${row.functionName}\u0000${row.url}\u0000${row.line || ""}`;
    const current = grouped.get(key) || {
      functionName: row.functionName,
      url: row.url,
      line: row.line,
      label: row.label,
      selfUs: 0,
      selfPct: 0,
    };
    current.selfUs += row.selfUs;
    current.selfPct = sampledUs > 0 ? (current.selfUs / sampledUs) * 100 : 0;
    grouped.set(key, current);
  }
  return [...grouped.values()];
}

function describeNode(node, sampledUs) {
  const frame = node.callFrame || {};
  const functionName = frame.functionName || "(anonymous)";
  const file = shortUrl(frame.url || "");
  const line = Number(frame.lineNumber) >= 0 ? Number(frame.lineNumber) + 1 : null;
  const location = file ? `${file}${line ? `:${line}` : ""}` : "";
  return {
    nodeId: node.id,
    functionName,
    url: frame.url || "",
    line,
    selfUs: node.selfUs,
    selfPct: sampledUs > 0 ? (node.selfUs / sampledUs) * 100 : 0,
    totalUs: node.totalUs,
    totalPct: sampledUs > 0 ? (node.totalUs / sampledUs) * 100 : 0,
    label: location ? `${functionName} — ${location}` : functionName,
  };
}

export function renderCpuFlameGraph(analysis, options = {}) {
  if (!analysis?.nodes || !analysis?.roots || !analysis?.summary) {
    throw new Error("renderCpuFlameGraph requires analyzed CPU profile data");
  }
  const width = options.width || 2400;
  const headerHeight = 134;
  const rowHeight = 22;
  const root = analysis.roots.sort((a, b) => b.totalUs - a.totalUs)[0];
  const depth = root ? treeDepth(root, analysis.nodes) : 1;
  const height = headerHeight + (depth + 1) * rowHeight + 26;
  const frames = [];

  const visit = (node, x, y, parentWidth) => {
    const parentTotalUs = node.parentId == null
      ? node.totalUs
      : analysis.nodes.get(node.parentId)?.totalUs || node.totalUs;
    const nodeWidth = parentWidth * (node.totalUs / Math.max(1, parentTotalUs));
    if (nodeWidth < (options.minWidthPx ?? 0.8)) return;
    const description = describeNode(node, analysis.summary.sampledUs);
    const label = fitLabel(description.label, nodeWidth);
    const fill = colorFor(description);
    const tooltip = escapeXml(
      `${description.label}\n`
      + `Inclusive ${(description.totalUs / 1000).toFixed(1)} ms (${description.totalPct.toFixed(1)}%)\n`
      + `Self ${(description.selfUs / 1000).toFixed(1)} ms (${description.selfPct.toFixed(1)}%)`,
    );
    frames.push(
      `<g><title>${tooltip}</title>`
      + `<rect x="${x.toFixed(2)}" y="${y.toFixed(2)}" `
      + `width="${Math.max(0.4, nodeWidth - 0.5).toFixed(2)}" height="${rowHeight - 1}" `
      + `rx="2" fill="${fill}"/>`
      + `<text x="${(x + 4).toFixed(2)}" y="${(y + 15).toFixed(2)}">`
      + `${escapeXml(label)}</text></g>`,
    );
    let childX = x;
    const children = (node.children || [])
      .map((id) => analysis.nodes.get(id))
      .filter(Boolean)
      .sort((a, b) => b.totalUs - a.totalUs);
    for (const child of children) {
      const childWidth = nodeWidth * (child.totalUs / Math.max(1, node.totalUs));
      visit(child, childX, y - rowHeight, nodeWidth);
      childX += childWidth;
    }
  };
  if (root) visit(root, 0, height - rowHeight - 8, width);

  const top = analysis.summary.topSelfByFunction.slice(0, 5)
    .map((row) => `${row.functionName} ${row.selfPct.toFixed(1)}%`)
    .join("  •  ");
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <style>
    text { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 12px; fill: #171717; pointer-events: none; }
    .title { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; font-size: 25px; font-weight: 700; fill: #111827; }
    .subtitle { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; font-size: 14px; fill: #4b5563; }
    g { pointer-events: none; }
  </style>
  <rect width="100%" height="100%" fill="#fafafa"/>
  <text class="title" x="18" y="34">${escapeXml(options.title || "RTS client CPU flame graph")}</text>
  <text class="subtitle" x="18" y="61">${analysis.summary.sampleCount.toLocaleString()} samples · ${(analysis.summary.sampledUs / 1_000_000).toFixed(2)} sampled CPU seconds · width = inclusive sampled time</text>
  <text class="subtitle" x="18" y="84">Top self time: ${escapeXml(top)}</text>
  <rect x="18" y="98" width="14" height="14" rx="2" fill="#fb7185"/><text class="subtitle" x="38" y="110">game client</text>
  <rect x="142" y="98" width="14" height="14" rx="2" fill="#f59e0b"/><text class="subtitle" x="162" y="110">Pixi</text>
  <rect x="222" y="98" width="14" height="14" rx="2" fill="#a7f3d0"/><text class="subtitle" x="242" y="110">browser/native</text>
  <rect x="358" y="98" width="14" height="14" rx="2" fill="#d1d5db"/><text class="subtitle" x="378" y="110">idle/GC</text>
  <line x1="0" y1="124" x2="${width}" y2="124" stroke="#d1d5db"/>
  ${frames.join("\n  ")}
</svg>\n`;
}

function treeDepth(node, nodes, active = new Set()) {
  if (active.has(node.id)) return 0;
  active.add(node.id);
  let depth = 1;
  for (const childId of node.children || []) {
    const child = nodes.get(childId);
    if (child) depth = Math.max(depth, 1 + treeDepth(child, nodes, active));
  }
  active.delete(node.id);
  return depth;
}

function colorFor(row) {
  if (row.functionName === "(idle)" || row.functionName === "(garbage collector)") return "#d1d5db";
  if (row.url.includes("pixi")) return "#f59e0b";
  if (row.url.includes("/client/src/") || row.url.includes("/src/")) return "#fb7185";
  if (row.url.startsWith("wasm://")) return "#60a5fa";
  if (!row.url) return "#a7f3d0";
  return "#c4b5fd";
}

function shortUrl(url) {
  if (!url) return "";
  if (url.startsWith("wasm://")) return "wasm";
  try {
    const parsed = new URL(url);
    return parsed.pathname.replace(/^\/client\/src\//, "client/").replace(/^\/client\//, "client/") || parsed.hostname;
  } catch {
    return url;
  }
}

function fitLabel(label, width) {
  const max = Math.floor((width - 8) / 7.15);
  if (max < 4) return "";
  if (label.length <= max) return label;
  return `${label.slice(0, Math.max(1, max - 1))}…`;
}

function escapeXml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

export function writeCpuFlameGraphArtifacts({
  profilePath,
  svgPath,
  summaryPath = svgPath.replace(/\.svg$/i, "-summary.json"),
  title = "RTS client CPU flame graph",
  width = 2400,
  minWidthPx = 0.8,
}) {
  const profile = JSON.parse(fs.readFileSync(profilePath, "utf8"));
  const analysis = analyzeCpuProfile(profile);
  const svg = renderCpuFlameGraph(analysis, { title, width, minWidthPx });
  fs.mkdirSync(path.dirname(svgPath), { recursive: true });
  fs.writeFileSync(svgPath, svg);
  fs.writeFileSync(summaryPath, `${JSON.stringify(analysis.summary, null, 2)}\n`);
  return { analysis, svg, svgPath, summaryPath };
}

export function parseCpuFlameGraphArgs(argv) {
  const parsed = {
    input: "",
    output: "",
    summary: "",
    title: "RTS client CPU flame graph",
    width: 2400,
    minWidthPx: 0.8,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    if (arg === "--input") parsed.input = path.resolve(value());
    else if (arg === "--output") parsed.output = path.resolve(value());
    else if (arg === "--summary") parsed.summary = path.resolve(value());
    else if (arg === "--title") parsed.title = value();
    else if (arg === "--width") parsed.width = Number(value());
    else if (arg === "--min-width") parsed.minWidthPx = Number(value());
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (!parsed.input || !parsed.output) {
    throw new Error(
      "usage: client-cpu-profile-to-flamegraph.mjs --input profile.cpuprofile "
      + "--output flamegraph.svg [--title text]",
    );
  }
  if (!Number.isFinite(parsed.width) || parsed.width <= 0) {
    throw new Error("--width must be a positive number");
  }
  if (!Number.isFinite(parsed.minWidthPx) || parsed.minWidthPx < 0) {
    throw new Error("--min-width must be a non-negative number");
  }
  return parsed;
}

function main() {
  const args = parseCpuFlameGraphArgs(process.argv.slice(2));
  const result = writeCpuFlameGraphArtifacts({
    profilePath: args.input,
    svgPath: args.output,
    summaryPath: args.summary || undefined,
    title: args.title,
    width: args.width,
    minWidthPx: args.minWidthPx,
  });

  console.log(`flame graph: ${result.svgPath}`);
  console.log(`profile summary: ${result.summaryPath}`);
  console.log(
    `sampled ${(result.analysis.summary.sampledUs / 1000).toFixed(1)} ms `
    + `across ${result.analysis.summary.sampleCount} samples`,
  );
  for (const row of result.analysis.summary.topSelfByFunction.slice(0, 15)) {
    console.log(`${row.selfPct.toFixed(1).padStart(5)}% self  ${row.label}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}
