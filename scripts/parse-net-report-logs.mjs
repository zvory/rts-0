#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const ANSI_PATTERN = /\x1B\[[0-?]*[ -/]*[@-~]/g;

const METRICS = [
  ["rtt_ms", "RTT"],
  ["rtt_max_ms", "RTT max"],
  ["snapshot_jitter_ms", "snapshot jitter"],
  ["snapshot_gap_max_ms", "snapshot gap"],
  ["snapshot_bytes_max", "payload bytes max"],
  ["snapshot_bytes_avg", "payload bytes avg"],
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
  ["ws_buffered_bytes", "WS buffered"],
  ["server_tick_ms", "server tick"],
  ["server_lag_ms", "server lag"],
  ["slow_tick_count", "slow ticks"],
  ["head_of_line_count", "head of line"],
  ["acknowledged_command_latency_ms", "legacy command ack"],
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
  ["wasm_tick_ms", "WASM tick"],
];

const SUMMARY_FIELDS = [
  "rtt_ms",
  "rtt_max_ms",
  "snapshot_jitter_ms",
  "snapshot_gap_max_ms",
  "snapshot_bytes_max",
  "snapshot_bytes_avg",
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
  "server_tick_ms",
  "server_lag_ms",
  "slow_tick_count",
  "head_of_line_count",
  "acknowledged_command_latency_ms",
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
    fields: ["max_snapshot_ms", "snapshot_ms", "compact_ms", "serialize_ms"],
  },
  {
    id: "websocket_writer_send",
    label: "WebSocket writer/send pressure",
    fields: ["send_ms", "bytes", "ws_buffered_bytes", "head_of_line_count"],
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
      "snapshot_parse_max_ms",
      "snapshot_decode_max_ms",
      "snapshot_apply_max_ms",
      "prediction_apply_max_ms",
      "frame_gap_max_ms",
      "frame_work_max_ms",
      "renderer_max_ms",
      "fps_estimate",
    ],
  },
  {
    id: "command_path",
    label: "command upload/receipt/sim/downstream/render delay",
    fields: [
      "acknowledged_command_latency_ms",
      "command_issue_to_server_receipt_max_ms",
      "command_server_receipt_to_sim_ack_max_ms",
      "command_issue_to_sim_ack_max_ms",
      "command_ack_snapshot_received_to_applied_max_ms",
      "oldest_pending_command_age_ms",
      "max_pending_command_count",
      "command_rejected",
    ],
  },
];

const WARN_THRESHOLD = {
  rtt_ms: 180,
  rtt_max_ms: 180,
  snapshot_jitter_ms: 20,
  snapshot_gap_max_ms: 100,
  snapshot_bytes_max: 256 * 1024,
  snapshot_bytes_avg: 128 * 1024,
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
  ws_buffered_bytes: 64 * 1024,
  server_tick_ms: 33,
  server_lag_ms: 33,
  tick_ms: 40,
  scheduler_lag_ms: 33,
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
  command_issue_to_server_receipt_max_ms: 180,
  command_issue_to_server_receipt_p95_ms: 180,
  command_server_receipt_to_sim_ack_max_ms: 66,
  command_server_receipt_to_sim_ack_p95_ms: 66,
  command_issue_to_sim_ack_max_ms: 180,
  command_issue_to_sim_ack_p95_ms: 180,
  command_ack_snapshot_received_to_applied_max_ms: 16,
  command_ack_snapshot_received_to_applied_p95_ms: 16,
  oldest_pending_command_age_ms: 180,
  max_pending_command_count: 8,
  command_rejected: 1,
};

function usage() {
  console.log(`Usage:
  node scripts/parse-net-report-logs.mjs [options] <fly-log.jsonl...>

Options:
  --format markdown|json|tsv   Output format for stdout. Default: markdown.
  --out-dir DIR                Write incident-summary.md, incident-summary.json, and incident-rows.tsv.
  -h, --help                   Show this help.

Input is Fly JSONL from scripts/fly-logs.sh search/recent or raw tracing text.
The parser extracts client_net_report, match_started, match_ended, performance tick summary,
performance snapshot timing, and performance writer timing rows. Malformed rows become warnings.`);
}

function parseArgs(argv) {
  const options = {
    format: "markdown",
    outDir: null,
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
    } else if (arg.startsWith("--")) {
      throw new Error(`unknown option: ${arg}`);
    } else {
      options.files.push(arg);
    }
  }

  if (!["markdown", "json", "tsv"].includes(options.format)) {
    throw new Error(`unsupported --format value: ${options.format}`);
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

  return {
    event,
    timestamp: lineTimestamp(line, parsedJson),
    source,
    sourceMatch: inferSourceMatch(source),
    lineNumber,
    message: cleanMessage,
    fields: normalizeFields(fields),
  };
}

