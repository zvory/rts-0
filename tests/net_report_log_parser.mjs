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

const frameBudgetDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-frame-budget-"));
try {
  const frameBudgetLog = path.join(frameBudgetDir, "frame-budget.log");
  writeFileSync(
    frameBudgetLog,
    [
      '2026-07-14T02:00:00Z INFO event="client_net_report" match_run_id="frame-budget-1" player_id=1 primary_issue="client_renderer_present" frame_gap_max_ms=22 frame_work_max_ms=21 frame_work_p95_ms=17 frame_work_budget_miss_count=9 present_budget_miss_count=4 renderer_max_ms=20 renderer_p95_ms=17 renderer_update_max_ms=13 renderer_update_p95_ms=8 renderer_present_max_ms=18 renderer_present_p95_ms=17 frame_raf_dispatch_max_ms=3 frame_raf_dispatch_p95_ms=2 frame_unattributed_max_ms=2 frame_unattributed_p95_ms=1 fps_estimate=58 server_tick_ms=4 server_lag_ms=0 "client network report"',
    ].join("\n") + "\n"
  );
  const frameBudgetParsed = JSON.parse(run(["--format", "json", frameBudgetLog]));
  const frameBudgetMatch = frameBudgetParsed.matches.find((match) => match.matchRunId === "frame-budget-1");
  assert.ok(frameBudgetMatch, "expected synthetic frame-budget match summary");
  const frameBudgetPlayer = frameBudgetMatch.players.find((player) => player.playerId === "1");
  assert.equal(frameBudgetPlayer.metrics.frame_work_budget_miss_count.max, 9);
  assert.equal(frameBudgetPlayer.metrics.present_budget_miss_count.max, 4);
  assert.equal(frameBudgetPlayer.metrics.renderer_update_p95_ms.max, 8);
  assert.equal(frameBudgetPlayer.metrics.renderer_present_p95_ms.max, 17);
  const frameBudgetMarkdown = run([frameBudgetLog]);
  assert.match(frameBudgetMarkdown, /60 FPS work-budget misses/);
  assert.match(frameBudgetMarkdown, /renderer present max\/p95/);
} finally {
  rmSync(frameBudgetDir, { recursive: true, force: true });
}

const snapshotPayloadDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-snapshot-payload-"));
try {
  const snapshotPayloadLog = path.join(snapshotPayloadDir, "snapshot-payload.log");
  const sectionsJson = JSON.stringify([
    { section: "entities", count: 800, bytes: 62000, pctX100: 5962 },
    { section: "visibility", count: 1200, bytes: 28000, pctX100: 2692 },
    { section: "netStatus", count: 26, bytes: 1800, pctX100: 173 },
  ]).replace(/"/g, '\\"');
  const kindsJson = JSON.stringify([
    { kind: "worker", count: 420, approxBytes: 34000, pctX100: 3269 },
    { kind: "rifleman", count: 260, approxBytes: 21000, pctX100: 2019 },
  ]).replace(/"/g, '\\"');
  writeFileSync(
    snapshotPayloadLog,
    [
      `2026-06-24T03:00:00Z INFO event="client_net_report" match_run_id="snapshot-payload-1" player_id=3 primary_issue="server_snapshot_lifecycle" rtt_max_ms=38 snapshot_gap_max_ms=44 snapshot_jitter_ms=2 snapshot_bytes_max=8192 snapshot_bytes_p95=4096 snapshot_bytes_avg=4000 snapshot_segment_budget_bytes=1280 snapshot_over_segment_budget_count=24 snapshot_over_segment_budget_pct_x100=9200 snapshot_byte_source="messagepack-application-payload" snapshot_codec="messagepack-compact" snapshot_codec_version=1 snapshot_frame_kind="binary" websocket_compression="none" frame_gap_max_ms=14 fps_estimate=60 server_tick_ms=5 server_lag_ms=0 server_snapshot_project_max_ms=12 server_snapshot_project_p95_ms=8 server_snapshot_compact_max_ms=7 server_snapshot_compact_p95_ms=4 server_snapshot_queue_age_max_ms=18 server_snapshot_queue_age_p95_ms=12 server_snapshot_serialize_max_ms=14 server_snapshot_serialize_p95_ms=10 server_snapshot_writer_send_max_ms=6 server_snapshot_writer_send_p95_ms=4 server_snapshot_payload_bytes_max=8192 server_snapshot_payload_bytes_p95=4096 server_snapshot_payload_bytes_avg=4000 server_snapshot_payload_bytes_total=104000 server_snapshot_payload_bytes_count=26 server_snapshot_writer_taken=26 server_snapshot_payload_sections="${sectionsJson}" server_snapshot_entity_kinds="${kindsJson}" server_reliable_drained_before_snapshot=0 server_reliable_drained_before_snapshot_max=0 server_snapshot_waited_behind_reliable=0 server_snapshot_sent=26 server_snapshot_send_age_max_ms=18 server_snapshot_slot_stored=26 server_snapshot_slot_replaced=0 server_snapshot_slot_closed=0 "client network report"`,
    ].join("\n") + "\n"
  );
  const snapshotPayloadParsed = JSON.parse(run(["--format", "json", snapshotPayloadLog]));
  const snapshotPayloadMatch = snapshotPayloadParsed.matches.find((match) => match.matchRunId === "snapshot-payload-1");
  assert.ok(snapshotPayloadMatch, "expected synthetic snapshot payload match summary");
  const snapshotPayloadPlayer = snapshotPayloadMatch.players.find((player) => player.playerId === "3");
  assert.ok(snapshotPayloadPlayer, "expected synthetic snapshot payload player summary");
  assert.equal(snapshotPayloadPlayer.metrics.server_snapshot_project_max_ms.max, 12);
  assert.equal(snapshotPayloadPlayer.metrics.server_snapshot_payload_bytes_p95.max, 4096);
  assert.equal(snapshotPayloadPlayer.snapshotPayload.sections[0].label, "entities");
  assert.equal(snapshotPayloadPlayer.snapshotPayload.sections[0].pctX100, 5962);
  assert.equal(snapshotPayloadPlayer.snapshotPayload.entityKinds[0].label, "worker");
  assert.equal(
    snapshotPayloadMatch.classifications.find((item) => item.id === "server_snapshot_projection")?.result,
    "indicated",
  );
  assert.equal(
    snapshotPayloadMatch.classifications.find((item) => item.id === "snapshot_payload_composition")?.result,
    "indicated",
  );
  const snapshotPayloadMarkdown = run([snapshotPayloadLog]);
  assert.match(snapshotPayloadMarkdown, /Snapshot Payload Composition/);
  assert.match(snapshotPayloadMarkdown, /entities 59\.62% 62000 bytes/);
  assert.match(snapshotPayloadMarkdown, /worker 32\.69% 34000 approx bytes/);
} finally {
  rmSync(snapshotPayloadDir, { recursive: true, force: true });
}

const pathingDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-pathing-"));
try {
  const pathingLog = path.join(pathingDir, "pathing.log");
  writeFileSync(
    pathingLog,
    [
      '2026-06-24T03:30:00Z INFO event="tick" room="beta pathing" match_run_id="pathing-1" tick=123 tick_ms=320 scheduler_lag_ms=0 sim_ms=314 fanout_ms=2 outcome_ms=0 players=2 spectators=0 ai_players=0 entities=900 units=640 buildings=80 resources=180 snapshots=2 snapshot_stored=2 snapshot_replaced=0 snapshot_closed=0 max_snapshot_ms=4 max_snapshot_entities=500 slowest_phase="awaiting_paths" slowest_phase_ms=297 pathing_passes=3 pathing_awaiting_start=96 pathing_promoted_awaiting_start=40 pathing_promote_queued_for_path=40 pathing_requests=64 pathing_processed=64 pathing_deferred=32 pathing_still_awaiting=32 pathing_success=60 pathing_failed=4 pathing_cache_hits=8 pathing_cache_misses=56 pathing_budget_exhausted=1 pathing_worst_request_ms=42 pathing_explored_nodes_max=9001 pathing_path_len_max=220 pathing_top_source="move" pathing_top_source_count=64 "performance tick summary"',
      '2026-06-24T03:30:00Z DEBUG event="pathing" room="beta pathing" match_run_id="pathing-1" tick=123 pass="awaiting_paths" awaiting_start=96 queued_for_path=0 requests_processed=64 requests_deferred=32 still_awaiting=32 path_success=60 path_failed=4 same_tile=0 cache_hits=8 cache_misses=56 path_budget_exhausted=1 coordinator_budget_exhausted=true total_request_ms=310 worst_request_ms=42 worst_request_bucket="34ms+" explored_nodes_max=9001 path_len_max=220 source_counts="move=64" queued_source_counts="none" group_size_buckets="none" path_len_buckets="129+=64" explored_node_buckets="8193+=1,2049-8192=63" cache_available=true complexity_available=true fuse_triggered=false "performance pathing diagnostics"',
      '2026-06-24T03:30:00Z DEBUG event="pathing" room="beta pathing" match_run_id="pathing-1" tick=123 pass="promote_queued_orders" awaiting_start=0 queued_for_path=40 requests_processed=0 requests_deferred=0 still_awaiting=40 path_success=0 path_failed=0 same_tile=0 cache_hits=0 cache_misses=0 path_budget_exhausted=0 coordinator_budget_exhausted=false total_request_ms=0 worst_request_ms=0 worst_request_bucket="0ms" explored_nodes_max=0 path_len_max=0 source_counts="none" queued_source_counts="attackMove=40" group_size_buckets="17-64=1" path_len_buckets="none" explored_node_buckets="none" cache_available=true complexity_available=true fuse_triggered=false "performance pathing diagnostics"',
    ].join("\n") + "\n"
  );
  const pathingParsed = JSON.parse(run(["--format", "json", pathingLog]));
  const pathingMatch = pathingParsed.matches.find((match) => match.matchRunId === "pathing-1");
  assert.ok(pathingMatch, "expected synthetic pathing match summary");
  assert.equal(pathingMatch.pathingRows, 2);
  assert.equal(pathingMatch.pathingDiagnostics.interpretation.primary, "path request volume");
  assert.equal(pathingMatch.pathingDiagnostics.topSources[0].label, "move");
  assert.equal(pathingMatch.pathingDiagnostics.passes[0].pass, "awaiting_paths");
  assert.equal(
    pathingMatch.classifications.find((item) => item.id === "server_pathing")?.result,
    "indicated",
  );
  assert.ok(
    pathingParsed.agentDigest.coverageMatrix.matches
      .find((match) => match.match === "pathing-1")
      ?.items.find((item) => item.id === "pathing_perf_rows")?.present,
    "expected pathing perf coverage to be present",
  );
  assert.equal(
    pathingParsed.agentDigest.topWindows.groups.find((group) => group.id === "server_pathing")?.windows[0]
      ?.match,
    "pathing-1",
  );
  const pathingMarkdown = run([pathingLog]);
  assert.match(pathingMarkdown, /Pathing Slow-Tick Diagnostics/);
  assert.match(pathingMarkdown, /path request volume/);
  assert.match(pathingMarkdown, /awaiting_paths/);

  const lowVolumePathingLog = path.join(pathingDir, "pathing-low-volume.log");
  writeFileSync(
    lowVolumePathingLog,
    [
      '2026-06-24T03:31:00Z DEBUG event="pathing" match_run_id="pathing-low" tick=1 pass="awaiting_paths" awaiting_start=32 queued_for_path=32 requests_processed=32 requests_deferred=0 still_awaiting=0 path_success=32 path_failed=0 same_tile=0 cache_hits=0 cache_misses=32 path_budget_exhausted=0 coordinator_budget_exhausted=false total_request_ms=8 worst_request_ms=1 worst_request_bucket="1-2ms" explored_nodes_max=64 path_len_max=10 source_counts="move=32" queued_source_counts="move=32" group_size_buckets="17-64=1" path_len_buckets="9-32=32" explored_node_buckets="1-512=32" cache_available=true complexity_available=true fuse_triggered=false "performance pathing diagnostics"',
      '2026-06-24T03:31:01Z DEBUG event="pathing" match_run_id="pathing-low" tick=2 pass="awaiting_paths" awaiting_start=32 queued_for_path=32 requests_processed=32 requests_deferred=0 still_awaiting=0 path_success=32 path_failed=0 same_tile=0 cache_hits=0 cache_misses=32 path_budget_exhausted=0 coordinator_budget_exhausted=false total_request_ms=8 worst_request_ms=1 worst_request_bucket="1-2ms" explored_nodes_max=64 path_len_max=10 source_counts="move=32" queued_source_counts="move=32" group_size_buckets="17-64=1" path_len_buckets="9-32=32" explored_node_buckets="1-512=32" cache_available=true complexity_available=true fuse_triggered=false "performance pathing diagnostics"',
      '2026-06-24T03:31:02Z DEBUG event="pathing" match_run_id="pathing-low" tick=3 pass="awaiting_paths" awaiting_start=32 queued_for_path=32 requests_processed=32 requests_deferred=0 still_awaiting=0 path_success=32 path_failed=0 same_tile=0 cache_hits=0 cache_misses=32 path_budget_exhausted=0 coordinator_budget_exhausted=false total_request_ms=8 worst_request_ms=1 worst_request_bucket="1-2ms" explored_nodes_max=64 path_len_max=10 source_counts="move=32" queued_source_counts="move=32" group_size_buckets="17-64=1" path_len_buckets="9-32=32" explored_node_buckets="1-512=32" cache_available=true complexity_available=true fuse_triggered=false "performance pathing diagnostics"',
    ].join("\n") + "\n"
  );
  const lowVolumeParsed = JSON.parse(run(["--format", "json", lowVolumePathingLog]));
  const lowVolumeMatch = lowVolumeParsed.matches.find((match) => match.matchRunId === "pathing-low");
  assert.ok(lowVolumeMatch, "expected low-volume pathing match summary");
  assert.equal(lowVolumeMatch.pathingDiagnostics.totalRequests, 96);
  assert.equal(lowVolumeMatch.pathingDiagnostics.processedMax, 32);
  assert.equal(lowVolumeMatch.pathingDiagnostics.interpretation.primary, "unknown");
} finally {
  rmSync(pathingDir, { recursive: true, force: true });
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
      '2026-06-24T02:00:00Z INFO event="client_net_report" match_run_id="command-1" player_id=4 primary_issue="command_density" rtt_max_ms=42 snapshot_gap_max_ms=144 snapshot_jitter_ms=28 snapshot_late_frame_count=3 predicted_snapshot_late_frame_count=2 frame_gap_max_ms=118 frame_work_max_ms=12 fps_estimate=55 commands_issued=24 command_burst_bucket_ms=250 command_burst_max=9 command_burst_frame_gap_max_ms=118 command_burst_worst_frame_phase="match.input" command_burst_worst_frame_phase_ms=14 command_issue_to_socket_send_accepted_max_ms=7 command_issue_to_socket_send_accepted_p95_ms=4 command_issue_to_server_receipt_max_ms=63 command_server_receipt_to_sim_ack_max_ms=70 command_issue_to_sim_ack_max_ms=81 command_rejected=0 command_family_move=12 command_family_attack_move=3 command_family_build=4 command_family_train=1 command_family_other=4 command_lifecycle_exemplars="[{\\"clientSeq\\":7,\\"family\\":\\"move\\",\\"issuedElapsedMs\\":125,\\"stage\\":\\"issueToSimAck\\",\\"stageMs\\":81}]" prediction_disable_user_count=0 prediction_disable_replay_count=1 prediction_disable_spectator_count=0 prediction_disable_compatibility_count=0 prediction_disable_wasm_count=0 prediction_disable_other_count=0 prediction_replay_max_ms=9 prediction_replay_max_ticks=10 prediction_replay_budget_exceeded_count=1 server_command_receipts_accepted=24 server_command_receipts_rejected=0 server_command_lifecycle_count=24 server_command_lifecycle_accepted=23 server_command_lifecycle_rejected=1 server_command_frame_deserialize_max_ms=3 server_command_frame_deserialize_p95_ms=2 server_command_deserialize_to_room_enqueue_max_ms=5 server_command_deserialize_to_room_enqueue_p95_ms=4 server_command_room_queue_max_ms=74 server_command_room_queue_p95_ms=50 server_command_room_handle_max_ms=6 server_command_room_handle_p95_ms=4 server_command_receipt_send_age_max_ms=132 server_command_receipt_send_age_p95_ms=100 server_command_accepted_to_sim_ack_max_ms=66 server_command_accepted_to_sim_ack_p95_ms=50 server_command_lifecycle_exemplars="[{\\"receivedUnixMs\\":1719194400000,\\"clientSeq\\":7,\\"family\\":\\"move\\",\\"stage\\":\\"serverRoomQueue\\",\\"stageMs\\":74}]" server_reliable_drained_before_snapshot=3 server_reliable_drained_before_snapshot_max=2 server_snapshot_waited_behind_reliable=1 server_snapshot_sent=50 server_snapshot_send_age_latest_ms=18 server_snapshot_send_age_max_ms=132 server_snapshot_send_age_avg_ms=12 server_snapshot_slot_stored=50 server_snapshot_slot_replaced=2 server_snapshot_slot_closed=0 server_tick_ms=4 server_lag_ms=1 "client network report"',
    ].join("\n") + "\n"
  );
  const commandParsed = JSON.parse(run(["--format", "json", commandLog]));
  const commandMatch = commandParsed.matches.find((match) => match.matchRunId === "command-1");
  assert.ok(commandMatch, "expected synthetic command-density match summary");
  const commandPlayer = commandMatch.players.find((player) => player.playerId === "4");
  assert.ok(commandPlayer, "expected synthetic command-density player summary");
  assert.equal(commandPlayer.metrics.command_burst_max.max, 9);
  assert.equal(commandPlayer.metrics.command_issue_to_socket_send_accepted_max_ms.max, 7);
  assert.equal(commandPlayer.metrics.server_command_room_queue_max_ms.max, 74);
  assert.equal(commandPlayer.metrics.server_snapshot_send_age_max_ms.max, 132);
  assert.equal(commandPlayer.metrics.predicted_snapshot_late_frame_count.max, 2);
  assert.equal(commandPlayer.commandLifecycle.familyCounts.move, 12);
  assert.equal(
    commandPlayer.commandLifecycle.stages.find((stage) => stage.id === "room_queue")?.maxMs,
    74,
  );
  assert.equal(commandPlayer.commandLifecycle.exemplars[0].stage, "issueToSimAck");
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
  assert.match(commandMarkdown, /Command Lifecycle Waterfall/);
  assert.match(commandMarkdown, /room_queue 74\/50ms server/);
  assert.match(commandMarkdown, /seq 7 move issueToSimAck 81ms/);
  assert.match(commandMarkdown, /3\/1\/132\/2/);
} finally {
  rmSync(commandDir, { recursive: true, force: true });
}

const clientFrameDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-client-frame-"));
try {
  const clientFrameLog = path.join(clientFrameDir, "client-frame.log");
  const framePhasesJson = JSON.stringify([
    { label: "match.renderer", count: 12, maxMs: 38, p95Ms: 24 },
    { label: "frame.unattributed", count: 12, maxMs: 7, p95Ms: 4 },
  ]).replace(/"/g, '\\"');
  const rendererPhasesJson = JSON.stringify([
    { label: "renderer.units", count: 12, maxMs: 22, p95Ms: 18 },
    { label: "renderer.fogDraw", count: 12, maxMs: 9, p95Ms: 4 },
  ]).replace(/"/g, '\\"');
  const countersJson = JSON.stringify([
    { label: "renderer.pixi.displayObject", samples: 180, frames: 12, total: 180, maxFrame: 30 },
    { label: "hud.dirty", samples: 6, frames: 6, total: 6, maxFrame: 1 },
  ]).replace(/"/g, '\\"');
  writeFileSync(
    clientFrameLog,
    [
      `2026-06-24T12:00:00Z INFO event="client_net_report" match_run_id="client-frame-1" player_id=6 primary_issue="client_renderer" rtt_max_ms=35 snapshot_gap_max_ms=140 snapshot_jitter_ms=2 snapshot_late_frame_count=3 predicted_snapshot_late_frame_count=0 predicted_snapshot_late_frame_pct_x100=0 prediction_active_late_frame_count=0 frame_gap_max_ms=120 frame_work_max_ms=45 frame_work_p95_ms=28 frame_raf_dispatch_max_ms=4 frame_raf_dispatch_p95_ms=1 frame_unattributed_max_ms=7 frame_unattributed_p95_ms=4 worst_frame_phase="match.renderer" worst_frame_phase_ms=38 renderer_max_ms=36 renderer_p95_ms=18 top_renderer_phase="renderer.units" top_renderer_phase_ms=22 top_render_diagnostic_group="renderer.pixi.displayObject" top_render_diagnostic_group_count=180 client_frame_phases="${framePhasesJson}" renderer_frame_phases="${rendererPhasesJson}" render_diagnostic_counters="${countersJson}" fps_estimate=28 prediction_replay_max_ms=0 prediction_replay_max_ticks=0 prediction_replay_budget_exceeded_count=0 correction_count=0 server_tick_ms=4 server_lag_ms=0 "client network report"`,
    ].join("\n") + "\n"
  );
  const clientFrameParsed = JSON.parse(run(["--format", "json", clientFrameLog]));
  const clientFrameMatch = clientFrameParsed.matches.find((match) => match.matchRunId === "client-frame-1");
  assert.ok(clientFrameMatch, "expected synthetic client-frame match summary");
  const clientFramePlayer = clientFrameMatch.players.find((player) => player.playerId === "6");
  assert.ok(clientFramePlayer, "expected synthetic client-frame player summary");
  assert.equal(clientFramePlayer.clientContext.interpretation.status, "sustained");
  assert.equal(clientFramePlayer.clientContext.likelyLocalPhase.label, "renderer.units");
  assert.equal(clientFramePlayer.clientContext.topRenderDiagnosticGroups[0].label, "renderer.pixi.displayObject");
  assert.match(
    clientFramePlayer.clientContext.lateSnapshotPredictionCoverage.interpretation,
    /no owned predicted overlay/,
  );
  assert.ok(
    clientFrameMatch.classifications
      .find((item) => item.id === "browser_processing")
      ?.evidenceFor.some((item) => item.includes("likely local phase renderer.units")),
    "expected browser-processing evidence to name the likely local phase",
  );
  const clientFrameMarkdown = run([clientFrameLog]);
  assert.match(clientFrameMarkdown, /Client Frame Context/);
  assert.match(clientFrameMarkdown, /renderer\.units/);
  assert.match(clientFrameMarkdown, /no owned predicted overlay/);
} finally {
  rmSync(clientFrameDir, { recursive: true, force: true });
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

const combinedDir = mkdtempSync(path.join(os.tmpdir(), "rts-net-report-parser-combined-"));
try {
  const combinedLog = path.join(combinedDir, "combined.log");
  writeFileSync(
    combinedLog,
    [
      '2026-06-24T11:00:00Z INFO event="client_net_report" match_run_id="combined-a" player_id=1 primary_issue="command_upload_delay" rtt_max_ms=30 snapshot_gap_max_ms=40 snapshot_jitter_ms=1 command_issue_to_server_receipt_max_ms=300 command_issue_to_sim_ack_max_ms=320 frame_gap_max_ms=10 frame_work_max_ms=3 fps_estimate=60 server_tick_ms=4 server_lag_ms=0 slow_tick_count=0 "client network report"',
      '2026-06-24T11:00:10Z INFO event="client_net_report" match_run_id="combined-b" player_id=2 primary_issue="frame_work" rtt_max_ms=35 snapshot_gap_max_ms=45 snapshot_jitter_ms=2 command_issue_to_server_receipt_max_ms=20 command_issue_to_sim_ack_max_ms=40 frame_gap_max_ms=150 frame_work_max_ms=45 fps_estimate=25 server_tick_ms=5 server_lag_ms=0 slow_tick_count=0 "client network report"',
    ].join("\n") + "\n"
  );
  const combinedParsed = JSON.parse(run(["--format", "json", combinedLog]));
  assert.deepEqual(
    combinedParsed.matches.map((match) => match.match).sort(),
    ["combined-a", "combined-b"],
  );
  const combinedCoverage = combinedParsed.agentDigest.coverageMatrix.matches;
  for (const matchId of ["combined-a", "combined-b"]) {
    const coverage = combinedCoverage.find((match) => match.match === matchId);
    assert.ok(coverage, `expected coverage for ${matchId}`);
    assert.equal(
      coverage.items.find((item) => item.id === "client_reports")?.rows,
      1,
      `expected only ${matchId} rows in its coverage item`,
    );
  }
  const commandTopWindow = combinedParsed.agentDigest.topWindows.groups.find((group) => group.id === "command")?.windows[0];
  assert.equal(commandTopWindow?.match, "combined-a");
} finally {
  rmSync(combinedDir, { recursive: true, force: true });
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
