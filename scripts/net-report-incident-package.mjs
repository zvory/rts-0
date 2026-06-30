import path from "node:path";

const PACKAGE_SCHEMA_VERSION = 1;
const TOP_WINDOW_LIMIT = 5;
const DEFAULT_TIMELINE_BAND_MS = 60_000;

const CLIENT_ROW_FIELDS = [
  "timestamp",
  "match_run_id",
  "player_id",
  "primary_issue",
  "elapsed_ms",
  "match_tick",
  "rtt_ms",
  "rtt_max_ms",
  "snapshot_gap_max_ms",
  "snapshot_jitter_ms",
  "command_issue_to_sim_ack_max_ms",
  "command_issue_to_socket_send_accepted_max_ms",
  "command_issue_to_server_receipt_max_ms",
  "command_server_receipt_to_sim_ack_max_ms",
  "server_command_room_queue_max_ms",
  "server_command_receipt_send_age_max_ms",
  "server_command_accepted_to_sim_ack_max_ms",
  "commands_issued",
  "command_burst_max",
  "frame_gap_max_ms",
  "frame_work_max_ms",
  "renderer_max_ms",
  "server_tick_ms",
  "server_lag_ms",
  "slow_tick_count",
  "snapshot_bytes_p95",
  "snapshot_over_segment_budget_pct_x100",
  "prediction_mode",
  "prediction_replay_max_ticks",
];

const SERVER_TICK_ROW_FIELDS = [
  "timestamp",
  "match_run_id",
  "tick",
  "tick_ms",
  "sim_ms",
  "scheduler_lag_ms",
  "entities",
  "units",
  "buildings",
  "slowest_phase",
  "slowest_phase_ms",
  "max_snapshot_ms",
  "snapshot_replaced",
  "snapshot_closed",
  "pathing_requests",
  "pathing_deferred",
  "pathing_budget_exhausted",
  "pathing_worst_request_ms",
  "pathing_explored_nodes_max",
  "pathing_top_source",
];

const TOP_WINDOW_GROUPS = [
  {
    id: "command",
    label: "command response and lifecycle",
    events: ["client_net_report"],
    fields: [
      "command_issue_to_sim_ack_max_ms",
      "command_issue_to_socket_send_accepted_max_ms",
      "command_issue_to_server_receipt_max_ms",
      "command_server_receipt_to_sim_ack_max_ms",
      "server_command_room_queue_max_ms",
      "server_command_receipt_send_age_max_ms",
      "server_command_accepted_to_sim_ack_max_ms",
      "oldest_pending_command_age_ms",
      "acknowledged_command_latency_ms",
    ],
  },
  {
    id: "network",
    label: "network and snapshot delivery",
    events: ["client_net_report"],
    fields: ["rtt_max_ms", "snapshot_gap_max_ms", "snapshot_jitter_ms"],
  },
  {
    id: "snapshot_payload",
    label: "snapshot payload and packet budget",
    events: ["client_net_report"],
    fields: ["snapshot_bytes_p95", "snapshot_bytes_max", "snapshot_over_segment_budget_pct_x100"],
  },
  {
    id: "server_tick",
    label: "server tick and slow phase",
    events: ["performance_tick"],
    fields: ["tick_ms", "scheduler_lag_ms", "slowest_phase_ms", "max_snapshot_ms"],
  },
  {
    id: "server_pathing",
    label: "server pathing volume and complexity",
    events: ["performance_tick", "performance_pathing"],
    fields: [
      "pathing_requests",
      "pathing_deferred",
      "pathing_budget_exhausted",
      "pathing_worst_request_ms",
      "pathing_explored_nodes_max",
      "requests_processed",
      "requests_deferred",
      "path_budget_exhausted",
      "worst_request_ms",
      "explored_nodes_max",
    ],
  },
  {
    id: "frame_render",
    label: "browser frame and render work",
    events: ["client_net_report"],
    fields: ["frame_gap_max_ms", "frame_work_max_ms", "renderer_max_ms", "fps_estimate"],
  },
  {
    id: "prediction",
    label: "prediction replay and late-snapshot coverage",
    events: ["client_net_report"],
    fields: [
      "prediction_replay_max_ms",
      "prediction_replay_max_ticks",
      "prediction_replay_budget_exceeded_count",
      "snapshot_late_frame_count",
      "predicted_snapshot_late_frame_count",
    ],
  },
  {
    id: "command_density",
    label: "command density",
    events: ["client_net_report"],
    fields: ["commands_issued", "command_burst_max", "server_command_receipts_accepted"],
  },
  {
    id: "outbound",
    label: "server outbound writer and reliable queue",
    events: ["client_net_report", "performance_writer"],
    fields: [
      "server_reliable_drained_before_snapshot_max",
      "server_snapshot_send_age_max_ms",
      "server_snapshot_slot_replaced",
      "send_ms",
      "bytes",
      "ws_buffered_bytes",
    ],
  },
];

