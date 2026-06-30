#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  attachAgentDigest,
  appendAgentDigestMarkdown,
  formatClientRowsTsv,
  formatEvidenceIndexJson,
  formatKeyMetricsJson,
  formatPackageReadme,
  formatServerTickRowsTsv,
} from "./net-report-incident-package.mjs";
import {
  appendCommandLifecycleMarkdown,
  summarizeCommandLifecycle,
} from "./net-report-command-lifecycle.mjs";

const ANSI_PATTERN = /\x1B\[[0-?]*[ -/]*[@-~]/g;
const SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES = 1280;
const SNAPSHOT_PACKET_BUDGET_RATE_WARN_X100 = 5000;
const DEFAULT_TIMELINE_BAND_MS = 60_000;

const METRICS = [
  ["rtt_ms", "RTT"],
  ["rtt_max_ms", "RTT max"],
  ["snapshot_jitter_ms", "snapshot jitter"],
  ["snapshot_gap_max_ms", "snapshot gap"],
  ["snapshot_late_frame_count", "late snapshot frames"],
  ["predicted_snapshot_late_frame_count", "predicted while late"],
  ["snapshot_bytes_max", "payload bytes max"],
  ["snapshot_bytes_p95", "payload bytes p95"],
  ["snapshot_bytes_avg", "payload bytes avg"],
  ["snapshot_over_segment_budget_pct_x100", "payload over budget"],
  ["snapshot_parse_max_ms", "parse max"],
  ["snapshot_parse_p95_ms", "parse p95"],
  ["snapshot_decode_max_ms", "decode max"],
  ["snapshot_decode_p95_ms", "decode p95"],
  ["snapshot_apply_max_ms", "apply max"],
  ["snapshot_apply_p95_ms", "apply p95"],
  ["prediction_apply_max_ms", "prediction apply max"],
  ["prediction_apply_p95_ms", "prediction apply p95"],
  ["snapshot_tick_gap_max", "snapshot tick gap"],
  ["snapshot_burst_max", "snapshot burst"],
  ["frame_gap_max_ms", "frame gap"],
  ["frame_work_max_ms", "frame work max"],
  ["frame_work_p95_ms", "frame work p95"],
  ["renderer_max_ms", "renderer max"],
  ["renderer_p95_ms", "renderer p95"],
  ["fps_estimate", "FPS estimate", "min"],
  ["commands_issued", "commands issued"],
  ["command_burst_max", "command burst max"],
  ["command_burst_frame_gap_max_ms", "command-burst frame gap"],
  ["ws_buffered_bytes", "WS buffered"],
  ["server_tick_ms", "server tick"],
  ["server_lag_ms", "server lag"],
  ["slow_tick_count", "slow ticks"],
  ["head_of_line_count", "head of line"],
  ["acknowledged_command_latency_ms", "legacy command ack"],
  ["command_issue_to_socket_send_accepted_max_ms", "command client send max"],
  ["command_issue_to_socket_send_accepted_p95_ms", "command client send p95"],
  ["command_issue_to_server_receipt_max_ms", "command upload max"],
  ["command_issue_to_server_receipt_p95_ms", "command upload p95"],
  ["command_server_receipt_to_sim_ack_max_ms", "server queue max"],
  ["command_server_receipt_to_sim_ack_p95_ms", "server queue p95"],
  ["command_issue_to_sim_ack_max_ms", "command response max"],
  ["command_issue_to_sim_ack_p95_ms", "command response p95"],
  ["command_ack_snapshot_received_to_applied_max_ms", "ack apply max"],
  ["command_ack_snapshot_received_to_applied_p95_ms", "ack apply p95"],
  ["oldest_pending_command_age_ms", "oldest pending command"],
  ["max_pending_command_count", "max pending commands"],
  ["correction_distance_px", "prediction correction"],
  ["prediction_disable_user_count", "prediction disabled/user"],
  ["prediction_disable_replay_count", "prediction disabled/replay"],
  ["prediction_disable_spectator_count", "prediction disabled/spectator"],
  ["prediction_disable_compatibility_count", "prediction disabled/compat"],
  ["prediction_disable_wasm_count", "prediction disabled/WASM"],
  ["prediction_disable_other_count", "prediction disabled/other"],
  ["wasm_tick_ms", "WASM tick"],
  ["prediction_replay_max_ms", "prediction replay max"],
  ["prediction_replay_max_ticks", "prediction replay ticks max"],
  ["prediction_replay_budget_exceeded_count", "prediction replay budget exceeded"],
  ["server_command_receipts_accepted", "server accepted receipts"],
  ["server_command_receipts_rejected", "server rejected receipts"],
  ["server_reliable_drained_before_snapshot", "server reliable before snapshots"],
  ["server_reliable_drained_before_snapshot_max", "server reliable before snapshot max"],
  ["server_snapshot_waited_behind_reliable", "snapshots waited behind reliable"],
  ["server_snapshot_send_age_max_ms", "server snapshot send age max"],
  ["server_snapshot_slot_replaced", "server snapshot slot replaced"],
  ["server_snapshot_project_max_ms", "server snapshot project max"],
  ["server_snapshot_project_p95_ms", "server snapshot project p95"],
  ["server_snapshot_compact_max_ms", "server snapshot compact max"],
  ["server_snapshot_compact_p95_ms", "server snapshot compact p95"],
  ["server_snapshot_queue_age_max_ms", "server snapshot queue age max"],
  ["server_snapshot_queue_age_p95_ms", "server snapshot queue age p95"],
  ["server_snapshot_serialize_max_ms", "server snapshot serialize max"],
  ["server_snapshot_serialize_p95_ms", "server snapshot serialize p95"],
  ["server_snapshot_writer_send_max_ms", "server snapshot writer send max"],
  ["server_snapshot_writer_send_p95_ms", "server snapshot writer send p95"],
  ["server_snapshot_payload_bytes_max", "server snapshot payload bytes max"],
  ["server_snapshot_payload_bytes_p95", "server snapshot payload bytes p95"],
  ["server_command_frame_deserialize_max_ms", "server command parse max"],
  ["server_command_deserialize_to_room_enqueue_max_ms", "server command enqueue max"],
  ["server_command_room_queue_max_ms", "server command room queue max"],
  ["server_command_room_handle_max_ms", "server command handle max"],
  ["server_command_receipt_send_age_max_ms", "server command receipt send age max"],
  ["server_command_accepted_to_sim_ack_max_ms", "server command accepted-to-ack max"],
];

const SUMMARY_FIELDS = [
  "rtt_ms",
  "rtt_max_ms",
  "snapshot_jitter_ms",
  "snapshot_gap_max_ms",
  "snapshot_late_frame_count",
  "predicted_snapshot_late_frame_count",
  "snapshot_bytes_max",
  "snapshot_bytes_p95",
  "snapshot_bytes_avg",
  "snapshot_segment_budget_bytes",
  "snapshot_over_segment_budget_count",
  "snapshot_over_segment_budget_pct_x100",
  "snapshot_parse_max_ms",
  "snapshot_parse_p95_ms",
  "snapshot_decode_max_ms",
  "snapshot_decode_p95_ms",
  "snapshot_apply_max_ms",
  "snapshot_apply_p95_ms",
  "frame_gap_max_ms",
  "frame_work_max_ms",
  "frame_work_p95_ms",
  "renderer_max_ms",
  "renderer_p95_ms",
  "fps_estimate",
  "commands_issued",
  "command_burst_bucket_ms",
  "command_burst_max",
  "command_burst_frame_gap_max_ms",
  "command_burst_worst_frame_phase_ms",
  "server_tick_ms",
  "server_lag_ms",
  "slow_tick_count",
  "head_of_line_count",
  "acknowledged_command_latency_ms",
  "command_issue_to_socket_send_accepted_max_ms",
  "command_issue_to_socket_send_accepted_p95_ms",
  "command_issue_to_server_receipt_max_ms",
  "command_issue_to_server_receipt_p95_ms",
  "command_server_receipt_to_sim_ack_max_ms",
  "command_server_receipt_to_sim_ack_p95_ms",
  "command_issue_to_sim_ack_max_ms",
  "command_issue_to_sim_ack_p95_ms",
  "command_ack_snapshot_received_to_applied_max_ms",
  "command_ack_snapshot_received_to_applied_p95_ms",
  "oldest_pending_command_age_ms",
  "max_pending_command_count",
  "command_family_move",
  "command_family_attack_move",
  "command_family_build",
  "command_family_train",
  "command_family_other",
  "prediction_disable_user_count",
  "prediction_disable_replay_count",
  "prediction_disable_spectator_count",
  "prediction_disable_compatibility_count",
  "prediction_disable_wasm_count",
  "prediction_disable_other_count",
  "prediction_replay_max_ms",
  "prediction_replay_max_ticks",
  "prediction_replay_budget_exceeded_count",
  "server_command_receipts_accepted",
  "server_command_receipts_rejected",
  "server_reliable_drained_before_snapshot",
  "server_reliable_drained_before_snapshot_max",
  "server_snapshot_waited_behind_reliable",
  "server_snapshot_sent",
  "server_snapshot_send_age_latest_ms",
  "server_snapshot_send_age_max_ms",
  "server_snapshot_send_age_avg_ms",
  "server_snapshot_slot_stored",
  "server_snapshot_slot_replaced",
  "server_snapshot_slot_closed",
  "server_snapshot_project_max_ms",
  "server_snapshot_project_p95_ms",
  "server_snapshot_compact_max_ms",
  "server_snapshot_compact_p95_ms",
  "server_snapshot_queue_age_max_ms",
  "server_snapshot_queue_age_p95_ms",
  "server_snapshot_serialize_max_ms",
  "server_snapshot_serialize_p95_ms",
  "server_snapshot_writer_send_max_ms",
  "server_snapshot_writer_send_p95_ms",
  "server_snapshot_payload_bytes_max",
  "server_snapshot_payload_bytes_p95",
  "server_snapshot_payload_bytes_avg",
  "server_snapshot_payload_bytes_total",
  "server_snapshot_writer_taken",
  "server_command_lifecycle_count",
  "server_command_lifecycle_accepted",
  "server_command_lifecycle_rejected",
  "server_command_frame_deserialize_max_ms",
  "server_command_frame_deserialize_p95_ms",
  "server_command_deserialize_to_room_enqueue_max_ms",
  "server_command_deserialize_to_room_enqueue_p95_ms",
  "server_command_room_queue_max_ms",
  "server_command_room_queue_p95_ms",
  "server_command_room_handle_max_ms",
  "server_command_room_handle_p95_ms",
  "server_command_receipt_send_age_max_ms",
  "server_command_receipt_send_age_p95_ms",
  "server_command_accepted_to_sim_ack_max_ms",
  "server_command_accepted_to_sim_ack_p95_ms",
];

const TRANSPORT_DIAGNOSTIC_FIELDS = [
  ["websocket_compression", "WebSocket compression"],
  ["websocket_extensions", "WebSocket extensions"],
  ["snapshot_byte_source", "snapshot byte source"],
  ["snapshot_codec", "snapshot codec"],
  ["snapshot_codec_version", "snapshot codec version"],
  ["snapshot_frame_kind", "snapshot frame kind"],
];

const ISSUE_GROUPS = [
  {
    id: "server_tick_scheduler",
    label: "server tick/scheduler pressure",
    fields: ["server_tick_ms", "server_lag_ms", "slow_tick_count", "tick_ms", "scheduler_lag_ms"],
  },
  {
    id: "server_snapshot_projection",
    label: "server snapshot projection/compact/serialization cost",
    fields: [
      "max_snapshot_ms",
      "snapshot_ms",
      "compact_ms",
      "serialize_ms",
      "server_snapshot_project_max_ms",
      "server_snapshot_project_p95_ms",
      "server_snapshot_compact_max_ms",
      "server_snapshot_compact_p95_ms",
      "server_snapshot_serialize_max_ms",
      "server_snapshot_serialize_p95_ms",
    ],
  },
  {
    id: "snapshot_payload_composition",
    label: "snapshot payload composition and packet-budget pressure",
    fields: [
      "snapshot_bytes_max",
      "snapshot_bytes_p95",
      "snapshot_over_segment_budget_pct_x100",
      "server_snapshot_payload_bytes_max",
      "server_snapshot_payload_bytes_p95",
      "server_snapshot_payload_bytes_avg",
    ],
  },
  {
    id: "websocket_writer_send",
    label: "WebSocket writer/send and outbound snapshot pressure",
    fields: [
      "send_ms",
      "bytes",
      "ws_buffered_bytes",
      "head_of_line_count",
      "server_reliable_drained_before_snapshot",
      "server_reliable_drained_before_snapshot_max",
      "server_snapshot_waited_behind_reliable",
      "server_snapshot_send_age_max_ms",
      "server_snapshot_queue_age_max_ms",
      "server_snapshot_queue_age_p95_ms",
      "server_snapshot_writer_send_max_ms",
      "server_snapshot_writer_send_p95_ms",
      "server_snapshot_slot_replaced",
      "server_snapshot_slot_closed",
    ],
  },
  {
    id: "client_network_delivery",
    label: "client network RTT/jitter/snapshot delivery gaps",
    fields: [
      "rtt_ms",
      "rtt_max_ms",
      "bad_rtt_samples",
      "snapshot_jitter_ms",
      "snapshot_gap_max_ms",
      "snapshot_late_frame_count",
      "jitter_samples",
      "snapshot_tick_gap_max",
      "snapshot_burst_max",
    ],
  },
  {
    id: "browser_processing",
    label: "browser payload parsing/decode/apply/frame work",
    fields: [
      "snapshot_bytes_max",
      "snapshot_bytes_p95",
      "snapshot_over_segment_budget_pct_x100",
      "snapshot_parse_max_ms",
      "snapshot_decode_max_ms",
      "snapshot_apply_max_ms",
      "prediction_apply_max_ms",
      "frame_gap_max_ms",
      "frame_work_max_ms",
      "command_burst_frame_gap_max_ms",
      "renderer_max_ms",
      "fps_estimate",
    ],
  },
  {
    id: "command_density",
    label: "command density and receipt volume",
    fields: [
      "commands_issued",
      "command_burst_max",
      "server_command_receipts_accepted",
      "server_command_receipts_rejected",
      "command_rejected",
    ],
  },
  {
    id: "command_path",
    label: "command upload/receipt/sim/downstream/render delay",
    fields: [
      "acknowledged_command_latency_ms",
      "command_issue_to_socket_send_accepted_max_ms",
      "command_issue_to_server_receipt_max_ms",
      "command_server_receipt_to_sim_ack_max_ms",
      "command_issue_to_sim_ack_max_ms",
      "command_ack_snapshot_received_to_applied_max_ms",
      "server_command_frame_deserialize_max_ms",
      "server_command_deserialize_to_room_enqueue_max_ms",
      "server_command_room_queue_max_ms",
      "server_command_room_handle_max_ms",
      "server_command_receipt_send_age_max_ms",
      "server_command_accepted_to_sim_ack_max_ms",
      "oldest_pending_command_age_ms",
      "max_pending_command_count",
      "command_rejected",
    ],
  },
  {
    id: "prediction_health",
    label: "prediction disable/replay/late-snapshot coverage",
    fields: [
      "prediction_disable_user_count",
      "prediction_disable_replay_count",
      "prediction_disable_spectator_count",
      "prediction_disable_compatibility_count",
      "prediction_disable_wasm_count",
      "prediction_disable_other_count",
      "prediction_replay_max_ms",
      "prediction_replay_max_ticks",
      "prediction_replay_budget_exceeded_count",
      "snapshot_late_frame_count",
      "predicted_snapshot_late_frame_count",
    ],
  },
];

const WARN_THRESHOLD = {
  rtt_ms: 180,
  rtt_max_ms: 180,
  snapshot_jitter_ms: 20,
  snapshot_gap_max_ms: 100,
  snapshot_late_frame_count: 1,
  predicted_snapshot_late_frame_count: 1,
  snapshot_bytes_max: 256 * 1024,
  snapshot_bytes_p95: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES + 1,
  snapshot_bytes_avg: 128 * 1024,
  snapshot_over_segment_budget_pct_x100: SNAPSHOT_PACKET_BUDGET_RATE_WARN_X100,
  snapshot_parse_max_ms: 16,
  snapshot_parse_p95_ms: 8,
  snapshot_decode_max_ms: 16,
  snapshot_decode_p95_ms: 8,
  snapshot_apply_max_ms: 16,
  snapshot_apply_p95_ms: 8,
  prediction_apply_max_ms: 16,
  prediction_apply_p95_ms: 8,
  snapshot_tick_gap_max: 3,
  snapshot_burst_max: 3,
  frame_gap_max_ms: 100,
  frame_work_max_ms: 33,
  frame_work_p95_ms: 24,
  renderer_max_ms: 33,
  renderer_p95_ms: 16,
  commands_issued: 20,
  command_burst_max: 6,
  command_burst_frame_gap_max_ms: 100,
  ws_buffered_bytes: 64 * 1024,
  server_tick_ms: 33,
  server_lag_ms: 33,
  tick_ms: 40,
  scheduler_lag_ms: 33,
  slowest_phase_ms: 33,
  max_snapshot_ms: 8,
  snapshot_ms: 8,
  compact_ms: 8,
  serialize_ms: 10,
  send_ms: 10,
  head_of_line_count: 1,
  slow_tick_count: 1,
  bad_rtt_samples: 1,
  jitter_samples: 1,
  acknowledged_command_latency_ms: 180,
  command_issue_to_socket_send_accepted_max_ms: 16,
  command_issue_to_socket_send_accepted_p95_ms: 16,
  command_issue_to_server_receipt_max_ms: 180,
  command_issue_to_server_receipt_p95_ms: 180,
  command_server_receipt_to_sim_ack_max_ms: 66,
  command_server_receipt_to_sim_ack_p95_ms: 66,
  command_issue_to_sim_ack_max_ms: 180,
  command_issue_to_sim_ack_p95_ms: 180,
  command_ack_snapshot_received_to_applied_max_ms: 16,
  command_ack_snapshot_received_to_applied_p95_ms: 16,
  server_command_frame_deserialize_max_ms: 8,
  server_command_frame_deserialize_p95_ms: 8,
  server_command_deserialize_to_room_enqueue_max_ms: 66,
  server_command_deserialize_to_room_enqueue_p95_ms: 66,
  server_command_room_queue_max_ms: 66,
  server_command_room_queue_p95_ms: 66,
  server_command_room_handle_max_ms: 66,
  server_command_room_handle_p95_ms: 66,
  server_command_receipt_send_age_max_ms: 100,
  server_command_receipt_send_age_p95_ms: 100,
  server_command_accepted_to_sim_ack_max_ms: 66,
  server_command_accepted_to_sim_ack_p95_ms: 66,
  oldest_pending_command_age_ms: 180,
  max_pending_command_count: 8,
  command_rejected: 1,
  prediction_disable_user_count: 1,
  prediction_disable_replay_count: 1,
  prediction_disable_spectator_count: 1,
  prediction_disable_compatibility_count: 1,
  prediction_disable_wasm_count: 1,
  prediction_disable_other_count: 1,
  prediction_replay_max_ms: 8,
  prediction_replay_max_ticks: 8,
  prediction_replay_budget_exceeded_count: 1,
  server_command_receipts_accepted: 20,
  server_command_receipts_rejected: 1,
  server_reliable_drained_before_snapshot_max: 2,
  server_snapshot_send_age_max_ms: 100,
  server_snapshot_slot_replaced: 1,
  server_snapshot_slot_closed: 1,
  server_snapshot_project_max_ms: 8,
  server_snapshot_project_p95_ms: 8,
  server_snapshot_compact_max_ms: 8,
  server_snapshot_compact_p95_ms: 8,
  server_snapshot_queue_age_max_ms: 100,
  server_snapshot_queue_age_p95_ms: 100,
  server_snapshot_serialize_max_ms: 10,
  server_snapshot_serialize_p95_ms: 10,
  server_snapshot_writer_send_max_ms: 10,
  server_snapshot_writer_send_p95_ms: 10,
  server_snapshot_payload_bytes_max: 256 * 1024,
  server_snapshot_payload_bytes_p95: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES + 1,
  server_snapshot_payload_bytes_avg: 128 * 1024,
};

function usage() {
  console.log(`Usage:
  node scripts/parse-net-report-logs.mjs [options] <fly-log.jsonl...>

Options:
  --format markdown|json|tsv   Output format for stdout. Default: markdown.
  --out-dir DIR                Write the markdown/json/tsv incident package.
  --timeline-band-ms MS        Timeline band width for the agent digest. Default: 60000.
  -h, --help                   Show this help.

Input is Fly JSONL from scripts/fly-logs.sh search/recent or raw tracing text.
The parser extracts client_net_report, match_started, match_ended, performance tick summary,
performance snapshot timing, and performance writer timing rows. Malformed rows become warnings.`);
}

function parseArgs(argv) {
  const options = {
    format: "markdown",
    outDir: null,
    timelineBandMs: DEFAULT_TIMELINE_BAND_MS,
    files: [],
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else if (arg === "--format") {
      options.format = argv[index + 1] || "";
      index += 1;
    } else if (arg.startsWith("--format=")) {
      options.format = arg.slice("--format=".length);
    } else if (arg === "--out-dir") {
      options.outDir = argv[index + 1] || "";
      index += 1;
    } else if (arg.startsWith("--out-dir=")) {
      options.outDir = arg.slice("--out-dir=".length);
    } else if (arg === "--timeline-band-ms") {
      options.timelineBandMs = Number(argv[index + 1] || "");
      index += 1;
    } else if (arg.startsWith("--timeline-band-ms=")) {
      options.timelineBandMs = Number(arg.slice("--timeline-band-ms=".length));
    } else if (arg.startsWith("--")) {
      throw new Error(`unknown option: ${arg}`);
    } else {
      options.files.push(arg);
    }
  }

  if (!["markdown", "json", "tsv"].includes(options.format)) {
    throw new Error(`unsupported --format value: ${options.format}`);
  }
  if (!Number.isFinite(options.timelineBandMs) || options.timelineBandMs < 1_000) {
    throw new Error("--timeline-band-ms must be a number of at least 1000");
  }
  return options;
}

function stripAnsi(value) {
  return value.replace(ANSI_PATTERN, "");
}

function lineTimestamp(raw, parsedJson) {
  if (parsedJson?.timestamp) {
    return parsedJson.timestamp;
  }
  const clean = stripAnsi(raw);
  return clean.match(/\b20\d\d-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z\b/)?.[0] || "";
}

function parseInputLine(line, source, lineNumber) {
  let parsedJson = null;
  let rawMessage = line;
  try {
    parsedJson = JSON.parse(line);
    rawMessage = parsedJson.message || "";
  } catch (error) {
    if (line.trim().startsWith("{")) {
      return {
        warning: `${source}:${lineNumber} is not valid JSONL: ${error.message}`,
      };
    }
  }

  const cleanMessage = stripAnsi(rawMessage);
  const fields = parseFields(cleanMessage);
  const event = normalizeEvent(fields.event, cleanMessage);
  if (!event) {
    return null;
  }
  const normalizedFields = normalizeFields(fields);
  const room = extractDelimitedField(cleanMessage, "room", "match_run_id");
  if (room) {
    normalizedFields.room = room;
  }

  return {
    event,
    timestamp: lineTimestamp(line, parsedJson),
    source,
    sourceMatch: inferSourceMatch(source),
    lineNumber,
    message: cleanMessage,
    fields: normalizedFields,
  };
}

function inferSourceMatch(source) {
  return path.basename(source).match(/match[-_]?(\d+)/i)?.[1] || path.basename(source);
}

function extractDelimitedField(message, key, nextKey) {
  const marker = `${key}=`;
  const start = message.indexOf(marker);
  if (start < 0) {
    return "";
  }
  const valueStart = start + marker.length;
  const next = message.slice(valueStart).match(new RegExp(`\\s+${nextKey}=`));
  if (!next) {
    return "";
  }
  const raw = message.slice(valueStart, valueStart + next.index).trim();
  return String(parseValue(raw)).trim();
}

function normalizeEvent(event, message) {
  if (event === "client_net_report") {
    return "client_net_report";
  }
  if (event === "match_started" || event === "match_ended") {
    return event;
  }
  if (event === "tick" || message.includes("performance tick summary")) {
    return "performance_tick";
  }
  if (event === "snapshot" || message.includes("performance snapshot timing")) {
    return "performance_snapshot";
  }
  if (event === "writer_send" || message.includes("performance writer timing")) {
    return "performance_writer";
  }
  return null;
}

function normalizeFields(fields) {
  const out = { ...fields };
  if (out["ctx.winner_id"] !== undefined && out.winner_id === undefined) {
    out.winner_id = parseSome(out["ctx.winner_id"]);
  }
  if (out["ctx.winner_team_id"] !== undefined && out.winner_team_id === undefined) {
    out.winner_team_id = parseSome(out["ctx.winner_team_id"]);
  }
  if (typeof out.participants === "string" && out.participants.startsWith("[")) {
    try {
      out.participants = JSON.parse(out.participants);
    } catch {
      // Keep the raw value.
    }
  }
  return out;
}

function parseSome(value) {
  const match = String(value).match(/^Some\(([^)]+)\)$/);
  if (!match) {
    return value;
  }
  return parseValue(match[1]);
}

