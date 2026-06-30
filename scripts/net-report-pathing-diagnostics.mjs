export function summarizePathingDiagnostics(rows) {
  const passes = new Map();
  const topRequestSources = new Map();
  const topQueuedSources = new Map();
  let worstRequestMaxMs = null;
  let exploredNodesMax = null;
  let pathLenMax = null;
  let totalRequests = 0;
  let processedMax = 0;
  let totalDeferred = 0;
  let budgetExhaustedCount = 0;

  for (const row of rows) {
    const passId = String(row.fields.pass || "unknown");
    const pass = passes.get(passId) || {
      pass: passId,
      rows: 0,
      awaitingStartMax: 0,
      queuedForPathMax: 0,
      processedMax: 0,
      deferredMax: 0,
      stillAwaitingMax: 0,
      budgetExhaustedCount: 0,
      worstRequestMaxMs: 0,
      exploredNodesMax: 0,
      pathLenMax: 0,
      requestSources: new Map(),
      queuedSources: new Map(),
    };
    pass.rows += 1;
    pass.awaitingStartMax = Math.max(pass.awaitingStartMax, numeric(row.fields.awaiting_start));
    pass.queuedForPathMax = Math.max(pass.queuedForPathMax, numeric(row.fields.queued_for_path));
    pass.processedMax = Math.max(pass.processedMax, numeric(row.fields.requests_processed));
    pass.deferredMax = Math.max(pass.deferredMax, numeric(row.fields.requests_deferred));
    pass.stillAwaitingMax = Math.max(pass.stillAwaitingMax, numeric(row.fields.still_awaiting));
    if (truthyField(row.fields.coordinator_budget_exhausted) || numeric(row.fields.path_budget_exhausted) > 0) {
      pass.budgetExhaustedCount += 1;
      budgetExhaustedCount += 1;
    }
    pass.worstRequestMaxMs = Math.max(pass.worstRequestMaxMs, numeric(row.fields.worst_request_ms));
    pass.exploredNodesMax = Math.max(pass.exploredNodesMax, numeric(row.fields.explored_nodes_max));
    pass.pathLenMax = Math.max(pass.pathLenMax, numeric(row.fields.path_len_max));
    mergeCountString(pass.requestSources, row.fields.source_counts);
    mergeCountString(pass.queuedSources, row.fields.queued_source_counts);
    mergeCountString(topRequestSources, row.fields.source_counts);
    mergeCountString(topQueuedSources, row.fields.queued_source_counts);

    totalRequests += numeric(row.fields.requests_processed);
    processedMax = Math.max(processedMax, numeric(row.fields.requests_processed));
    totalDeferred = Math.max(totalDeferred, numeric(row.fields.requests_deferred));
    worstRequestMaxMs = maxNullable(worstRequestMaxMs, row.fields.worst_request_ms);
    exploredNodesMax = maxNullable(exploredNodesMax, row.fields.explored_nodes_max);
    pathLenMax = maxNullable(pathLenMax, row.fields.path_len_max);
    passes.set(passId, pass);
  }

  const passSummaries = [...passes.values()]
    .map((pass) => {
      const { requestSources, queuedSources, ...summary } = pass;
      return {
        ...summary,
        topSources: topCounts(preferredSourceMap(requestSources, queuedSources)),
        queuedSources: topCounts(queuedSources),
      };
    })
    .sort((a, b) => b.worstRequestMaxMs - a.worstRequestMaxMs || a.pass.localeCompare(b.pass));

  const summary = {
    rows: rows.length,
    totalRequests,
    processedMax,
    totalDeferred,
    budgetExhaustedCount,
    worstRequestMaxMs,
    exploredNodesMax,
    pathLenMax,
    topSources: topCounts(preferredSourceMap(topRequestSources, topQueuedSources)),
    queuedSources: topCounts(topQueuedSources),
    passes: passSummaries,
  };
  summary.interpretation = interpretPathingDiagnostics(summary);
  return summary;
}

export function addPathingPhaseEvidence(rows, evidenceFor, evidenceAgainst, slowestPhaseWarnMs) {
  const pathingPhases = new Set(["awaiting_paths", "promote_queued_orders", "promoted_awaiting_paths"]);
  const phaseRows = rows
    .filter(
      (row) =>
        row.event === "performance_tick" &&
        pathingPhases.has(String(row.fields.slowest_phase || "")) &&
        Number.isFinite(row.fields.slowest_phase_ms),
    )
    .sort((a, b) => Number(b.fields.slowest_phase_ms) - Number(a.fields.slowest_phase_ms));
  const worst = phaseRows[0];
  if (worst && worst.fields.slowest_phase_ms >= slowestPhaseWarnMs) {
    evidenceFor.push(
      `serverTick slowest_phase ${worst.fields.slowest_phase} ${worst.fields.slowest_phase_ms}ms pathing phase`,
    );
  } else if (worst) {
    evidenceAgainst.push(
      `serverTick slowest pathing phase ${worst.fields.slowest_phase} ${worst.fields.slowest_phase_ms}ms below ${slowestPhaseWarnMs}`,
    );
  }
}

