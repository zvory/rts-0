export class StatusBadge {
  /**
   * @param {HTMLElement|null} rootEl
   */
  constructor(rootEl) {
    this.root = rootEl;
    this.version = "unknown";
    this.metrics = null;
    this._render();
  }

  setVersion(version) {
    this.version = version || "unknown";
    this._render();
  }

  setMatchMetrics(metrics) {
    this.metrics = metrics || null;
    this._render();
  }

  clearMatchMetrics() {
    this.metrics = null;
    this._render();
  }

  _render() {
    if (!this.root) return;
    const metrics = this.metrics;
    const issueTokens = metrics ? [
      formatIssue("lat", metrics.issues.latency),
      formatIssue("slow", metrics.issues.slowTick),
      formatIssue("hol", metrics.issues.headOfLine),
      formatIssue("jit", metrics.issues.jitter),
    ].filter(Boolean) : [];

    this.root.innerHTML =
      `<div class="status-badge-build">${escapeHtml(this.version)}</div>` +
      (metrics
        ? `<div class="status-badge-metrics">` +
            metricSpan("rtt", formatMs(metrics.latencyMs), metrics.issues.latency.active) +
            metricSpan("tick", formatMs(metrics.serverTickMs), metrics.issues.slowTick.active) +
            metricSpan("lag", formatMs(metrics.serverLagMs), metrics.issues.slowTick.active) +
            metricSpan("jit", formatMs(metrics.jitterMs), metrics.issues.jitter.active) +
          `</div>` +
          `<div class="status-badge-issues">${
            issueTokens.length
              ? issueTokens.join("")
              : `<span class="status-badge-issue-ok">issues ok</span>`
          }</div>`
        : "");
  }
}

function metricSpan(label, value, active) {
  return `<span class="status-badge-metric${active ? " is-active" : ""}">${label} ${value}</span>`;
}

function formatIssue(label, issue) {
  if (!issue || (!issue.count && !issue.active)) return "";
  return `<span class="status-badge-issue${issue.active ? " is-active" : ""}">${label}${issue.active ? "!" : ""} ${issue.count}</span>`;
}

function formatMs(value) {
  return Number.isFinite(value) ? `${Math.max(0, Math.round(value))}ms` : "--";
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