function parseFields(message) {
  const fields = {};
  let index = 0;
  while (index < message.length) {
    const keyMatch = message.slice(index).match(/\b([A-Za-z_][A-Za-z0-9_.]*)=/);
    if (!keyMatch) {
      break;
    }
    const key = keyMatch[1];
    index += keyMatch.index + keyMatch[0].length;
    const [rawValue, nextIndex] = readValue(message, index);
    fields[key] = parseValue(rawValue);
    index = nextIndex;
  }
  return fields;
}

function readValue(message, start) {
  while (message[start] === " ") {
    start += 1;
  }
  if (message[start] === "\"") {
    let escaped = false;
    for (let index = start + 1; index < message.length; index += 1) {
      const ch = message[index];
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === "\"") {
        return [message.slice(start, index + 1), index + 1];
      }
    }
    return [message.slice(start), message.length];
  }

  if (message[start] === "[") {
    return readBalanced(message, start, "[", "]");
  }
  if (message.startsWith("Some(", start)) {
    return readBalanced(message, start + "Some".length, "(", ")", "Some");
  }

  let end = start;
  while (end < message.length && !/\s/.test(message[end])) {
    end += 1;
  }
  return [message.slice(start, end), end];
}

function readBalanced(message, openIndex, open, close, prefix = "") {
  let depth = 0;
  let inQuote = false;
  let escaped = false;
  for (let index = openIndex; index < message.length; index += 1) {
    const ch = message[index];
    if (inQuote) {
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === "\"") {
        inQuote = false;
      }
      continue;
    }
    if (ch === "\"") {
      inQuote = true;
    } else if (ch === open) {
      depth += 1;
    } else if (ch === close) {
      depth -= 1;
      if (depth === 0) {
        const raw = message.slice(openIndex, index + 1);
        return [`${prefix}${raw}`, index + 1];
      }
    }
  }
  return [message.slice(openIndex), message.length];
}

