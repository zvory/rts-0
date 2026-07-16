const MAX_STACK_DEPTH = 128;
const MAX_TOP_ROWS = 40;

export function analyzeSelfProfile(trace) {
  if (!trace || !Array.isArray(trace.samples) || !Array.isArray(trace.stacks) ||
      !Array.isArray(trace.frames) || !Array.isArray(trace.resources)) {
    throw new Error("The browser profiler returned an invalid trace");
  }
  const root = treeNode("(root)", "", null);
  const deltas = sampleDurations(trace.samples);
  let sampledMs = 0;

  for (let index = 0; index < trace.samples.length; index += 1) {
    const durationMs = deltas[index];
    if (!(durationMs > 0)) continue;
    const path = stackPath(trace, trace.samples[index]?.stackId);
    sampledMs += durationMs;
    root.totalMs += durationMs;
    let cursor = root;
    for (const frame of path) {
      const key = `${frame.name}\u0000${frame.url}\u0000${frame.line ?? ""}`;
      let child = cursor.children.get(key);
      if (!child) {
        child = treeNode(frame.name, frame.url, frame.line);
        cursor.children.set(key, child);
      }
      child.totalMs += durationMs;
      cursor = child;
    }
    cursor.selfMs += durationMs;
  }

  const rows = [];
  collectRows(root, sampledMs, rows);
  return {
    schemaVersion: 1,
    sampleCount: trace.samples.length,
    sampledMs: round1(sampledMs),
    topSelf: rows
      .filter((row) => row.selfMs > 0)
      .sort((a, b) => b.selfMs - a.selfMs)
      .slice(0, MAX_TOP_ROWS),
    topInclusive: rows
      .filter((row) => row.totalMs > 0)
      .sort((a, b) => b.totalMs - a.totalMs)
      .slice(0, MAX_TOP_ROWS),
    root,
  };
}

export function selfProfileSummary(analysis) {
  return {
    schemaVersion: analysis.schemaVersion,
    sampleCount: analysis.sampleCount,
    sampledMs: analysis.sampledMs,
    topSelf: analysis.topSelf,
    topInclusive: analysis.topInclusive,
  };
}

