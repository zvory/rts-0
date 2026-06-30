export function missingDiagnosticGroups(rows, issueGroups) {
  const fields = new Set();
  for (const row of rows) {
    for (const key of Object.keys(row.fields)) {
      fields.add(key);
    }
  }
  const missing = [];
  for (const group of issueGroups) {
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
  if (!fields.has("pathing_requests") && !fields.has("requests_processed")) {
    missing.push("server pathing diagnostics: no path request/source/complexity fields in input");
  }
  if (!fields.has("prediction_disable_wasm_count") || !fields.has("prediction_replay_max_ms")) {
    missing.push("prediction disable reason/replay budget detail: no matching fields in input");
  }
  if (!fields.has("predicted_snapshot_late_frame_count")) {
    missing.push("predicted snapshot coverage during late snapshot frames: no matching fields in input");
  }
  if (!fields.has("client_frame_phases") || !fields.has("frame_raf_dispatch_max_ms")) {
    missing.push("client frame phase context: no RAF/unattributed/top-phase fields in input");
  }
  if (!fields.has("renderer_frame_phases") || !fields.has("render_diagnostic_counters")) {
    missing.push("client renderer diagnostic groups: no top renderer phase/counter fields in input");
  }
  return missing;
}
