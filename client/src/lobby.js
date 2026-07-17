// Lobby — the pre-match screen (`#lobby-screen`): pre-join browser, identity entry, the
// player list, and ready/start controls. Talks to the server through `net` (join/ready/start)
// and renders `lobby` server messages. See docs/design/client-ui.md §4.1 (Lobby) and
// docs/design/protocol.md §2.2 (`lobby` payload).
//
// Screen transitions are NOT this module's job: it only toggles its own visibility via
// show()/hide(). main.js owns the lobby↔game switch and subscribes via `onGameStart(cb)`
// (fired when the server sends `start`). The entered name is persisted in localStorage.

import { LOBBY_KIND, S } from "./protocol.js";
import {
  LobbyBrowserView,
  LobbyCreateModal,
  lobbyJoinIntent,
  suggestLobbyName,
} from "./lobby_browser_view.js";
import {
  DEFAULT_AI_PROFILE_ID,
  MAX_LOBBY_TEAMS,
  LobbyRosterView,
  PLAYABLE_FACTIONS,
  shouldAcceptSpectatorDrop,
  shouldAcceptTeamDrop,
  splitLobbyPlayers,
  teamSlotsForLobby,
} from "./lobby_view.js";

const NAME_STORAGE_KEY = "rts.playerName";
const NAME_UPDATE_DEBOUNCE_MS = 250;
export const LOBBY_BROWSER_REFRESH_INTERVAL_MS = 5000;
export const LOBBY_BROWSER_ACTIVITY_WINDOW_MS = 30000;
const LOBBY_BROWSER_ACTIVITY_EVENTS = Object.freeze([
  "pointerdown",
  "pointermove",
  "click",
  "keydown",
  "scroll",
  "touchstart",
]);

const DEFAULT_MAX_PLAYERS = 4;
const COUNTDOWN_SOUND_BY_WORD = Object.freeze({
  "3": "countdown_drei",
  three: "countdown_drei",
  drei: "countdown_drei",
  "2": "countdown_zwei",
  two: "countdown_zwei",
  zwei: "countdown_zwei",
  zvei: "countdown_zwei",
  "1": "countdown_eins",
  one: "countdown_eins",
  eins: "countdown_eins",
});
const COUNTDOWN_SOUND_BY_INDEX = Object.freeze([
  "countdown_drei",
  "countdown_zwei",
  "countdown_eins",
]);

export {
  DEFAULT_AI_PROFILE_ID,
  MAX_LOBBY_TEAMS,
  PLAYABLE_FACTIONS,
  shouldAcceptSpectatorDrop,
  shouldAcceptTeamDrop,
  teamSlotsForLobby,
};

export function countdownSoundId(word, index = -1, total = 0) {
  const normalized = String(word || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "");
  if (COUNTDOWN_SOUND_BY_WORD[normalized]) return COUNTDOWN_SOUND_BY_WORD[normalized];
  if (total === COUNTDOWN_SOUND_BY_INDEX.length && index >= 0) {
    return COUNTDOWN_SOUND_BY_INDEX[index] || null;
  }
  return null;
}

export function betaFactionSelectEnabledForLocation(locationLike) {
  const host = String(locationLike?.hostname || "").toLowerCase();
  const path = String(locationLike?.pathname || "");
  return (
    host.includes("beta") ||
    path.startsWith("/beta") ||
    host === "localhost" ||
    host === "127.0.0.1" ||
    host === "0.0.0.0" ||
    host === "::1" ||
    host.endsWith(".localhost") ||
    host === ""
  );
}

export function lobbyBrowserAutoRefreshEligible({
  enabled = true,
  joined = false,
  actionPending = false,
  screenHidden = false,
  documentHidden = false,
  lastActivityAt = 0,
  now = Date.now(),
  activityWindowMs = LOBBY_BROWSER_ACTIVITY_WINDOW_MS,
} = {}) {
  const inactiveForMs = now - lastActivityAt;
  return !!enabled &&
    !joined &&
    !actionPending &&
    !screenHidden &&
    !documentHidden &&
    Number.isFinite(lastActivityAt) &&
    Number.isFinite(now) &&
    inactiveForMs >= 0 &&
    inactiveForMs <= activityWindowMs;
}

/**
 * The lobby screen controller.
 */