const COVERAGE_CLASSES = [
  {
    id: "client_reports",
    label: "client reports",
    owner: "browser ClientNetReport uploaded through the server log",
    resetWindow: "client report window, normally about ten seconds",
    privacy: "bounded aggregate fields only; no raw commands, entity ids, targets, or browser traces",
    caveat: "client-observed timing can indicate pressure but cannot prove packet loss or host CPU alone",
  },
  {
    id: "server_tick_rows",
    label: "server slow tick rows",
    owner: "server performance tracing",
    resetWindow: "one structured row per logged slow or sampled tick",
    privacy: "aggregate room shape and phase timings; no fogged entity positions",
    caveat: "missing rows mean not logged or unavailable, not zero server cost",
  },
  {
    id: "snapshot_perf_rows",
    label: "snapshot projection rows",
    owner: "server performance tracing",
    resetWindow: "per logged snapshot projection row when tracing includes it",
    privacy: "aggregate timings and counts; no raw snapshot payloads",
    caveat: "unavailable rows cannot contradict snapshot projection cost",
  },
  {
    id: "writer_rows",
    label: "writer send rows",
    owner: "server WebSocket writer tracing",
    resetWindow: "per logged send row when tracing includes it",
    privacy: "aggregate send timing and bytes only",
    caveat: "client outbound counters can hint at backlog, but missing writer rows are not zero send cost",
  },
  {
    id: "pathing_perf_rows",
    label: "pathing diagnostics rows",
    owner: "server movement/pathing coordinator performance tracing",
    resetWindow: "one bounded row per instrumented pathing pass on logged slow or sampled ticks",
    privacy: "aggregate request counts, stable source labels, cache/budget signals, and buckets only; no paths, positions, or unit ids",
    caveat: "available only when the tick itself was logged by perf tracing; missing rows cannot prove pathing was cheap",
  },
  {
    id: "command_lifecycle",
    label: "command lifecycle fields",
    owner: "client command diagnostics plus server room/writer lifecycle counters",
    resetWindow: "client report window",
    privacy: "client sequence aggregates and bounded top exemplars only; no command payloads or unit lists",
    caveat: "server and client clocks are not synchronized, so compare server-owned stages within the server block and client-observed stages as delivery/apply observations",
  },
  {
    id: "client_frame_render",
    label: "client frame/render fields",
    owner: "browser frame profiler report window",
    resetWindow: "client report window",
    privacy: "bounded phase/counter summaries; no raw frame records or traces",
    caveat: "local browser scheduling and hardware still affect these numbers",
  },
  {
    id: "replay_metadata",
    label: "replay metadata",
    owner: "incident artifact directory",
    resetWindow: "whole preserved match",
    privacy: "presence metadata only in this parser package",
    caveat: "the parser does not load replay blobs from normal log input",
  },
  {
    id: "db_summary_metadata",
    label: "DB summary metadata",
    owner: "match-history summary artifact",
    resetWindow: "whole match",
    privacy: "summary fields only when provided beside logs",
    caveat: "normal parser log input does not imply DB summary metadata is present",
  },
];

const FIELD_CATALOG = {
  maxCommandResponseMs: {
    unit: "milliseconds",
    owner: "client command lifecycle aggregate with server lifecycle supplements when present",
    resetWindow: "parser timeline band; max of report-window command response fields",
    privacy: "aggregate sequence timing, no command payload",
    caveat: "bounded top exemplars identify only client sequence, family, stage, and time; they do not include command payloads",
  },
  maxSnapshotGapMs: {
    unit: "milliseconds",
    owner: "browser snapshot cadence aggregate",
    resetWindow: "parser timeline band",
    privacy: "aggregate timing only",
    caveat: "indicates delivery gap at the browser, not the specific network hop that caused it",
  },
  maxRttMs: {
    unit: "milliseconds",
    owner: "browser ping/RTT aggregate",
    resetWindow: "parser timeline band",
    privacy: "aggregate timing only",
    caveat: "RTT spikes do not prove packet loss or retransmits",
  },
  maxServerQueueMs: {
    unit: "milliseconds",
    owner: "server command room queue aggregate when present, otherwise client receipt-to-sim acknowledgement aggregate",
    resetWindow: "parser timeline band",
    privacy: "aggregate sequence timing, no command payload",
    caveat: "server room queue and client receipt-to-ack are distinct stages; older logs may still combine them",
  },
  maxPayloadP95Bucket: {
    unit: "payload byte bucket",
    owner: "client snapshot payload aggregate",
    resetWindow: "parser timeline band",
    privacy: "payload size only, no raw snapshot bytes",
    caveat: "application payload bytes exclude WebSocket/TLS/TCP/IP overhead",
  },
  maxFrameWorkMs: {
    unit: "milliseconds",
    owner: "browser frame profiler aggregate",
    resetWindow: "parser timeline band",
    privacy: "bounded phase aggregate only",
    caveat: "does not include an opt-in Chrome trace or raw frame records",
  },
  slowTickCount: {
    unit: "rows",
    owner: "server performance tracing",
    resetWindow: "parser timeline band",
    privacy: "aggregate server phase timing only",
    caveat: "counts logged slow/sampled rows, not every tick unless tracing mode logs every tick",
  },
  maxPathingRequests: {
    unit: "requests",
    owner: "server movement/pathing coordinator performance tracing",
    resetWindow: "parser timeline band",
    privacy: "aggregate request counts only; no paths, positions, or unit ids",
    caveat: "counts only logged slow/sampled ticks and excludes ticks with perf tracing disabled",
  },
  maxPathingWorstRequestMs: {
    unit: "milliseconds",
    owner: "server movement/pathing coordinator performance tracing",
    resetWindow: "parser timeline band",
    privacy: "worst aggregate request timing bucket source only; no raw path data",
    caveat: "a slow request indicates pathing pressure but does not by itself explain browser delivery or render delay",
  },
};

