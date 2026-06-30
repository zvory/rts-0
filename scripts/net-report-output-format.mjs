export function metricPctX100Max(player, field) {
  return formatPctX100(player.metrics[field]?.max);
}

export function metricMax(player, field) {
  return formatValue(player.metrics[field]?.max);
}

export function metricMin(player, field) {
  return formatValue(player.metrics[field]?.min);
}

export function formatTransportDiagnostics(transport, fields) {
  return fields.map(([field, label]) => {
    const summary = transport[camelCase(field)];
    return `${label} ${formatTransportCounts(summary)}`;
  }).join("; ");
}

export function formatPctX100(value) {
  if (value === null || value === undefined || value === "") {
    return "n/a";
  }
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return "n/a";
  }
  return `${(number / 100).toFixed(2).replace(/\.?0+$/, "")}%`;
}

export function formatValue(value) {
  return value === null || value === undefined || value === "" ? "n/a" : String(value);
}

export function formatTsv(report, summaryFields) {
  const headers = [
    "match",
    "player_id",
    "reports",
    "primary_issues",
    ...summaryFields.flatMap((field) => [`${field}_max`, `${field}_p95`]),
  ];
  const lines = [headers.join("\t")];
  for (const match of report.matches) {
    for (const player of match.players) {
      lines.push(
        [
          match.match,
          player.playerId,
          player.reportCount,
          player.primaryIssues.map((issue) => `${issue.issue}:${issue.count}`).join(","),
          ...summaryFields.flatMap((field) => [
            player.metrics[field]?.max ?? "",
            player.metrics[field]?.p95 ?? "",
          ]),
        ]
          .map(tsvCell)
          .join("\t"),
      );
    }
  }
  return `${lines.join("\n")}\n`;
}

function formatTransportCounts(summary) {
  if (!summary || summary.samples === 0 || summary.values.length === 0) {
    return "n/a";
  }
  return summary.values.map((item) => `${item.value}=${item.count}`).join(", ");
}

function camelCase(value) {
  return value.replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
}

function tsvCell(value) {
  return String(value).replace(/\t/g, " ").replace(/\r?\n/g, " ");
}
