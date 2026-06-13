// Lobby — the pre-match screen (`#lobby-screen`): name/room entry, the player list, and
// ready/start controls. Talks to the server through `net` (join/ready/start) and renders
// `lobby` server messages. See docs/design/client-ui.md §4.1 (Lobby) and
// docs/design/protocol.md §2.2 (`lobby` payload).
//
// Screen transitions are NOT this module's job: it only toggles its own visibility via
// show()/hide(). main.js owns the lobby↔game switch and subscribes via `onGameStart(cb)`
// (fired when the server sends `start`). The entered name is persisted in localStorage.

import { S } from "./protocol.js";

const NAME_STORAGE_KEY = "rts.playerName";

/** Max players in a match (humans + AI). Mirrors the server's `MAX_PLAYERS`. */
const MAX_PLAYERS = 4;

export const TEAM_PRESETS = Object.freeze([
  { id: "ffa", label: "FFA", teams: [] },
  { id: "solo", label: "Solo", teams: [{ id: 1, cap: 1 }] },
  { id: "1v2", label: "1v2", teams: [{ id: 1, cap: 1 }, { id: 2, cap: 2 }] },
  { id: "1v3", label: "1v3", teams: [{ id: 1, cap: 1 }, { id: 2, cap: 3 }] },
  { id: "2v2", label: "2v2", teams: [{ id: 1, cap: 2 }, { id: 2, cap: 2 }] },
]);

export function presetById(id) {
  return TEAM_PRESETS.find((preset) => preset.id === id) || TEAM_PRESETS[0];
}

export function teamSlotsForPreset(presetId, players = []) {
  const preset = presetById(presetId);
  if (preset.id === "ffa") {
    return players
      .filter((player) => !player.isSpectator)
      .map((player) => ({ id: Number(player.teamId) || Number(player.id), cap: 1 }));
  }
  return preset.teams.map((team) => ({ ...team }));
}

/**
 * The lobby screen controller.
 */
export class Lobby {
  /**
   * @param {HTMLElement} rootEl the `#lobby-screen` section.
   * @param {import("./net.js").Net} net network seam (join/ready/start + event bus).
   */
  constructor(rootEl, net) {
    this.root = rootEl;
    this.net = net;

    // Form + room blocks.
    this.elName = rootEl.querySelector("#lobby-name");
    this.elRoom = rootEl.querySelector("#lobby-room");
    this.btnJoin = rootEl.querySelector("#lobby-join");
    this.chkSpectator = rootEl.querySelector("#lobby-spectator");
    this.chkSpectatorInput = this.chkSpectator?.querySelector("input[type='checkbox']") || null;
    this.roomBlock = rootEl.querySelector(".lobby-room");
    this.elPlayers = rootEl.querySelector("#lobby-players");
    this.btnReady = rootEl.querySelector("#lobby-ready");
    this.btnAddAi = rootEl.querySelector("#lobby-add-ai");
    this.chkQuickstart = rootEl.querySelector("#lobby-quickstart");
    this.chkQuickstartInput = this.chkQuickstart?.querySelector("input[type='checkbox']") || null;
    this.btnStart = rootEl.querySelector("#lobby-start");
    this.elStatus = rootEl.querySelector("#lobby-status");
    this.selMap = rootEl.querySelector("#lobby-map");
    this.elMapDisplay = rootEl.querySelector("#lobby-map-display");
    this.selTeamPreset = rootEl.querySelector("#lobby-team-preset");
    this.elTeamPresetDisplay = rootEl.querySelector("#lobby-team-preset-display");

    // Local lobby state.
    this._joined = false;
    this._ready = false;
    this._spectator = false;
    this._hostId = null;
    this._canStart = false;
    this._quickstart = false;
    this._teamPreset = "ffa";
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

    // Bound handlers kept so they can be removed in destroy().
    this._onLobby = (m) => this._renderLobby(m);
    this._onMatchCountdown = (m) => this._renderMatchCountdown(m);
    this._onStart = () => this._handleStart();
    this._onJoinReplayPrompt = (m) => this._handleJoinReplayPrompt(m);
    this._onError = (m) => this.setStatus((m && m.msg) || "Error", true);
    this._onOpen = () => this.setStatus("Connected.");
    this._onClose = () => this.setStatus("Disconnected from server.", true);
    this._onReplayPromptKeydown = (ev) => this._handleReplayPromptKeydown(ev);

    this._restoreName();
    this._buildReplayPrompt();
    this._wireDom();
    this._wireNet();
  }