export function attachAgentDigest(report, rows, config = {}) {
  const timelineBandMs = config.timelineBandMs || DEFAULT_TIMELINE_BAND_MS;
  const digest = {
    schemaVersion: PACKAGE_SCHEMA_VERSION,
    generatedAt: report.generatedAt,
    timelineBandMs,
    fieldCatalog: FIELD_CATALOG,
    sourceManifest: buildSourceManifest(rows),
    coverageMatrix: buildCoverageMatrix(report.matches, rows),
    timelineBands: buildTimelineBands(rows, timelineBandMs),
    topWindows: buildTopWindows(rows, config.warnThreshold || {}),
    matches: report.matches.map((match) => buildMatchDigest(match)),
  };
  digest.summary = buildOverallSummary(digest.matches);
  digest.unknowns = collectUnknowns(report.matches, digest.coverageMatrix);
  report.agentDigest = digest;
  return report;
}

export function appendAgentDigestMarkdown(lines, digest) {
  if (!digest) return;
  lines.push("");
  lines.push("## Agent Digest");
  lines.push("");
  for (const item of digest.summary.primaryDiagnoses) {
    lines.push(`- ${item.match}: ${item.diagnosis}`);
  }
  if (digest.summary.biggestUnknowns.length > 0) {
    lines.push("");
    lines.push("Biggest unknowns:");
    for (const unknown of digest.summary.biggestUnknowns.slice(0, 5)) {
      lines.push(`- ${unknown}`);
    }
  }
  lines.push("");
  lines.push("What this does and does not prove:");
  for (const note of digest.summary.proofNotes) {
    lines.push(`- ${note}`);
  }
}

export function formatPackageReadme(report, files) {
  const digest = report.agentDigest;
  const lines = ["# Incident Package", "", `Generated: ${report.generatedAt}`, ""];
  appendAgentDigestMarkdown(lines, digest);
  lines.push("");
  lines.push("## Package Contents");
  for (const [key, file] of Object.entries(files)) {
    lines.push(`- ${path.basename(file)}: ${packageFileDescription(key)}`);
  }
  lines.push("");
  lines.push("## Coverage Matrix");
  for (const match of digest.coverageMatrix.matches) {
    lines.push(`- ${match.match}: ${match.items.map((item) => `${item.id}=${item.present ? "present" : "not logged or unavailable"}`).join(", ")}`);
  }
  lines.push("");
  lines.push("## Top Bad Windows");
  for (const group of digest.topWindows.groups) {
    const first = group.windows[0];
    if (!first) {
      lines.push(`- ${group.id}: no threshold-crossing windows`);
    } else {
      lines.push(`- ${group.id}: worst ${first.timestamp} match ${first.match} ${first.playerId ? `player ${first.playerId} ` : ""}${first.summary}`);
    }
  }
  lines.push("");
  lines.push("## Timeline Bands");
  for (const band of digest.timelineBands.filter((item) => item.issueCount > 0).slice(0, 20)) {
    lines.push(
      `- ${band.startAt}: cmd ${fmt(band.maxCommandResponseMs)}ms, gap ${fmt(band.maxSnapshotGapMs)}ms, RTT ${fmt(band.maxRttMs)}ms, payload ${band.maxPayloadP95Bucket}, slow ticks ${band.slowTickCount}, pathing req ${fmt(band.maxPathingRequests)}`,
    );
  }
  lines.push("");
  lines.push("## Manual Reading Notes");
  lines.push("- Start with `key-metrics.json`, then read the Agent Digest above, then inspect `client-net-rows.tsv` and `server-tick-rows.tsv` only for the named worst windows.");
  lines.push("- Missing snapshot/writer/per-packet rows are not logged or unavailable; do not interpret them as zero cost.");
  return `${lines.join("\n")}\n`;
}

