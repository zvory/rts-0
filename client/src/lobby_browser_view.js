// Lobby browser view helpers for the pre-join lobby screen.
// The Lobby controller owns networking and refreshes; this module owns row state and DOM rendering.

import { LOBBY_KIND } from "./protocol.js";

const JOIN_STATE_RANK = Object.freeze({
  open: 0,
  fullSpectatorOnly: 1,
  inGame: 2,
  starting: 3,
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
  inGame: "Spectate",
  stale: "Stale",
});

const PUBLIC_LOBBY_NAME_MAX_BYTES = 64;
const RESERVED_LOBBY_PREFIXES = Object.freeze([
  "__dev_scenario__:",
  "__replay_artifact__:",
  "__match_replay__",
  "__replay_branch__",
  "__lab__:",
]);
const DEFAULT_LOBBY_OWNER_NAME = "Commander";
const SUGGESTED_LOBBY_SUFFIX = "'s lobby";

export function normalizeLobbySummary(row = {}) {
  const joinState = normalizedJoinState(row.joinState);
  const kind = normalizedLobbyKind(row.kind);
  const room = boundedText(row.room, "Unnamed lobby");
  const maxSlots = Math.max(0, integerOr(row.maxSlots, 0));
  return {
    room,
    kind,
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
  const { state, kind } = normalizeStateAndKind(joinState);
  if (kind === LOBBY_KIND.REPLAY && (state === "open" || state === "fullSpectatorOnly" || state === "inGame")) {
    return "Replay";
  }
  return JOIN_STATE_LABEL[state] || JOIN_STATE_LABEL.stale;
}

export function lobbyActionLabel(joinState) {
  const { state, kind } = normalizeStateAndKind(joinState);
  if (kind === LOBBY_KIND.REPLAY && (state === "open" || state === "fullSpectatorOnly" || state === "inGame")) {
    return "Join replay";
  }
  return JOIN_STATE_ACTION[state] || JOIN_STATE_ACTION.stale;
}

export function lobbyJoinIntent(row = {}) {
  const state = normalizedJoinState(row?.joinState);
  const kind = normalizedLobbyKind(row?.kind);
  if (kind === LOBBY_KIND.REPLAY) {
    if (state === "open" || state === "fullSpectatorOnly" || state === "inGame") {
      return { state, joinable: true, spectator: true };
    }
    return { state, joinable: false, spectator: true };
  }
  if (state === "open") return { state, joinable: true, spectator: false };
  if (state === "fullSpectatorOnly") return { state, joinable: true, spectator: true };
  if (state === "inGame") return { state, joinable: true, spectator: true };
  return { state, joinable: false, spectator: false };
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

export function validateLobbyName(rawName) {
  const room = String(rawName ?? "").trim();
  if (!room) return { ok: false, room, error: "Lobby name is required." };
  if (utf8ByteLength(room) > PUBLIC_LOBBY_NAME_MAX_BYTES) {
    return { ok: false, room, error: "Lobby name is too long." };
  }
  if (/[\u0000-\u001f\u007f]/u.test(room)) {
    return { ok: false, room, error: "Lobby name contains unsupported characters." };
  }
  if (RESERVED_LOBBY_PREFIXES.some((prefix) => room.startsWith(prefix))) {
    return { ok: false, room, error: "Lobby name is reserved." };
  }
  return { ok: true, room, error: "" };
}

export function suggestLobbyName(playerName) {
  const ownerName = normalizeLobbyOwnerName(playerName) || DEFAULT_LOBBY_OWNER_NAME;
  const suggested = fittedSuggestedLobbyName(ownerName);
  if (validateLobbyName(suggested).ok) return suggested;
  return fittedSuggestedLobbyName(`${DEFAULT_LOBBY_OWNER_NAME} ${ownerName}`);
}

export class LobbyBrowserView {
  constructor(rootEl) {
    this.root = rootEl;
    this.rowsRoot = rootEl?.querySelector("#lobby-browser-rows") || null;
    this.statusEl = rootEl?.querySelector("#lobby-browser-status") || null;
    this.rows = [];
    this.onCreateLobby = null;
    this.onJoinLobby = null;
    this.actionsDisabled = false;
  }

  render({
    rows,
    loading = false,
    loaded = true,
    error = "",
    nowMs = Date.now(),
    actionsDisabled = false,
    onCreateLobby,
    onJoinLobby,
  } = {}) {
    if (!this.root || !this.rowsRoot) return;
    if (Array.isArray(rows)) this.rows = sortLobbySummaries(rows);
    if (onCreateLobby !== undefined) this.onCreateLobby = onCreateLobby;
    if (onJoinLobby !== undefined) this.onJoinLobby = onJoinLobby;
    this.actionsDisabled = !!actionsDisabled || !!error;
    this.root.classList.toggle("has-error", !!error);
    if (this.statusEl) {
      const text = statusText({ loading, loaded, error });
      this.statusEl.textContent = text;
      this.statusEl.hidden = !text;
    }
    if (loading && this.rows.length === 0) {
      this.rowsRoot.replaceChildren(this._buildStateRow("Loading lobbies..."));
      return;
    }
    if (!loaded && this.rows.length === 0) {
      this.rowsRoot.replaceChildren(this._buildStateRow("Refresh to load lobbies."));
      return;
    }
    if (this.rows.length === 0) {
      this.rowsRoot.replaceChildren(this._buildEmptyState(error));
      return;
    }
    this.rowsRoot.replaceChildren(
      ...this.rows.map((row) => this._buildRow(row, { nowMs })),
    );
  }

  destroy() {
    this.rows = [];
    this.onCreateLobby = null;
    this.onJoinLobby = null;
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
    action.disabled = this.actionsDisabled || !this.onCreateLobby;
    action.textContent = "Create lobby";
    if (!action.disabled) {
      action.addEventListener("click", () => this.onCreateLobby?.(action));
    }
    el.append(title, action);
    return el;
  }

  _buildRow(row, { nowMs }) {
    const intent = lobbyJoinIntent(row);
    const state = intent.state;
    const canJoin = !this.actionsDisabled && intent.joinable;
    const disabled = !canJoin || !this.onJoinLobby;
    const el = document.createElement("article");
    el.className = `lobby-browser-row is-${state}`;
    if (row.kind === LOBBY_KIND.REPLAY) el.classList.add("is-replay");
    el.dataset.joinState = state;
    el.dataset.kind = row.kind;
    if (state === "starting" || state === "stale") {
      el.classList.add("is-muted");
    }

    const lobby = document.createElement("div");
    lobby.className = "lobby-browser-cell lobby-browser-lobby";
    const name = document.createElement("strong");
    name.textContent = row.room;
    name.title = row.room;
    const status = document.createElement("span");
    status.className = `lobby-browser-status-chip is-${state}`;
    if (row.kind === LOBBY_KIND.REPLAY) status.classList.add("is-replay");
    status.textContent = lobbyStatusLabel({ joinState: state, kind: row.kind });
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
    button.textContent = lobbyActionLabel({ joinState: state, kind: row.kind });
    button.dataset.room = row.room;
    if (!button.disabled) {
      button.addEventListener("click", () => this.onJoinLobby?.(row, { spectator: intent.spectator }));
    }
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

export class LobbyCreateModal {
  constructor(hostEl, { onSubmit } = {}) {
    this.host = hostEl;
    this.onSubmit = typeof onSubmit === "function" ? onSubmit : null;
    this.root = null;
    this.input = null;
    this.errorEl = null;
    this.cancelButton = null;
    this.submitButton = null;
    this.returnFocus = null;
    this.pending = false;
    this.dirty = false;
    this._onKeydown = (ev) => this._handleKeydown(ev);
  }

  open(trigger = null, { initialValue = "" } = {}) {
    this._build();
    if (!this.root || !this.input) return;
    this.returnFocus = isHTMLElement(trigger) ? trigger : activeHTMLElement();
    this.input.value = String(initialValue ?? "");
    this.dirty = false;
    this.pending = false;
    this.setError("");
    this._syncSubmitState();
    this.root.hidden = false;
    document.addEventListener?.("keydown", this._onKeydown);
    defer(() => this.input?.focus());
  }

  close({ restoreFocus = true } = {}) {
    if (!this.root || this.root.hidden) return;
    this.root.hidden = true;
    this.pending = false;
    document.removeEventListener?.("keydown", this._onKeydown);
    const focusTarget = this.returnFocus;
    this.returnFocus = null;
    if (restoreFocus && isHTMLElement(focusTarget)) focusTarget.focus();
  }

  destroy() {
    document.removeEventListener?.("keydown", this._onKeydown);
    this.root?.remove();
    this.root = null;
    this.input = null;
    this.errorEl = null;
    this.cancelButton = null;
    this.submitButton = null;
    this.returnFocus = null;
  }

  setError(message) {
    if (!this.errorEl) return;
    this.errorEl.textContent = message || "";
    this.errorEl.hidden = !message;
  }

  setPending(pending) {
    this.pending = !!pending;
    this._syncSubmitState({ showError: false });
  }

  _build() {
    if (this.root || !this.host) return;

    const root = document.createElement("div");
    root.className = "lobby-alert lobby-create-modal";
    root.hidden = true;
    root.addEventListener("click", (ev) => {
      if (ev.target === root && !this.pending) this.close();
    });

    const dialog = document.createElement("div");
    dialog.className = "lobby-alert-box lobby-create-box";
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-labelledby", "lobby-create-title");
    dialog.setAttribute("aria-describedby", "lobby-create-error");

    const eyebrow = document.createElement("div");
    eyebrow.className = "lobby-alert-eyebrow";
    eyebrow.textContent = "New lobby";

    const title = document.createElement("h2");
    title.id = "lobby-create-title";
    title.textContent = "Create Lobby";

    const label = document.createElement("label");
    label.className = "lobby-create-field";
    label.textContent = "Lobby name";

    const input = document.createElement("input");
    input.id = "lobby-create-name";
    input.type = "text";
    input.maxLength = PUBLIC_LOBBY_NAME_MAX_BYTES;
    input.autocomplete = "off";
    input.addEventListener("input", () => {
      this.dirty = true;
      this._syncSubmitState();
    });
    input.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        this._submit();
      }
    });
    label.appendChild(input);

    const error = document.createElement("p");
    error.id = "lobby-create-error";
    error.className = "lobby-create-error";
    error.setAttribute("role", "alert");
    error.hidden = true;

    const actions = document.createElement("div");
    actions.className = "lobby-alert-actions";

    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "btn";
    cancel.textContent = "Cancel";
    cancel.addEventListener("click", () => {
      if (!this.pending) this.close();
    });

    const submit = document.createElement("button");
    submit.type = "button";
    submit.className = "btn primary";
    submit.textContent = "Create lobby";
    submit.addEventListener("click", () => this._submit());

    actions.append(cancel, submit);
    dialog.append(eyebrow, title, label, error, actions);
    root.appendChild(dialog);
    this.host.appendChild(root);

    this.root = root;
    this.input = input;
    this.errorEl = error;
    this.cancelButton = cancel;
    this.submitButton = submit;
  }

  _syncSubmitState({ showError = this.dirty } = {}) {
    if (!this.input || !this.submitButton) return;
    const result = validateLobbyName(this.input.value);
    this.submitButton.disabled = this.pending || !result.ok;
    this.input.setAttribute("aria-invalid", result.ok ? "false" : "true");
    if (showError && !result.ok) this.setError(result.error);
    if (result.ok && !this.pending && showError) this.setError("");
  }

  async _submit() {
    if (this.pending || !this.input) return;
    this.dirty = true;
    const result = validateLobbyName(this.input.value);
    if (!result.ok) {
      this._syncSubmitState({ showError: true });
      this.input.focus();
      return;
    }
    if (!this.onSubmit) return;
    this.setPending(true);
    this.setError("");
    const shouldClose = await this.onSubmit(result.room);
    if (shouldClose !== false) {
      this.close({ restoreFocus: false });
    } else {
      this.setPending(false);
      this.input?.focus();
    }
  }

  _handleKeydown(ev) {
    if (!this.root || this.root.hidden) return;
    if (ev.key === "Escape") {
      ev.preventDefault();
      if (!this.pending) this.close();
      return;
    }
    if (ev.key !== "Tab") return;

    const focusables = [this.input, this.cancelButton, this.submitButton]
      .filter((el) => el && !el.disabled);
    if (focusables.length === 0) return;
    const current = document.activeElement;
    const currentIndex = focusables.indexOf(current);
    const nextIndex = ev.shiftKey
      ? (currentIndex <= 0 ? focusables.length : currentIndex) - 1
      : (currentIndex + 1) % focusables.length;
    ev.preventDefault();
    focusables[nextIndex].focus();
  }
}

function statusText({ loading, loaded, error }) {
  if (error) return "Refresh failed";
  if (loading) return "Refreshing";
  if (!loaded) return "Not refreshed";
  return "";
}

function slotsLabel(row) {
  if (row.kind === LOBBY_KIND.REPLAY) {
    const count = Number(row.spectatorCount) || 0;
    return `${count} spectator${count === 1 ? "" : "s"}`;
  }
  const base = `${row.occupiedSlots} / ${row.maxSlots}`;
  return row.spectatorCount > 0 ? `${base} +${row.spectatorCount} obs` : base;
}

function normalizeStateAndKind(joinState) {
  if (joinState && typeof joinState === "object") {
    return {
      state: normalizedJoinState(joinState.joinState),
      kind: normalizedLobbyKind(joinState.kind),
    };
  }
  return {
    state: normalizedJoinState(joinState),
    kind: LOBBY_KIND.NORMAL,
  };
}

function normalizedJoinState(joinState) {
  const value = String(joinState || "").trim();
  return Object.prototype.hasOwnProperty.call(JOIN_STATE_RANK, value) ? value : "stale";
}

function normalizedLobbyKind(kind) {
  return kind === LOBBY_KIND.REPLAY ? LOBBY_KIND.REPLAY : LOBBY_KIND.NORMAL;
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

function normalizeLobbyOwnerName(value) {
  return String(value ?? "")
    .replace(/[\u0000-\u001f\u007f]/gu, "")
    .replace(/\s+/g, " ")
    .trim();
}

function fittedSuggestedLobbyName(ownerName) {
  const suffixBytes = utf8ByteLength(SUGGESTED_LOBBY_SUFFIX);
  const maxOwnerBytes = Math.max(0, PUBLIC_LOBBY_NAME_MAX_BYTES - suffixBytes);
  let fittedOwner = "";
  for (const char of ownerName) {
    const next = `${fittedOwner}${char}`;
    if (utf8ByteLength(next) > maxOwnerBytes) break;
    fittedOwner = next;
  }
  const candidate = `${(fittedOwner.trim() || DEFAULT_LOBBY_OWNER_NAME)}${SUGGESTED_LOBBY_SUFFIX}`;
  if (!RESERVED_LOBBY_PREFIXES.some((prefix) => candidate.startsWith(prefix))) return candidate;
  return fittedSuggestedLobbyName(`Player ${ownerName}`);
}

function utf8ByteLength(value) {
  if (typeof TextEncoder !== "undefined") return new TextEncoder().encode(value).length;
  return encodeURIComponent(value).replace(/%[0-9a-f]{2}/gi, "x").length;
}

function activeHTMLElement() {
  return isHTMLElement(document.activeElement) ? document.activeElement : null;
}

function defer(fn) {
  const timer = typeof window !== "undefined" && typeof window.setTimeout === "function"
    ? window.setTimeout.bind(window)
    : setTimeout;
  timer(fn, 0);
}

function isHTMLElement(value) {
  return typeof HTMLElement !== "undefined"
    ? value instanceof HTMLElement
    : !!value && typeof value.focus === "function";
}
