export function summarizeClientContext(rows, warnThreshold) {
  const metrics = Object.fromEntries(
    [
      "frame_gap_max_ms",
      "frame_work_max_ms",
      "frame_work_p95_ms",
      "frame_raf_dispatch_max_ms",
      "frame_raf_dispatch_p95_ms",
      "frame_unattributed_max_ms",
      "frame_unattributed_p95_ms",
      "frame_work_budget_miss_count",
      "present_budget_miss_count",
      "renderer_max_ms",
      "renderer_p95_ms",
      "renderer_update_max_ms",
      "renderer_update_p95_ms",
      "renderer_present_max_ms",
      "renderer_present_p95_ms",
      "top_renderer_phase_ms",
      "snapshot_late_frame_count",
      "predicted_snapshot_late_frame_count",
      "predicted_snapshot_late_frame_pct_x100",
      "prediction_active_late_frame_count",
      "prediction_replay_max_ms",
      "prediction_replay_max_ticks",
      "prediction_replay_budget_exceeded_count",
      "correction_distance_px",
      "correction_count",
    ].map((field) => [field, summarizeField(rows, field)])
  );
  const framePhases = summarizePhaseArrayField(rows, "client_frame_phases");
  const rendererPhases = summarizePhaseArrayField(rows, "renderer_frame_phases");
  const counterGroups = summarizeCounterArrayField(rows, "render_diagnostic_counters");
  const likelyLocalPhase = likelyClientPhase(metrics, framePhases, rendererPhases, warnThreshold);
  const lateFrames = metricMaxValue(metrics.snapshot_late_frame_count);
  const predictedLateFrames = metricMaxValue(metrics.predicted_snapshot_late_frame_count);
  const predictedLatePct = metricMaxValue(metrics.predicted_snapshot_late_frame_pct_x100);
  const activeLateFrames = metricMaxValue(metrics.prediction_active_late_frame_count);
  return {
    interpretation: interpretLocalFrameWork(metrics, warnThreshold),
    likelyLocalPhase,
    topFramePhases: framePhases,
    topRendererPhases: rendererPhases,
    topRenderDiagnosticGroups: counterGroups,
    worstFramePhases: summarizeStringField(rows, "worst_frame_phase"),
    commandBurstWorstFramePhases: summarizeStringField(rows, "command_burst_worst_frame_phase"),
    topRendererPhaseLabels: summarizeStringField(rows, "top_renderer_phase"),
    topRenderDiagnosticGroupLabels: summarizeStringField(rows, "top_render_diagnostic_group"),
    frameBudget: {
      workMisses: metricMaxValue(metrics.frame_work_budget_miss_count),
      presentMisses: metricMaxValue(metrics.present_budget_miss_count),
      updateMaxMs: metricMaxValue(metrics.renderer_update_max_ms),
      updateP95Ms: metricMaxValue(metrics.renderer_update_p95_ms),
      presentMaxMs: metricMaxValue(metrics.renderer_present_max_ms),
      presentP95Ms: metricMaxValue(metrics.renderer_present_p95_ms),
      rafDispatchMaxMs: metricMaxValue(metrics.frame_raf_dispatch_max_ms),
      rafDispatchP95Ms: metricMaxValue(metrics.frame_raf_dispatch_p95_ms),
      unattributedMaxMs: metricMaxValue(metrics.frame_unattributed_max_ms),
      unattributedP95Ms: metricMaxValue(metrics.frame_unattributed_p95_ms),
    },
    lateSnapshotPredictionCoverage: {
      lateFrames,
      predictedLateFrames,
      predictedLatePctX100: predictedLatePct,
      activePredictionLateFrames: activeLateFrames,
      interpretation: interpretLateSnapshotPredictionCoverage(
        lateFrames,
        predictedLateFrames,
        predictedLatePct,
        activeLateFrames,
        metrics,
        warnThreshold,
      ),
    },
    prediction: {
      replayMaxMs: metricMaxValue(metrics.prediction_replay_max_ms),
      replayMaxTicks: metricMaxValue(metrics.prediction_replay_max_ticks),
      replayBudgetExceededCount: metricMaxValue(metrics.prediction_replay_budget_exceeded_count),
      correctionDistancePx: metricMaxValue(metrics.correction_distance_px),
      correctionCount: metricMaxValue(metrics.correction_count),
    },
  };
}