function parseValue(raw) {
  if (raw === "true") {
    return true;
  }
  if (raw === "false") {
    return false;
  }
  if (/^-?\d+(?:\.\d+)?$/.test(raw)) {
    return Number(raw);
  }
  if (raw.startsWith("\"") && raw.endsWith("\"")) {
    try {
      return JSON.parse(raw);
    } catch {
      return raw.slice(1, -1);
    }
  }
  return raw;
}

function readRows(files) {
  const rows = [];
  const warnings = [];
  for (const file of files) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (!line.trim()) {
        return;
      }
      const parsed = parseInputLine(line, file, index + 1);
      if (parsed?.warning) {
        warnings.push(parsed.warning);
      } else if (parsed) {
        rows.push(parsed);
      }
    });
  }
  return { rows, warnings };
}

function analyze(rows, warnings) {
  const matches = new Map();
  const unmatched = [];
  for (const row of rows) {
    const matchKey = /^\d+$/.test(row.sourceMatch || "")
      ? `source:${row.sourceMatch}`
      : row.fields.match_run_id || row.sourceMatch || "unknown";
    const match = ensureMatch(matches, matchKey, row.sourceMatch);
    match.rows.push(row);
    if (row.event === "match_started") {
      match.started.push(row);
      applyMatchFields(match, row.fields);
    } else if (row.event === "match_ended") {
      match.ended.push(row);
      applyMatchFields(match, row.fields);
    } else if (row.event === "client_net_report") {
      applyMatchFields(match, row.fields);
      addPlayerReport(match, row);
    } else if (row.event === "performance_tick") {
      match.serverTicks.push(row);
    } else if (row.event === "performance_snapshot") {
      match.snapshots.push(row);
    } else if (row.event === "performance_writer") {
      match.writers.push(row);
    } else {
      unmatched.push(row);
    }
  }

  const matchSummaries = [...matches.values()].map(finalizeMatch);
  return {
    generatedAt: new Date().toISOString(),
    input: {
      files: [...new Set(rows.map((row) => row.source))],
      rows: rows.length,
      warnings: warnings.length,
    },
    warnings,
    matches: matchSummaries,
    unmatched: unmatched.length,
  };
}

