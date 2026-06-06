// Bootstrap & wiring. See DESIGN.md §4.1 (module contracts) and §4.
//
// This is the only place that knows about *all* the modules. It owns:
//   - the WebSocket lifecycle (via Net),
//   - the lobby <-> game screen transition,
//   - constructing the in-game modules from the `start` payload, and
//   - the requestAnimationFrame render loop (with snapshot interpolation).
//
// Everything below codes strictly against the export signatures in DESIGN.md §4.1;
// the modules themselves are owned by other files. Keep this layer thin: it should
// orchestrate, not implement game logic.

import { Net } from "./net.js";
import { Lobby } from "./lobby.js";
import { Audio, SOUND_MANIFEST } from "./audio.js";
import { S } from "./protocol.js";
import { TOAST_MS } from "./alerts.js";
import { buildAudioSettings, devWatchConfig, dom, formatScore, wsUrl } from "./bootstrap.js";
import { Match } from "./match.js";
import { StatusBadge } from "./status_badge.js";

/**
 * App-level heartbeat interval (ms). The server drops connections idle for 40s,
 * so we ping well inside that window to keep a healthy connection alive.
 */
const HEARTBEAT_MS = 15000;

/**
 * The whole application. A single instance is created on load. It can host
 * many sequential matches: a match is torn down on `gameOver` and a fresh one
 * begins on the next `start`, all on the same Net connection.
 */
export class App {
  constructor() {
    /** @type {Net} persistent connection across lobby + matches. */
    this.net = new Net(wsUrl());
    this.devWatch = devWatchConfig();
    /**
     * Audio engine. Long-lived across matches: the AudioContext is unlocked
     * by the user's first gesture (anywhere in the page), and we want that
     * unlock to survive lobby->match->lobby transitions.
     * @type {Audio}
     */
    this.audio = new Audio();
    void this.audio.preload(SOUND_MANIFEST);
    if (dom.settingsMenu) buildAudioSettings(this.audio, dom.settingsMenu);
    this.statusBadge = new StatusBadge(dom.version);
    /** @type {Lobby} */
    this.lobby = new Lobby(dom.lobbyScreen, this.net);
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
    this.onBackToLobby = this.onBackToLobby.bind(this);
    this.onOpen = this.onOpen.bind(this);
    this.onClose = this.onClose.bind(this);
  }

  /** Connect, wire global server messages, and show the lobby. */
  async start() {
    this.net.on(S.START, this.onStart);
    this.net.on(S.ERROR, this.onError);
    this.net.on(S.GAME_OVER, this.onGameOver);
    this.net.on("open", this.onOpen);
    this.net.on("close", this.onClose);
    dom.gameOverButton.addEventListener("click", this.onBackToLobby);

    void this.loadVersion();
    this.lobby.show();
    this.applyDevBanner();
    try {
      await this.net.connect();
      this.maybeAutoJoinDevWatch();
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
    this.net.join(name, this.devWatch.room, true);
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    const label = this.devWatch.kind === "scenario" ? "scenario" : "self-play watch";
    this.lobby.setStatus(`Starting local ${label}...`);
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
    // If a previous match somehow lingers, tear it down first.
    if (this.match) this.match.destroy();

    dom.lobbyScreen.hidden = true;
    if (dom.devLinks) dom.devLinks.hidden = true;
    dom.gameScreen.hidden = false;
    dom.gameOver.hidden = true;
    this.clearScoreboard();

    this.match = new Match(
      this.net,
      payload,
      (msg) => this.showToast(msg),
      this.devWatch,
      this.audio,
      this.statusBadge,
    );
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
    if (this.match) {
      this.match.destroy();
      this.match = null;
    }
    this.statusBadge.clearMatchMetrics();
    dom.gameOver.hidden = true;
    this.clearScoreboard();
    dom.gameScreen.hidden = true;
    dom.lobbyScreen.hidden = false;
    if (dom.devLinks) dom.devLinks.hidden = false;
    this.lobby.show();
  }

  /**
   * Pop a transient toast (server notices, connection problems, etc.).
   * Re-arming the timer keeps the latest message visible.
   * @param {string} text
   */
  showToast(text) {
    if (!text) return;
    dom.toast.textContent = text;
    dom.toast.hidden = false;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = window.setTimeout(() => {
      dom.toast.hidden = true;
    }, TOAST_MS);
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
      this.statusBadge.setVersion(text.trim() || "unknown");
    } catch {
      this.statusBadge.setVersion("unknown");
    }
  }
}
