// Bootstrap & wiring. See docs/design/client-ui.md §4.1 (module contracts) and §4.
//
// This is the only place that knows about *all* the modules. It owns:
//   - the WebSocket lifecycle (via Net),
//   - the lobby <-> game screen transition,
//   - constructing the in-game modules from the `start` payload, and
//   - the requestAnimationFrame render loop (with snapshot interpolation).
//
// Everything below codes strictly against the export signatures in docs/design/client-ui.md §4.1;
// the modules themselves are owned by other files. Keep this layer thin: it should
// orchestrate, not implement game logic.

import { Net } from "./net.js";
import { Lobby } from "./lobby.js";
import { BranchStaging } from "./branch_staging.js";
import { Audio, SOUND_MANIFEST } from "./audio.js";
import { S } from "./protocol.js";
import { TOAST_MS } from "./alerts.js";
import {
  devWatchConfig,
  diagnostics,
  dom,
  formatScore,
  replayLaunchConfig,
  wsUrl,
} from "./bootstrap.js";
import { Match } from "./match.js";
import { MatchHistory } from "./match_history.js";
import { ReplayViewer } from "./replay_viewer.js";
import { StatusBadge } from "./status_badge.js";
import {
  HotkeyProfileService,
  buildHotkeyCommandCatalog,
} from "./hotkey_profiles.js";
import { buildCommandCardContextCatalog } from "./hud_command_card.js";
import { SettingsContainer } from "./settings_container.js";
import { buildSettingsTabs } from "./settings_panels.js";

/**
 * App-level heartbeat interval (ms). The server drops connections idle for 40s,
 * so we ping well inside that window to keep a healthy connection alive.
 */
const HEARTBEAT_MS = 15000;

export function shouldWarnBeforeUnload({
  match = null,
  inReplayPlayback = false,
  allowUnloadWithoutWarning = false,
} = {}) {
  return !allowUnloadWithoutWarning && (!!match || !!inReplayPlayback);
}

/**
 * The whole application. A single instance is created on load. It can host
 * many sequential matches: a live match can roll straight into post-match replay
 * playback, then tear down cleanly when the user returns to lobby, all on the
 * same Net connection.
 */
export class App {
  constructor() {
    /** @type {Net} persistent connection across lobby + matches. */
    this.net = new Net(wsUrl(), diagnostics);
    this.devWatch = devWatchConfig();
    this.replayLaunch = replayLaunchConfig();
    /**
     * Audio engine. Long-lived across matches: the AudioContext is unlocked
     * by the user's first gesture (anywhere in the page), and we want that
     * unlock to survive lobby->match->lobby transitions.
     * @type {Audio}
     */
    this.audio = new Audio();
    void this.audio.preload(SOUND_MANIFEST);
    this.statusBadge = new StatusBadge(dom.version);
    this.hotkeyProfiles = new HotkeyProfileService({
      catalog: buildHotkeyCommandCatalog(buildCommandCardContextCatalog()),
    });
    globalThis.rtsHotkeys = this.hotkeyProfiles;
    this.settings = new SettingsContainer({
      button: dom.settingsButton,
      menu: dom.settingsMenu,
    });
    /** @type {Lobby} */
    this.lobby = new Lobby(dom.lobbyScreen, this.net);
    this.branchStaging = new BranchStaging(dom.branchScreen, this.net);
    /** @type {MatchHistory|null} Lazy-init when the lobby first shows. */
    this.matchHistory = null;
    /** @type {Match|null} the currently running match, if any. */
    this.match = null;
    /** @type {number|undefined} pending toast hide timer. */
    this.toastTimer = undefined;
    /** @type {number|undefined} heartbeat interval id while connected. */
    this.heartbeatTimer = undefined;
    /** Whether the WebSocket has ever reached open in this page session. */
    this.hasConnected = false;

    // Bind handlers once so we can off() them symmetrically.
    this.onStart = this.onStart.bind(this);
    this.onError = this.onError.bind(this);
    this.onGameOver = this.onGameOver.bind(this);
    this.onShutdownWarning = this.onShutdownWarning.bind(this);
    this.onBackToLobby = this.onBackToLobby.bind(this);
    this.onCloseScorePanel = this.onCloseScorePanel.bind(this);
    this.onGameOverOverlayClick = this.onGameOverOverlayClick.bind(this);
    this.onOpen = this.onOpen.bind(this);
    this.onClose = this.onClose.bind(this);
    this.onReplayBranchCreated = this.onReplayBranchCreated.bind(this);
    this.onBeforeUnload = this.onBeforeUnload.bind(this);
    this.inReplayPlayback = false;
    this.allowUnloadWithoutWarning = false;
    this.pendingCameraView = null;
    this.mountLobbySettings();
  }