export function formatEvidenceIndexJson(report, files) {
  return `${JSON.stringify(
    {
      schemaVersion: PACKAGE_SCHEMA_VERSION,
      generatedAt: report.generatedAt,
      outputFiles: Object.fromEntries(Object.entries(files).map(([key, file]) => [key, path.basename(file)])),
      sourceManifest: report.agentDigest.sourceManifest,
      coverageMatrix: report.agentDigest.coverageMatrix,
      fieldCatalog: FIELD_CATALOG,
      provenance: {
        parser: "scripts/parse-net-report-logs.mjs",
        privacyBoundary:
          "Package files contain bounded aggregate diagnostics and filtered metric rows, not raw command payloads, raw snapshots, entity ids, target ids, player text, stack traces, secrets, or browser-local traces.",
      },
    },
    null,
    2,
  )}\n`;
}

export function formatKeyMetricsJson(report) {
  return `${JSON.stringify(
    {
      schemaVersion: PACKAGE_SCHEMA_VERSION,
      generatedAt: report.generatedAt,
      input: report.input,
      summary: report.agentDigest.summary,
      matches: report.agentDigest.matches,
      topWindows: report.agentDigest.topWindows,
      timelineBands: report.agentDigest.timelineBands,
      unknowns: report.agentDigest.unknowns,
    },
    null,
    2,
  )}\n`;
}

export function formatClientRowsTsv(rows) {
  return formatRowsTsv(
    rows.filter((row) => row.event === "client_net_report"),
    CLIENT_ROW_FIELDS,
  );
}

export function formatServerTickRowsTsv(rows) {
  return formatRowsTsv(
    rows.filter((row) => row.event === "performance_tick"),
    SERVER_TICK_ROW_FIELDS,
  );
}

function buildMatchDigest(match) {
  const classifications = match.classifications.map((item) => ({
    id: item.id,
    label: item.label,
    status: item.status || (item.result === "indicated" ? "indicated" : "unknown"),
    evidenceFor: item.evidenceFor || item.evidence || [],
    evidenceAgainst: item.evidenceAgainst || [],
    unavailable: item.unavailable || [],
  }));
  return {
    match: `Match ${match.match}`,
    matchId: match.match,
    matchRunId: match.matchRunId,
    buildIds: match.buildIds || [],
    roomNames: match.rooms?.length ? match.rooms : match.room ? [match.room] : [],
    participants: match.participants || [],
    utcWindow: {
      start: match.startedAt || firstDefined(match.players.map((player) => player.firstReportAt)),
      end: match.endedAt || firstDefined(match.players.map((player) => player.lastReportAt).reverse()),
    },
    diagnosis: diagnoseMatch(classifications),
    classifications,
    biggestUnknowns: matchUnknowns(match, classifications),
    keyMetrics: {
      reportRows: match.reportRows,
      serverTickRows: match.serverTickRows,
      pathingRows: match.pathingRows,
      snapshotRows: match.snapshotRows,
      writerRows: match.writerRows,
      pathing: match.pathingDiagnostics?.interpretation,
      playerMetrics: match.players.map((player) => ({
        playerId: player.playerId,
        reports: player.reportCount,
        primaryIssues: player.primaryIssues,
        rttMaxMs: metric(player, "rtt_max_ms", "max"),
        snapshotGapMaxMs: metric(player, "snapshot_gap_max_ms", "max"),
        commandResponseMaxMs:
          metric(player, "command_issue_to_sim_ack_max_ms", "max") ?? metric(player, "acknowledged_command_latency_ms", "max"),
        commandResponseP95Ms:
          metric(player, "command_issue_to_sim_ack_p95_ms", "max") ?? metric(player, "acknowledged_command_latency_ms", "p95"),
        clientSendMaxMs: metric(player, "command_issue_to_socket_send_accepted_max_ms", "max"),
        serverQueueMaxMs:
          metric(player, "server_command_room_queue_max_ms", "max") ??
          metric(player, "command_server_receipt_to_sim_ack_max_ms", "max"),
        serverReceiptSendAgeMaxMs: metric(player, "server_command_receipt_send_age_max_ms", "max"),
        serverAcceptedToSimAckMaxMs: metric(player, "server_command_accepted_to_sim_ack_max_ms", "max"),
        frameWorkMaxMs: metric(player, "frame_work_max_ms", "max"),
        rendererMaxMs: metric(player, "renderer_max_ms", "max"),
        payloadP95MaxBytes: metric(player, "snapshot_bytes_p95", "max"),
        commandBurstMax: metric(player, "command_burst_max", "max"),
      })),
    },
  };
}