export function addClientFrameContextEvidence(players, evidenceFor, evidenceAgainst) {
  for (const player of players) {
    const context = player.clientContext;
    if (!context || context.interpretation.status === "unknown") continue;
    const phase = context.likelyLocalPhase?.label || "unknown";
    const detail = `player ${player.playerId} likely local phase ${phase}: ${context.interpretation.text}`;
    if (context.interpretation.status === "sustained") {
      evidenceFor.push(detail);
    } else {
      evidenceAgainst.push(detail);
    }
  }
}

export function appendClientFrameContextMarkdown(lines, players, { formatPctX100 }) {
  const rows = players
    .map((player) => ({ player, context: player.clientContext }))
    .filter(({ context }) => hasClientFrameContext(context));
  if (rows.length === 0) return;

  lines.push("");
  lines.push("### Client Frame Context");
  for (const { player, context } of rows) {
    const phase = context.likelyLocalPhase
      ? `${context.likelyLocalPhase.label}${context.likelyLocalPhase.maxMs ? ` max ${context.likelyLocalPhase.maxMs}ms` : ""}`
      : "n/a";
    const renderer = context.topRendererPhases[0]
      ? `${context.topRendererPhases[0].label} max ${context.topRendererPhases[0].maxMs}ms p95 ${context.topRendererPhases[0].p95Ms}ms`
      : "n/a";
    const counter = context.topRenderDiagnosticGroups[0]
      ? `${context.topRenderDiagnosticGroups[0].label} total ${context.topRenderDiagnosticGroups[0].total} max/frame ${context.topRenderDiagnosticGroups[0].maxFrame}`
      : "n/a";
    const coverage = context.lateSnapshotPredictionCoverage;
    const budget = context.frameBudget;
    const budgetDetail = `${budget.workMisses} 60 FPS work-budget misses; ${budget.presentMisses} 60 FPS present-budget misses; renderer update max/p95 ${budget.updateMaxMs}/${budget.updateP95Ms}ms; renderer present max/p95 ${budget.presentMaxMs}/${budget.presentP95Ms}ms; RAF dispatch max/p95 ${budget.rafDispatchMaxMs}/${budget.rafDispatchP95Ms}ms; unattributed max/p95 ${budget.unattributedMaxMs}/${budget.unattributedP95Ms}ms`;
    const lateCoverage = coverage.lateFrames
      ? `${coverage.predictedLateFrames}/${coverage.lateFrames} predicted (${formatPctX100(coverage.predictedLatePctX100)})`
      : "no late snapshot frames";
    lines.push(
      `- player ${player.playerId}: ${context.interpretation.text}; likely phase ${phase}; ${budgetDetail}; renderer ${renderer}; diagnostics ${counter}; late-snapshot prediction ${lateCoverage}; ${coverage.interpretation}.`
    );
  }
}

function interpretLocalFrameWork(metrics, warnThreshold) {
  const frameP95 = metricMaxValue(metrics.frame_work_p95_ms);
  const frameMax = metricMaxValue(metrics.frame_work_max_ms);
  const rendererP95 = metricMaxValue(metrics.renderer_p95_ms);
  const rendererMax = metricMaxValue(metrics.renderer_max_ms);
  const rafP95 = metricMaxValue(metrics.frame_raf_dispatch_p95_ms);
  const rafMax = metricMaxValue(metrics.frame_raf_dispatch_max_ms);
  const unattributedP95 = metricMaxValue(metrics.frame_unattributed_p95_ms);
  const unattributedMax = metricMaxValue(metrics.frame_unattributed_max_ms);
  if (
    frameP95 >= warnThreshold.frame_work_p95_ms ||
    rendererP95 >= warnThreshold.renderer_p95_ms ||
    rafP95 >= warnThreshold.frame_raf_dispatch_p95_ms ||
    unattributedP95 >= warnThreshold.frame_unattributed_p95_ms
  ) {
    return {
      status: "sustained",
      text: "local frame work was a sustained bottleneck in at least one report window",
    };
  }
  if (
    frameMax >= warnThreshold.frame_work_max_ms ||
    rendererMax >= warnThreshold.renderer_max_ms ||
    rafMax >= warnThreshold.frame_raf_dispatch_max_ms ||
    unattributedMax >= warnThreshold.frame_unattributed_max_ms
  ) {
    return {
      status: "spike_not_sustained",
      text: "local frame work spiked, but p95 did not show a sustained bottleneck",
    };
  }
  if (
    metrics.frame_work_max_ms ||
    metrics.renderer_max_ms ||
    metrics.frame_raf_dispatch_max_ms ||
    metrics.frame_unattributed_max_ms
  ) {
    return {
      status: "not_sustained",
      text: "local frame work was not the sustained bottleneck in these report windows",
    };
  }
  return {
    status: "unknown",
    text: "local frame phase context was not logged in this artifact",
  };
}