export class Lobby {
  /**
   * @param {HTMLElement} rootEl the `#lobby-screen` section.
   * @param {import("./net.js").Net} net network seam (join/ready/start + event bus).
   * @param {import("./audio.js").Audio|null} [audio] shared app audio engine.
   * @param {{ensureConnected?: Function, disconnectWhenIdle?: Function, autoRefreshLobbies?: boolean}} [options]
   */
  constructor(rootEl, net, audio = null, options = {}) {
    this.root = rootEl;
    this.net = net;
    this.audio = audio;

    // Form + room blocks.
    this.elName = rootEl.querySelector("#lobby-name");
    this.elRoom = rootEl.querySelector("#lobby-room");
    this.btnJoin = rootEl.querySelector("#lobby-join");
    this.btnCreateLobby = rootEl.querySelector("#lobby-create");
    this.btnRefreshLobbies = rootEl.querySelector("#lobby-browser-refresh");
    this.elSetupKicker = rootEl.querySelector("#lobby-setup-kicker");
    this.elSetupTitle = rootEl.querySelector("#lobby-setup-title");
    this.roomBlock = rootEl.querySelector(".lobby-room");
    this.elPlayers = rootEl.querySelector("#lobby-players");
    this.elRoomDisplay = rootEl.querySelector("#lobby-room-display");
    this.elMapSummary = rootEl.querySelector("#lobby-map-summary");
    this.elSeatsSummary = rootEl.querySelector("#lobby-seats-summary");
    this.elSeatsSummaryCell = this.elSeatsSummary?.parentElement || null;
    this.elObserversSummary = rootEl.querySelector("#lobby-observers-summary");
    this.btnReady = rootEl.querySelector("#lobby-ready");
    this.btnStart = rootEl.querySelector("#lobby-start");
    this.elStatus = rootEl.querySelector("#lobby-status");
    this.selMap = rootEl.querySelector("#lobby-map");
    this.rosterView = new LobbyRosterView(this.elPlayers);
    this.browserView = new LobbyBrowserView(rootEl.querySelector("#lobby-browser"));
    this.createModal = new LobbyCreateModal(rootEl, {
      onSubmit: (room) => this._submitCreateLobby(room),
    });

    // Local lobby state.
    this._joined = false;
    this._ready = false;
    this._spectator = false;
    this._hostId = null;
    this._canStart = false;
    this._roomKind = LOBBY_KIND.NORMAL;
    this._teamPreset = "custom";
    this._selectedMap = "";
    this._availableMaps = [];
    /** Total seated players (humans + AI) from the latest lobby message. */
    this._playerCount = 0;
    /** @type {Array<() => void>} subscribers for the server `start` message. */
    this._startCbs = [];
    /** @type {HTMLElement|null} large pre-match countdown overlay. */
    this._countdownEl = null;
    /** @type {number[]} active countdown timeout ids. */
    this._countdownTimers = [];
    this._countdownActive = false;
    /**
     * @type {{root: HTMLElement, title: HTMLElement, body: HTMLElement, cancel: HTMLButtonElement, confirm: HTMLButtonElement}|null}
     */
    this._replayPrompt = null;
    this._pendingReplayRoom = "";
    this._promptReturnFocus = null;
    this._browserAbort = null;
    this._browserLoading = false;
    this._browserLoaded = false;
    this._browserConnected = false;
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._browserAutoRefreshEnabled = options.autoRefreshLobbies !== false;
    this._browserActivityTracking = false;
    this._browserAutoRefreshTimer = undefined;
    this._nameUpdateTimer = undefined;
    this._lastSentName = "";
    this._lastBrowserActivityAt = 0;
    this._lastBrowserRefreshAt = 0;
    this._ensureConnection = typeof options.ensureConnected === "function"
      ? options.ensureConnected
      : async () => this._browserConnected;
    this._disconnectWhenIdle = typeof options.disconnectWhenIdle === "function"
      ? options.disconnectWhenIdle
      : () => {};
    this._fetchImpl =
      typeof window !== "undefined" && typeof window.fetch === "function"
        ? window.fetch.bind(window)
        : null;

    // Bound handlers kept so they can be removed in destroy().
    this._onLobby = (m) => this._renderLobby(m);
    this._onMatchCountdown = (m) => this._renderMatchCountdown(m);
    this._onStart = () => this._handleStart();
    this._onJoinReplayPrompt = (m) => this._handleJoinReplayPrompt(m);
    this._onError = (m) => this._handleServerError((m && m.msg) || "Error");
    this._onOpen = () => {
      this._browserConnected = true;
      this._renderLobbyBrowser();
      this._reflectCreateButton();
      this.setStatus("Connected.");
    };
    this._onClose = () => {
      this._browserConnected = false;
      if (this._joined || this._browserActionPending) {
        this._browserActionPending = false;
        this._renderLobbyBrowser({ error: "Disconnected." });
        this.setStatus("Disconnected from server.", true);
      } else {
        this._renderLobbyBrowser();
      }
      this._reflectCreateButton();
    };
    this._onReplayPromptKeydown = (ev) => this._handleReplayPromptKeydown(ev);
    this._onNameInput = () => this._scheduleNameUpdate();
    this._onNameChange = () => this._flushNameUpdate();
    this._onBrowserActivity = () => this._noteLobbyBrowserActivity();
    this._onBrowserVisibilityChange = () => {
      if (document.hidden) {
        this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
        return;
      }
      this._noteLobbyBrowserActivity({ refreshIfStale: true });
    };

    this._restoreName();
    this._reflectJoinedState();
    this._buildReplayPrompt();
    this._wireDom();
    this._wireNet();
    this._wireLobbyBrowserActivity();
    this._renderLobbyBrowser();
  }

  // --- Visibility ------------------------------------------------------------

  /** Show the lobby screen. */
  show() {
    const enteringBrowser = !this._browserActivityTracking || !!this.root.hidden;
    this.root.hidden = false;
    this._browserActivityTracking = true;
    this._renderLobbyBrowser();
    if (
      enteringBrowser &&
      this._browserAutoRefreshEnabled &&
      (typeof document === "undefined" || !document.hidden)
    ) {
      void this._refreshLobbyBrowser({ loading: !this._browserLoaded });
    }
  }

  /** Hide the lobby screen (main.js reveals the game screen). */
  hide() {
    this.root.hidden = true;
    this._browserActivityTracking = false;
    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
  }

  /**
   * Return the controller to the pre-join lobby browser state after leaving a room.
   * This is client-local UI state; the App/server own the actual room detach.
   */
  resetToBrowser({ status = "" } = {}) {
    this._browserActivityTracking = false;
    this._joined = false;
    this._ready = false;
    this._spectator = false;
    this._hostId = null;
    this._canStart = false;
    this._roomKind = LOBBY_KIND.NORMAL;
    this._teamPreset = "custom";
    this._selectedMap = "";
    this._availableMaps = [];
    this._playerCount = 0;
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._pendingReplayRoom = "";
    this._promptReturnFocus = null;

    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
    this._cancelNameUpdate();
    this._lastSentName = "";
    this._clearCountdown();
    this._hideReplayPrompt(false);
    if (this.elPlayers) this.elPlayers.innerHTML = "";
    this._reflectSummary("", []);
    this._reflectJoinedState(false);
    this._reflectReadyButton();
    this._reflectStartButton();
    this._reflectMap();
    this._reflectTeamPreset();
    this._renderLobbyBrowser({ error: "" });
    this.setStatus(status);
  }

