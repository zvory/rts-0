import { playerAnalysisRows } from "./observer_analysis_rows.js";

export function renderResearchMetric({ analysis, players, upgrades, renderPlayerHeading }) {
  const wrap = document.createElement("div");
  wrap.className = "replay-analysis-metric replay-research";

  const heading = document.createElement("div");
  heading.className = "replay-analysis-metric-heading";
  heading.textContent = "Completed research";
  wrap.appendChild(heading);

  if (!analysis) {
    wrap.appendChild(renderEmpty("Waiting for observer analysis"));
    return wrap;
  }

  const rows = playerAnalysisRows({ analysis, players });
  if (!rows.length) {
    wrap.appendChild(renderEmpty("No players"));
    return wrap;
  }

  for (const player of rows) {
    wrap.appendChild(renderPlayerHeading(player));
    if (!player.upgrades.length) {
      wrap.appendChild(renderEmpty("No completed research", "replay-research-empty"));
      continue;
    }
    for (const upgrade of player.upgrades) {
      const definition = upgrades[upgrade];
      const row = document.createElement("div");
      row.className = "replay-research-row";

      const icon = document.createElement("span");
      icon.className = "replay-analysis-kind-icon";
      icon.textContent = definition?.icon || "R";

      const label = document.createElement("span");
      label.className = "replay-research-label";
      label.textContent = definition?.label || humanizeUpgrade(upgrade);

      row.append(icon, label);
      wrap.appendChild(row);
    }
  }
  return wrap;
}

function renderEmpty(text, className = "") {
  const empty = document.createElement("div");
  empty.className = `replay-analysis-empty ${className}`.trim();
  empty.textContent = text;
  return empty;
}

function humanizeUpgrade(upgrade) {
  return String(upgrade || "Research")
    .split("_")
    .filter(Boolean)
    .map((word) => word[0]?.toUpperCase() + word.slice(1))
    .join(" ");
}
