// Match history table on the lobby screen. Fetches resolved matches from `/api/matches` and
// renders the most recent entries. Rows expand to show the per-player score screen.
//
// The server is the source of truth. This module never writes; an empty response (no DB
// configured server-side) renders an empty state.

const DEFAULT_LIMIT = 20;

export class MatchHistory {
  /**
   * @param {HTMLElement} hostEl container element (kept; this module owns its inner DOM).
   * @param {{limit?: number, fetchImpl?: typeof fetch, onReplayRoom?: Function}} [opts]
   */
  constructor(hostEl, opts = {}) {
    this.host = hostEl;
    this.limit = opts.limit ?? DEFAULT_LIMIT;
    this.fetchImpl = opts.fetchImpl ?? window.fetch.bind(window);
    this.onReplayRoom = typeof opts.onReplayRoom === "function" ? opts.onReplayRoom : null;
    /** Currently-expanded row id (number) or null. */
    this._expandedId = null;
    /** Latest fetched rows kept for re-render on row expansion. */
    this._rows = [];
    /** AbortController for the in-flight request. */
    this._ac = null;
    /** Row id currently launching a replay, or null. */
    this._launchingId = null;
    /** Per-row replay launch errors. */
    this._launchErrors = new Map();
    this._loading = false;

    this._render();
    this.refresh();
  }

  /** Re-fetch and re-render. Safe to call multiple times. */
  async refresh() {
    if (this._ac) this._ac.abort();
    const controller = new AbortController();
    this._ac = controller;
    this._loading = true;
    this._setLoading();
    try {
      const url = `/api/matches?limit=${encodeURIComponent(this.limit)}`;
      const res = await this.fetchImpl(url, { signal: controller.signal });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const rows = await res.json();
      if (!Array.isArray(rows)) throw new Error("malformed response");
      if (this._ac !== controller) return;
      this._ac = null;
      this._rows = rows;
      this._loading = false;
      this._renderRows();
    } catch (err) {
      if (this._ac !== controller) return;
      this._ac = null;
      this._loading = false;
      if (err && err.name === "AbortError") {
        this._reflectRefreshButton();
        return;
      }
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
    this._refreshButton = document.createElement("button");
    this._refreshButton.type = "button";
    this._refreshButton.className = "btn match-history-refresh";
    this._refreshButton.textContent = "Refresh";
    this._refreshButton.addEventListener("click", () => void this.refresh());
    header.appendChild(title);
    header.appendChild(this._refreshButton);
    wrap.appendChild(header);

    this._tableHost = document.createElement("div");
    this._tableHost.className = "match-history-table-host";
    wrap.appendChild(this._tableHost);

    this.host.appendChild(wrap);
  }

  _setLoading() {
    this._reflectRefreshButton();
    this._tableHost.innerHTML = `<p class="match-history-status">Loading…</p>`;
  }

  _setError(msg) {
    this._reflectRefreshButton();
    const p = document.createElement("p");
    p.className = "match-history-status error";
    p.textContent = `Could not load match history (${msg}).`;
    this._tableHost.innerHTML = "";
    this._tableHost.appendChild(p);
  }

  _renderRows() {
    this._reflectRefreshButton();
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
        <th scope="col">Replay #</th>
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
      tr.appendChild(td(formatReplayNumber(row.replayNumber)));
      tr.appendChild(td(formatRelative(row.startedAt)));
      tr.appendChild(td(row.mapName || "—"));
      tr.appendChild(td((row.participants || []).join(", ")));
      tr.appendChild(td(matchHistoryWinnerLabel(row)));
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
        detailTd.colSpan = 6;
        detailTd.appendChild(this._renderDetail(row));
        detailTr.appendChild(detailTd);
        tbody.appendChild(detailTr);
      }
    }
    table.appendChild(tbody);
    this._tableHost.appendChild(table);
  }

  _reflectRefreshButton() {
    if (!this._refreshButton) return;
    this._refreshButton.disabled = this._loading;
    this._refreshButton.textContent = this._loading ? "Refreshing..." : "Refresh";
  }

  _toggleRow(id) {
    this._expandedId = this._expandedId === id ? null : id;
    this._renderRows();
  }

  _renderDetail(row) {
    const wrap = document.createElement("div");
    wrap.className = "match-history-detail-wrap";
    wrap.appendChild(this._renderReplayAction(row));
    wrap.appendChild(renderScoreScreen(row.scoreScreen));
    return wrap;
  }

  _renderReplayAction(row) {
    const action = document.createElement("div");
    action.className = "match-history-replay";

    if (row.replayAvailable) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "match-history-replay-button";
      btn.textContent = this._launchingId === row.id ? "Launching..." : "Watch replay";
      btn.disabled = this._launchingId === row.id;
      btn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        void this._launchReplay(row.id);
      });
      action.appendChild(btn);
    }
    if (row.replayUnavailableReason) {
      const reason = document.createElement("span");
      reason.className = "match-history-replay-reason";
      reason.textContent = row.replayUnavailableReason;
      action.appendChild(reason);
    }

    const launchError = this._launchErrors.get(row.id);
    if (launchError) {
      const error = document.createElement("span");
      error.className = "match-history-replay-error";
      error.textContent = launchError;
      action.appendChild(error);
    }

    return action;
  }

  async _launchReplay(id) {
    if (this._launchingId != null) return;
    this._launchingId = id;
    this._launchErrors.delete(id);
    this._renderRows();
    try {
      const room = await requestReplayRoom(id, this.fetchImpl);
      if (this.onReplayRoom) {
        const handled = await this.onReplayRoom(room, id);
        if (handled === false) throw new Error("Replay lobby could not be joined.");
        this._launchingId = null;
        this._renderRows();
        return;
      }
      const url = new URL("/", window.location.href);
      url.searchParams.set("replayRoom", room);
      window.location.assign(url.toString());
    } catch (err) {
      this._launchingId = null;
      this._launchErrors.set(
        id,
        err && err.message ? String(err.message) : "Replay could not be launched.",
      );
      this._renderRows();
    }
  }
}

export async function requestReplayRoom(id, fetchImpl = window.fetch.bind(window)) {
  const matchId = Number(id);
  if (!Number.isSafeInteger(matchId) || matchId <= 0) {
    throw new Error("Replay link has an invalid match id.");
  }
  const res = await fetchImpl(`/api/matches/${encodeURIComponent(matchId)}/replay`, {
    method: "POST",
  });
  if (!res.ok) throw new Error(await replayLaunchError(res));
  const payload = await res.json();
  const room = typeof payload?.room === "string" ? payload.room : "";
  if (!room) throw new Error("Replay launch did not return a room.");
  return room;
}

export function matchHistoryWinnerLabel(row) {
  if (row?.outcome === "aborted") return "Aborted";
  if (row?.winnerName) return row.winnerName;
  if (row?.outcome === "draw") return "Draw";
  return "—";
}

function td(text) {
  const el = document.createElement("td");
  el.textContent = text;
  return el;
}

function formatReplayNumber(replayNumber) {
  const value = Number(replayNumber);
  return Number.isSafeInteger(value) && value > 0 ? String(value) : "—";
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
      <th scope="col">APM</th>
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
    tr.appendChild(td(String(s.apm ?? 0)));
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

async function replayLaunchError(res) {
  try {
    const payload = await res.json();
    if (payload && typeof payload.error === "string" && payload.error.trim()) {
      return payload.error;
    }
  } catch {
    // Fall through to status text.
  }
  return `Replay could not be launched (HTTP ${res.status}).`;
}
