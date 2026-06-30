#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const script = path.join(repoRoot, "scripts", "parse-net-report-logs.mjs");
const logs = [
  path.join(
    repoRoot,
    "docs",
    "network-incident-examples",
    "2026-06-19-beta-matt-alex",
    "fly-match-54-all.jsonl"
  ),
  path.join(
    repoRoot,
    "docs",
    "network-incident-examples",
    "2026-06-19-beta-matt-alex",
    "fly-match-55-all.jsonl"
  ),
];
const soupmanLog = path.join(
  repoRoot,
  "docs",
  "network-incident-examples",
  "2026-06-30-beta-soupman-alex-lag",
  "match-103-runid-logs.jsonl"
);

function run(args) {
  return execFileSync("node", [script, ...args], {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

const help = run(["--help"]);
assert.match(help, /client_net_report/);
assert.match(help, /performance writer timing/);

const parsed = JSON.parse(run(["--format", "json", ...logs]));
assert.equal(parsed.warnings.length, 0);
assert.equal(parsed.matches.length, 2);

const match54 = parsed.matches.find((match) => match.match === "54");
assert.ok(match54, "expected match 54 summary");
assert.equal(match54.matchRunId, "main-1781830463862-000004");
assert.deepEqual(match54.participants, ["alex", "<b>matt</b>"]);
assert.equal(match54.reportRows, 35);
assert.equal(match54.serverTickRows, 0);

const player5 = match54.players.find((player) => player.playerId === "5");
assert.ok(player5, "expected Matt/player 5 summary");
assert.equal(player5.metrics.rtt_max_ms.max, 759);
assert.equal(player5.metrics.snapshot_gap_max_ms.max, 1077);
assert.equal(player5.metrics.frame_gap_max_ms.max, 700);
assert.equal(player5.metrics.fps_estimate.min, 15);
assert.equal(player5.metrics.snapshot_bytes_max, null);
assert.equal(player5.metrics.snapshot_bytes_p95, null);

const serverClassification = match54.classifications.find(
  (item) => item.id === "server_tick_scheduler"
);
const networkClassification = match54.classifications.find(
  (item) => item.id === "client_network_delivery"
);
const browserClassification = match54.classifications.find((item) => item.id === "browser_processing");
assert.equal(serverClassification.result, "not indicated");
assert.equal(serverClassification.status, "contradicted");
assert.ok(
  serverClassification.evidenceAgainst.some((item) => item.includes("server_tick_ms")),
  "expected clean server tick fields to be evidence against server pressure"
);
assert.equal(networkClassification.result, "indicated");
assert.equal(networkClassification.status, "indicated");
assert.equal(browserClassification.result, "indicated");
assert.equal(
  match54.classifications.find((item) => item.id === "server_snapshot_projection")?.status,
  "unavailable",
);
assert.match(match54.transportNote, /Unsupported/);
assert.ok(
  match54.missing.some((item) => item.includes("server snapshot projection")),
  "expected unavailable newer server snapshot perf data to be reported as missing"
);
assert.ok(
  match54.missing.some((item) => item.includes("packet-budget")),
  "expected unavailable packet-budget fields to be reported as missing"
);

const markdown = run([...logs]);
assert.match(markdown, /## Match 54/);
assert.match(markdown, /## Agent Digest/);
assert.match(markdown, /player 5 frame_gap_max_ms max 700/);
assert.match(markdown, /packet loss, retransmits, or per-packet browser transport data/);
assert.match(markdown, /payload p95/);

const packetDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-packet-"));
try {
  const packetLog = path.join(packetDir, "packet.log");
  writeFileSync(
    packetLog,
    [
      '2026-06-19T02:00:00Z INFO event="client_net_report" match_run_id="packet-1" player_id=2 primary_issue="packet_budget_pressure" rtt_max_ms=40 snapshot_gap_max_ms=33 snapshot_jitter_ms=0 snapshot_bytes_max=4096 snapshot_byte_source="messagepack-application-payload" snapshot_codec="messagepack-compact" snapshot_codec_version=1 snapshot_frame_kind="binary" snapshot_bytes_p95=2048 snapshot_bytes_avg=1800 snapshot_segment_budget_bytes=1280 snapshot_over_segment_budget_count=180 snapshot_over_segment_budget_pct_x100=6000 websocket_extensions="" websocket_compression="none" frame_gap_max_ms=16 fps_estimate=60 server_tick_ms=4 server_lag_ms=0 "client network report"',
    ].join("\n") + "\n"
  );
  const packetParsed = JSON.parse(run(["--format", "json", packetLog]));
  const packetMatch = packetParsed.matches.find((match) => match.matchRunId === "packet-1");
  assert.ok(packetMatch, "expected synthetic packet-budget match summary");
  const packetPlayer = packetMatch.players.find((player) => player.playerId === "2");
  assert.ok(packetPlayer, "expected synthetic packet-budget player summary");
  assert.equal(packetPlayer.metrics.snapshot_bytes_p95.max, 2048);
  assert.equal(packetPlayer.metrics.snapshot_segment_budget_bytes.max, 1280);
  assert.equal(packetPlayer.metrics.snapshot_over_segment_budget_pct_x100.max, 6000);
  assert.equal(packetPlayer.transport.websocketCompression.values[0].value, "none");
  assert.equal(packetMatch.transport.snapshotByteSource.values[0].value, "messagepack-application-payload");
  assert.equal(packetMatch.transport.snapshotCodec.values[0].value, "messagepack-compact");
  assert.equal(packetMatch.transport.snapshotCodecVersion.values[0].value, "1");
  assert.equal(packetMatch.transport.snapshotFrameKind.values[0].value, "binary");
  assert.equal(packetMatch.missing.some((item) => item.includes("packet-budget")), false);
  assert.equal(packetMatch.missing.some((item) => item.includes("compression negotiation")), false);
  assert.equal(packetMatch.missing.some((item) => item.includes("codec/frame")), false);
  const packetMarkdown = run([packetLog]);
  assert.match(packetMarkdown, /payload p95/);
  assert.match(packetMarkdown, /60%/);
  assert.match(packetMarkdown, /Transport diagnostics:/);
  assert.match(packetMarkdown, /WebSocket compression none=1/);
} finally {
  rmSync(packetDir, { recursive: true, force: true });
}

const tsv = run(["--format=tsv", ...logs]);
assert.match(tsv, /^match\tplayer_id\treports/m);
assert.match(tsv, /^54\t5\t17\tprediction_disabled:17/m);

const commandDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-command-"));
try {
  const commandLog = path.join(commandDir, "command.log");
  writeFileSync(
    commandLog,
    [
      '2026-06-24T02:00:00Z INFO event="client_net_report" match_run_id="command-1" player_id=4 primary_issue="command_density" rtt_max_ms=42 snapshot_gap_max_ms=144 snapshot_jitter_ms=28 snapshot_late_frame_count=3 predicted_snapshot_late_frame_count=2 frame_gap_max_ms=118 frame_work_max_ms=12 fps_estimate=55 commands_issued=24 command_burst_bucket_ms=250 command_burst_max=9 command_burst_frame_gap_max_ms=118 command_burst_worst_frame_phase="match.input" command_burst_worst_frame_phase_ms=14 command_issue_to_sim_ack_max_ms=81 command_rejected=0 prediction_disable_user_count=0 prediction_disable_replay_count=1 prediction_disable_spectator_count=0 prediction_disable_compatibility_count=0 prediction_disable_wasm_count=0 prediction_disable_other_count=0 prediction_replay_max_ms=9 prediction_replay_max_ticks=10 prediction_replay_budget_exceeded_count=1 server_command_receipts_accepted=24 server_command_receipts_rejected=0 server_reliable_drained_before_snapshot=3 server_reliable_drained_before_snapshot_max=2 server_snapshot_waited_behind_reliable=1 server_snapshot_sent=50 server_snapshot_send_age_latest_ms=18 server_snapshot_send_age_max_ms=132 server_snapshot_send_age_avg_ms=12 server_snapshot_slot_stored=50 server_snapshot_slot_replaced=2 server_snapshot_slot_closed=0 server_tick_ms=4 server_lag_ms=1 "client network report"',
    ].join("\n") + "\n"
  );
  const commandParsed = JSON.parse(run(["--format", "json", commandLog]));
  const commandMatch = commandParsed.matches.find((match) => match.matchRunId === "command-1");
  assert.ok(commandMatch, "expected synthetic command-density match summary");
  const commandPlayer = commandMatch.players.find((player) => player.playerId === "4");
  assert.ok(commandPlayer, "expected synthetic command-density player summary");
  assert.equal(commandPlayer.metrics.command_burst_max.max, 9);
  assert.equal(commandPlayer.metrics.server_snapshot_send_age_max_ms.max, 132);
  assert.equal(commandPlayer.metrics.predicted_snapshot_late_frame_count.max, 2);
  assert.equal(
    commandMatch.classifications.find((item) => item.id === "command_density")?.result,
    "indicated",
  );
  assert.equal(
    commandMatch.classifications.find((item) => item.id === "websocket_writer_send")?.result,
    "indicated",
  );
  const commandMarkdown = run([commandLog]);
  assert.match(commandMarkdown, /cmds\/burst/);
  assert.match(commandMarkdown, /24\/9/);
  assert.match(commandMarkdown, /3\/1\/132\/2/);
} finally {
  rmSync(commandDir, { recursive: true, force: true });
}

const sustainedCommandDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-sustained-command-"));
try {
  const sustainedCommandLog = path.join(sustainedCommandDir, "sustained-command.log");
  writeFileSync(
    sustainedCommandLog,
    [
      '2026-06-24T10:48:10Z INFO event="client_net_report" match_run_id="sustained-command-1" player_id=1 primary_issue="command_density" rtt_max_ms=76 snapshot_gap_max_ms=59 snapshot_jitter_ms=18 frame_gap_max_ms=64 fps_estimate=60 commands_issued=42 command_burst_bucket_ms=250 command_burst_max=2 command_burst_frame_gap_max_ms=64 command_rejected=0 server_command_receipts_accepted=42 server_command_receipts_rejected=0 server_reliable_drained_before_snapshot=615 server_reliable_drained_before_snapshot_max=1 server_snapshot_waited_behind_reliable=615 server_snapshot_sent=615 server_snapshot_send_age_latest_ms=0 server_snapshot_send_age_max_ms=0 server_snapshot_send_age_avg_ms=0 server_snapshot_slot_stored=615 server_snapshot_slot_replaced=0 server_snapshot_slot_closed=0 server_tick_ms=3 server_lag_ms=0 "client network report"',
    ].join("\n") + "\n"
  );
  const sustainedParsed = JSON.parse(run(["--format", "json", sustainedCommandLog]));
  const sustainedMatch = sustainedParsed.matches.find((match) => match.matchRunId === "sustained-command-1");
  assert.ok(sustainedMatch, "expected sustained command-density match summary");
  assert.equal(
    sustainedMatch.classifications.find((item) => item.id === "command_density")?.result,
    "indicated",
  );
  assert.equal(
    sustainedMatch.classifications.find((item) => item.id === "websocket_writer_send")?.result,
    "not indicated",
  );
} finally {
  rmSync(sustainedCommandDir, { recursive: true, force: true });
}

const outDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-"));
try {
  const out = run(["--out-dir", outDir, ...logs]);
  assert.match(out, /incident-summary\.md/);
  assert.ok(existsSync(path.join(outDir, "incident-summary.md")));
  assert.ok(existsSync(path.join(outDir, "incident-summary.json")));
  assert.ok(existsSync(path.join(outDir, "incident-rows.tsv")));
  assert.ok(existsSync(path.join(outDir, "README.md")));
  assert.ok(existsSync(path.join(outDir, "evidence-index.json")));
  assert.ok(existsSync(path.join(outDir, "key-metrics.json")));
  assert.ok(existsSync(path.join(outDir, "client-net-rows.tsv")));
  assert.ok(existsSync(path.join(outDir, "server-tick-rows.tsv")));
  assert.match(readFileSync(path.join(outDir, "incident-summary.md"), "utf8"), /## Match 55/);
  assert.match(readFileSync(path.join(outDir, "README.md"), "utf8"), /Agent Digest/);
  const evidenceIndex = JSON.parse(readFileSync(path.join(outDir, "evidence-index.json"), "utf8"));
  assert.equal(evidenceIndex.schemaVersion, 1);
  assert.equal(evidenceIndex.sourceManifest.sources.length, 2);
  assert.ok(
    evidenceIndex.coverageMatrix.matches.some((match) =>
      match.items.some((item) => item.id === "client_reports" && item.present)
    ),
  );
} finally {
  rmSync(outDir, { recursive: true, force: true });
}

const soupmanParsed = JSON.parse(run(["--format", "json", soupmanLog]));
const soupmanDigest = soupmanParsed.agentDigest;
assert.ok(soupmanDigest, "expected agent digest in JSON output");
assert.match(soupmanDigest.summary.primaryDiagnoses[0].diagnosis, /Mixed server-side and client\/network/);
for (const minute of ["00:21", "00:22", "00:23", "00:24", "00:25", "00:26", "00:27", "00:28"]) {
  const band = soupmanDigest.timelineBands.find((item) => item.startAt.startsWith(`2026-06-30T${minute}`));
  assert.ok(band, `expected Soupman/Alex timeline band for ${minute}Z`);
  assert.ok(band.maxCommandResponseMs >= 498, `expected command pressure in ${minute}Z band`);
  assert.ok(band.maxSnapshotGapMs >= 551, `expected snapshot gap pressure in ${minute}Z band`);
}
const soupmanServerTop = soupmanDigest.topWindows.groups.find((group) => group.id === "server_tick")?.windows[0];
assert.ok(soupmanServerTop, "expected server tick top window");
assert.match(soupmanServerTop.timestamp, /2026-06-30T00:40:/);
assert.equal(soupmanServerTop.fields.slowest_phase_ms, 297);
assert.ok(
  soupmanDigest.unknowns.some((item) => item.text.includes("writer send detail: not logged or unavailable")),
  "expected missing writer rows to be explicit unknowns"
);

console.log("net report log parser test passed");
