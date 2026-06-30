export function summarizeSnapshotPayload(rows, summarizeField) {
  const payloadBytes = summarizeField(rows, "server_snapshot_payload_bytes_total");
  return {
    samples: rows.filter((row) => row.fields.server_snapshot_payload_sections !== undefined).length,
    payloadBytes,
    sections: aggregateSnapshotPayloadEntries(rows, "server_snapshot_payload_sections", {
      labelField: "section",
      bytesField: "bytes",
    }),
    entityKinds: aggregateSnapshotPayloadEntries(rows, "server_snapshot_entity_kinds", {
      labelField: "kind",
      bytesField: "approxBytes",
    }),
  };
}

export function appendSnapshotPayloadMarkdown(lines, players, { formatValue, formatPctX100 }) {
  const rows = players.filter((player) => player.snapshotPayload?.samples > 0);
  if (rows.length === 0) {
    return;
  }
  lines.push("");
  lines.push("### Snapshot Payload Composition");
  lines.push(
    "| player | lifecycle max ms project/compact/queue/serialize/send | server payload bytes p95/max | top sections | top entity kinds |"
  );
  lines.push("| --- | --- | ---: | --- | --- |");
  for (const player of rows) {
    lines.push(
      [
        player.playerId,
        [
          metricMax(player, "server_snapshot_project_max_ms", formatValue),
          metricMax(player, "server_snapshot_compact_max_ms", formatValue),
          metricMax(player, "server_snapshot_queue_age_max_ms", formatValue),
          metricMax(player, "server_snapshot_serialize_max_ms", formatValue),
          metricMax(player, "server_snapshot_writer_send_max_ms", formatValue),
        ].join("/"),
        `${metricMax(player, "server_snapshot_payload_bytes_p95", formatValue)}/${metricMax(
          player,
          "server_snapshot_payload_bytes_max",
          formatValue,
        )}`,
        formatSnapshotPayloadEntries(player.snapshotPayload.sections, "bytes", { formatValue, formatPctX100 }),
        formatSnapshotPayloadEntries(player.snapshotPayload.entityKinds, "approx bytes", {
          formatValue,
          formatPctX100,
        }),
      ]
        .join(" | ")
        .replace(/^/, "| ")
        .replace(/$/, " |")
    );
  }
}

function aggregateSnapshotPayloadEntries(rows, field, { labelField, bytesField }) {
  const totals = new Map();
  let totalBytes = 0;
  for (const row of rows) {
    const rowTotal = Number(row.fields.server_snapshot_payload_bytes_total);
    if (Number.isFinite(rowTotal) && rowTotal > 0) {
      totalBytes += rowTotal;
    }
    for (const entry of parseJsonArrayField(row.fields[field])) {
      const label = String(entry?.[labelField] || "unknown").slice(0, 64);
      const count = positiveNumber(entry?.count);
      const bytes = positiveNumber(entry?.[bytesField]);
      if (!label || (count === 0 && bytes === 0)) {
        continue;
      }
      const current = totals.get(label) || { label, count: 0, bytes: 0, samples: 0 };
      current.count += count;
      current.bytes += bytes;
      current.samples += 1;
      totals.set(label, current);
    }
  }
  if (totalBytes <= 0) {
    totalBytes = [...totals.values()].reduce((sum, entry) => sum + entry.bytes, 0);
  }
  return [...totals.values()]
    .sort((a, b) => b.bytes - a.bytes || b.count - a.count || a.label.localeCompare(b.label))
    .slice(0, 8)
    .map((entry) => ({
      ...entry,
      pctX100: totalBytes > 0 ? Math.round((entry.bytes * 10000) / totalBytes) : 0,
    }));
}

function parseJsonArrayField(value) {
  if (Array.isArray(value)) {
    return value;
  }
  if (typeof value !== "string" || value.length === 0) {
    return [];
  }
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function positiveNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}

function formatSnapshotPayloadEntries(entries, byteLabel, { formatValue, formatPctX100 }) {
  if (!Array.isArray(entries) || entries.length === 0) {
    return "n/a";
  }
  return entries
    .slice(0, 4)
    .map((entry) => `${entry.label} ${formatPctX100(entry.pctX100)} ${formatValue(entry.bytes)} ${byteLabel}`)
    .join(", ");
}

function metricMax(player, field, formatValue) {
  return formatValue(player.metrics[field]?.max);
}