function buildOverallSummary(matches) {
  const primaryDiagnoses = matches.map((match) => ({
    match: match.match,
    diagnosis: match.diagnosis,
  }));
  const biggestUnknowns = [...new Set(matches.flatMap((match) => match.biggestUnknowns))];
  const proofNotes = [
    "Supported diagnoses are thresholded correlations from existing logs, not fixes or causal proof by themselves.",
    "Packet loss, retransmits, per-packet browser transport behavior, and raw snapshot contents are unavailable in normal preserved logs.",
    "Missing snapshot/writer rows mean not logged or unavailable, not zero projection or send cost.",
  ];
  return { primaryDiagnoses, biggestUnknowns, proofNotes };
}

function diagnoseMatch(classifications) {
  const indicated = classifications.filter((item) => item.status === "indicated").map((item) => item.label);
  if (indicated.length === 0) {
    return "No pressure class crossed current thresholds; rely on unknowns and raw rows before assigning blame.";
  }
  const hasServer = indicated.some((label) => label.includes("server tick"));
  const hasNetwork = indicated.some((label) => label.includes("client network") || label.includes("command upload"));
  if (hasServer && hasNetwork) {
    return `Mixed server-side and client/network pressure indicated: ${indicated.join("; ")}.`;
  }
  return `Supported pressure indicated: ${indicated.join("; ")}.`;
}

function matchUnknowns(match, classifications) {
  const unknowns = [];
  for (const item of classifications) {
    if (item.status === "unavailable" || item.status === "unknown") {
      unknowns.push(`${item.label}: not logged or unavailable`);
    }
  }
  for (const missing of match.missing || []) {
    unknowns.push(missing.replace(": no matching fields in input", ": not logged or unavailable"));
  }
  if (match.snapshotRows === 0) {
    unknowns.push("snapshot projection detail: not logged or unavailable");
  }
  if (match.writerRows === 0) {
    unknowns.push("writer send detail: not logged or unavailable");
  }
  if (!match.pathingRows) {
    unknowns.push("pathing diagnostics detail: not logged or unavailable");
  }
  return [...new Set(unknowns)].slice(0, 10);
}

