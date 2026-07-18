export function playerAnalysisRows({ analysis, players }) {
  const metadata = new Map();
  for (const player of players || []) {
    const id = Number(player?.id);
    if (!Number.isFinite(id) || id <= 0) continue;
    metadata.set(id, {
      id,
      name: player?.name || `Player ${id}`,
      color: player?.color || "#e7dfc5",
    });
  }

  const rows = [];
  for (const player of analysis?.players || []) {
    const meta = metadata.get(player.id) || {};
    rows.push({
      id: player.id,
      name: meta.name || `Player ${player.id}`,
      color: safeCssColor(meta.color || "#e7dfc5"),
      units: player.units,
      production: player.production,
      upgrades: player.upgrades,
      unitsLost: player.unitsLost,
      resourcesLost: player.resourcesLost,
      resources: player.resources,
      aiDiagnostics: player.aiDiagnostics,
    });
  }
  rows.sort((a, b) => a.id - b.id);
  return rows;
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}
