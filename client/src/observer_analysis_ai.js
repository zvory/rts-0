export function renderAiDiagnosticsMetric({ analysis, rows = [] } = {}) {
  const wrap = renderMetric("replay-ai-diagnostics", "Latest decisions");
  const aiRows = rows.filter((row) => row.aiDiagnostics);
  if (!analysis) {
    wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
    return wrap;
  }
  if (!aiRows.length) {
    wrap.appendChild(renderEmptyMetric("No AI diagnostics"));
    return wrap;
  }

  for (const player of aiRows) {
    wrap.appendChild(renderPlayerHeading(player));
    wrap.appendChild(renderTraceMeta(player.aiDiagnostics));
    wrap.appendChild(renderTraceLines(player.aiDiagnostics.lines));
  }
  return wrap;
}

export function normalizeAiDiagnostics(diagnostics) {
  if (!diagnostics || typeof diagnostics !== "object") return null;
  const profileId = String(diagnostics.profileId || "").trim();
  const lines = Array.isArray(diagnostics.lines)
    ? diagnostics.lines.map((line) => String(line || "")).filter(Boolean)
    : [];
  if (!profileId || lines.length === 0) return null;
  return {
    profileId,
    traceTick: Math.max(0, Math.trunc(Number(diagnostics.traceTick) || 0)),
    lines,
  };
}

function renderMetric(className, headingText) {
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

function renderPlayerHeading(player) {
  const heading = document.createElement("div");
  heading.className = "replay-analysis-player-heading";

  const swatch = document.createElement("span");
  swatch.className = "replay-analysis-player-swatch";
  swatch.setAttribute("style", `background:${safeCssColor(player.color)};`);
  swatch.setAttribute("aria-hidden", "true");

  const name = document.createElement("span");
  name.className = "replay-analysis-player-name";
  name.textContent = player.name;

  heading.append(swatch, name);
  return heading;
}

function renderTraceMeta(diagnostics) {
  const meta = document.createElement("div");
  meta.className = "replay-ai-diagnostics-meta";
  const profile = document.createElement("span");
  profile.textContent = diagnostics.profileId;
  const tick = document.createElement("span");
  tick.textContent = `tick ${formatValue(diagnostics.traceTick)}`;
  meta.append(profile, tick);
  return meta;
}

function renderTraceLines(traceLines) {
  const lines = document.createElement("div");
  lines.className = "replay-ai-diagnostics-lines";
  for (const lineText of traceLines) {
    const line = document.createElement("div");
    line.className = "replay-ai-diagnostics-line";
    line.textContent = lineText;
    lines.appendChild(line);
  }
  return lines;
}

function formatValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}
