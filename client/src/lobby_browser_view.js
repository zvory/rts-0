// Lobby browser view helpers for the pre-join lobby screen.
// The Lobby controller owns networking and polling; this module owns row state and DOM rendering.

const JOIN_STATE_RANK = Object.freeze({
  open: 0,
  fullSpectatorOnly: 1,
  starting: 2,
  inGame: 3,
  stale: 4,
});

const JOIN_STATE_LABEL = Object.freeze({
  open: "Open",
  fullSpectatorOnly: "Full",
  starting: "Starting",
  inGame: "In match",
  stale: "Stale",
});

const JOIN_STATE_ACTION = Object.freeze({
  open: "Join lobby",
  fullSpectatorOnly: "Join as spectator",
  starting: "Starting",
  inGame: "In match",
  stale: "Stale",
});

export const LOBBY_BROWSER_POLL_MS = 1500;

export function normalizeLobbySummary(row = {}) {
  const joinState = normalizedJoinState(row.joinState);
  const room = boundedText(row.room, "Unnamed lobby");
  const maxSlots = Math.max(0, integerOr(row.maxSlots, 0));
  return {
    room,
    hostName: boundedText(row.hostName, "No host"),
    map: boundedText(row.map, "Default"),
    createdAtUnixMs: Math.max(0, integerOr(row.createdAtUnixMs, 0)),
    occupiedSlots: Math.max(0, integerOr(row.occupiedSlots, 0)),
    maxSlots,
    spectatorCount: Math.max(0, integerOr(row.spectatorCount, 0)),
    phase: boundedText(row.phase, "lobby"),
    joinState,
  };
}

export function sortLobbySummaries(rows = []) {
  return rows
    .map(normalizeLobbySummary)
    .sort((a, b) => {
      const rank = joinStateRank(a.joinState) - joinStateRank(b.joinState);
      if (rank !== 0) return rank;
      const age = b.createdAtUnixMs - a.createdAtUnixMs;
      if (age !== 0) return age;
      return a.room.localeCompare(b.room);
    });
}

export function lobbyStatusLabel(joinState) {
  return JOIN_STATE_LABEL[normalizedJoinState(joinState)] || JOIN_STATE_LABEL.stale;
}

export function lobbyActionLabel(joinState) {
  return JOIN_STATE_ACTION[normalizedJoinState(joinState)] || JOIN_STATE_ACTION.stale;
}