function collectUnknowns(matches, coverageMatrix) {
  const unknowns = [];
  for (const match of matches) {
    for (const text of matchUnknowns(match, match.classifications || [])) {
      unknowns.push({ match: match.match, text });
    }
  }
  for (const match of coverageMatrix.matches) {
    for (const item of match.items.filter((entry) => !entry.present)) {
      unknowns.push({ match: match.match, text: `${item.label}: not logged or unavailable` });
    }
  }
  const seen = new Set();
  return unknowns.filter((item) => {
    const key = `${item.match}:${item.text}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function buildSourceManifest(rows) {
  const sources = new Map();
  for (const row of rows) {
    const source = sources.get(row.source) || {
      path: row.source,
      evidenceKind: inferEvidenceKind(row.source),
      rowCount: 0,
      eventCounts: {},
      matchIds: new Set(),
      matchRunIds: new Set(),
      buildIds: new Set(),
      roomNames: new Set(),
      participants: new Set(),
      utcStart: "",
      utcEnd: "",
      evidenceClassesPresent: new Set(),
    };
    source.rowCount += 1;
    source.eventCounts[row.event] = (source.eventCounts[row.event] || 0) + 1;
    if (/^\d+$/.test(row.sourceMatch || "")) source.matchIds.add(row.sourceMatch);
    addSet(source.matchRunIds, row.fields.match_run_id);
    addSet(source.buildIds, row.fields.build_id);
    addSet(source.roomNames, row.fields.room);
    for (const participant of Array.isArray(row.fields.participants) ? row.fields.participants : []) {
      source.participants.add(String(participant));
    }
    source.evidenceClassesPresent.add(evidenceClassForRow(row));
    updateWindow(source, row.timestamp);
    sources.set(row.source, source);
  }
  return {
    sources: [...sources.values()].map((source) => ({
      ...source,
      matchIds: [...source.matchIds].sort(),
      matchRunIds: [...source.matchRunIds].sort(),
      buildIds: [...source.buildIds].sort(),
      roomNames: [...source.roomNames].sort(),
      participants: [...source.participants].sort(),
      evidenceClassesPresent: [...source.evidenceClassesPresent].sort(),
    })),
  };
}

function buildCoverageMatrix(matches, rows) {
  return {
    matches: matches.map((match) => {
      const matchRows = rowsForMatch(rows, match);
      return {
        match: match.match,
        matchRunId: match.matchRunId,
        items: COVERAGE_CLASSES.map((coverage) => coverageItem(coverage, match, matchRows)),
        missingFields: match.missing || [],
      };
    }),
  };
}

function coverageItem(coverage, match, rows) {
  const checks = {
    client_reports: rows.filter((row) => row.event === "client_net_report").length,
    server_tick_rows: rows.filter((row) => row.event === "performance_tick").length,
    snapshot_perf_rows: rows.filter((row) => row.event === "performance_snapshot").length,
    writer_rows: rows.filter((row) => row.event === "performance_writer").length,
    pathing_perf_rows: rows.filter((row) => row.event === "performance_pathing").length,
    command_lifecycle: rows.filter((row) =>
      row.fields.command_issue_to_server_receipt_max_ms !== undefined ||
      row.fields.server_command_room_queue_max_ms !== undefined
    ).length,
    client_frame_render: rows.filter((row) => row.fields.frame_work_max_ms !== undefined || row.fields.renderer_max_ms !== undefined).length,
    replay_metadata: hasArtifactNear(rows, "replay"),
    db_summary_metadata: hasArtifactNear(rows, "db-summary"),
  };
  const rowsAvailable = checks[coverage.id] || 0;
  return {
    id: coverage.id,
    label: coverage.label,
    present: rowsAvailable > 0,
    rows: rowsAvailable,
    owner: coverage.owner,
    resetWindow: coverage.resetWindow,
    privacy: coverage.privacy,
    caveat: coverage.caveat,
    matchReportedRows: match.reportRows,
  };
}

function buildTimelineBands(rows, bandMs) {
  const bands = new Map();
  for (const row of rows) {
    const ms = timestampMs(row.timestamp);
    if (ms === null) continue;
    const startMs = Math.floor(ms / bandMs) * bandMs;
    const band = bands.get(startMs) || newTimelineBand(startMs, bandMs);
    band.rowCount += 1;
    band.matches.add(rowMatchLabel(row));
    addSet(band.matchRunIds, row.fields.match_run_id);
    if (row.fields.player_id !== undefined) band.playerIds.add(String(row.fields.player_id));
    if (row.fields.primary_issue) countMap(band.primaryIssues, row.fields.primary_issue);
    if (row.event === "client_net_report") updateClientBand(band, row.fields);
    if (row.event === "performance_tick") updateTickBand(band, row.fields);
    if (row.event === "performance_pathing") updatePathingBand(band, row.fields);
    if (row.event === "performance_snapshot") updateMax(band, "maxSnapshotProjectionMs", row.fields.snapshot_ms ?? row.fields.total_ms);
    if (row.event === "performance_writer") {
      updateMax(band, "maxWriterSendMs", row.fields.send_ms);
      updateMax(band, "maxWriterBytes", row.fields.bytes);
    }
    bands.set(startMs, band);
  }
  return [...bands.values()].sort((a, b) => a.startMs - b.startMs).map(finalizeBand);
}

function newTimelineBand(startMs, bandMs) {
  return {
    startMs,
    startAt: new Date(startMs).toISOString(),
    endAt: new Date(startMs + bandMs).toISOString(),
    rowCount: 0,
    clientReportRows: 0,
    slowTickCount: 0,
    issueCount: 0,
    matches: new Set(),
    matchRunIds: new Set(),
    playerIds: new Set(),
    primaryIssues: new Map(),
    slowestPhaseCounts: new Map(),
  };
}

function updateClientBand(band, fields) {
  band.clientReportRows += 1;
  updateMax(band, "maxCommandResponseMs", fields.command_issue_to_sim_ack_max_ms ?? fields.acknowledged_command_latency_ms);
  updateMax(band, "maxCommandClientSendMs", fields.command_issue_to_socket_send_accepted_max_ms);
  updateMax(band, "maxCommandUploadMs", fields.command_issue_to_server_receipt_max_ms);
  updateMax(band, "maxServerQueueMs", fields.server_command_room_queue_max_ms ?? fields.command_server_receipt_to_sim_ack_max_ms);
  updateMax(band, "maxServerReceiptSendAgeMs", fields.server_command_receipt_send_age_max_ms);
  updateMax(band, "maxServerAcceptedToSimAckMs", fields.server_command_accepted_to_sim_ack_max_ms);
  updateMax(band, "maxSnapshotGapMs", fields.snapshot_gap_max_ms);
  updateMax(band, "maxRttMs", fields.rtt_max_ms);
  updateMax(band, "maxPayloadP95Bytes", fields.snapshot_bytes_p95);
  updateMax(band, "maxPayloadOverBudgetPctX100", fields.snapshot_over_segment_budget_pct_x100);
  updateMax(band, "maxFrameWorkMs", fields.frame_work_max_ms);
  updateMax(band, "maxRendererMs", fields.renderer_max_ms);
  updateMax(band, "maxFrameGapMs", fields.frame_gap_max_ms);
  updateMax(band, "maxCommandsIssued", fields.commands_issued);
  updateMax(band, "maxCommandBurst", fields.command_burst_max);
}

function updateTickBand(band, fields) {
  band.slowTickCount += 1;
  updateMax(band, "maxServerTickMs", fields.tick_ms);
  updateMax(band, "maxSchedulerLagMs", fields.scheduler_lag_ms);
  updateMax(band, "maxSlowestPhaseMs", fields.slowest_phase_ms);
  updateMax(band, "maxSnapshotProjectionMs", fields.max_snapshot_ms);
  updateMax(band, "maxPathingRequests", fields.pathing_requests);
  updateMax(band, "maxPathingWorstRequestMs", fields.pathing_worst_request_ms);
  if (fields.slowest_phase) countMap(band.slowestPhaseCounts, fields.slowest_phase);
}

function updatePathingBand(band, fields) {
  updateMax(band, "maxPathingRequests", fields.requests_processed);
  updateMax(band, "maxPathingDeferred", fields.requests_deferred);
  updateMax(band, "maxPathingWorstRequestMs", fields.worst_request_ms);
  updateMax(band, "maxPathingExploredNodes", fields.explored_nodes_max);
  if (fields.pass) countMap(band.slowestPhaseCounts, `pathing:${fields.pass}`);
}

function finalizeBand(band) {
  const out = {
    ...band,
    matches: [...band.matches].sort(),
    matchRunIds: [...band.matchRunIds].sort(),
    playerIds: [...band.playerIds].sort(),
    primaryIssues: topCounts(band.primaryIssues),
    slowestPhaseCounts: topCounts(band.slowestPhaseCounts),
    maxPayloadP95Bucket: payloadBucket(band.maxPayloadP95Bytes),
  };
  out.issueCount = countBandIssues(out);
  delete out.startMs;
  return out;
}

function buildTopWindows(rows, warnThreshold) {
  return {
    limit: TOP_WINDOW_LIMIT,
    groups: TOP_WINDOW_GROUPS.map((group) => ({
      id: group.id,
      label: group.label,
      windows: rows
        .filter((row) => group.events.includes(row.event))
        .map((row) => scoreWindow(row, group, warnThreshold))
        .filter((item) => item.score >= 1)
        .sort((a, b) => b.score - a.score || a.timestamp.localeCompare(b.timestamp))
        .slice(0, TOP_WINDOW_LIMIT),
    })),
  };
}

function scoreWindow(row, group, warnThreshold) {
  let best = { score: 0, field: "", value: null, threshold: null };
  const fieldValues = {};
  for (const field of group.fields) {
    const value = Number(row.fields[field]);
    if (!Number.isFinite(value)) continue;
    fieldValues[field] = value;
    const threshold = field === "fps_estimate" ? 30 : warnThreshold[field] || inferredThreshold(field);
    const score = field === "fps_estimate" ? threshold / Math.max(value, 1) : value / threshold;
    if (score > best.score) best = { score, field, value, threshold };
  }
  return {
    timestamp: row.timestamp,
    match: rowMatchLabel(row),
    matchRunId: row.fields.match_run_id || "",
    playerId: row.fields.player_id !== undefined ? String(row.fields.player_id) : "",
    source: row.source,
    lineNumber: row.lineNumber,
    score: Number(best.score.toFixed(2)),
    dominantField: best.field,
    dominantValue: best.value,
    threshold: best.threshold,
    fields: fieldValues,
    summary: best.field ? `${best.field}=${best.value} threshold=${best.threshold}` : "no numeric field",
  };
}

function formatRowsTsv(rows, fields) {
  const headers = [...fields, "source", "line"].join("\t");
  const lines = [headers];
  for (const row of rows) {
    lines.push([...fields.map((field) => (field === "timestamp" ? row.timestamp : row.fields[field])), row.source, row.lineNumber].map(tsv).join("\t"));
  }
  return `${lines.join("\n")}\n`;
}

function rowsForMatch(rows, match) {
  const sources = new Set(match.sourceMatches || []);
  if (!match.matchRunId) {
    return rows.filter((row) => sources.has(row.sourceMatch));
  }
  const sourceRows = rows.filter((row) => sources.has(row.sourceMatch));
  const sourceRunIds = new Set(sourceRows.map((row) => row.fields.match_run_id).filter(Boolean));
  const canUseSourceFallback = sourceRunIds.size <= 1;
  return rows.filter(
    (row) =>
      row.fields.match_run_id === match.matchRunId ||
      (canUseSourceFallback && row.fields.match_run_id === undefined && sources.has(row.sourceMatch)),
  );
}

function rowMatchLabel(row) {
  if (/^\d+$/.test(row.sourceMatch || "")) {
    return row.sourceMatch;
  }
  return row.fields.match_run_id || row.sourceMatch || "unknown";
}

function evidenceClassForRow(row) {
  if (row.event === "client_net_report") return "client_reports";
  if (row.event === "performance_tick") return "server_tick_rows";
  if (row.event === "performance_pathing") return "pathing_perf_rows";
  if (row.event === "performance_snapshot") return "snapshot_perf_rows";
  if (row.event === "performance_writer") return "writer_rows";
  return row.event;
}

function inferEvidenceKind(source) {
  const text = source.toLowerCase();
  if (text.includes("network-incident-examples") || text.includes("fly-") || text.includes("runid-logs")) return "beta_incident";
  if (text.includes("client-perf") || text.includes("replay")) return "local_replay_perf_harness";
  if (text.includes("stress") || text.includes("synthetic") || text.includes("packet") || text.includes("command")) return "synthetic_stress";
  return "unknown";
}

function packageFileDescription(key) {
  return {
    readme: "agent-first digest and reading guide",
    evidenceIndex: "source manifest, coverage matrix, field catalog, and provenance",
    keyMetrics: "stable JSON digest with classifications, top windows, timeline bands, and unknowns",
    markdown: "backwards-compatible parser markdown summary with digest preface",
    json: "backwards-compatible parser JSON summary plus `agentDigest`",
    tsv: "backwards-compatible per-player aggregate rows",
    clientRows: "filtered client report windows for sorting and spot checks",
    serverTickRows: "filtered server slow-tick rows for pathing/scheduler spot checks",
  }[key] || "package output";
}

function countBandIssues(band) {
  return [
    band.maxCommandResponseMs >= 180,
    band.maxSnapshotGapMs >= 100,
    band.maxRttMs >= 180,
    band.maxServerQueueMs >= 66,
    band.maxPayloadOverBudgetPctX100 >= 5000,
    band.maxFrameWorkMs >= 33,
    band.maxRendererMs >= 33,
    band.maxServerTickMs >= 40,
    band.maxPathingRequests >= 64,
    band.maxPathingWorstRequestMs >= 8,
  ].filter(Boolean).length;
}

function metric(player, field, key) {
  return player.metrics[field]?.[key] ?? null;
}

function updateMax(target, key, value) {
  const number = Number(value);
  if (Number.isFinite(number)) target[key] = Math.max(target[key] ?? Number.NEGATIVE_INFINITY, number);
}

function updateWindow(source, timestamp) {
  if (!timestamp) return;
  if (!source.utcStart || timestamp < source.utcStart) source.utcStart = timestamp;
  if (!source.utcEnd || timestamp > source.utcEnd) source.utcEnd = timestamp;
}

function addSet(set, value) {
  if (value !== undefined && value !== null && value !== "") set.add(String(value));
}

function countMap(map, key) {
  map.set(String(key), (map.get(String(key)) || 0) + 1);
}

function topCounts(map) {
  return [...map.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([value, count]) => ({ value, count }));
}

function payloadBucket(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) return "not logged or unavailable";
  for (const limit of [1280, 1536, 2048, 4096, 8192, 16384, 32768, 65536, 131072, 262144]) {
    if (number <= limit) return `<=${limit}`;
  }
  return ">262144";
}

function inferredThreshold(field) {
  if (field.includes("pathing_requests") || field === "requests_processed") return 64;
  if (field.includes("explored_nodes")) return 4096;
  if (field.includes("pathing_worst_request") || field === "worst_request_ms") return 8;
  if (field.includes("pathing_deferred") || field === "requests_deferred") return 1;
  if (field.includes("bytes")) return 1280;
  if (field.includes("count") || field.includes("accepted")) return 1;
  if (field.includes("tick") || field.includes("phase") || field.includes("_ms")) return 33;
  return 1;
}

function firstDefined(values) {
  return values.find((value) => value !== undefined && value !== null && value !== "") || "";
}

function timestampMs(value) {
  const ms = Date.parse(value || "");
  return Number.isFinite(ms) ? ms : null;
}

function hasArtifactNear(rows, needle) {
  return rows.some((row) => row.source.toLowerCase().includes(needle)) ? 1 : 0;
}

function fmt(value) {
  return value === undefined || value === null || value === Number.NEGATIVE_INFINITY ? "n/a" : String(value);
}

function tsv(value) {
  return String(value ?? "").replace(/\t/g, " ").replace(/\r?\n/g, " ");
}