function inferSourceMatch(source) {
  return path.basename(source).match(/match[-_]?(\d+)/i)?.[1] || path.basename(source);
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
  if (fields.room) {
    match.room = fields.room;
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
  const groups = ISSUE_GROUPS.map((group) => classifyGroup(group, players, matchPerf)).filter(Boolean);
  const missing = missingDiagnosticGroups(allRows);

  return {
    match: sourceLabel(match),
    matchRunId: match.matchRunId || "",
    sourceMatches: [...match.sourceMatches].sort(),
    room: match.room || "",
    map: match.map || "",
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
    classifications: groups,
    missing,
    transportNote:
      "Unsupported: Fly logs and ClientNetReport do not expose packet loss, retransmits, or per-packet browser transport data.",
  };
}

function sourceLabel(match) {
  return [...match.sourceMatches].sort().join("+") || match.key;
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
    evidence,
  };
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

function classifyGroup(group, players, matchPerf) {
  const evidence = [];
  for (const player of players) {
    for (const field of group.fields) {
      const metric = player.metrics[field];
      if (!metric) {
        continue;
      }
      const threshold = WARN_THRESHOLD[field];
      if (field === "fps_estimate") {
        if (metric.min <= 30) {
          evidence.push(`player ${player.playerId} ${field} min ${metric.min}`);
        }
      } else if (threshold !== undefined && metric.max >= threshold) {
        evidence.push(`player ${player.playerId} ${field} max ${metric.max}`);
      }
    }
  }
  for (const [bucketName, bucket] of Object.entries(matchPerf)) {
    for (const field of group.fields) {
      const metric = bucket[field];
      const threshold = WARN_THRESHOLD[field];
      if (metric && threshold !== undefined && metric.max >= threshold) {
        evidence.push(`${bucketName} ${field} max ${metric.max}`);
      }
    }
  }

  if (evidence.length === 0) {
    return {
      id: group.id,
      label: group.label,
      result: "not indicated",
      evidence: [],
    };
  }
  return {
    id: group.id,
    label: group.label,
    result: "indicated",
    evidence,
  };
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
  return missing;
}

function formatMarkdown(report) {
  const lines = [];
  lines.push("# Network Incident Summary");
  lines.push("");
  lines.push(`Generated: ${report.generatedAt}`);
  lines.push(`Input rows: ${report.input.rows}`);
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

    lines.push("");
    lines.push("| player | reports | primary issues | RTT max | snapshot gap max | jitter max | payload max | parse/decode/apply max | frame gap max | frame work max | renderer max | FPS min | command response max | server tick max | server lag max |");
    lines.push("| --- | ---: | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
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
          `${metricMax(player, "snapshot_parse_max_ms")}/${metricMax(player, "snapshot_decode_max_ms")}/${metricMax(player, "snapshot_apply_max_ms")}`,
          metricMax(player, "frame_gap_max_ms"),
          metricMax(player, "frame_work_max_ms"),
          metricMax(player, "renderer_max_ms"),
          metricMin(player, "fps_estimate"),
          metricMax(player, "command_issue_to_sim_ack_max_ms") !== "n/a"
            ? metricMax(player, "command_issue_to_sim_ack_max_ms")
            : metricMax(player, "acknowledged_command_latency_ms"),
          metricMax(player, "server_tick_ms"),
          metricMax(player, "server_lag_ms"),
        ].join(" | ").replace(/^/, "| ").replace(/$/, " |")
      );
    }

    lines.push("");
    lines.push("### Classification");
    for (const classification of match.classifications) {
      const suffix =
        classification.evidence.length > 0 ? `: ${classification.evidence.slice(0, 4).join("; ")}` : "";
      lines.push(`- ${classification.label}: ${classification.result}${suffix}`);
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

function metricMax(player, field) {
  return formatValue(player.metrics[field]?.max);
}

function metricMin(player, field) {
  return formatValue(player.metrics[field]?.min);
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

function tsvCell(value) {
  return String(value).replace(/\t/g, " ").replace(/\r?\n/g, " ");
}

function writeOutputs(report, outDir) {
  mkdirSync(outDir, { recursive: true });
  const files = {
    markdown: path.join(outDir, "incident-summary.md"),
    json: path.join(outDir, "incident-summary.json"),
    tsv: path.join(outDir, "incident-rows.tsv"),
  };
  writeFileSync(files.markdown, formatMarkdown(report));
  writeFileSync(files.json, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(files.tsv, formatTsv(report));
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

  if (options.outDir) {
    const files = writeOutputs(report, options.outDir);
    console.log(`wrote ${files.markdown}`);
    console.log(`wrote ${files.json}`);
    console.log(`wrote ${files.tsv}`);
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
