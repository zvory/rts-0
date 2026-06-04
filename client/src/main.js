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
import { GameState } from "./state.js";
import { Camera } from "./camera.js";
import { Renderer } from "./renderer.js";
import { Fog } from "./fog.js";
import { Input } from "./input.js";
import { HUD } from "./hud.js";
import { Minimap } from "./minimap.js";
import { Lobby } from "./lobby.js";
import { Audio, SOUND_MANIFEST, noticeSoundId } from "./audio.js";
import { machineGunnerHasAudibleTarget, machineGunSoundKey } from "./combat_audio.js";
import { S, EVENT, KIND } from "./protocol.js";
import { SNAPSHOT_MS, INTERP_DELAY_MS } from "./config.js";

/** How long (ms) a #toast notice stays on screen before fading out. */
const TOAST_MS = 2600;

/**
 * App-level heartbeat interval (ms). The server drops connections idle for 40s,
 * so we ping well inside that window to keep a healthy connection alive.
 */
const HEARTBEAT_MS = 15000;
const KAR98K_GAIN = 0.25;

const COMBAT_SOUNDS = Object.freeze({
  [KIND.TANK]: {
    ids: ["combat_tank_01", "combat_tank_06"],
    priority: 4,
    gain: 2,
  },
  [KIND.RIFLEMAN]: {
    ids: ["combat_rifle_02", "combat_rifle_03"],
    priority: 2,
    gain: KAR98K_GAIN,
  },
  [KIND.AT_TEAM]: {
    ids: ["combat_tank_01", "combat_tank_06"],
    priority: 4,
    gain: 2,
  },
  [KIND.MACHINE_GUNNER]: {
    ids: ["combat_mg_burst_02", "combat_mg_burst_03"],
    priority: 2.5,
  },
});

/**
 * Derive the WebSocket endpoint from the current page location, so the client
 * connects back to whichever host/port served it (the Rust process serves both).
 * @returns {string} e.g. "ws://localhost:8080/ws" or "wss://host/ws"
 */