  // --- Visibility ------------------------------------------------------------

  /** Show the lobby screen. */
  show() {
    this.root.hidden = false;
  }

  /** Hide the lobby screen (main.js reveals the game screen). */
  hide() {
    this.root.hidden = true;
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

  // --- DOM wiring ------------------------------------------------------------

  _wireDom() {
    // Join: send join, persist name, reveal the room block. The server confirms with a
    // `lobby` message which fills in the player list.
    this.btnJoin.addEventListener("click", () => this._join());
    // Enter in the name/room fields also joins.
    for (const el of [this.elName, this.elRoom]) {
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
      if (this._spectator) return;
      this._ready = !this._ready;
      this.net.ready(this._ready);
      this._reflectReadyButton();
    });

    if (this.chkSpectatorInput) {
      this.chkSpectatorInput.addEventListener("change", () => {
        this._spectator = !!this.chkSpectatorInput.checked;
        this._ready = false;
        this._reflectReadyButton();
        if (this._joined) this.net.setSpectator(this._spectator);
      });
    }

    // Start: host-only; the server ignores it from non-hosts but we also gate the UI.
    this.btnStart.addEventListener("click", () => {
      if (this.btnStart.disabled) return;
      this.net.start();
    });

    if (this.chkQuickstartInput) {
      this.chkQuickstartInput.addEventListener("change", () => {
        this.net.setQuickstart(!!this.chkQuickstartInput.checked);
      });
    }

    // Add AI: host-only. The server ignores it from non-hosts / when full, but we gate the UI too.
    if (this.btnAddAi) {
      this.btnAddAi.addEventListener("click", () => {
        if (this.btnAddAi.disabled) return;
        this.net.addAi();
      });
    }

    // Map selector: host-only. Non-hosts see the selected map as a label.
    if (this.selMap) {
      this.selMap.addEventListener("change", () => {
        const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
        if (!isHost || this.selMap.disabled) return;
        this.net.selectMap(this.selMap.value);
      });
    }

    if (this.selTeamPreset) {
      this._populateTeamPresetOptions();
      this.selTeamPreset.addEventListener("change", () => {
        const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
        if (!isHost || this.selTeamPreset.disabled) return;
        this.net.setTeamPreset(this.selTeamPreset.value);
      });
    }
  }

