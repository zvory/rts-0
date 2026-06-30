const COMMAND_WATERFALL_STAGES = [
  {
    id: "client_send",
    label: "client issue -> WebSocket send accepted",
    maxField: "command_issue_to_socket_send_accepted_max_ms",
    p95Field: "command_issue_to_socket_send_accepted_p95_ms",
    source: "client",
  },
  {
    id: "ingress_receipt",
    label: "client issue/send -> server receipt",
    maxField: "command_issue_to_server_receipt_max_ms",
    p95Field: "command_issue_to_server_receipt_p95_ms",
    source: "client receipt",
  },
  {
    id: "server_parse",
    label: "server frame receive -> deserialize",
    maxField: "server_command_frame_deserialize_max_ms",
    p95Field: "server_command_frame_deserialize_p95_ms",
    source: "server",
  },
  {
    id: "room_enqueue",
    label: "server deserialize -> room event queued",
    maxField: "server_command_deserialize_to_room_enqueue_max_ms",
    p95Field: "server_command_deserialize_to_room_enqueue_p95_ms",
    source: "server",
  },
  {
    id: "room_queue",
    label: "room event queued -> room actor handling",
    maxField: "server_command_room_queue_max_ms",
    p95Field: "server_command_room_queue_p95_ms",
    source: "server",
  },
  {
    id: "room_handle",
    label: "room actor handling -> receipt queued",
    maxField: "server_command_room_handle_max_ms",
    p95Field: "server_command_room_handle_p95_ms",
    source: "server",
  },
  {
    id: "receipt_delivery",
    label: "receipt queued -> writer sent",
    maxField: "server_command_receipt_send_age_max_ms",
    p95Field: "server_command_receipt_send_age_p95_ms",
    source: "server writer",
  },
  {
    id: "sim_ack",
    label: "accepted into sim queue -> snapshot ack",
    maxField: "server_command_accepted_to_sim_ack_max_ms",
    p95Field: "server_command_accepted_to_sim_ack_p95_ms",
    fallbackMaxField: "command_server_receipt_to_sim_ack_max_ms",
    fallbackP95Field: "command_server_receipt_to_sim_ack_p95_ms",
    source: "server",
    fallbackSource: "client combined receipt->ack",
  },
  {
    id: "ack_apply",
    label: "ack snapshot received -> client applied",
    maxField: "command_ack_snapshot_received_to_applied_max_ms",
    p95Field: "command_ack_snapshot_received_to_applied_p95_ms",
    source: "client",
  },
];

export function summarizeCommandLifecycle(rows) {
  return {
    stages: COMMAND_WATERFALL_STAGES.map((stage) => summarizeCommandStage(rows, stage)),
    familyCounts: summarizeCommandFamilies(rows),
    exemplars: parseCommandLifecycleExemplars(rows),
  };
}

export function appendCommandLifecycleMarkdown(lines, players) {
  const playersWithLifecycle = players.filter(
    (player) =>
      player.commandLifecycle?.stages?.some((stage) => stage.samples > 0) ||
      player.commandLifecycle?.exemplars?.length > 0,
  );
  if (playersWithLifecycle.length === 0) return;

  lines.push("");
  lines.push("### Command Lifecycle Waterfall");
  for (const player of playersWithLifecycle) {
    const stages = player.commandLifecycle.stages
      .filter((stage) => stage.samples > 0)
      .map((stage) => {
        const suffix = stage.combined ? " combined" : "";
        return `${stage.id} ${fmtMs(stage.maxMs)}/${fmtMs(stage.p95Ms)}ms ${stage.source}${suffix}`;
      });
    lines.push(`- Player ${player.playerId}: ${stages.join("; ") || "not logged or unavailable"}`);
    const familyCounts = Object.entries(player.commandLifecycle.familyCounts || {})
      .filter(([, count]) => count > 0)
      .map(([family, count]) => `${family}=${count}`)
      .join(", ");
    if (familyCounts) {
      lines.push(`  Families: ${familyCounts}`);
    }
    if (player.commandLifecycle.exemplars?.length > 0) {
      lines.push(
        `  Top exemplars: ${player.commandLifecycle.exemplars
          .map(
            (entry) =>
              `${entry.source} seq ${entry.clientSeq} ${entry.family} ${entry.stage} ${entry.stageMs}ms at ${entry.rowTimestamp || "n/a"}`,
          )
          .join("; ")}`,
      );
    }
  }
}