export function formatLobbyAge(createdAtUnixMs, nowMs = Date.now()) {
  const created = Number(createdAtUnixMs);
  const now = Number(nowMs);
  if (!Number.isFinite(created) || created <= 0 || !Number.isFinite(now)) return "-";
  const elapsedSeconds = Math.max(0, Math.floor((now - created) / 1000));
  if (elapsedSeconds < 60) return "just now";
  const minutes = Math.floor(elapsedSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

export class LobbyBrowserView {
  constructor(rootEl) {
    this.root = rootEl;
    this.rowsRoot = rootEl?.querySelector("#lobby-browser-rows") || null;
    this.statusEl = rootEl?.querySelector("#lobby-browser-status") || null;
    this.rows = [];
  }

  render({
    rows,
    loading = false,
    connected = true,
    error = "",
    nowMs = Date.now(),
  } = {}) {
    if (!this.root || !this.rowsRoot) return;
    if (Array.isArray(rows)) this.rows = sortLobbySummaries(rows);
    this.root.classList.toggle("is-disconnected", !connected);
    this.root.classList.toggle("has-error", !!error);
    if (this.statusEl) {
      this.statusEl.textContent = statusText({ loading, connected, error });
    }
    if (loading && this.rows.length === 0) {
      this.rowsRoot.replaceChildren(this._buildStateRow("Loading lobbies..."));
      return;
    }
    if (this.rows.length === 0) {
      this.rowsRoot.replaceChildren(this._buildEmptyState(error));
      return;
    }
    this.rowsRoot.replaceChildren(
      ...this.rows.map((row) => this._buildRow(row, { connected, nowMs })),
    );
  }

  destroy() {
    this.rows = [];
    if (this.rowsRoot) this.rowsRoot.replaceChildren();
    this.root = null;
    this.rowsRoot = null;
    this.statusEl = null;
  }

  _buildStateRow(text) {
    const el = document.createElement("div");
    el.className = "lobby-browser-state";
    el.setAttribute("role", "status");
    el.textContent = text;
    return el;
  }

  _buildEmptyState(error) {
    const el = document.createElement("div");
    el.className = "lobby-browser-empty";
    const title = document.createElement("strong");
    title.textContent = error ? "Lobby list unavailable" : "No lobbies";
    const action = document.createElement("button");
    action.type = "button";
    action.className = "btn primary";
    action.disabled = true;
    action.textContent = "Create lobby";
    el.append(title, action);
    return el;
  }

  _buildRow(row, { connected, nowMs }) {
    const disabled = true;
    const state = normalizedJoinState(row.joinState);
    const el = document.createElement("article");
    el.className = `lobby-browser-row is-${state}`;
    el.dataset.joinState = state;
    if (!connected || state === "inGame" || state === "starting" || state === "stale") {
      el.classList.add("is-muted");
    }

    const lobby = document.createElement("div");
    lobby.className = "lobby-browser-cell lobby-browser-lobby";
    const name = document.createElement("strong");
    name.textContent = row.room;
    name.title = row.room;
    const status = document.createElement("span");
    status.className = `lobby-browser-status-chip is-${state}`;
    status.textContent = lobbyStatusLabel(state);
    lobby.append(name, status);

    const host = this._buildMetaCell("Host", row.hostName, "host");
    const map = this._buildMetaCell("Map", row.map, "map");
    const made = this._buildMetaCell("Made", formatLobbyAge(row.createdAtUnixMs, nowMs), "made");
    made.title = row.createdAtUnixMs > 0 ? new Date(row.createdAtUnixMs).toLocaleString() : "";
    const slots = this._buildMetaCell("Slots", slotsLabel(row), "slots");
    const action = document.createElement("div");
    action.className = "lobby-browser-cell lobby-browser-action";
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn";
    button.disabled = disabled;
    button.textContent = lobbyActionLabel(state);
    action.appendChild(button);

    el.append(lobby, host, map, made, slots, action);
    return el;
  }

  _buildMetaCell(labelText, valueText, key) {
    const el = document.createElement("div");
    el.className = `lobby-browser-cell lobby-browser-${key}`;
    const label = document.createElement("span");
    label.className = "lobby-browser-mobile-label";
    label.textContent = labelText;
    const value = document.createElement("b");
    value.textContent = valueText;
    value.title = valueText;
    el.append(label, value);
    return el;
  }
}

function statusText({ loading, connected, error }) {
  if (!connected) return "Disconnected";
  if (error) return "Refresh failed";
  if (loading) return "Refreshing";
  return "Live";
}

function slotsLabel(row) {
  const base = `${row.occupiedSlots} / ${row.maxSlots}`;
  return row.spectatorCount > 0 ? `${base} +${row.spectatorCount} obs` : base;
}

function normalizedJoinState(joinState) {
  const value = String(joinState || "").trim();
  return Object.prototype.hasOwnProperty.call(JOIN_STATE_RANK, value) ? value : "stale";
}

function joinStateRank(joinState) {
  return JOIN_STATE_RANK[normalizedJoinState(joinState)] ?? JOIN_STATE_RANK.stale;
}

function integerOr(value, fallback) {
  const n = Number(value);
  return Number.isFinite(n) ? Math.trunc(n) : fallback;
}

function boundedText(value, fallback) {
  const text = String(value ?? "").trim();
  return text || fallback;
}