function ensureMatch(matches, key, sourceMatch) {
  if (!matches.has(key)) {
    matches.set(key, {
      key,
      sourceMatches: new Set(sourceMatch ? [sourceMatch] : []),
      rows: [],
      started: [],
      ended: [],
      players: new Map(),
      serverTicks: [],
      snapshots: [],
      writers: [],
      participants: [],
      buildIds: new Set(),
      rooms: new Set(),
    });
  }
  const match = matches.get(key);
  if (sourceMatch) {
    match.sourceMatches.add(sourceMatch);
  }
  return match;
}

function applyMatchFields(match, fields) {
  if (fields.match_run_id) {
    match.matchRunId = fields.match_run_id;
  }
  if (fields.build_id) {
    match.buildIds.add(String(fields.build_id));
  }
  if (fields.room) {
    match.room = fields.room;
    match.rooms.add(String(fields.room));
  }
  if (fields.map) {
    match.map = fields.map;
  }
  if (Array.isArray(fields.participants)) {
    match.participants = fields.participants;
  }
  for (const key of [
    "duration_ms",
    "duration_ticks",
    "winner_id",
    "winner_team_id",
    "slow_tick_count",
    "max_head_of_line_count",
    "humans",
    "ai",
    "seed",
    "mode",
  ]) {
    if (fields[key] !== undefined) {
      match[key] = fields[key];
    }
  }
  if (fields.players !== undefined) {
    match.player_count = fields.players;
  }
}