  // --- Public hook -----------------------------------------------------------

  /**
   * Register a callback invoked when the server sends `start` (the match begins).
   * main.js uses this to construct the game and switch screens.
   * @param {() => void} cb
   */
  onGameStart(cb) {
    if (typeof cb === "function") this._startCbs.push(cb);
  }

  async joinReplayLobby(room) {
    const replayRoom = String(room || "").trim();
    if (!replayRoom || this._browserActionPending) return false;
    if (!await this._connectForAction()) return false;
    this._beginBrowserJoin(
      { room: replayRoom, kind: LOBBY_KIND.REPLAY },
      { spectator: true, replayLobby: true },
    );
    return true;
  }

  // --- DOM wiring ------------------------------------------------------------

  _wireDom() {
    // Join: send join, persist name, reveal the room block. The server confirms with a
    // `lobby` message which fills in the player list.
    this.btnJoin.addEventListener("click", () => this._join());
    this.btnCreateLobby?.addEventListener("click", () => this._openCreateLobby(this.btnCreateLobby));
    this.btnRefreshLobbies?.addEventListener("click", () => {
      void this.refreshLobbyBrowser();
    });
    this.elName?.addEventListener("input", this._onNameInput);
    this.elName?.addEventListener("change", this._onNameChange);
    // The hidden room field keeps legacy/dev joins available without exposing manual room entry.
    for (const el of [this.elRoom]) {
      if (!el) continue;
      el.addEventListener("keydown", (ev) => {
        if (ev.key === "Enter") {
          ev.preventDefault();
          this._join();
        }
      });
    }

    // Ready: toggle local ready and tell the server.
    this.btnReady.addEventListener("click", () => {
      if (this._spectator || this._isReplayLobby()) return;
      this._ready = !this._ready;
      this.net.ready(this._ready);
      this._reflectReadyButton();
    });

    // Start: host-only; the server ignores it from non-hosts but we also gate the UI.
    this.btnStart.addEventListener("click", () => {
      if (this.btnStart.disabled) return;
      this._flushNameUpdate();
      this.net.start();
    });

    // Map selector: host-only. Non-hosts see the selected map as a label.
    if (this.selMap) {
      this.selMap.addEventListener("change", () => {
        const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
        if (!isHost || this.selMap.disabled) return;
        this.net.selectMap(this.selMap.value);
      });
    }

  }

  async _join() {
    const name = (this.elName && this.elName.value.trim()) || "Commander";
    const room = (this.elRoom && this.elRoom.value.trim()) || "main";
    if (!await this._connectForAction()) return;
    this._sendJoin({ name, room, spectator: false });
    this._joined = true;
    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
    this._spectator = false;
    this._reflectJoinedState(false);
    this.setStatus(`Joining "${room}"…`);
    this._reflectReadyButton();
  }

  async _joinBrowserLobby(row, { preflight = true, spectator = false } = {}) {
    const room = String(row?.room || "").trim();
    if (!room || this._browserActionPending) return;
    const intent = lobbyJoinIntent(row);
    if (preflight && !intent.joinable) {
      this.setStatus(`Lobby "${room}" is not joinable.`, true);
      void this._refreshLobbyBrowser({ force: true });
      return;
    }
    if (!preflight) {
      if (!await this._connectForAction()) return;
      this._beginBrowserJoin(row, {
        spectator: !!spectator,
        replayOk: !!intent.replayOk,
      });
      return;
    }
    this._browserActionPending = true;
    this._pendingBrowserJoinRoom = room;
    this._renderLobbyBrowser();
    this._reflectCreateButton();
    const latestRows = await this._refreshLobbyBrowser({ force: true });
    if (this._joined || this.root.hidden) return;
    if (!Array.isArray(latestRows)) {
      this._cancelPendingBrowserJoin("Lobby list unavailable.", {
        rows: [],
        listError: "Lobby list unavailable.",
      });
      return;
    }
    const latestRow = latestRows.find((candidate) => String(candidate?.room || "").trim() === room);
    if (!latestRow) {
      this._cancelPendingBrowserJoin(`Lobby "${room}" is no longer available.`, { rows: latestRows });
      return;
    }
    const latestIntent = lobbyJoinIntent(latestRow);
    if (!latestIntent.joinable) {
      this._cancelPendingBrowserJoin(`Lobby "${room}" is no longer joinable.`, { rows: latestRows });
      return;
    }
    if (!await this._connectForAction()) {
      this._cancelPendingBrowserJoin("Server connection unavailable.", { rows: latestRows });
      return;
    }
    this._beginBrowserJoin(latestRow, {
      spectator: latestIntent.spectator,
      replayOk: !!latestIntent.replayOk,
    });
  }

  _beginBrowserJoin(row, { spectator = false, replayLobby = false, replayOk = false } = {}) {
    const room = String(row?.room || "").trim();
    if (!room) return;
    const name = (this.elName && this.elName.value.trim()) || "Commander";
    this._browserActionPending = true;
    this._pendingBrowserJoinRoom = room;
    this._spectator = !!spectator;
    if (this.elRoom) this.elRoom.value = room;
    this._sendJoin({ name, room, spectator: !!spectator, replayOk: !!replayOk });
    const isReplay = replayLobby || row?.kind === LOBBY_KIND.REPLAY;
    this.setStatus(isReplay
      ? `Joining replay lobby "${room}"...`
      : `Joining "${room}"${spectator ? " as spectator" : ""}...`);
    this._renderLobbyBrowser();
    this._reflectReadyButton();
    this._reflectCreateButton();
  }