function wsUrl() {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${window.location.host}/ws`;
}

function devWatchConfig() {
  const params = new URLSearchParams(window.location.search);
  const replay = (params.get("replay") || "").trim();
  if (window.location.pathname !== "/dev/selfplay" && !params.has("watchSelfplay")) {
    return null;
  }
  const room = replay
    ? `__dev_selfplay__replay:${replay}`
    : "__dev_selfplay__live";
  return {
    room,
    noFog: true,
    banner: replay ? `local dev  self-play replay  no fog  ${replay}` : "local dev  self-play  no fog",
  };
}

/** Cached DOM handles for the pinned ids in index.html (see its DOM contract). */
const dom = {
  version: document.getElementById("version"),
  lobbyScreen: document.getElementById("lobby-screen"),
  gameScreen: document.getElementById("game-screen"),
  viewport: document.getElementById("viewport"),
  minimap: document.getElementById("minimap"),
  toast: document.getElementById("toast"),
  gameOver: document.getElementById("game-over"),
  gameOverText: document.getElementById("game-over-text"),
  gameOverScores: document.getElementById("game-over-scores"),
  gameOverButton: document.getElementById("game-over-button"),
  settingsButton: document.getElementById("settings-button"),
  settingsMenu: document.getElementById("settings-menu"),
  giveUpOpen: document.getElementById("give-up-open"),
  giveUpConfirm: document.getElementById("give-up-confirm"),
  giveUpCancel: document.getElementById("give-up-cancel"),
  giveUpConfirmButton: document.getElementById("give-up-confirm-button"),
  devBanner: document.getElementById("dev-banner"),
  replaySpeed: document.getElementById("replay-speed"),
};

/**
 * The whole application. A single instance is created on load. It can host
 * many sequential matches: a match is torn down on `gameOver` and a fresh one
 * begins on the next `start`, all on the same Net connection.
 */
class App {
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
    this.net.join(name, this.devWatch.room);
    if (this.lobby?.roomBlock) this.lobby.roomBlock.hidden = true;
    this.lobby.setStatus("Starting local self-play watch…");
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
    dom.gameScreen.hidden = false;
    dom.gameOver.hidden = true;
    this.clearScoreboard();

    this.match = new Match(
      this.net,
      payload,
      (msg) => this.showToast(msg),
      this.devWatch,
      this.audio,
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
    dom.gameOver.hidden = true;
    this.clearScoreboard();
    dom.gameScreen.hidden = true;
    dom.lobbyScreen.hidden = false;
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
    if (!dom.version) return;
    try {
      const res = await fetch("/version", { cache: "no-store" });
      if (!res.ok) throw new Error(`version request failed: ${res.status}`);
      const text = await res.text();
      dom.version.textContent = text.trim() || "unknown";
    } catch {
      dom.version.textContent = "unknown";
    }
  }
}

function formatScore(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return "0";
  return Math.trunc(n).toLocaleString();
}

function isTextEntry(target) {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}

/**
 * One running match. Owns the in-game modules and the render loop, and knows
 * how to dispose of itself so the App can start a brand-new match afterwards.
 */
class Match {
  /**
   * @param {Net} net live connection (shared, not owned)
   * @param {object} payload §2.3 start payload
   * @param {(msg: string) => void} toast surface a notice in the App's toast
   */
  constructor(net, payload, toast, devWatch, audio) {
    this.net = net;
    this.toast = toast;
    this.devWatch = devWatch;
    this.audio = audio;
    this.missingCombatSoundKinds = new Set();
    this.activeMachineGunSoundKeys = new Map();
    this.replaySpeedHandler = null;
    this.giveUpSent = false;

    // --- Build the module graph from the static start payload (DESIGN.md §4.1). ---
    this.state = new GameState(payload);
    this.camera = new Camera();
    this.renderer = new Renderer(dom.viewport);
    this.fog = new Fog(this.state.map.width, this.state.map.height, this.state.map.terrain);
    this.fog.setRevealAll(!!this.devWatch?.noFog);
    this.hud = new HUD(dom.gameScreen, this.state, this.net);
    this.minimap = new Minimap(dom.minimap, this.state, this.camera, this.fog, this.net);
    this.input = new Input(
      dom.viewport,
      this.camera,
      this.state,
      this.net,
      this.renderer,
      this.fog,
      this.audio,
    );

    // Draw the static terrain once into the renderer's cached layer.
    this.renderer.buildStaticMap(this.state.map);

    // Size the camera to the map and the current viewport, then center on home.
    this.applyBounds();
    this.centerOnHome();

    // --- Render loop state. ---
    this.running = true;
    this.lastFrame = performance.now();
    this.tickFn = this.frame.bind(this);
    this.rafId = undefined;

    // --- Listeners (bound so they can be removed on destroy). ---
    this.onSnapshot = (m) => {
      this.state.applySnapshot(m);
      this.stopInactiveMachineGunSounds();
      this.handleSnapshotEvents(m.events || []);
    };
    this.onResize = this.handleResize.bind(this);
    this.onMenuKeyDown = this.handleMenuKeyDown.bind(this);
    this.onSettingsClick = this.toggleSettingsMenu.bind(this);
    this.onGiveUpOpen = this.openGiveUpConfirm.bind(this);
    this.onGiveUpCancel = this.closeGiveUpConfirm.bind(this);
    this.onGiveUpConfirm = this.requestGiveUp.bind(this);
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    window.addEventListener("resize", this.onResize);
    window.addEventListener("keydown", this.onMenuKeyDown, true);
    dom.settingsButton?.addEventListener("click", this.onSettingsClick);
    dom.giveUpOpen?.addEventListener("click", this.onGiveUpOpen);
    dom.giveUpCancel?.addEventListener("click", this.onGiveUpCancel);
    dom.giveUpConfirmButton?.addEventListener("click", this.onGiveUpConfirm);

    this.rafId = requestAnimationFrame(this.tickFn);

    // Show replay speed controls only when watching a replay.
    const isReplay = this.devWatch?.room?.includes("__dev_selfplay__replay:");
    if (isReplay && dom.replaySpeed) {
      dom.replaySpeed.hidden = false;
      this.replaySpeedHandler = (e) => {
        const btn = e.target.closest(".spd-btn");
        if (!btn) return;
        if (btn.dataset.seekBack !== undefined) {
          const ticksBack = parseInt(btn.dataset.seekBack, 10);
          if (!isFinite(ticksBack) || ticksBack <= 0) return;
          this.net.seekReplay(ticksBack);
          return;
        }
        const speed = parseFloat(btn.dataset.speed);
        if (!isFinite(speed)) return;
        this.net.setReplaySpeed(speed);
        for (const b of dom.replaySpeed.querySelectorAll(".spd-btn:not(.seek-btn)")) {
          b.classList.toggle("active", b === btn);
        }
      };
      dom.replaySpeed.addEventListener("click", this.replaySpeedHandler);
    }
  }

  handleMenuKeyDown(ev) {
    if (ev.code !== "Escape" || ev.repeat || isTextEntry(ev.target)) return;
    if (dom.giveUpConfirm && !dom.giveUpConfirm.hidden) {
      ev.preventDefault();
      ev.stopPropagation();
      this.closeGiveUpConfirm();
      return;
    }
    if (dom.settingsMenu && !dom.settingsMenu.hidden) {
      ev.preventDefault();
      ev.stopPropagation();
      this.closeSettingsMenu();
    }
  }

  toggleSettingsMenu() {
    if (!dom.settingsMenu || this.giveUpSent) return;
    if (dom.giveUpConfirm && !dom.giveUpConfirm.hidden) this.closeGiveUpConfirm();
    dom.settingsMenu.hidden = !dom.settingsMenu.hidden;
    dom.settingsButton?.setAttribute("aria-expanded", String(!dom.settingsMenu.hidden));
  }

  closeSettingsMenu() {
    if (!dom.settingsMenu) return;
    dom.settingsMenu.hidden = true;
    dom.settingsButton?.setAttribute("aria-expanded", "false");
  }

  openGiveUpConfirm() {
    if (!dom.giveUpConfirm || this.giveUpSent) return;
    this.closeSettingsMenu();
    dom.giveUpConfirm.hidden = false;
    dom.giveUpConfirmButton?.focus();
  }

  closeGiveUpConfirm() {
    if (!dom.giveUpConfirm) return;
    dom.giveUpConfirm.hidden = true;
    if (dom.giveUpConfirmButton) {
      dom.giveUpConfirmButton.disabled = false;
      dom.giveUpConfirmButton.textContent = "Give up";
    }
  }

  closeMenus() {
    this.closeSettingsMenu();
    if (dom.giveUpConfirm) dom.giveUpConfirm.hidden = true;
  }

  requestGiveUp() {
    if (this.giveUpSent) return;
    this.giveUpSent = true;
    if (dom.giveUpConfirmButton) {
      dom.giveUpConfirmButton.disabled = true;
      dom.giveUpConfirmButton.textContent = "Giving up...";
    }
    this.net.giveUp();
  }

  /** Compute world/viewport sizes and push them into the camera. */
  applyBounds() {
    const { width, height, tileSize } = this.state.map;
    this.camera.setBounds(
      width * tileSize,
      height * tileSize,
      dom.viewport.clientWidth,
      dom.viewport.clientHeight,
    );
  }

  /** Center the camera on this player's own starting tile (Industrial Center location). */
  centerOnHome() {
    const me = this.state.players.find((p) => p.id === this.state.playerId);
    const ts = this.state.map.tileSize;
    if (me) {
      // +0.5 so we center on the middle of the start tile, not its corner.
      this.camera.centerOn((me.startTileX + 0.5) * ts, (me.startTileY + 0.5) * ts);
    } else {
      // Defensive fallback: center on the map if our player isn't listed.
      this.camera.centerOn(
        (this.state.map.width * ts) / 2,
        (this.state.map.height * ts) / 2,
      );
    }
  }

  /** Keep the Pixi canvas and camera clamp in sync with the window. */
  handleResize() {
    const w = dom.viewport.clientWidth;
    const h = dom.viewport.clientHeight;
    this.renderer.resize(w, h);
    this.applyBounds();
  }

  /**
   * Interpolation alpha for this frame. We render slightly in the past
   * (INTERP_DELAY_MS) and blend between the two most recent snapshots based on
   * how far wall-clock time has advanced past the older one, normalized to the
   * expected snapshot interval. Clamped to [0,1] so a missed snapshot freezes
   * on the latest pose instead of extrapolating.
   * @returns {number} 0..1
   */
  computeAlpha() {
    const { prevRecvTime, currRecvTime } = this.snapshotTimes();
    if (prevRecvTime == null || currRecvTime == null) return 1;
    const renderTime = performance.now() - INTERP_DELAY_MS;
    const span = currRecvTime - prevRecvTime || SNAPSHOT_MS;
    const a = (renderTime - prevRecvTime) / span;
    return a < 0 ? 0 : a > 1 ? 1 : a;
  }

  /**
   * Read the two latest snapshot receive timestamps stamped by GameState.
   * GameState owns the buffer; we only need its two recv times for timing.
   * Tolerant of a couple of likely field shapes so we stay decoupled.
   * @returns {{prevRecvTime: number|null, currRecvTime: number|null}}
   */
  snapshotTimes() {
    const s = this.state;
    let prev = s.prevRecvTime;
    let curr = s.currRecvTime;
    if (prev == null && s.prev && typeof s.prev.recvTime === "number") {
      prev = s.prev.recvTime;
    }
    if (curr == null && s.current && typeof s.current.recvTime === "number") {
      curr = s.current.recvTime;
    }
    return {
      prevRecvTime: typeof prev === "number" ? prev : null,
      currRecvTime: typeof curr === "number" ? curr : null,
    };
  }

  /**
   * Surface one snapshot's transient events exactly once. Notices become toasts
   * and alerts; combat/death events drive spatial sounds.
   */
  handleSnapshotEvents(events) {
    if (!events || !events.length) return;
    for (const ev of events) {
      if (ev && ev.e === EVENT.NOTICE && ev.msg) {
        this.toast(ev.msg);
        if (this.audio) {
          this.audio.play(noticeSoundId(ev.msg), { category: "alert", priority: 3 });
        }
      } else if (ev && ev.e === EVENT.ATTACK) {
        this.playAttackSound(ev);
      }
    }
  }

  playAttackSound(ev) {
    if (!this.audio) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const to = typeof ev.to === "number" ? this.state.entityById(ev.to) : null;
    const pos = from || to;
    if (!pos || typeof pos.x !== "number" || typeof pos.y !== "number") return;

    const kind = from?.kind || KIND.RIFLEMAN;
    let spec = COMBAT_SOUNDS[kind];
    if (!spec) {
      spec = COMBAT_SOUNDS[KIND.RIFLEMAN];
      if (!this.missingCombatSoundKinds.has(kind)) {
        this.missingCombatSoundKinds.add(kind);
        console.warn(`audio: missing combat sound mapping for ${kind}, using rifle`);
      }
    }
    const id = this.audio.pickVariant(spec.ids);
    if (!id) return;
    const category = from && from.owner === this.state.playerId ? "combat_self" : "combat_other";
    const key =
      kind === KIND.MACHINE_GUNNER && typeof ev.from === "number"
        ? machineGunSoundKey(ev.from)
        : undefined;
    const played = this.audio.play(id, {
      x: pos.x,
      y: pos.y,
      category,
      priority: spec.priority,
      gain: spec.gain,
      key,
    });
    if (played && key) this.activeMachineGunSoundKeys.set(ev.from, key);
  }

  stopInactiveMachineGunSounds() {
    if (!this.audio || this.activeMachineGunSoundKeys.size === 0) return;
    for (const [id, key] of this.activeMachineGunSoundKeys) {
      if (machineGunnerHasAudibleTarget(this.state.entityById(id))) continue;
      this.audio.stopByKey(key);
      this.activeMachineGunSoundKeys.delete(id);
    }
  }

  stopAllMachineGunSounds() {
    if (!this.audio) {
      this.activeMachineGunSoundKeys.clear();
      return;
    }
    for (const key of this.activeMachineGunSoundKeys.values()) {
      this.audio.stopByKey(key);
    }
    this.activeMachineGunSoundKeys.clear();
  }

  /**
   * One animation frame: advance time-based systems, then render.
   * Order matches DESIGN.md §4.1 main.js loop description.
   * @param {number} now high-res timestamp from rAF
   */
  frame(now) {
    if (!this.running) return;

    const dt = (now - this.lastFrame) / 1000; // seconds since last frame
    this.lastFrame = now;

    const alpha = this.computeAlpha();

    this.camera.update(dt, this.input);
    if (this.audio) {
      this.audio.setListener(
        this.camera.x + this.camera.viewW / (2 * this.camera.zoom),
        this.camera.y + this.camera.viewH / (2 * this.camera.zoom),
        this.camera.zoom,
        this.camera.viewW,
      );
    }
    this.input.update(dt);
    this.fog.update(this.ownEntities(), this.state.map.tileSize);

    this.renderer.render(this.state, this.camera, this.fog, alpha);
    this.hud.update();
    this.minimap.render();

    this.rafId = requestAnimationFrame(this.tickFn);
  }

  /**
   * This player's own units & buildings, used to drive the local fog overlay.
   * Resource nodes (owner 0) never grant vision.
   * @returns {object[]}
   */
  ownEntities() {
    const all = this.state.entitiesInterpolated(1);
    const me = this.state.playerId;
    return all.filter((e) => e.owner === me);
  }

  /** Pause the loop (used while the game-over overlay is up). Idempotent. */
  stop() {
    this.running = false;
    this.closeMenus();
    if (this.rafId !== undefined) {
      cancelAnimationFrame(this.rafId);
      this.rafId = undefined;
    }
  }

  /**
   * Fully dispose of the match: stop the loop, drop listeners, and destroy any
   * module that exposes a destroy()/teardown() hook. After this the App can
   * build a fresh Match on the next `start`. Best-effort and idempotent.
   */
  destroy() {
    this.stop();
    this.stopAllMachineGunSounds();
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    window.removeEventListener("resize", this.onResize);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    dom.settingsButton?.removeEventListener("click", this.onSettingsClick);
    dom.giveUpOpen?.removeEventListener("click", this.onGiveUpOpen);
    dom.giveUpCancel?.removeEventListener("click", this.onGiveUpCancel);
    dom.giveUpConfirmButton?.removeEventListener("click", this.onGiveUpConfirm);
    if (dom.replaySpeed && this.replaySpeedHandler) {
      dom.replaySpeed.removeEventListener("click", this.replaySpeedHandler);
      dom.replaySpeed.hidden = true;
    }
    // Let modules release DOM/WebGL resources if they own any.
    for (const m of [this.input, this.minimap, this.hud, this.renderer, this.fog]) {
      if (m && typeof m.destroy === "function") {
        try {
          m.destroy();
        } catch {
          /* never let one module's teardown block the rest */
        }
      }
    }
  }
}

/**
 * Inject Phase-1 volume sliders into the in-match gear menu (#settings-menu).
 * Sliders persist via Audio's localStorage layer; the rows are inserted above
 * any pre-existing menu items (e.g. "Give up").
 *
 * UX choices for phase 1:
 *  - Combat slider is bound to both `combat_self` and `combat_other` so the
 *    player adjusts "combat noise" as one thing. Splitting them is a phase 4
 *    decision once we have actual combat sounds wired.
 *  - All other categories surface one-to-one.
 *
 * @param {import("./audio.js").Audio} audio
 * @param {HTMLElement} menuEl
 */
function buildAudioSettings(audio, menuEl) {
  if (menuEl.querySelector(".audio-settings")) return; // idempotent

  const wrap = document.createElement("div");
  wrap.className = "audio-settings";

  const rows = [
    {
      label: "Master",
      get: () => audio.getMasterVolume(),
      set: (v) => audio.setMasterVolume(v),
    },
    {
      label: "Alerts",
      get: () => audio.getCategoryVolume("alert"),
      set: (v) => audio.setCategoryVolume("alert", v),
    },
    {
      label: "UI",
      get: () => audio.getCategoryVolume("ui"),
      set: (v) => audio.setCategoryVolume("ui", v),
    },
    {
      label: "Combat",
      get: () => audio.getCategoryVolume("combat_self"),
      set: (v) => {
        audio.setCategoryVolume("combat_self", v);
        audio.setCategoryVolume("combat_other", v);
      },
    },
    {
      label: "Voices",
      get: () => audio.getCategoryVolume("unit_voice"),
      set: (v) => audio.setCategoryVolume("unit_voice", v),
    },
    {
      label: "Ambient",
      get: () => audio.getCategoryVolume("ambient"),
      set: (v) => audio.setCategoryVolume("ambient", v),
    },
  ];

  for (const row of rows) {
    const r = document.createElement("label");
    r.className = "audio-slider";

    const label = document.createElement("span");
    label.className = "audio-slider-label";
    label.textContent = row.label;

    const input = document.createElement("input");
    input.type = "range";
    input.min = "0";
    input.max = "1";
    input.step = "0.01";
    input.value = String(row.get());
    input.addEventListener("input", () => row.set(parseFloat(input.value)));

    r.append(label, input);
    wrap.appendChild(r);
  }

  menuEl.insertBefore(wrap, menuEl.firstChild);
}

// --- Entry point ---------------------------------------------------------
const app = new App();
app.start();

// Debug/introspection handle. Harmless in production; lets dev tooling and the
// integration tests inspect live match state (e.g. `__rts.match.state.selection`).
if (typeof window !== "undefined") window.__rts = app;
