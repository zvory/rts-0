export class StatusBadge {
  /**
   * @param {HTMLElement|null} rootEl
   */
  constructor(rootEl) {
    this.root = rootEl;
    this.version = "unknown";
    this.metrics = null;
    this.copyButtonText = "copy";
    this.copyResetTimer = undefined;
    this.onClick = (ev) => this._handleClick(ev);
    this._render();
    this.root?.addEventListener("click", this.onClick);
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

  destroy() {
    this.root?.removeEventListener("click", this.onClick);
    if (this.copyResetTimer) window.clearTimeout(this.copyResetTimer);
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
      `<div class="status-badge-content">` +
        `<div class="status-badge-build">${escapeHtml(this.version)}</div>` +
        (metrics
          ? `<div class="status-badge-metrics">` +
              metricSpan("rtt", formatMs(metrics.latencyMs), metrics.issues.latency.active) +
              metricSpan("fps", formatFps(metrics.fps), false) +
              metricSpan("1m fps", formatFps(metrics.fpsOneMinute), false) +
              metricSpan("tick", formatMs(metrics.serverTickMs), metrics.issues.slowTick.active) +
              metricSpan("lag", formatMs(metrics.serverLagMs), metrics.issues.slowTick.active) +
              metricSpan("jit", formatMs(metrics.jitterMs), metrics.issues.jitter.active) +
            `</div>` +
            `<div class="status-badge-issues">${
              issueTokens.length
                ? issueTokens.join("")
                : `<span class="status-badge-issue-ok">issues ok</span>`
            }</div>`
          : "") +
      `</div>` +
      `<button class="status-badge-copy" type="button" title="Copy debug info" aria-label="Copy debug info">${this.copyButtonText}</button>`;
  }

  async _handleClick(ev) {
    const button = ev.target?.closest?.(".status-badge-copy");
    if (!button || !this.root?.contains(button)) return;
    ev.preventDefault();
    ev.stopPropagation();

    const text = this._copyText();
    try {
      await copyText(text);
      this._showCopied(button);
    } catch {
      this._showCopyFailed(button);
    }
  }

  _copyText() {
    const metrics = this.metrics;
    const lines = [`build ${this.version || "unknown"}`];
    if (metrics) {
      lines.push(
        [
          `rtt ${formatMs(metrics.latencyMs)}`,
          `fps ${formatFps(metrics.fps)}`,
          `1m fps ${formatFps(metrics.fpsOneMinute)}`,
          `tick ${formatMs(metrics.serverTickMs)}`,
          `lag ${formatMs(metrics.serverLagMs)}`,
          `jit ${formatMs(metrics.jitterMs)}`,
        ].join("  "),
      );
      const issueTexts = [
        formatIssueText("lat", metrics.issues.latency),
        formatIssueText("slow", metrics.issues.slowTick),
        formatIssueText("hol", metrics.issues.headOfLine),
        formatIssueText("jit", metrics.issues.jitter),
      ].filter(Boolean);
      lines.push(issueTexts.length ? issueTexts.join("  ") : "issues ok");
    }
    return lines.join("\n");
  }

  _showCopied(button) {
    this._setCopyButtonText(button, "copied");
  }

  _showCopyFailed(button) {
    this._setCopyButtonText(button, "failed");
  }

  _setCopyButtonText(button, text) {
    this.copyButtonText = text;
    button.textContent = text;
    if (this.copyResetTimer) window.clearTimeout(this.copyResetTimer);
    this.copyResetTimer = window.setTimeout(() => {
      this.copyButtonText = "copy";
      this.root?.querySelector(".status-badge-copy")?.replaceChildren("copy");
      this.copyResetTimer = undefined;
    }, 1200);
  }
}

function metricSpan(label, value, active) {
  return `<span class="status-badge-metric${active ? " is-active" : ""}">${label} ${value}</span>`;
}

function formatIssue(label, issue) {
  if (!issue || (!issue.count && !issue.active)) return "";
  return `<span class="status-badge-issue${issue.active ? " is-active" : ""}">${label}${issue.active ? "!" : ""} ${issue.count}</span>`;
}

function formatIssueText(label, issue) {
  if (!issue || (!issue.count && !issue.active)) return "";
  const count = issue?.count || 0;
  return `${label}${issue?.active ? "!" : ""} ${count}`;
}

function formatMs(value) {
  return Number.isFinite(value) ? `${Math.max(0, Math.round(value))}ms` : "--";
}

function formatFps(value) {
  return Number.isFinite(value) ? String(Math.max(0, Math.round(value))) : "--";
}

async function copyText(text) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      // Some browser/privacy combinations expose navigator.clipboard but reject
      // the write; keep the click-gesture fallback path available.
    }
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.className = "status-badge-copy-fallback";
  document.body.appendChild(area);
  area.select();
  const copied = document.execCommand("copy");
  area.remove();
  if (!copied) throw new Error("copy failed");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