function addPlayerReport(match, row) {
  const playerId = String(row.fields.player_id ?? "unknown");
  if (!match.players.has(playerId)) {
    match.players.set(playerId, {
      playerId,
      reports: [],
      primaryIssues: new Map(),
    });
  }
  const player = match.players.get(playerId);
  player.reports.push(row);
  const issue = row.fields.primary_issue || "unknown";
  player.primaryIssues.set(issue, (player.primaryIssues.get(issue) || 0) + 1);
}

function finalizeMatch(match) {
  const allRows = match.rows;
  const players = [...match.players.values()]
    .map(finalizePlayer)
    .sort((a, b) => String(a.playerId).localeCompare(String(b.playerId), undefined, { numeric: true }));

  const matchPerf = summarizePerf(match);
  const groups = ISSUE_GROUPS.map((group) => classifyGroup(group, players, matchPerf, allRows)).filter(Boolean);
  const missing = missingDiagnosticGroups(allRows);
  const transport = summarizeTransport(allRows);

  return {
    match: sourceLabel(match),
    matchRunId: match.matchRunId || "",
    sourceMatches: [...match.sourceMatches].sort(),
    buildIds: [...match.buildIds].sort(),
    rooms: [...match.rooms].sort(),
    room: match.room || "",
    map: match.map || "",
    seed: match.seed,
    mode: match.mode || "",
    participants: match.participants,
    startedAt: firstTimestamp(match.started),
    endedAt: firstTimestamp(match.ended),
    durationMs: match.duration_ms,
    durationTicks: match.duration_ticks,
    winnerId: match.winner_id,
    winnerTeamId: match.winner_team_id,
    reportRows: players.reduce((sum, player) => sum + player.reportCount, 0),
    serverTickRows: match.serverTicks.length,
    snapshotRows: match.snapshots.length,
    writerRows: match.writers.length,
    players,
    matchPerf,
    transport,
    classifications: groups,
    missing,
    transportNote: transportNoteFor(transport),
  };
}