function likelyClientPhase(metrics, framePhases, rendererPhases, warnThreshold) {
  if (
    metricMaxValue(metrics.present_budget_miss_count) > 0 ||
    metricMaxValue(metrics.renderer_present_p95_ms) >= warnThreshold.renderer_present_p95_ms ||
    metricMaxValue(metrics.renderer_present_max_ms) >= warnThreshold.renderer_present_max_ms
  ) {
    return { label: "renderer.present", reason: "actual present crossed the 60 FPS or renderer threshold" };
  }
  if (
    metricMaxValue(metrics.renderer_update_p95_ms) >= warnThreshold.renderer_update_p95_ms ||
    metricMaxValue(metrics.renderer_update_max_ms) >= warnThreshold.renderer_update_max_ms
  ) {
    return { label: "renderer.update", reason: "renderer scene update crossed threshold" };
  }
  if (
    metricMaxValue(metrics.frame_raf_dispatch_p95_ms) >= warnThreshold.frame_raf_dispatch_p95_ms ||
    metricMaxValue(metrics.frame_raf_dispatch_max_ms) >= warnThreshold.frame_raf_dispatch_max_ms
  ) {
    return { label: "frame.rafDispatch", reason: "RAF dispatch delay crossed threshold" };
  }
  if (
    metricMaxValue(metrics.renderer_p95_ms) >= warnThreshold.renderer_p95_ms ||
    metricMaxValue(metrics.renderer_max_ms) >= warnThreshold.renderer_max_ms ||
    metricMaxValue(metrics.top_renderer_phase_ms) >= warnThreshold.top_renderer_phase_ms
  ) {
    const renderer = rendererPhases[0];
    return {
      label: renderer?.label || "match.renderer",
      reason: "renderer work crossed threshold",
      maxMs: renderer?.maxMs,
      p95Ms: renderer?.p95Ms,
    };
  }
  if (
    metricMaxValue(metrics.frame_unattributed_p95_ms) >= warnThreshold.frame_unattributed_p95_ms ||
    metricMaxValue(metrics.frame_unattributed_max_ms) >= warnThreshold.frame_unattributed_max_ms
  ) {
    return { label: "frame.unattributed", reason: "unattributed frame work crossed threshold" };
  }
  if (
    metricMaxValue(metrics.frame_work_budget_miss_count) > 0 ||
    metricMaxValue(metrics.frame_work_p95_ms) >= warnThreshold.frame_work_p95_ms ||
    metricMaxValue(metrics.frame_work_max_ms) >= warnThreshold.frame_work_max_ms
  ) {
    const phase = framePhases[0];
    return {
      label: phase?.label || "frame.work",
      reason: "overall frame work crossed threshold",
      maxMs: phase?.maxMs,
      p95Ms: phase?.p95Ms,
    };
  }
  return framePhases[0]
    ? {
        label: framePhases[0].label,
        reason: "largest uploaded local phase, below sustained thresholds",
        maxMs: framePhases[0].maxMs,
        p95Ms: framePhases[0].p95Ms,
      }
    : null;
}