export function renderSelfProfileFlamegraph(analysis, { title = "RTS client JS flame graph" } = {}) {
  const width = 1800;
  const headerHeight = 104;
  const rowHeight = 22;
  const depth = treeDepth(analysis.root);
  const height = headerHeight + Math.max(2, depth) * rowHeight + 24;
  const frames = [];

  const visit = (node, x, y, availableWidth) => {
    const children = [...node.children.values()].sort((a, b) => b.totalMs - a.totalMs);
    let childX = x;
    for (const child of children) {
      const childWidth = availableWidth * child.totalMs / Math.max(0.001, node.totalMs);
      if (childWidth >= 0.8) {
        const label = frameLabel(child);
        const tooltip = `${label}\nInclusive ${child.totalMs.toFixed(1)} ms (${percent(child.totalMs, analysis.sampledMs)}%)\nSelf ${child.selfMs.toFixed(1)} ms (${percent(child.selfMs, analysis.sampledMs)}%)`;
        frames.push(
          `<g><title>${escapeXml(tooltip)}</title><rect x="${childX.toFixed(2)}" y="${y.toFixed(2)}" width="${Math.max(0.4, childWidth - 0.5).toFixed(2)}" height="${rowHeight - 1}" rx="2" fill="${colorFor(child)}"/><text x="${(childX + 4).toFixed(2)}" y="${(y + 15).toFixed(2)}">${escapeXml(fitLabel(label, childWidth))}</text></g>`,
        );
        visit(child, childX, y - rowHeight, childWidth);
      }
      childX += childWidth;
    }
  };
  visit(analysis.root, 0, height - rowHeight - 8, width);

  const top = analysis.topSelf.slice(0, 5)
    .map((row) => `${row.name} ${percent(row.selfMs, analysis.sampledMs)}%`)
    .join("  •  ");
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <style>text{font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;font-size:12px;fill:#171717;pointer-events:none}.title{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;font-size:24px;font-weight:700}.subtitle{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;font-size:14px;fill:#4b5563}</style>
  <rect width="100%" height="100%" fill="#fafafa"/>
  <text class="title" x="18" y="34">${escapeXml(title)}</text>
  <text class="subtitle" x="18" y="60">${analysis.sampleCount} samples · ${analysis.sampledMs.toFixed(1)} sampled ms · width = inclusive sampled time</text>
  <text class="subtitle" x="18" y="84">Top self time: ${escapeXml(top || "no attributed samples")}</text>
  <line x1="0" y1="96" x2="${width}" y2="96" stroke="#d1d5db"/>
  ${frames.join("\n  ")}
</svg>\n`;
}

function sampleDurations(samples) {
  const raw = [];
  for (let index = 0; index + 1 < samples.length; index += 1) {
    const delta = Number(samples[index + 1]?.timestamp) - Number(samples[index]?.timestamp);
    if (Number.isFinite(delta) && delta > 0 && delta <= 1000) raw.push(delta);
  }
  const sorted = raw.slice().sort((a, b) => a - b);
  const fallback = sorted.length ? sorted[Math.floor(sorted.length / 2)] : 10;
  return samples.map((sample, index) => {
    if (index + 1 >= samples.length) return fallback;
    const delta = Number(samples[index + 1]?.timestamp) - Number(sample?.timestamp);
    return Number.isFinite(delta) && delta > 0 && delta <= 1000 ? delta : fallback;
  });
}

function stackPath(trace, stackId) {
  if (!Number.isInteger(stackId) || !trace.stacks[stackId]) {
    return [{ name: "(idle/unattributed)", url: "", line: null }];
  }
  const reversed = [];
  const visited = new Set();
  let currentId = stackId;
  while (Number.isInteger(currentId) && trace.stacks[currentId] &&
      reversed.length < MAX_STACK_DEPTH && !visited.has(currentId)) {
    visited.add(currentId);
    const stack = trace.stacks[currentId];
    const frame = trace.frames[stack.frameId] || {};
    const resource = Number.isInteger(frame.resourceId) ? trace.resources[frame.resourceId] : null;
    reversed.push({
      name: String(frame.name || "(anonymous)").slice(0, 160),
      url: String(resource?.url || "").slice(0, 500),
      line: Number.isInteger(frame.line) ? frame.line : null,
    });
    currentId = stack.parentId;
  }
  return reversed.reverse();
}

function treeNode(name, url, line) {
  return { name, url, line, selfMs: 0, totalMs: 0, children: new Map() };
}

function collectRows(node, sampledMs, out) {
  if (node.name !== "(root)") {
    out.push({
      name: node.name,
      url: node.url,
      line: node.line,
      selfMs: round1(node.selfMs),
      selfPct: round1(100 * node.selfMs / Math.max(0.001, sampledMs)),
      totalMs: round1(node.totalMs),
      totalPct: round1(100 * node.totalMs / Math.max(0.001, sampledMs)),
    });
  }
  for (const child of node.children.values()) collectRows(child, sampledMs, out);
}

function treeDepth(node) {
  let depth = 1;
  for (const child of node.children.values()) depth = Math.max(depth, 1 + treeDepth(child));
  return depth;
}

function frameLabel(node) {
  const file = shortUrl(node.url);
  const location = file ? `${file}${node.line != null ? `:${node.line}` : ""}` : "";
  return location ? `${node.name} — ${location}` : node.name;
}

function shortUrl(url) {
  if (!url) return "";
  try {
    const parsed = new URL(url);
    return parsed.pathname.replace(/^\/client\/src\//, "client/").replace(/^\/src\//, "client/") || parsed.hostname;
  } catch {
    return String(url).slice(0, 100);
  }
}

function colorFor(node) {
  if (node.name.includes("idle") || node.name.includes("garbage collector")) return "#d1d5db";
  if (node.url.includes("pixi")) return "#f59e0b";
  if (node.url.includes("/src/")) return "#fb7185";
  if (!node.url) return "#a7f3d0";
  return "#c4b5fd";
}

function fitLabel(label, width) {
  const max = Math.floor((width - 8) / 7.1);
  if (max < 4) return "";
  return label.length <= max ? label : `${label.slice(0, Math.max(1, max - 1))}…`;
}

function percent(value, total) {
  return (100 * value / Math.max(0.001, total)).toFixed(1);
}

function round1(value) {
  return Math.round(Number(value || 0) * 10) / 10;
}

function escapeXml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}