function sourceLabel(match) {
  const sourceMatches = [...match.sourceMatches].sort();
  const numericSources = sourceMatches.filter((source) => /^\d+$/.test(source));
  if (numericSources.length > 0) {
    return numericSources.join("+");
  }
  return match.matchRunId || sourceMatches.join("+") || match.key;
}

function firstTimestamp(rows) {
  return rows.find((row) => row.timestamp)?.timestamp || "";
}

function finalizePlayer(player) {
  const reports = player.reports;
  const values = Object.fromEntries(
    SUMMARY_FIELDS.map((field) => [field, summarizeField(reports, field, field === "fps_estimate" ? "min" : "max")])
  );
  const issues = [...player.primaryIssues.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([issue, count]) => ({ issue, count }));
  const evidence = ISSUE_GROUPS.map((group) => groupEvidence(group.fields, reports)).filter(
    (item) => item.evidence.length > 0
  );

  return {
    playerId: player.playerId,
    reportCount: reports.length,
    firstReportAt: firstTimestamp(reports),
    lastReportAt: reports.slice().reverse().find((row) => row.timestamp)?.timestamp || "",
    primaryIssues: issues,
    metrics: values,
    transport: summarizeTransport(reports),
    commandLifecycle: summarizeCommandLifecycle(reports),
    snapshotPayload: summarizeSnapshotPayload(reports),
    evidence,
  };
}

function summarizeTransport(rows) {
  return Object.fromEntries(
    TRANSPORT_DIAGNOSTIC_FIELDS.map(([field, label]) => [
      camelCase(field),
      {
        label,
        samples: rows.filter((row) => row.fields[field] !== undefined).length,
        values: summarizeStringField(rows, field),
      },
    ])
  );
}

