// Match history table on the lobby screen. Fetches resolved matches from `/api/matches` and
// renders the most recent entries. Rows expand to show the per-player score screen.
//
// The server is the source of truth. This module never writes; an empty response (no DB
// configured server-side) renders an empty state.

const DEFAULT_LIMIT = 20;

export class MatchHistory {
  /**
   * @param {HTMLElement} hostEl container element (kept; this module owns its inner DOM).
   * @param {{limit?: number, fetchImpl?: typeof fetch}} [opts]
   */
  constructor(hostEl, opts = {}) {
    this.host = hostEl;
    this.limit = opts.limit ?? DEFAULT_LIMIT;
    this.fetchImpl = opts.fetchImpl ?? window.fetch.bind(window);
    /** Currently-expanded row id (number) or null. */
    this._expandedId = null;
    /** Latest fetched rows kept for re-render on row expansion. */
    this._rows = [];
    /** AbortController for the in-flight request. */
    this._ac = null;

    this._render();
    this.refresh();
  }

  /** Re-fetch and re-render. Safe to call multiple times. */
  async refresh() {
    if (this._ac) this._ac.abort();
    this._ac = new AbortController();
    this._setLoading();
    try {
      const url = `/api/matches?limit=${encodeURIComponent(this.limit)}`;
      const res = await this.fetchImpl(url, { signal: this._ac.signal });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const rows = await res.json();
      if (!Array.isArray(rows)) throw new Error("malformed response");
      this._rows = rows;
      this._renderRows();
    } catch (err) {
      if (err && err.name === "AbortError") return;
      this._setError(err && err.message ? String(err.message) : "Failed to load");
    }
  }

  destroy() {
    if (this._ac) this._ac.abort();
    this._ac = null;
    if (this.host) this.host.innerHTML = "";
    this.host = null;
  }

  // --- Rendering ------------------------------------------------------------

  _render() {
    this.host.innerHTML = "";
    const wrap = document.createElement("section");
    wrap.className = "match-history-panel";

    const header = document.createElement("div");
    header.className = "match-history-header";
    const title = document.createElement("h2");
    title.textContent = "Recent matches";
    header.appendChild(title);
    wrap.appendChild(header);

    this._tableHost = document.createElement("div");
    this._tableHost.className = "match-history-table-host";
    wrap.appendChild(this._tableHost);

    this.host.appendChild(wrap);
  }

  _setLoading() {
    this._tableHost.innerHTML = `<p class="match-history-status">Loading…</p>`;
  }

  _setError(msg) {
    const p = document.createElement("p");
    p.className = "match-history-status error";
    p.textContent = `Could not load match history (${msg}).`;
    this._tableHost.innerHTML = "";
    this._tableHost.appendChild(p);
  }

  _renderRows() {
    this._tableHost.innerHTML = "";
    if (this._rows.length === 0) {
      const p = document.createElement("p");
      p.className = "match-history-status";
      p.textContent = "No matches played yet.";
      this._tableHost.appendChild(p);
      return;
    }

    const table = document.createElement("table");
    table.className = "match-history-table";

    const thead = document.createElement("thead");
    thead.innerHTML = `
      <tr>
        <th scope="col">When</th>
        <th scope="col">Map</th>
        <th scope="col">Players</th>
        <th scope="col">Winner</th>
        <th scope="col">Length</th>
      </tr>`;
    table.appendChild(thead);

    const tbody = document.createElement("tbody");
    for (const row of this._rows) {
      const tr = document.createElement("tr");
      tr.className = "match-history-row";
      tr.dataset.id = String(row.id);
      tr.tabIndex = 0;
      tr.setAttribute("role", "button");
      tr.setAttribute("aria-expanded", this._expandedId === row.id ? "true" : "false");
      tr.appendChild(td(formatRelative(row.startedAt)));
      tr.appendChild(td(row.mapName || "—"));
      tr.appendChild(td((row.participants || []).join(", ")));
      tr.appendChild(td(row.winnerName ? row.winnerName : row.outcome === "draw" ? "Draw" : "—"));
      tr.appendChild(td(formatDuration(row.durationMs)));
      tr.addEventListener("click", () => this._toggleRow(row.id));
      tr.addEventListener("keydown", (ev) => {
        if (ev.key === "Enter" || ev.key === " ") {
          ev.preventDefault();
          this._toggleRow(row.id);
        }
      });
      tbody.appendChild(tr);

      if (this._expandedId === row.id) {
        const detailTr = document.createElement("tr");
        detailTr.className = "match-history-detail";
        const detailTd = document.createElement("td");
        detailTd.colSpan = 5;
        detailTd.appendChild(renderScoreScreen(row.scoreScreen));
        detailTr.appendChild(detailTd);
        tbody.appendChild(detailTr);
      }
    }
    table.appendChild(tbody);
    this._tableHost.appendChild(table);
  }

  _toggleRow(id) {
    this._expandedId = this._expandedId === id ? null : id;
    this._renderRows();
  }
}

function td(text) {
  const el = document.createElement("td");
  el.textContent = text;
  return el;
}

function formatDuration(ms) {
  if (typeof ms !== "number" || ms < 0) return "—";
  const totalSec = Math.floor(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatRelative(iso) {
  const t = Date.parse(iso);
  if (!Number.isFinite(t)) return "—";
  const diffSec = Math.max(0, (Date.now() - t) / 1000);
  if (diffSec < 60) return `${Math.floor(diffSec)}s ago`;
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return new Date(t).toLocaleDateString();
}

function renderScoreScreen(scores) {
  const wrap = document.createElement("div");
  wrap.className = "match-history-score";
  if (!Array.isArray(scores) || scores.length === 0) {
    wrap.textContent = "No score detail recorded.";
    return wrap;
  }
  const table = document.createElement("table");
  table.className = "match-history-score-table";
  table.innerHTML = `
    <thead><tr>
      <th scope="col">Player</th>
      <th scope="col">Units</th>
      <th scope="col">Structures</th>
      <th scope="col">Kills</th>
      <th scope="col">Lost</th>
      <th scope="col">Bldgs killed</th>
      <th scope="col">Bldgs lost</th>
    </tr></thead>`;
  const tbody = document.createElement("tbody");
  for (const s of scores) {
    const tr = document.createElement("tr");
    const nameTd = td(s.name || "—");
    if (s.color) {
      const swatch = document.createElement("span");
      swatch.className = "match-history-color";
      swatch.style.background = s.color;
      nameTd.prepend(swatch);
    }
    tr.appendChild(nameTd);
    tr.appendChild(td(String(s.unitScore ?? 0)));
    tr.appendChild(td(String(s.structureScore ?? 0)));
    tr.appendChild(td(String(s.unitsKilled ?? 0)));
    tr.appendChild(td(String(s.unitsLost ?? 0)));
    tr.appendChild(td(String(s.buildingsKilled ?? 0)));
    tr.appendChild(td(String(s.buildingsLost ?? 0)));
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  wrap.appendChild(table);
  return wrap;
}