  _cancelPendingBrowserJoin(message, { rows, listError = "" } = {}) {
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._spectator = false;
    this._renderLobbyBrowser({ rows: Array.isArray(rows) ? rows : undefined, error: listError });
    this.setStatus(message, true);
    this._reflectReadyButton();
    this._reflectCreateButton();
  }

  _sendJoin({ name, room, spectator, replayOk = false }) {
    this._cancelNameUpdate();
    this._lastSentName = name;
    this._persistName(name);
    this.net.join(name, room, spectator, replayOk);
  }

  _openCreateLobby(trigger) {
    if (this._joined || this._browserActionPending) return;
    this.createModal?.open(trigger, {
      initialValue: suggestLobbyName(this.elName?.value, this.browserView?.rows),
    });
  }

  async _submitCreateLobby(room) {
    if (!this._fetchImpl) {
      this.createModal?.setError("Network disconnected.");
      return false;
    }
    this._browserActionPending = true;
    this._renderLobbyBrowser();
    this._reflectCreateButton();
    if (!await this._connectForAction({ reportError: false })) {
      this._browserActionPending = false;
      this.createModal?.setError("Server connection unavailable.");
      this._renderLobbyBrowser();
      this._reflectCreateButton();
      return false;
    }
    try {
      const response = await this._fetchImpl("/api/lobbies", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ room }),
      });
      if (!response?.ok) {
        this._browserActionPending = false;
        this.createModal?.setError(await readLobbyApiError(response));
        await this._refreshLobbyBrowser({ force: true });
        this._disconnectWhenIdle();
        return false;
      }
      const payload = await response.json().catch(() => ({}));
      const createdRoom = String(payload?.room || room).trim() || room;
      // The socket can close while the HTTP reservation is in flight. Recheck
      // it before sending the join so a successful create cannot leave the UI
      // waiting forever on a message that was never sent.
      if (!await this._connectForAction({ reportError: false })) {
        this._browserActionPending = false;
        this.createModal?.setError("Lobby was created, but the server connection was lost. Try joining it again.");
        this._renderLobbyBrowser();
        this._reflectCreateButton();
        return false;
      }
      this._beginBrowserJoin({ room: createdRoom }, { spectator: false });
      return true;
    } catch (_) {
      this._browserActionPending = false;
      this.createModal?.setError("Network disconnected.");
      await this._refreshLobbyBrowser({ force: true });
      this._disconnectWhenIdle();
      return false;
    }
  }

  // --- Net wiring ------------------------------------------------------------

  _wireNet() {
    this.net.on(S.LOBBY, this._onLobby);
    this.net.on(S.MATCH_COUNTDOWN, this._onMatchCountdown);
    this.net.on(S.START, this._onStart);
    this.net.on(S.JOIN_REPLAY_PROMPT, this._onJoinReplayPrompt);
    this.net.on(S.ERROR, this._onError);
    this.net.on("open", this._onOpen);
    this.net.on("close", this._onClose);
  }

  _wireLobbyBrowserActivity() {
    if (!this._browserAutoRefreshEnabled || typeof document === "undefined") return;
    for (const eventName of LOBBY_BROWSER_ACTIVITY_EVENTS) {
      document.addEventListener(eventName, this._onBrowserActivity, { passive: true });
    }
    document.addEventListener("visibilitychange", this._onBrowserVisibilityChange);
  }

  /** Tear down listeners (not normally needed for a single-screen lifetime). */
  destroy() {
    this.net.off(S.LOBBY, this._onLobby);
    this.net.off(S.MATCH_COUNTDOWN, this._onMatchCountdown);
    this.net.off(S.START, this._onStart);
    this.net.off(S.JOIN_REPLAY_PROMPT, this._onJoinReplayPrompt);
    this.net.off(S.ERROR, this._onError);
    this.net.off("open", this._onOpen);
    this.net.off("close", this._onClose);
    this.elName?.removeEventListener("input", this._onNameInput);
    this.elName?.removeEventListener("change", this._onNameChange);
    if (this._browserAutoRefreshEnabled && typeof document !== "undefined") {
      for (const eventName of LOBBY_BROWSER_ACTIVITY_EVENTS) {
        document.removeEventListener(eventName, this._onBrowserActivity);
      }
      document.removeEventListener("visibilitychange", this._onBrowserVisibilityChange);
    }
    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
    this._cancelNameUpdate();
    this.browserView?.destroy();
    this.createModal?.destroy();
    this._clearCountdown();
    this._hideReplayPrompt(false);
    this._replayPrompt?.root.remove();
    this._replayPrompt = null;
  }

  // --- Rendering -------------------------------------------------------------

  /**
   * Render a `lobby` server message (§2.2): room, hostId, players[], canStart.
   * @param {{room:string,hostId:number,players:Array,canStart:boolean}} m
   */
  _renderLobby(m) {
    if (!m) return;
    this._hostId = m.hostId;
    this._canStart = !!m.canStart;
    this._roomKind = normalizeLobbyKind(m.kind);
    this._teamPreset = m.teamPreset || "custom";
    this._selectedMap = m.map || "";
    this._availableMaps = Array.isArray(m.maps) ? m.maps : [];

    // Once a lobby arrives we are definitively joined; make sure the room block shows.
    this._joined = true;
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
    this._reflectJoinedState(true);

    const players = m.players || [];
    this._playerCount = players.filter((p) => !p.isSpectator).length;
    this._reflectSummary(m.room, players);
    this._renderPlayers(players);
    this._reflectStartButton();
    this._reflectMap();
    this._reflectTeamPreset();

    this.setStatus("");
  }

  /** Rebuild the player list: color swatch, name, (host) tag, ready check. */
  _renderPlayers(players) {
    const ul = this.elPlayers;
    if (!ul) return;
    ul.innerHTML = "";

    const myId = this.net.playerId;
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    const mine = players.find((player) => player.id === myId);
    if (mine) {
      this._ready = !!mine.ready;
      this._spectator = !!mine.isSpectator;
    }
    this._reflectReadyButton();
    this.rosterView.render({
      players,
      myId,
      hostId: this._hostId,
      isHost,
      countdownActive: this._countdownActive,
      spectatorOnly: this._isReplayLobby(),
      playerCount: this._playerCount,
      maxPlayers: this._selectedMapMaxPlayers(),
      betaFactionSelect: this._betaFactionSelectEnabled(),
      onAddAi: (teamId) => this.net.addAi(teamId, DEFAULT_AI_PROFILE_ID),
      onRemoveAi: (id) => this.net.removeAi(id),
      onSetAiProfile: (id, aiProfileId) => this.net.setAiProfile(id, aiProfileId),
      onSetTeam: (id, teamId) => this.net.setTeam(id, teamId),
      onSetSpectator: (id, spectator) => this.net.setSpectator(spectator, id),
      onSetFaction: (factionId) => this.net.setFaction(factionId),
    });
  }

  _betaFactionSelectEnabled() {
    return betaFactionSelectEnabledForLocation(window.location);
  }

  _selectedMapMaxPlayers() {
    return mapMaxPlayers(this._availableMaps.find((entry) => entry.name === this._selectedMap));
  }

  /** Render the map selector in the summary row for hosts, or the map name for non-hosts. */
  _reflectMap() {
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    const entry = this._availableMaps.find((e) => e.name === this._selectedMap);
    const label = entry ? entry.name : (this._selectedMap || "Chokes");
    if (this._isReplayLobby()) {
      if (this.selMap) {
        this.selMap.disabled = true;
        this.selMap.hidden = true;
      }
      if (this.elMapSummary) {
        this.elMapSummary.textContent = label;
        this.elMapSummary.hidden = false;
      }
      return;
    }
    if (this.selMap) {
      // Rebuild the option list only when the available maps have changed.
      // Each entry is {name, description, minPlayers, maxPlayers}; name is the stable key.
      const currentOptions = Array.from(this.selMap.options).map((o) => o.value);
      const mapsChanged =
        currentOptions.length !== this._availableMaps.length ||
        currentOptions.some((v, i) => v !== this._availableMaps[i].name);
      if (mapsChanged) {
        this.selMap.innerHTML = "";
        for (const entry of this._availableMaps) {
          const opt = document.createElement("option");
          opt.value = entry.name;
          opt.textContent = entry.name;
          this.selMap.appendChild(opt);
        }
      }
      this.selMap.value = this._selectedMap;
      this.selMap.disabled = this._countdownActive || !isHost;
      this.selMap.hidden = !isHost;
    }
    if (this.elMapSummary) {
      this.elMapSummary.textContent = label;
      this.elMapSummary.hidden = isHost;
    }
  }

  /** Enable Start only for the host and only when the server says the match can start. */
  _reflectStartButton() {
    if (!this.btnStart) return;
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    this.btnStart.disabled = this._countdownActive || !(isHost && this._canStart);
    this.btnStart.classList.toggle("host-only", isHost);
    this.btnStart.textContent = this._isReplayLobby() ? "Start replay" : "Start match";
  }

  /** Reflect the local ready state on the Ready button (label + pressed style). */
  _reflectReadyButton() {
    if (!this.btnReady) return;
    this.btnReady.hidden = this._isReplayLobby();
    this.btnReady.textContent = this._ready ? "Unready" : "Ready";
    if (this._spectator) this.btnReady.textContent = "Observing";
    this.btnReady.disabled = this._countdownActive || this._spectator;
    this.btnReady.classList.toggle("active", this._ready);
    this.btnReady.setAttribute("aria-pressed", this._ready ? "true" : "false");
  }

  // --- Status / errors -------------------------------------------------------

  /**
   * Display a status or error line in `#lobby-status`.
   * @param {string} text
   * @param {boolean} [isError=false] color it as an error.
   */
  setStatus(text, isError = false) {
    if (!this.elStatus) return;
    this.elStatus.textContent = text || "";
    this.elStatus.classList.toggle("error", !!isError);
  }

  _handleServerError(message) {
    this.setStatus(message, true);
    if (!this._browserActionPending) return;
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._joined = false;
    this._spectator = false;
    this._reflectJoinedState(false);
    this._reflectReadyButton();
    this._reflectCreateButton();
    void this._refreshLobbyBrowser({ force: true });
    this._disconnectWhenIdle();
  }

  _reflectCreateButton() {
    if (!this.btnCreateLobby) return;
    const busy = this._browserActionPending;
    this.btnCreateLobby.hidden = this._joined;
    this.btnCreateLobby.disabled = this._joined || busy || !this._fetchImpl;
  }

  _reflectRefreshButton() {
    if (!this.btnRefreshLobbies) return;
    this.btnRefreshLobbies.hidden = this._joined;
    this.btnRefreshLobbies.disabled =
      this._joined || this._browserLoading || this._browserActionPending;
    this.btnRefreshLobbies.textContent = this._browserLoading ? "Refreshing..." : "Refresh";
  }

  _browserAutoRefreshIsEligible(now = Date.now()) {
    return lobbyBrowserAutoRefreshEligible({
      enabled: this._browserAutoRefreshEnabled,
      joined: this._joined,
      actionPending: this._browserActionPending,
      screenHidden: !this._browserActivityTracking || !!this.root?.hidden,
      documentHidden: typeof document !== "undefined" && !!document.hidden,
      lastActivityAt: this._lastBrowserActivityAt,
      now,
    });
  }

  _noteLobbyBrowserActivity({ refreshIfStale = false } = {}) {
    if (
      !this._browserAutoRefreshEnabled ||
      !this._browserActivityTracking ||
      this._joined ||
      this.root?.hidden
    ) return;
    if (typeof document !== "undefined" && document.hidden) return;
    const now = Date.now();
    this._lastBrowserActivityAt = now;
    this._startLobbyBrowserAutoRefresh();
    if (
      !this._browserLoaded ||
      (refreshIfStale && now - this._lastBrowserRefreshAt >= LOBBY_BROWSER_REFRESH_INTERVAL_MS)
    ) {
      void this._refreshLobbyBrowser({ loading: !this._browserLoaded });
    }
  }

  _startLobbyBrowserAutoRefresh() {
    if (!this._browserAutoRefreshIsEligible()) return;
    if (this._browserAutoRefreshTimer !== undefined || typeof window === "undefined") return;
    this._browserAutoRefreshTimer = window.setInterval(() => {
      const now = Date.now();
      if (!this._browserAutoRefreshIsEligible(now)) {
        if (!this._browserActionPending) {
          this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
        }
        return;
      }
      if (now - this._lastBrowserRefreshAt >= LOBBY_BROWSER_REFRESH_INTERVAL_MS) {
        void this._refreshLobbyBrowser();
      }
    }, LOBBY_BROWSER_REFRESH_INTERVAL_MS);
  }

  _stopLobbyBrowserAutoRefresh({ cancelRequest = false } = {}) {
    if (this._browserAutoRefreshTimer !== undefined && typeof window !== "undefined") {
      window.clearInterval(this._browserAutoRefreshTimer);
    }
    this._browserAutoRefreshTimer = undefined;
    if (cancelRequest) this._cancelLobbyBrowserRefresh();
  }

  async _connectForAction({ reportError = true } = {}) {
    if (this._browserConnected) return true;
    this.setStatus("Connecting to server...");
    try {
      await this._ensureConnection();
      if (this._browserConnected) return true;
      throw new Error("Connection did not open");
    } catch (_) {
      if (reportError) this.setStatus("Server connection unavailable.", true);
      return false;
    }
  }

  _cancelLobbyBrowserRefresh() {
    this._browserAbort?.abort();
    this._browserAbort = null;
    this._browserLoading = false;
    this._reflectRefreshButton();
  }

  refreshLobbyBrowser() {
    return this._refreshLobbyBrowser({ loading: true, force: true });
  }

  async _refreshLobbyBrowser({ loading = false, force = false } = {}) {
    if (this._joined || this.root.hidden || !this.browserView || !this._fetchImpl) return;
    if (this._browserLoading) {
      if (!force) return;
      this._browserAbort?.abort();
    }
    this._browserLoading = true;
    this._lastBrowserRefreshAt = Date.now();
    this._renderLobbyBrowser({ loading, error: "" });
    this._reflectRefreshButton();
    const controller = typeof AbortController !== "undefined" ? new AbortController() : null;
    this._browserAbort = controller;
    try {
      const response = await this._fetchImpl("/api/lobbies", {
        cache: "no-store",
        signal: controller?.signal,
      });
      if (!response?.ok) {
        throw new Error(`Lobby browser request failed (${response?.status || "network"})`);
      }
      const rows = await response.json();
      if (controller && this._browserAbort !== controller) return null;
      if (this._joined || this.root.hidden) {
        this._browserLoading = false;
        return null;
      }
      this._browserLoading = false;
      this._browserAbort = null;
      this._browserLoaded = true;
      const normalizedRows = Array.isArray(rows) ? rows : [];
      this._renderLobbyBrowser({
        rows: normalizedRows,
        error: "",
      });
      this._reflectRefreshButton();
      return normalizedRows;
    } catch (err) {
      if (controller && this._browserAbort !== controller) return null;
      if (err?.name === "AbortError") {
        this._browserLoading = false;
        this._browserAbort = null;
        this._reflectRefreshButton();
        return null;
      }
      this._browserLoading = false;
      this._browserAbort = null;
      this._browserLoaded = true;
      this._renderLobbyBrowser({ error: "Lobby list unavailable." });
      this._reflectRefreshButton();
      return null;
    }
  }

  _renderLobbyBrowser({ rows, loading = false, error = "" } = {}) {
    this.browserView?.render({
      rows,
      loading,
      loaded: this._browserLoaded,
      error,
      nowMs: Date.now(),
      actionsDisabled: this._browserActionPending,
      onCreateLobby: (trigger) => this._openCreateLobby(trigger),
      onJoinLobby: (row, options) => this._joinBrowserLobby(row, options),
    });
    this._reflectRefreshButton();
  }

  _reflectSummary(room, players) {
    const { seatedPlayers, spectatorPlayers } = splitLobbyPlayers(players);
    const mapEntry = this._availableMaps.find((entry) => entry.name === this._selectedMap);
    const mapLabel = mapEntry ? mapEntry.name : (this._selectedMap || "Chokes");
    if (this.elRoomDisplay) this.elRoomDisplay.textContent = room || "main";
    if (this.elMapSummary) this.elMapSummary.textContent = mapLabel;
    if (this.elSeatsSummary) this.elSeatsSummary.textContent = this._isReplayLobby()
      ? ""
      : `${seatedPlayers.length} / ${this._selectedMapMaxPlayers()}`;
    if (this.elSeatsSummaryCell) this.elSeatsSummaryCell.hidden = this._isReplayLobby();
    if (this.elObserversSummary) this.elObserversSummary.textContent = String(spectatorPlayers.length);
  }

  // --- Start handoff ---------------------------------------------------------

  /** The server signaled match start: fire subscribers (main.js switches screens). */
  _handleStart() {
    this._cancelNameUpdate();
    this._clearCountdown();
    this._hideReplayPrompt(false);
    // Active replay rows transition directly from the browser to `start`, without an
    // intermediate lobby payload. Treat `start` as the authoritative join completion so the
    // hidden lobby does not remain action-pending or keep its refresh timer alive during play.
    this._joined = true;
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
    for (const cb of this._startCbs) {
      try {
        cb();
      } catch (err) {
        // A faulty subscriber must not break the others or the lobby.
      }
    }
  }

  _renderMatchCountdown(m) {
    const words = Array.isArray(m?.words) && m.words.length
      ? m.words.map((word) => String(word))
      : ["Drei!", "Zwei!", "Eins!"];
    const durationMs = Math.max(1000, Number(m?.durationMs) || words.length * 1000);
    const wordMs = Math.max(250, durationMs / words.length);

    this._clearCountdown();
    this._countdownActive = true;
    this._reflectReadyButton();
    this._reflectStartButton();
    this._reflectMap();
    this._reflectTeamPreset();
    this.setStatus("Match starting...");

    const overlay = document.createElement("div");
    overlay.className = "match-countdown";
    overlay.setAttribute("role", "status");
    overlay.setAttribute("aria-live", "assertive");
    this._countdownEl = overlay;
    this.root.appendChild(overlay);

    const showWord = (word, index) => {
      if (!this._countdownEl) return;
      this._countdownEl.textContent = word;
      this._countdownEl.classList.remove("pulse");
      // Restart the animation for each word.
      void this._countdownEl.offsetWidth;
      this._countdownEl.classList.add("pulse");
      this._playCountdownWord(word, index, words.length);
    };

    words.forEach((word, index) => {
      const delay = Math.round(index * wordMs);
      if (delay <= 0) {
        showWord(word, index);
      } else {
        this._countdownTimers.push(window.setTimeout(() => showWord(word, index), delay));
      }
    });
    this._countdownTimers.push(window.setTimeout(() => this._clearCountdown(), durationMs + 1000));
  }

  _playCountdownWord(word, index, total) {
    const id = countdownSoundId(word, index, total);
    if (!id || !this.audio) return;
    this.audio.playUI(id, {
      priority: 8,
      pitchVariance: 0,
      dedupKey: `match-countdown:${index}`,
    });
  }

  _clearCountdown() {
    for (const timer of this._countdownTimers) window.clearTimeout(timer);
    this._countdownTimers = [];
    this._countdownActive = false;
    if (this._countdownEl) {
      this._countdownEl.remove();
      this._countdownEl = null;
    }
    this._reflectReadyButton();
    this._reflectStartButton();
    this._reflectMap();
    this._reflectTeamPreset();
  }

  /** Hide the deprecated team preset controls if older markup is present. */
  _reflectTeamPreset() {
    const teamRow = this.root.querySelector(".lobby-team-row");
    if (teamRow) teamRow.hidden = true;
  }

  _handleJoinReplayPrompt(m) {
    const room = (m?.room || "").trim() || ((this.elRoom && this.elRoom.value.trim()) || "main");
    this._joined = false;
    this._browserActionPending = false;
    this._pendingBrowserJoinRoom = "";
    this._reflectJoinedState(false);
    this.setStatus(`Room "${room}" is watching a replay.`, true);
    this._showReplayPrompt(room);
  }

  _joinReplayRoom(room) {
    const name = (this.elName && this.elName.value.trim()) || "Commander";
    if (this.elRoom) this.elRoom.value = room;
    this._sendJoin({ name, room, spectator: true, replayOk: true });
    this._joined = true;
    this._stopLobbyBrowserAutoRefresh({ cancelRequest: true });
    this._spectator = true;
    this._reflectJoinedState(false);
    this.setStatus(`Joining replay in "${room}"...`);
    this._reflectReadyButton();
  }

  _buildReplayPrompt() {
    if (this._replayPrompt) return;

    const root = document.createElement("div");
    root.className = "lobby-alert";
    root.hidden = true;
    root.addEventListener("click", (ev) => {
      if (ev.target === root) this._hideReplayPrompt(true);
    });

    const dialog = document.createElement("div");
    dialog.className = "lobby-alert-box";
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-labelledby", "replay-join-title");
    dialog.setAttribute("aria-describedby", "replay-join-body");

    const eyebrow = document.createElement("div");
    eyebrow.className = "lobby-alert-eyebrow";
    eyebrow.textContent = "Replay channel";

    const title = document.createElement("h2");
    title.id = "replay-join-title";

    const body = document.createElement("p");
    body.id = "replay-join-body";

    const actions = document.createElement("div");
    actions.className = "lobby-alert-actions";

    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "btn";
    cancel.textContent = "Stand down";
    cancel.addEventListener("click", () => this._hideReplayPrompt(true));

    const confirm = document.createElement("button");
    confirm.type = "button";
    confirm.className = "btn primary";
    confirm.textContent = "Join as spectator";
    confirm.addEventListener("click", () => {
      const room = this._pendingReplayRoom || "main";
      this._hideReplayPrompt(false);
      this._joinReplayRoom(room);
    });

    actions.append(cancel, confirm);
    dialog.append(eyebrow, title, body, actions);
    root.appendChild(dialog);
    this.root.appendChild(root);
    this._replayPrompt = { root, title, body, cancel, confirm };
  }

  _showReplayPrompt(room) {
    if (!this._replayPrompt) this._buildReplayPrompt();
    if (!this._replayPrompt) return;
    this._pendingReplayRoom = room;
    this._promptReturnFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : this.btnJoin;
    this._replayPrompt.title.textContent = `Join replay in "${room}"?`;
    this._replayPrompt.body.textContent =
      "This room is already playing back a finished battle. Joining will place you in observer mode without changing the room for current viewers.";
    this._replayPrompt.root.hidden = false;
    document.addEventListener("keydown", this._onReplayPromptKeydown);
    window.setTimeout(() => this._replayPrompt?.confirm.focus(), 0);
  }

  _hideReplayPrompt(restoreFocus) {
    if (!this._replayPrompt || this._replayPrompt.root.hidden) return;
    this._replayPrompt.root.hidden = true;
    document.removeEventListener("keydown", this._onReplayPromptKeydown);
    const returnFocus = this._promptReturnFocus;
    this._promptReturnFocus = null;
    this._pendingReplayRoom = "";
    if (restoreFocus && returnFocus instanceof HTMLElement) returnFocus.focus();
  }

  _handleReplayPromptKeydown(ev) {
    if (!this._replayPrompt || this._replayPrompt.root.hidden) return;
    if (ev.key === "Escape") {
      ev.preventDefault();
      this._hideReplayPrompt(true);
      return;
    }
    if (ev.key !== "Tab") return;

    const focusables = [this._replayPrompt.cancel, this._replayPrompt.confirm];
    const current = document.activeElement;
    const currentIndex = focusables.indexOf(current);
    const nextIndex = ev.shiftKey
      ? (currentIndex <= 0 ? focusables.length : currentIndex) - 1
      : (currentIndex + 1) % focusables.length;
    ev.preventDefault();
    focusables[nextIndex].focus();
  }

  // --- Name persistence ------------------------------------------------------

  _restoreName() {
    if (!this.elName) return;
    try {
      const saved = window.localStorage.getItem(NAME_STORAGE_KEY);
      if (saved && !this.elName.value) this.elName.value = saved;
    } catch (_) {
      // localStorage may be unavailable (private mode); ignore.
    }
  }

  _persistName(name) {
    try {
      window.localStorage.setItem(NAME_STORAGE_KEY, name);
    } catch (_) {
      // Ignore storage failures.
    }
  }

  _scheduleNameUpdate() {
    if (!this._joined || typeof window === "undefined") return;
    this._cancelNameUpdate();
    this._nameUpdateTimer = window.setTimeout(
      () => this._flushNameUpdate(),
      NAME_UPDATE_DEBOUNCE_MS,
    );
  }

  _flushNameUpdate() {
    this._cancelNameUpdate();
    if (!this._joined) return;
    const name = (this.elName?.value.trim()) || "Commander";
    this._persistName(name);
    if (name === this._lastSentName) return;
    this._lastSentName = name;
    this.net.setName(name);
  }

  _cancelNameUpdate() {
    if (this._nameUpdateTimer !== undefined && typeof window !== "undefined") {
      window.clearTimeout(this._nameUpdateTimer);
    }
    this._nameUpdateTimer = undefined;
  }

  _reflectJoinedState(hasLobby = this._joined && this._hostId != null) {
    this.root.classList.toggle("is-joined", !!hasLobby);
    this.root.classList.toggle("is-joining", this._joined && !hasLobby);
    this.root.classList.toggle("is-replay-lobby", !!hasLobby && this._isReplayLobby());
    if (this.roomBlock) this.roomBlock.hidden = !hasLobby;
    if (this.elSetupKicker) {
      this.elSetupKicker.textContent = hasLobby
        ? (this._isReplayLobby() ? "Group replay" : "Host controls")
        : "Commander";
    }
    if (this.elSetupTitle) {
      this.elSetupTitle.textContent = hasLobby
        ? (this._isReplayLobby() ? "Replay lobby" : "Match setup")
        : "Lobby browser";
    }
    if (this.btnJoin) this.btnJoin.textContent = hasLobby ? "Switch room" : "Join room";
    this._reflectCreateButton();
    this._reflectRefreshButton();
  }

  isJoinedOrJoining() {
    return this._joined || this._browserActionPending;
  }

  _isReplayLobby() {
    return this._roomKind === LOBBY_KIND.REPLAY;
  }
}

function normalizeLobbyKind(kind) {
  return kind === LOBBY_KIND.REPLAY ? LOBBY_KIND.REPLAY : LOBBY_KIND.NORMAL;
}

function mapMaxPlayers(entry) {
  const value = Number(entry?.maxPlayers);
  if (!Number.isFinite(value)) return DEFAULT_MAX_PLAYERS;
  return Math.min(DEFAULT_MAX_PLAYERS, Math.max(1, Math.trunc(value)));
}

async function readLobbyApiError(response) {
  try {
    const payload = await response.json();
    if (payload?.error) return String(payload.error);
  } catch (_) {
    // Fall through to status-based copy below.
  }
  if (response?.status === 503) return "Server is draining for deploy; new lobbies are disabled.";
  if (response?.status === 400) return "Lobby name is invalid.";
  return `Create lobby failed (${response?.status || "network"}).`;
}