function summarizeSnapshotPayload(rows) {
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

function summarizeStringField(rows, field) {
  const counts = new Map();
  for (const row of rows) {
    if (row.fields[field] === undefined) {
      continue;
    }
    const value = transportValue(row.fields[field]);
    counts.set(value, (counts.get(value) || 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([value, count]) => ({ value, count }));
}

function transportValue(value) {
  const text = String(value ?? "");
  return text.length > 0 ? text.slice(0, 128) : "(empty)";
}

function transportNoteFor(transport) {
  const base =
    "Unsupported: Fly logs and ClientNetReport do not expose packet loss, retransmits, or per-packet browser transport data. Packet-budget fields are payload bytes only and exclude WebSocket/TLS/TCP/IP overhead.";
  const compression = transport.websocketCompression;
  if (!compression || compression.samples === 0) {
    return `${base} WebSocket compression negotiation was not reported by these rows.`;
  }
  const negotiated = compression.values.some((item) => item.value === "permessage-deflate");
  if (negotiated) {
    return `${base} Client reports say permessage-deflate negotiated; snapshot byte fields still measure application payload bytes, not compressed wire bytes.`;
  }
  return `${base} Client reports did not show negotiated WebSocket compression.`;
}

function summarizeField(rows, field, mode = "max") {
  const numbers = rows.map((row) => row.fields[field]).filter((value) => Number.isFinite(value));
  if (numbers.length === 0) {
    return null;
  }
  const sorted = numbers.slice().sort((a, b) => a - b);
  return {
    min: sorted[0],
    max: sorted[sorted.length - 1],
    p95: percentile(sorted, 0.95),
    selected: mode === "min" ? sorted[0] : sorted[sorted.length - 1],
    samples: numbers.length,
  };
}

function percentile(sortedValues, percentileValue) {
  if (sortedValues.length === 0) {
    return null;
  }
  const index = Math.min(sortedValues.length - 1, Math.ceil(sortedValues.length * percentileValue) - 1);
  return sortedValues[index];
}

function summarizePerf(match) {
  return {
    serverTick: summarizeRows(match.serverTicks, [
      "tick_ms",
      "scheduler_lag_ms",
      "sim_ms",
      "fanout_ms",
      "max_snapshot_ms",
      "snapshot_replaced",
      "snapshot_closed",
    ]),
    snapshots: summarizeRows(match.snapshots, [
      "snapshot_ms",
      "compact_ms",
      "total_ms",
      "entities",
      "resource_deltas",
      "events",
    ]),
    writers: summarizeRows(match.writers, ["serialize_ms", "send_ms", "bytes"]),
  };
}

function summarizeRows(rows, fields) {
  const out = {
    rows: rows.length,
  };
  for (const field of fields) {
    out[field] = summarizeField(rows, field);
  }
  return out;
}

function classifyGroup(group, players, matchPerf, rows) {
  const evidenceFor = [];
  const evidenceAgainst = [];
  let availableSamples = 0;
  for (const player of players) {
    for (const field of group.fields) {
      const metric = player.metrics[field];
      if (!metric) {
        continue;
      }
      availableSamples += metric.samples || 1;
      addClassificationEvidence(`player ${player.playerId}`, field, metric, evidenceFor, evidenceAgainst);
    }
  }
  for (const [bucketName, bucket] of Object.entries(matchPerf)) {
    for (const field of group.fields) {
      const metric = bucket[field];
      if (metric) {
        availableSamples += metric.samples || 1;
        addClassificationEvidence(bucketName, field, metric, evidenceFor, evidenceAgainst);
      }
    }
  }

  const rawFieldSet = new Set(rows.flatMap((row) => Object.keys(row.fields)));
  const sawRawField = group.fields.some((field) => rawFieldSet.has(field));
  const status = classificationStatus(evidenceFor, evidenceAgainst, availableSamples, sawRawField);
  const evidence = evidenceFor.slice();

  return {
    id: group.id,
    label: group.label,
    result: status === "indicated" || status === "weak" ? "indicated" : "not indicated",
    status,
    evidence,
    evidenceFor,
    evidenceAgainst: evidenceAgainst.slice(0, 8),
    unavailable:
      status === "unavailable"
        ? group.fields.map((field) => `${field}: not logged or unavailable in this artifact`)
        : [],
  };
}

function addClassificationEvidence(scope, field, metric, evidenceFor, evidenceAgainst) {
  const threshold = WARN_THRESHOLD[field];
  if (field === "fps_estimate") {
    if (Number.isFinite(metric.min) && metric.min <= 30) {
      evidenceFor.push(`${scope} ${field} min ${metric.min} at or below 30`);
    } else if (Number.isFinite(metric.min)) {
      evidenceAgainst.push(`${scope} ${field} min ${metric.min} above 30`);
    }
    return;
  }
  if (threshold === undefined) {
    return;
  }
  if (Number.isFinite(metric.max) && metric.max >= threshold) {
    evidenceFor.push(`${scope} ${field} max ${metric.max} at or above ${threshold}`);
  } else if (Number.isFinite(metric.max)) {
    evidenceAgainst.push(`${scope} ${field} max ${metric.max} below ${threshold}`);
  }
}

function classificationStatus(evidenceFor, evidenceAgainst, availableSamples, sawRawField) {
  if (evidenceFor.length >= 2) {
    return "indicated";
  }
  if (evidenceFor.length === 1) {
    return availableSamples > 1 ? "indicated" : "weak";
  }
  if (!sawRawField && availableSamples === 0) {
    return "unavailable";
  }
  if (evidenceAgainst.length > 0) {
    return "contradicted";
  }
  return "unknown";
}

function groupEvidence(fields, rows) {
  const evidence = [];
  for (const [field, label, mode] of METRICS) {
    if (!fields.includes(field)) {
      continue;
    }
    const summary = summarizeField(rows, field, mode || "max");
    if (!summary) {
      continue;
    }
    const selected = mode === "min" ? summary.min : summary.max;
    evidence.push(`${label} ${mode === "min" ? "min" : "max"} ${selected} (row p95 ${summary.p95})`);
  }
  return {
    label: ISSUE_GROUPS.find((group) => group.fields.some((field) => fields.includes(field)))?.label || "evidence",
    evidence,
  };
}

function missingDiagnosticGroups(rows) {
  const fields = new Set();
  for (const row of rows) {
    for (const key of Object.keys(row.fields)) {
      fields.add(key);
    }
  }
  const missing = [];
  for (const group of ISSUE_GROUPS) {
    if (!group.fields.some((field) => fields.has(field))) {
      missing.push(`${group.label}: no matching fields in input`);
    }
  }
  if (!fields.has("snapshot_bytes_p95") && !fields.has("snapshot_over_segment_budget_pct_x100")) {
    missing.push("snapshot packet-budget payload p95/rate: no matching fields in input");
  }
  if (!fields.has("websocket_compression")) {
    missing.push("WebSocket compression negotiation: no matching fields in input");
  }
  if (!fields.has("snapshot_byte_source")) {
    missing.push("snapshot byte measurement source: no matching fields in input");
  }
  if (!fields.has("snapshot_codec") || !fields.has("snapshot_frame_kind")) {
    missing.push("snapshot codec/frame kind: no matching fields in input");
  }
  if (!fields.has("command_burst_max")) {
    missing.push("command burst density: no matching fields in input");
  }
  if (!fields.has("server_reliable_drained_before_snapshot")) {
    missing.push("server reliable/snapshot outbound pressure: no matching fields in input");
  }
  if (!fields.has("server_snapshot_project_max_ms") || !fields.has("server_snapshot_serialize_max_ms")) {
    missing.push("server snapshot lifecycle window: no projection/compact/serialize fields in input");
  }
  if (!fields.has("server_snapshot_payload_sections")) {
    missing.push("server snapshot payload composition: no section/entity-kind fields in input");
  }
  if (!fields.has("prediction_disable_wasm_count") || !fields.has("prediction_replay_max_ms")) {
    missing.push("prediction disable reason/replay budget detail: no matching fields in input");
  }
  if (!fields.has("predicted_snapshot_late_frame_count")) {
    missing.push("predicted snapshot coverage during late snapshot frames: no matching fields in input");
  }
  return missing;
}

function formatMarkdown(report) {
  const lines = [];
  lines.push("# Network Incident Summary");
  lines.push("");
  lines.push(`Generated: ${report.generatedAt}`);
  lines.push(`Input rows: ${report.input.rows}`);
  appendAgentDigestMarkdown(lines, report.agentDigest);
  if (report.warnings.length > 0) {
    lines.push("");
    lines.push("## Warnings");
    for (const warning of report.warnings) {
      lines.push(`- ${warning}`);
    }
  }

  for (const match of report.matches) {
    lines.push("");
    lines.push(`## Match ${match.match}`);
    lines.push("");
    lines.push(`- Sources: ${match.sourceMatches.join(", ") || "unknown"}`);
    if (match.matchRunId) {
      lines.push(`- Match run id: ${match.matchRunId}`);
    }
    if (match.participants.length > 0) {
      lines.push(`- Participants: ${match.participants.join(", ")}`);
    }
    if (match.durationTicks !== undefined) {
      lines.push(`- Duration: ${formatValue(match.durationMs)} ms / ${formatValue(match.durationTicks)} ticks`);
    }
    lines.push(
      `- Rows: ${match.reportRows} client reports, ${match.serverTickRows} tick, ${match.snapshotRows} snapshot, ${match.writerRows} writer`
    );
    lines.push(`- Transport diagnostics: ${formatTransportDiagnostics(match.transport)}`);

    lines.push("");
    lines.push("| player | reports | primary issues | RTT max | snapshot gap max | jitter max | payload max | payload p95 | over budget | parse/decode/apply max | frame gap max | frame work max | renderer max | FPS min | cmds/burst | cmd response max | server outbound | server tick max | server lag max |");
    lines.push("| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- | ---: | --- | ---: | ---: |");
    for (const player of match.players) {
      lines.push(
        [
          player.playerId,
          player.reportCount,
          player.primaryIssues.map((issue) => `${issue.issue}=${issue.count}`).join(", ") || "none",
          metricMax(player, "rtt_max_ms"),
          metricMax(player, "snapshot_gap_max_ms"),
          metricMax(player, "snapshot_jitter_ms"),
          metricMax(player, "snapshot_bytes_max"),
          metricMax(player, "snapshot_bytes_p95"),
          metricPctX100Max(player, "snapshot_over_segment_budget_pct_x100"),
          `${metricMax(player, "snapshot_parse_max_ms")}/${metricMax(player, "snapshot_decode_max_ms")}/${metricMax(player, "snapshot_apply_max_ms")}`,
          metricMax(player, "frame_gap_max_ms"),
          metricMax(player, "frame_work_max_ms"),
          metricMax(player, "renderer_max_ms"),
          metricMin(player, "fps_estimate"),
          `${metricMax(player, "commands_issued")}/${metricMax(player, "command_burst_max")}`,
          metricMax(player, "command_issue_to_sim_ack_max_ms") !== "n/a"
            ? metricMax(player, "command_issue_to_sim_ack_max_ms")
            : metricMax(player, "acknowledged_command_latency_ms"),
          [
            metricMax(player, "server_reliable_drained_before_snapshot"),
            metricMax(player, "server_snapshot_waited_behind_reliable"),
            metricMax(player, "server_snapshot_send_age_max_ms"),
            metricMax(player, "server_snapshot_slot_replaced"),
          ].join("/"),
          metricMax(player, "server_tick_ms"),
          metricMax(player, "server_lag_ms"),
        ].join(" | ").replace(/^/, "| ").replace(/$/, " |")
      );
    }

    appendSnapshotPayloadMarkdown(lines, match.players);
    appendCommandLifecycleMarkdown(lines, match.players);

    lines.push("");
    lines.push("### Classification");
    for (const classification of match.classifications) {
      const status = classification.status || classification.result;
      const suffix =
        classification.evidenceFor?.length > 0
          ? `: for ${classification.evidenceFor.slice(0, 4).join("; ")}`
          : classification.evidenceAgainst?.length > 0
            ? `: against ${classification.evidenceAgainst.slice(0, 3).join("; ")}`
            : classification.unavailable?.length > 0
              ? `: ${classification.unavailable.slice(0, 2).join("; ")}`
              : "";
      lines.push(`- ${classification.label}: ${status}${suffix}`);
    }
    lines.push(`- Transport/WebTransport theory: ${match.transportNote}`);

    if (match.missing.length > 0) {
      lines.push("");
      lines.push("### Missing Data");
      for (const missing of match.missing) {
        lines.push(`- ${missing}`);
      }
    }
  }
  return `${lines.join("\n")}\n`;
}

function appendSnapshotPayloadMarkdown(lines, players) {
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
          metricMax(player, "server_snapshot_project_max_ms"),
          metricMax(player, "server_snapshot_compact_max_ms"),
          metricMax(player, "server_snapshot_queue_age_max_ms"),
          metricMax(player, "server_snapshot_serialize_max_ms"),
          metricMax(player, "server_snapshot_writer_send_max_ms"),
        ].join("/"),
        `${metricMax(player, "server_snapshot_payload_bytes_p95")}/${metricMax(player, "server_snapshot_payload_bytes_max")}`,
        formatSnapshotPayloadEntries(player.snapshotPayload.sections, "bytes"),
        formatSnapshotPayloadEntries(player.snapshotPayload.entityKinds, "approx bytes"),
      ]
        .join(" | ")
        .replace(/^/, "| ")
        .replace(/$/, " |")
    );
  }
}