function summarizeCommandStage(rows, stage) {
  const maxMetric = summarizeField(rows, stage.maxField);
  const p95Metric = summarizeField(rows, stage.p95Field);
  if (maxMetric || p95Metric) {
    return {
      id: stage.id,
      label: stage.label,
      source: stage.source,
      maxMs: maxMetric?.max ?? null,
      p95Ms: p95Metric?.max ?? null,
      samples: maxMetric?.samples ?? p95Metric?.samples ?? 0,
      combined: false,
    };
  }
  if (stage.fallbackMaxField || stage.fallbackP95Field) {
    const fallbackMax = summarizeField(rows, stage.fallbackMaxField);
    const fallbackP95 = summarizeField(rows, stage.fallbackP95Field);
    if (fallbackMax || fallbackP95) {
      return {
        id: stage.id,
        label: stage.label,
        source: stage.fallbackSource,
        maxMs: fallbackMax?.max ?? null,
        p95Ms: fallbackP95?.max ?? null,
        samples: fallbackMax?.samples ?? fallbackP95?.samples ?? 0,
        combined: true,
      };
    }
  }
  return {
    id: stage.id,
    label: stage.label,
    source: stage.source,
    maxMs: null,
    p95Ms: null,
    samples: 0,
    combined: false,
  };
}

function summarizeCommandFamilies(rows) {
  return {
    move: maxNumericField(rows, "command_family_move"),
    attackMove: maxNumericField(rows, "command_family_attack_move"),
    build: maxNumericField(rows, "command_family_build"),
    train: maxNumericField(rows, "command_family_train"),
    other: maxNumericField(rows, "command_family_other"),
  };
}

function maxNumericField(rows, field) {
  const metric = summarizeField(rows, field);
  return metric?.max ?? 0;
}

function parseCommandLifecycleExemplars(rows) {
  const exemplars = [];
  for (const row of rows) {
    for (const [field, source] of [
      ["command_lifecycle_exemplars", "client"],
      ["server_command_lifecycle_exemplars", "server"],
    ]) {
      const parsed = parseJsonArray(row.fields[field]);
      for (const entry of parsed) {
        exemplars.push({
          source,
          rowTimestamp: row.timestamp,
          playerId: String(row.fields.player_id ?? ""),
          clientSeq: Number(entry.clientSeq) || 0,
          family: stableCommandFamily(entry.family),
          stage: stableCommandStage(entry.stage),
          stageMs: Number(entry.stageMs) || 0,
          issuedElapsedMs: Number(entry.issuedElapsedMs) || null,
          receivedUnixMs: Number(entry.receivedUnixMs) || null,
        });
      }
    }
  }
  return exemplars.sort((a, b) => b.stageMs - a.stageMs || a.clientSeq - b.clientSeq).slice(0, 5);
}

function summarizeField(rows, field) {
  const numbers = rows.map((row) => row.fields[field]).filter((value) => Number.isFinite(value));
  if (numbers.length === 0) return null;
  const sorted = numbers.slice().sort((a, b) => a - b);
  return {
    max: sorted[sorted.length - 1],
    samples: numbers.length,
  };
}

function parseJsonArray(value) {
  if (typeof value !== "string" || value.trim() === "") return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function stableCommandFamily(value) {
  const text = String(value || "other");
  return ["move", "attackMove", "build", "train", "other"].includes(text) ? text : "other";
}

function stableCommandStage(value) {
  return String(value || "unknown").replace(/[^A-Za-z0-9_.:-]/g, "_").slice(0, 64);
}

function fmtMs(value) {
  return value === null || value === undefined ? "n/a" : String(value);
}