function interpretLateSnapshotPredictionCoverage(lateFrames, predictedLateFrames, predictedLatePct, activeLateFrames, metrics, warnThreshold) {
  if (!lateFrames) {
    return "no late snapshot frames in the report window";
  }
  const replayMaxMs = metricMaxValue(metrics.prediction_replay_max_ms);
  const replayExceeded = metricMaxValue(metrics.prediction_replay_budget_exceeded_count);
  const correctionCount = metricMaxValue(metrics.correction_count);
  if (replayExceeded > 0 || replayMaxMs >= warnThreshold.prediction_replay_max_ms) {
    return "prediction replay work was high while snapshots were late in the same window";
  }
  if (correctionCount > 0) {
    return "prediction corrected during a window that also had late snapshots";
  }
  if (predictedLateFrames > 0 || predictedLatePct > 0 || activeLateFrames > 0) {
    return "owned prediction coverage was present while snapshots were late";
  }
  return "snapshots were late and no owned predicted overlay was reported";
}

function summarizePhaseArrayField(rows, field) {
  const groups = new Map();
  for (const row of rows) {
    for (const entry of parseReportArray(row.fields[field])) {
      const label = safeLabel(entry?.label);
      if (!label) continue;
      const group = groups.get(label) || {
        label,
        reports: 0,
        count: 0,
        maxMs: 0,
        p95Ms: 0,
      };
      group.reports += 1;
      group.count += positiveNumber(entry?.count);
      group.maxMs = Math.max(group.maxMs, positiveNumber(entry?.maxMs));
      group.p95Ms = Math.max(group.p95Ms, positiveNumber(entry?.p95Ms));
      groups.set(label, group);
    }
  }
  return [...groups.values()]
    .sort((a, b) => b.maxMs - a.maxMs || b.p95Ms - a.p95Ms || b.count - a.count || a.label.localeCompare(b.label))
    .slice(0, 5);
}

function summarizeCounterArrayField(rows, field) {
  const groups = new Map();
  for (const row of rows) {
    for (const entry of parseReportArray(row.fields[field])) {
      const label = safeLabel(entry?.label);
      if (!label) continue;
      const group = groups.get(label) || {
        label,
        reports: 0,
        samples: 0,
        frames: 0,
        total: 0,
        maxFrame: 0,
      };
      group.reports += 1;
      group.samples += positiveNumber(entry?.samples);
      group.frames += positiveNumber(entry?.frames);
      group.total += positiveNumber(entry?.total);
      group.maxFrame = Math.max(group.maxFrame, positiveNumber(entry?.maxFrame));
      groups.set(label, group);
    }
  }
  return [...groups.values()]
    .sort((a, b) => b.total - a.total || b.maxFrame - a.maxFrame || a.label.localeCompare(b.label))
    .slice(0, 5);
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

function summarizeStringField(rows, field) {
  const counts = new Map();
  for (const row of rows) {
    if (row.fields[field] === undefined) continue;
    const value = String(row.fields[field] ?? "").slice(0, 128) || "(empty)";
    counts.set(value, (counts.get(value) || 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([value, count]) => ({ value, count }));
}

function percentile(sortedValues, percentileValue) {
  if (sortedValues.length === 0) return null;
  const index = Math.min(sortedValues.length - 1, Math.ceil(sortedValues.length * percentileValue) - 1);
  return sortedValues[index];
}

function parseReportArray(value) {
  if (Array.isArray(value)) return value;
  if (typeof value !== "string" || !value.trim().startsWith("[")) return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function safeLabel(value) {
  const label = String(value || "").replace(/[^A-Za-z0-9_.:-]/g, "_").slice(0, 64);
  return label || "";
}

function positiveNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}

function metricMaxValue(metric) {
  return Number.isFinite(metric?.max) ? metric.max : 0;
}

function hasClientFrameContext(context) {
  if (!context) return false;
  return context.interpretation.status !== "unknown" ||
    context.frameBudget.workMisses > 0 ||
    context.frameBudget.presentMisses > 0 ||
    context.frameBudget.updateMaxMs > 0 ||
    context.frameBudget.presentMaxMs > 0 ||
    context.topFramePhases.length > 0 ||
    context.topRendererPhases.length > 0 ||
    context.topRenderDiagnosticGroups.length > 0 ||
    context.lateSnapshotPredictionCoverage.lateFrames > 0;
}