function formatSnapshotPayloadEntries(entries, byteLabel) {
  if (!Array.isArray(entries) || entries.length === 0) {
    return "n/a";
  }
  return entries
    .slice(0, 4)
    .map((entry) => `${entry.label} ${formatPctX100(entry.pctX100)} ${formatValue(entry.bytes)} ${byteLabel}`)
    .join(", ");
}

function metricMax(player, field) {
  return formatValue(player.metrics[field]?.max);
}

function metricPctX100Max(player, field) {
  return formatPctX100(player.metrics[field]?.max);
}

function metricMin(player, field) {
  return formatValue(player.metrics[field]?.min);
}

function formatTransportDiagnostics(transport) {
  return TRANSPORT_DIAGNOSTIC_FIELDS.map(([field, label]) => {
    const summary = transport[camelCase(field)];
    return `${label} ${formatTransportCounts(summary)}`;
  }).join("; ");
}

function formatTransportCounts(summary) {
  if (!summary || summary.samples === 0 || summary.values.length === 0) {
    return "n/a";
  }
  return summary.values.map((item) => `${item.value}=${item.count}`).join(", ");
}

function formatPctX100(value) {
  if (value === null || value === undefined || value === "") {
    return "n/a";
  }
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return "n/a";
  }
  return `${(number / 100).toFixed(2).replace(/\.?0+$/, "")}%`;
}

function formatValue(value) {
  return value === null || value === undefined || value === "" ? "n/a" : String(value);
}

function formatTsv(report) {
  const headers = [
    "match",
    "player_id",
    "reports",
    "primary_issues",
    ...SUMMARY_FIELDS.flatMap((field) => [`${field}_max`, `${field}_p95`]),
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
          ...SUMMARY_FIELDS.flatMap((field) => [
            player.metrics[field]?.max ?? "",
            player.metrics[field]?.p95 ?? "",
          ]),
        ]
          .map(tsvCell)
          .join("\t")
      );
    }
  }
  return `${lines.join("\n")}\n`;
}

function camelCase(value) {
  return value.replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
}

function tsvCell(value) {
  return String(value).replace(/\t/g, " ").replace(/\r?\n/g, " ");
}

function writeOutputs(report, rows, outDir) {
  mkdirSync(outDir, { recursive: true });
  const files = {
    readme: path.join(outDir, "README.md"),
    evidenceIndex: path.join(outDir, "evidence-index.json"),
    keyMetrics: path.join(outDir, "key-metrics.json"),
    markdown: path.join(outDir, "incident-summary.md"),
    json: path.join(outDir, "incident-summary.json"),
    tsv: path.join(outDir, "incident-rows.tsv"),
    clientRows: path.join(outDir, "client-net-rows.tsv"),
    serverTickRows: path.join(outDir, "server-tick-rows.tsv"),
  };
  writeFileSync(files.readme, formatPackageReadme(report, files));
  writeFileSync(files.evidenceIndex, formatEvidenceIndexJson(report, files));
  writeFileSync(files.keyMetrics, formatKeyMetricsJson(report));
  writeFileSync(files.markdown, formatMarkdown(report));
  writeFileSync(files.json, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(files.tsv, formatTsv(report));
  writeFileSync(files.clientRows, formatClientRowsTsv(rows));
  writeFileSync(files.serverTickRows, formatServerTickRowsTsv(rows));
  return files;
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`error: ${error.message}`);
    usage();
    process.exit(2);
  }

  if (options.help) {
    usage();
    return;
  }
  if (options.files.length === 0) {
    console.error("error: at least one log file is required");
    usage();
    process.exit(2);
  }

  const { rows, warnings } = readRows(options.files);
  const report = analyze(rows, warnings);
  attachAgentDigest(report, rows, {
    timelineBandMs: options.timelineBandMs,
    issueGroups: ISSUE_GROUPS,
    warnThreshold: WARN_THRESHOLD,
  });

  if (options.outDir) {
    const files = writeOutputs(report, rows, options.outDir);
    for (const file of Object.values(files)) {
      console.log(`wrote ${file}`);
    }
    return;
  }

  if (options.format === "json") {
    console.log(JSON.stringify(report, null, 2));
  } else if (options.format === "tsv") {
    process.stdout.write(formatTsv(report));
  } else {
    process.stdout.write(formatMarkdown(report));
  }
}

main();