  _join() {
    const name = (this.elName && this.elName.value.trim()) || "Commander";
    const room = (this.elRoom && this.elRoom.value.trim()) || "main";
    const spectator = !!this.chkSpectatorInput?.checked;
    this._persistName(name);
    this.net.join(name, room, spectator);
    this._joined = true;
    this._spectator = spectator;
    if (this.roomBlock) this.roomBlock.hidden = false;
    this.setStatus(`Joining "${room}"…`);
    this._reflectReadyButton();
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

  /** Tear down listeners (not normally needed for a single-screen lifetime). */
  destroy() {
    this.net.off(S.LOBBY, this._onLobby);
    this.net.off(S.MATCH_COUNTDOWN, this._onMatchCountdown);
    this.net.off(S.START, this._onStart);
    this.net.off(S.JOIN_REPLAY_PROMPT, this._onJoinReplayPrompt);
    this.net.off(S.ERROR, this._onError);
    this.net.off("open", this._onOpen);
    this.net.off("close", this._onClose);
    this._clearCountdown();
    this._hideReplayPrompt(false);
    this._replayPrompt?.root.remove();
    this._replayPrompt = null;
  }

  // --- Rendering -------------------------------------------------------------

  /**
   * Render a `lobby` server message (§2.2): room, hostId, players[], canStart.
   * @param {{room:string,hostId:number,players:Array,canStart:boolean,quickstart:boolean}} m
   */
  _renderLobby(m) {
    if (!m) return;
    this._hostId = m.hostId;
    this._canStart = !!m.canStart;
    this._quickstart = !!m.quickstart;
    this._teamPreset = m.teamPreset || "ffa";
    this._selectedMap = m.map || "";
    this._availableMaps = Array.isArray(m.maps) ? m.maps : [];

    // Once a lobby arrives we are definitively joined; make sure the room block shows.
    this._joined = true;
    if (this.roomBlock) this.roomBlock.hidden = false;

    const players = m.players || [];
    this._playerCount = players.filter((p) => !p.isSpectator).length;
    this._renderPlayers(players);
    this._reflectStartButton();
    this._reflectAddAiButton();
    this._reflectQuickstart();
    this._reflectMap();
    this._reflectTeamPreset();

    const participantCount = this._playerCount;
    const spectatorCount = players.filter((p) => p.isSpectator).length;
    const specText = spectatorCount > 0
      ? `, ${spectatorCount} spectator${spectatorCount === 1 ? "" : "s"}`
      : "";
    this.setStatus(
      `Room "${m.room}" — ${participantCount} player${participantCount === 1 ? "" : "s"}${specText}.`,
    );
  }

  /** Rebuild the player list: color swatch, name, (host) tag, ready check. */
  _renderPlayers(players) {
    const ul = this.elPlayers;
    if (!ul) return;
    ul.innerHTML = "";

    const myId = this.net.playerId;
    const seatedPlayers = players.filter((player) => !player.isSpectator);
    const spectatorPlayers = players.filter((player) => player.isSpectator);
    const slots = teamSlotsForPreset(this._teamPreset, seatedPlayers);
    const renderedIds = new Set();
    for (const slot of slots) {
      const teamPlayers = seatedPlayers.filter((player) => Number(player.teamId) === Number(slot.id));
      this._appendTeamHeader(ul, slot, teamPlayers.length);
      for (const p of teamPlayers) {
        renderedIds.add(p.id);
        ul.appendChild(this._buildPlayerRow(p, slots, myId));
      }
    }

    for (const p of seatedPlayers.filter((player) => !renderedIds.has(player.id))) {
      const slot = { id: Number(p.teamId) || Number(p.id), cap: 1 };
      this._appendTeamHeader(ul, slot, 1);
      ul.appendChild(this._buildPlayerRow(p, [slot], myId));
    }

    if (spectatorPlayers.length > 0) {
      this._appendSpectatorHeader(ul, spectatorPlayers.length);
      for (const p of spectatorPlayers) {
        ul.appendChild(this._buildPlayerRow(p, slots, myId));
      }
    }
  }

  _appendTeamHeader(ul, slot, count) {
    const li = document.createElement("li");
    li.className = "team-row";
    const label = document.createElement("span");
    label.className = "team-row-label";
    label.textContent = `Team ${slot.id}`;
    const countEl = document.createElement("span");
    countEl.className = "team-row-count";
    countEl.textContent = `${count}/${slot.cap}`;
    li.append(label, countEl);

    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    if (isHost) {
      const add = document.createElement("button");
      add.type = "button";
      add.className = "team-add-ai btn";
      add.textContent = "+ AI";
      add.title = `Add AI to Team ${slot.id}`;
      add.disabled = this._countdownActive || this._playerCount >= MAX_PLAYERS || count >= slot.cap;
      add.addEventListener("click", () => {
        if (!add.disabled) this.net.addAi(slot.id);
      });
      li.appendChild(add);
    }

    ul.appendChild(li);
  }

  _appendSpectatorHeader(ul, count) {
    const li = document.createElement("li");
    li.className = "team-row spectator-row";
    const label = document.createElement("span");
    label.className = "team-row-label";
    label.textContent = "Spectators";
    const countEl = document.createElement("span");
    countEl.className = "team-row-count";
    countEl.textContent = String(count);
    li.append(label, countEl);
    ul.appendChild(li);
  }

  _buildPlayerRow(p, slots, myId) {
    const li = document.createElement("li");
    li.className = "player-row";
    if (p.id === myId) li.classList.add("is-you");

    const swatch = document.createElement("span");
    swatch.className = "player-color";
    swatch.style.background = p.color || "#888";

    const name = document.createElement("span");
    name.className = "player-name";
    name.textContent = p.name || `Player ${p.id}`;

    const tags = document.createElement("span");
    tags.className = "player-tags";
    if (p.id === this._hostId) {
      const host = document.createElement("span");
      host.className = "tag host";
      host.textContent = "(host)";
      tags.appendChild(host);
    }
    if (p.isAi) {
      li.classList.add("is-ai");
      const bot = document.createElement("span");
      bot.className = "tag ai";
      bot.textContent = "AI";
      tags.appendChild(bot);
    }
    if (p.isSpectator) {
      li.classList.add("is-spectator");
      const spec = document.createElement("span");
      spec.className = "tag spectator";
      spec.textContent = "Spectator";
      tags.appendChild(spec);
    }

    li.appendChild(swatch);
    li.appendChild(name);
    li.appendChild(tags);
    this._appendTeamAssignment(li, p, slots);

    if (p.isAi) {
      // AI players are always "ready"; the host gets a remove control instead of a check.
      const iAmHost = this.net.playerId != null && this.net.playerId === this._hostId;
      if (iAmHost) {
        const remove = document.createElement("button");
        remove.className = "player-remove btn";
        remove.type = "button";
        remove.textContent = "✕";
        remove.title = "Remove AI";
        remove.setAttribute("aria-label", `Remove ${p.name || "AI"}`);
        remove.addEventListener("click", () => this.net.removeAi(p.id));
        li.appendChild(remove);
      } else {
        const ready = document.createElement("span");
        ready.className = "player-ready ready";
        ready.textContent = "✓ Ready";
        li.appendChild(ready);
      }
    } else if (p.isSpectator) {
      const ready = document.createElement("span");
      ready.className = "player-ready spectator";
      ready.textContent = "Observing";
      li.appendChild(ready);
    } else {
      const ready = document.createElement("span");
      ready.className = "player-ready" + (p.ready ? " ready" : "");
      ready.textContent = p.ready ? "✓ Ready" : "...";
      li.appendChild(ready);
    }

    // Keep our own ready toggle in sync with the authoritative server state.
    if (p.id === myId) {
      this._ready = !!p.ready;
      this._spectator = !!p.isSpectator;
      if (this.chkSpectatorInput) this.chkSpectatorInput.checked = this._spectator;
      this._reflectReadyButton();
    }
    return li;
  }

  _appendTeamAssignment(li, player, slots) {
    if (player.isSpectator) return;
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    if (!isHost || slots.length <= 1) {
      const label = document.createElement("span");
      label.className = "player-team-label";
      label.textContent = `Team ${player.teamId}`;
      li.appendChild(label);
      return;
    }

    const select = document.createElement("select");
    select.className = "player-team-select";
    select.setAttribute("aria-label", `Team for ${player.name || `Player ${player.id}`}`);
    select.disabled = this._countdownActive;
    for (const slot of slots) {
      const opt = document.createElement("option");
      opt.value = String(slot.id);
      opt.textContent = `Team ${slot.id}`;
      select.appendChild(opt);
    }
    select.value = String(player.teamId);
    select.addEventListener("change", () => {
      if (!select.disabled) this.net.setTeam(player.id, Number(select.value));
    });
    li.appendChild(select);
  }

  /**
   * Show the Add AI button only to the host, disabling it when the room is full
   * ([`MAX_PLAYERS`]). The server enforces both rules regardless; this is just UI gating.
   */
  _reflectAddAiButton() {
    if (!this.btnAddAi) return;
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    this.btnAddAi.hidden = !isHost;
    this.btnAddAi.disabled = this._countdownActive || this._playerCount >= MAX_PLAYERS;
  }

  /** Show the debug mode toggle only to the host and keep it synced. */
  _reflectQuickstart() {
    if (!this.chkQuickstart) return;
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    this.chkQuickstart.hidden = !isHost;
    this.chkQuickstart.disabled = this._countdownActive || !isHost;
    this.chkQuickstartInput.checked = !!this._quickstart;
  }

  /** Render the map selector (host) or map name label (non-host). */
  _reflectMap() {
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    if (this.selMap) {
      // Rebuild the option list only when the available maps have changed.
      // Each entry is {name, description}; name is the stable key, description is display text.
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
    if (this.elMapDisplay) {
      const entry = this._availableMaps.find((e) => e.name === this._selectedMap);
      const label = entry ? entry.name : this._selectedMap;
      this.elMapDisplay.textContent = `Map: ${label}`;
      this.elMapDisplay.hidden = isHost;
    }
  }

  /** Enable Start only for the host and only when the server says the match can start. */
  _reflectStartButton() {
    if (!this.btnStart) return;
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    this.btnStart.disabled = this._countdownActive || !(isHost && this._canStart);
    this.btnStart.classList.toggle("host-only", isHost);
  }

  /** Reflect the local ready state on the Ready button (label + pressed style). */
  _reflectReadyButton() {
    if (!this.btnReady) return;
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

  // --- Start handoff ---------------------------------------------------------

  /** The server signaled match start: fire subscribers (main.js switches screens). */
  _handleStart() {
    this._clearCountdown();
    this._hideReplayPrompt(false);
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
    this._reflectAddAiButton();
    this._reflectQuickstart();
    this._reflectMap();
    this._reflectTeamPreset();
    this.setStatus("Match starting...");

    const overlay = document.createElement("div");
    overlay.className = "match-countdown";
    overlay.setAttribute("role", "status");
    overlay.setAttribute("aria-live", "assertive");
    this._countdownEl = overlay;
    this.root.appendChild(overlay);

    const showWord = (word) => {
      if (!this._countdownEl) return;
      this._countdownEl.textContent = word;
      this._countdownEl.classList.remove("pulse");
      // Restart the animation for each word.
      void this._countdownEl.offsetWidth;
      this._countdownEl.classList.add("pulse");
    };

    words.forEach((word, index) => {
      const delay = Math.round(index * wordMs);
      if (delay <= 0) {
        showWord(word);
      } else {
        this._countdownTimers.push(window.setTimeout(() => showWord(word), delay));
      }
    });
    this._countdownTimers.push(window.setTimeout(() => this._clearCountdown(), durationMs + 1000));
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
    this._reflectAddAiButton();
    this._reflectQuickstart();
    this._reflectMap();
    this._reflectTeamPreset();
  }

  _populateTeamPresetOptions() {
    if (!this.selTeamPreset || this.selTeamPreset.options.length) return;
    for (const preset of TEAM_PRESETS) {
      const opt = document.createElement("option");
      opt.value = preset.id;
      opt.textContent = preset.label;
      this.selTeamPreset.appendChild(opt);
    }
  }

  /** Render the team preset selector (host) or preset label (non-host). */
  _reflectTeamPreset() {
    const isHost = this.net.playerId != null && this.net.playerId === this._hostId;
    const preset = presetById(this._teamPreset);
    if (this.selTeamPreset) {
      this._populateTeamPresetOptions();
      this.selTeamPreset.value = preset.id;
      this.selTeamPreset.disabled = this._countdownActive || !isHost;
      this.selTeamPreset.hidden = !isHost;
    }
    if (this.elTeamPresetDisplay) {
      this.elTeamPresetDisplay.textContent = `Preset: ${preset.label}`;
      this.elTeamPresetDisplay.hidden = isHost;
    }
  }

  _handleJoinReplayPrompt(m) {
    const room = (m?.room || "").trim() || ((this.elRoom && this.elRoom.value.trim()) || "main");
    this._joined = false;
    this.setStatus(`Room "${room}" is watching a replay.`, true);
    this._showReplayPrompt(room);
  }

  _joinReplayRoom(room) {
    const name = (this.elName && this.elName.value.trim()) || "Commander";
    this._persistName(name);
    if (this.elRoom) this.elRoom.value = room;
    this.net.join(name, room, true, true);
    this._joined = true;
    this._spectator = true;
    if (this.chkSpectatorInput) this.chkSpectatorInput.checked = true;
    if (this.roomBlock) this.roomBlock.hidden = false;
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
}