  /** Connect, wire global server messages, and show the lobby. */
  async start() {
    this.net.on(S.START, this.onStart);
    this.net.on(S.ERROR, this.onError);
    this.net.on(S.GAME_OVER, this.onGameOver);
    this.net.on(S.REPLAY_BRANCH_CREATED, this.onReplayBranchCreated);
    this.net.on(S.SHUTDOWN_WARNING, this.onShutdownWarning);
    this.net.on("open", this.onOpen);
    this.net.on("close", this.onClose);
    dom.gameOverButton.addEventListener("click", this.onBackToLobby);
    dom.gameOverClose?.addEventListener("click", this.onCloseScorePanel);
    dom.gameOver.addEventListener("click", this.onGameOverOverlayClick);
    window.addEventListener("beforeunload", this.onBeforeUnload);

    void this.loadVersion();
    this.lobby.show();
    this.mountLobbySettings();
    this._mountMatchHistory();
    this.applyDevBanner();
    try {
      await this.net.connect();
      if (this.replayLaunch) this.maybeAutoJoinReplay();
      else this.maybeAutoJoinDevWatch();
    } catch (err) {
      this.showConnectionWarning();
    }
  }

  applyDevBanner() {
    if (!dom.devBanner) return;
    if (!this.devWatch) {
      dom.devBanner.hidden = true;
      return;
    }
    dom.devBanner.textContent = this.devWatch.banner;
    dom.devBanner.hidden = false;
  }

  maybeAutoJoinDevWatch() {
    if (!this.devWatch) return;
    const name = "Spectator";
    if (this.lobby?.elName) this.lobby.elName.value = name;
    if (this.lobby?.elRoom) this.lobby.elRoom.value = this.devWatch.room;
    this.net.join(name, this.devWatch.room, true, this.devWatch.kind === "replay");
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    const label = this.devWatch.kind === "scenario" ? "scenario" : "self-play watch";
    this.lobby.setStatus(`Starting local ${label}...`);
  }

  maybeAutoJoinReplay() {
    const name = "Spectator";
    if (this.lobby?.elName) this.lobby.elName.value = name;
    if (this.lobby?.elRoom) this.lobby.elRoom.value = this.replayLaunch.room;
    this.net.join(name, this.replayLaunch.room, true, true);
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    this.lobby.setStatus("Starting replay...");
  }

  /**
   * Server rejected something (bad join, room full, illegal start, ...).
   * Surface it to the player; the lobby is the most likely context.
   * @param {{msg: string}} m
   */
  onError(m) {
    this.showToast(m && m.msg ? m.msg : "Server error");
  }

  /**
   * Server is draining for deploy. Surface the platform deadline and keep it visible in the
   * lobby status too, because new match starts are disabled while active matches wind down.
   * @param {{deadlineUnixMs?: number, secondsRemaining?: number}} m
   */
  onShutdownWarning(m) {
    const seconds = this.shutdownSecondsRemaining(m);
    const text =
      seconds > 0
        ? `Server deploy in ${this.formatDuration(seconds)}. New matches are disabled.`
        : "Server deploy in progress. New matches are disabled.";
    this.showToast(text, 8000);
    if (this.lobby) this.lobby.setStatus(text, true);
  }

  /**
   * Socket opened: start an app-level heartbeat so a healthy connection is never
   * dropped by the server's idle timeout. We only ping once the socket is open,
   * so we never spam pings before then.
   */
  onOpen() {
    this.hasConnected = true;
    this.stopHeartbeat();
    this.heartbeatTimer = window.setInterval(() => this.net.ping(), HEARTBEAT_MS);
  }

  /** Socket closed: stop the heartbeat so we don't leak the interval. */
  onClose() {
    this.stopHeartbeat();
    const text = this.hasConnected
      ? "Server connection lost. Refresh when the server is available."
      : "Unable to connect to the server. Make sure it is running, then refresh.";
    this.showConnectionWarning(text);
  }