export function appendPathingDiagnosticsMarkdown(lines, diagnostics) {
  if (!diagnostics || diagnostics.rows === 0) {
    return;
  }
  lines.push("");
  lines.push("### Pathing Slow-Tick Diagnostics");
  lines.push(
    `- Interpretation: ${diagnostics.interpretation.primary}; ${diagnostics.interpretation.detail}`,
  );
  lines.push(
    `- Top sources: ${
      diagnostics.topSources.length > 0
        ? diagnostics.topSources.map((item) => `${item.label}=${item.count}`).join(", ")
        : "n/a"
    }`,
  );
  lines.push("");
  lines.push("| pass | rows | awaiting start max | queued for path max | processed max | deferred max | budget rows | worst request max | explored nodes max | path len max | top sources |");
  lines.push("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |");
  for (const pass of diagnostics.passes) {
    lines.push(
      [
        pass.pass,
        pass.rows,
        pass.awaitingStartMax,
        pass.queuedForPathMax,
        pass.processedMax,
        pass.deferredMax,
        pass.budgetExhaustedCount,
        pass.worstRequestMaxMs,
        pass.exploredNodesMax,
        pass.pathLenMax,
        pass.topSources.length > 0 ? pass.topSources.map((item) => `${item.label}=${item.count}`).join(", ") : "n/a",
      ].join(" | ").replace(/^/, "| ").replace(/$/, " |"),
    );
  }
}

function interpretPathingDiagnostics(summary) {
  if (summary.rows === 0) {
    return {
      primary: "unavailable",
      detail: "Pathing diagnostics were not logged in this artifact.",
    };
  }
  const queuePass = summary.passes.find((pass) => pass.pass === "promote_queued_orders");
  const queuePromotion =
    queuePass && (queuePass.queuedForPathMax >= 16 || queuePass.processedMax >= 16);
  if (summary.totalDeferred > 0 || summary.budgetExhaustedCount > 0 || summary.processedMax >= 64) {
    return {
      primary: "path request volume",
      detail: `Processed up to ${summary.processedMax} path requests in one logged pass (${summary.totalRequests} total across rows), with ${summary.totalDeferred} still awaiting/deferred and ${summary.budgetExhaustedCount} budget-exhausted rows.`,
    };
  }
  if ((summary.exploredNodesMax ?? 0) >= 4096 || (summary.worstRequestMaxMs ?? 0) >= 8) {
    return {
      primary: "path complexity",
      detail: `Worst request ${formatValue(summary.worstRequestMaxMs)}ms, explored nodes max ${formatValue(summary.exploredNodesMax)}, path length max ${formatValue(summary.pathLenMax)}.`,
    };
  }
  if (queuePromotion) {
    return {
      primary: "queue promotion",
      detail: `Queued promotion staged up to ${queuePass.queuedForPathMax} units for pathing in one logged pass.`,
    };
  }
  return {
    primary: "unknown",
    detail: "Pathing rows were present but did not cross volume, complexity, or queue-promotion thresholds.",
  };
}

function numeric(value) {
  return Number.isFinite(value) ? value : 0;
}

function maxNullable(current, value) {
  if (!Number.isFinite(value)) {
    return current;
  }
  return current === null ? value : Math.max(current, value);
}

function truthyField(value) {
  return value === true || value === "true" || value === 1 || value === "1";
}

function mergeCountString(target, value) {
  if (typeof value !== "string" || value === "" || value === "none") {
    return;
  }
  for (const part of value.split(",")) {
    const [label, rawCount] = part.split("=");
    const count = Number(rawCount);
    if (!label || !Number.isFinite(count)) {
      continue;
    }
    target.set(label, (target.get(label) || 0) + count);
  }
}

function topCounts(map, limit = 5) {
  return [...map.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([label, count]) => ({ label, count }));
}

function preferredSourceMap(requestSources, queuedSources) {
  return requestSources.size > 0 ? requestSources : queuedSources;
}

function formatValue(value) {
  return value === null || value === undefined || value === "" ? "n/a" : String(value);
}