  /** Clear the heartbeat interval if one is running. Idempotent. */
  stopHeartbeat() {
    if (this.heartbeatTimer !== undefined) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = undefined;
    }
  }

  /**
   * Match is beginning. Build the game-side modules from the static `start`
   * payload, swap screens, and kick off the render loop.
   * @param {object} payload §2.3 start payload
   */
  onStart(payload) {
    diagnostics.mark("app.onStart.begin", {
      map: payload?.map ? `${payload.map.width}x${payload.map.height}` : undefined,
      terrain: payload?.map?.terrain?.length,
      resources: payload?.map?.resources?.length,
      players: payload?.players?.length,
      spectator: payload?.spectator,
    });
    const startsReplay = !!payload?.replay;
    const preserveScorePanel = startsReplay && !dom.gameOver.hidden;

    const carriedCamera = this.takeMatchCameraView() || this.pendingCameraView;
    this.pendingCameraView = null;

    // If a previous match somehow lingers, tear it down first.
    if (this.match) this.match.destroy();
    this.inReplayPlayback = startsReplay;

    dom.gameScreen.classList.remove("branch-background");
    dom.lobbyScreen.hidden = true;
    this.branchStaging.hide();
    if (dom.devLinks) dom.devLinks.hidden = true;
    dom.gameScreen.hidden = false;
    if (!preserveScorePanel) {
      dom.gameOver.hidden = true;
      this.clearScoreboard();
    }

    const MatchClass = startsReplay ? ReplayViewer : Match;
    this.match = new MatchClass(
      this.net,
      payload,
      (msg) => this.showToast(msg),
      this.devWatch,
      this.audio,
      this.statusBadge,
      diagnostics,
      {
        initialCamera: carriedCamera,
        hotkeyProfiles: this.hotkeyProfiles,
        settings: this.settings,
      },
    );
    diagnostics.mark("app.onStart.end");
  }

  onReplayBranchCreated(m) {
    const branchRoom = (m?.branchRoom || "").trim();
    if (!branchRoom) return;
    if (this.match) {
      this.pendingCameraView = this.takeMatchCameraView();
      if (typeof this.match.freezeForBranchStagingBackground === "function") {
        this.match.freezeForBranchStagingBackground();
      } else {
        this.match.destroy();
        this.match = null;
      }
    }
    this.inReplayPlayback = false;
    this.statusBadge.clearMatchMetrics();
    dom.gameOver.hidden = true;
    this.clearScoreboard();
    dom.gameScreen.hidden = false;
    dom.gameScreen.classList.add("branch-background");
    dom.lobbyScreen.hidden = true;
    if (dom.devLinks) dom.devLinks.hidden = true;
    this.branchStaging.show();

    const name = (this.lobby?.elName && this.lobby.elName.value.trim()) || "Commander";
    if (this.lobby?.elRoom) this.lobby.elRoom.value = branchRoom;
    this.net.join(name, branchRoom, true, true);
  }

  takeMatchCameraView() {
    if (!this.match || typeof this.match.cameraView !== "function") return null;
    const view = this.match.cameraView();
    return Number.isFinite(view?.x) && Number.isFinite(view?.y) && Number.isFinite(view?.zoom)
      ? view
      : null;
  }

  /**
   * Match resolved. Show the overlay with the right verdict; the button
   * (wired in start()) returns to the lobby and tears the match down.
   * @param {{winnerId: number|null, you: "won"|"lost"|"draw"}} m
   */
  onGameOver(m) {
    const verdict = m && m.you ? m.you : "draw";
    const text =
      verdict === "won" ? "Victory" : verdict === "lost" ? "Defeat" : "Draw";
    if (verdict === "won") this.audio.play("victory", { category: "ui", priority: 5 });
    else if (verdict === "lost") this.audio.play("defeat", { category: "ui", priority: 5 });
    dom.gameOverText.textContent = text;
    dom.gameOverText.dataset.verdict = verdict; // lets CSS tint win/lose/draw
    this.renderScoreboard(Array.isArray(m?.scores) ? m.scores : [], m?.winnerId ?? null);
    dom.gameOver.hidden = false;
    // Freeze the loop but keep the final frame visible behind the overlay.
    if (this.match) this.match.stop();
  }

  /**
   * Render the frozen score snapshot carried by the gameOver message.
   * @param {Array<object>} scores
   * @param {number|null} winnerId
   */
  renderScoreboard(scores, winnerId) {
    const root = dom.gameOverScores;
    if (!root) return;
    root.replaceChildren();
    if (!scores.length) {
      root.hidden = true;
      return;
    }

    const table = document.createElement("table");
    table.className = "score-table";
    const thead = document.createElement("thead");
    const header = document.createElement("tr");
    const columns = [
      ["player", "Player"],
      ["unitScore", "Unit score"],
      ["structureScore", "Structure score"],
      ["unitsKilled", "Units killed"],
      ["unitsLost", "Units lost"],
      ["buildingsKilled", "Buildings killed"],
      ["buildingsLost", "Buildings lost"],
    ];
    for (const [, label] of columns) {
      const th = document.createElement("th");
      th.scope = "col";
      th.textContent = label;
      header.appendChild(th);
    }
    thead.appendChild(header);
    table.appendChild(thead);

    const tbody = document.createElement("tbody");
    for (const score of scores) {
      const tr = document.createElement("tr");
      const id = Number(score?.id);
      if (Number.isFinite(id) && id === this.net.playerId) tr.classList.add("you");
      if (winnerId != null && Number.isFinite(id) && id === Number(winnerId)) {
        tr.classList.add("winner");
      }

      const player = document.createElement("td");
      player.className = "score-player";
      const swatch = document.createElement("span");
      swatch.className = "score-swatch";
      swatch.style.backgroundColor = typeof score?.color === "string" ? score.color : "#888";
      const name = document.createElement("span");
      name.className = "score-name";
      name.textContent = score?.name || (Number.isFinite(id) ? `Player ${id}` : "Player");
      player.append(swatch, name);
      tr.appendChild(player);

      for (const [key] of columns.slice(1)) {
        const td = document.createElement("td");
        td.className = "score-number";
        td.textContent = formatScore(score?.[key]);
        tr.appendChild(td);
      }
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    root.appendChild(table);
    root.hidden = false;
  }

  clearScoreboard() {
    if (!dom.gameOverScores) return;
    dom.gameOverScores.replaceChildren();
    dom.gameOverScores.hidden = true;
  }

  /** "Back to lobby" button: tear down the match and restore the lobby. */
  onBackToLobby() {
    if (this.replayLaunch) {
      this.allowUnloadWithoutWarning = true;
      window.location.assign(new URL("/", window.location.href).toString());
      return;
    }
    if (this.inReplayPlayback) this.net.returnToLobby();
    if (this.match) {
      this.match.destroy();
      this.match = null;
    }
    this.inReplayPlayback = false;
    this.statusBadge.clearMatchMetrics();
    dom.gameOver.hidden = true;
    this.clearScoreboard();
    dom.gameScreen.hidden = true;
    dom.gameScreen.classList.remove("branch-background");
    if (dom.branchScreen) this.branchStaging.hide();
    dom.lobbyScreen.hidden = false;
    if (dom.devLinks) dom.devLinks.hidden = false;
    this.lobby.show();
    this.mountLobbySettings();
    // A new match row may have just been written server-side; pull the freshest list.
    if (this.matchHistory) this.matchHistory.refresh();
    else this._mountMatchHistory();
  }

  mountLobbySettings() {
    this.settings?.setContext({
      kind: "lobby",
      spectator: false,
      replay: false,
      tabs: buildSettingsTabs({
        audio: this.audio,
        hotkeyProfiles: this.hotkeyProfiles,
        game: { kind: "lobby" },
      }),
    });
  }

  onBeforeUnload(ev) {
    if (!shouldWarnBeforeUnload(this)) return;
    ev.preventDefault();
    ev.returnValue = true;
    return true;
  }

  onCloseScorePanel() {
    dom.gameOver.hidden = true;
  }

  onGameOverOverlayClick(ev) {
    if (ev.target === dom.gameOver) this.onCloseScorePanel();
  }

  _mountMatchHistory() {
    const host = document.getElementById("match-history-host");
    if (!host) return;
    if (this.matchHistory) return;
    this.matchHistory = new MatchHistory(host);
  }

  /**
   * Pop a transient toast (server notices, connection problems, etc.).
   * Re-arming the timer keeps the latest message visible.
   * @param {string} text
   */
  showToast(text, timeoutMs = TOAST_MS) {
    if (!text) return;
    dom.toast.textContent = text;
    dom.toast.hidden = false;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = window.setTimeout(() => {
      dom.toast.hidden = true;
    }, timeoutMs);
  }

  shutdownSecondsRemaining(m) {
    const deadline = Number(m?.deadlineUnixMs);
    if (Number.isFinite(deadline) && deadline > 0) {
      return Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
    }
    const fallback = Number(m?.secondsRemaining);
    return Number.isFinite(fallback) && fallback > 0 ? Math.ceil(fallback) : 0;
  }

  formatDuration(seconds) {
    const total = Math.max(0, Math.ceil(seconds));
    const minutes = Math.floor(total / 60);
    const rem = total % 60;
    if (minutes <= 0) return `${rem}s`;
    if (rem === 0) return `${minutes}m`;
    return `${minutes}m ${rem}s`;
  }

  /**
   * Surface server connection failures in both the global toast and the lobby
   * status line, so the warning is visible before a match starts.
   * @param {string} [text]
   */
  showConnectionWarning(
    text = "Unable to connect to the server. Make sure it is running, then refresh.",
  ) {
    this.showToast(text);
    if (this.lobby) this.lobby.setStatus(text, true);
  }

  /** Fetch and display the build version in the shared top-left badge. */
  async loadVersion() {
    try {
      const res = await fetch("/version", { cache: "no-store" });
      if (!res.ok) throw new Error(`version request failed: ${res.status}`);
      const text = await res.text();
      const version = text.trim() || "unknown";
      globalThis.__RTS_BUILD__ = version;
      this.statusBadge.setVersion(version);
    } catch {
      globalThis.__RTS_BUILD__ = "unknown";
      this.statusBadge.setVersion("unknown");
    }
  }
}
