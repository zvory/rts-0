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
import { S, EVENT } from "./protocol.js";
import { SNAPSHOT_MS, INTERP_DELAY_MS } from "./config.js";

/** How long (ms) a #toast notice stays on screen before fading out. */
const TOAST_MS = 2600;

/**
 * App-level heartbeat interval (ms). The server drops connections idle for 40s,
 * so we ping well inside that window to keep a healthy connection alive.
 */
const HEARTBEAT_MS = 15000;

/**
 * Derive the WebSocket endpoint from the current page location, so the client
 * connects back to whichever host/port served it (the Rust process serves both).
 * @returns {string} e.g. "ws://localhost:8080/ws" or "wss://host/ws"
 */
function wsUrl() {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${window.location.host}/ws`;
}

/** Cached DOM handles for the pinned ids in index.html (see its DOM contract). */
const dom = {
  lobbyScreen: document.getElementById("lobby-screen"),
  gameScreen: document.getElementById("game-screen"),
  viewport: document.getElementById("viewport"),
  minimap: document.getElementById("minimap"),
  toast: document.getElementById("toast"),
  gameOver: document.getElementById("game-over"),
  gameOverText: document.getElementById("game-over-text"),
  gameOverButton: document.getElementById("game-over-button"),
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

    this.lobby.show();
    try {
      await this.net.connect();
    } catch (err) {
      this.showConnectionWarning();
    }
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

    this.match = new Match(this.net, payload, (msg) => this.showToast(msg));
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
    dom.gameOverText.textContent = text;
    dom.gameOverText.dataset.verdict = verdict; // lets CSS tint win/lose/draw
    dom.gameOver.hidden = false;
    // Freeze the loop but keep the final frame visible behind the overlay.
    if (this.match) this.match.stop();
  }

  /** "Back to lobby" button: tear down the match and restore the lobby. */
  onBackToLobby() {
    if (this.match) {
      this.match.destroy();
      this.match = null;
    }
    dom.gameOver.hidden = true;
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
  constructor(net, payload, toast) {
    this.net = net;
    this.toast = toast;

    // --- Build the module graph from the static start payload (DESIGN.md §4.1). ---
    this.state = new GameState(payload);
    this.camera = new Camera();
    this.renderer = new Renderer(dom.viewport);
    this.fog = new Fog(this.state.map.width, this.state.map.height);
    this.hud = new HUD(dom.gameScreen, this.state, this.net);
    this.minimap = new Minimap(dom.minimap, this.state, this.camera, this.fog, this.net);
    this.input = new Input(
      dom.viewport,
      this.camera,
      this.state,
      this.net,
      this.renderer,
      this.fog,
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
    this.onSnapshot = (m) => this.state.applySnapshot(m);
    this.onResize = this.handleResize.bind(this);
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    window.addEventListener("resize", this.onResize);

    this.rafId = requestAnimationFrame(this.tickFn);
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

  /** Surface this snapshot's `notice` events as toasts (visual flavor only). */
  drainNotices() {
    const events = this.state.events;
    if (!events || !events.length) return;
    for (const ev of events) {
      if (ev && ev.e === EVENT.NOTICE && ev.msg) this.toast(ev.msg);
    }
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
    this.input.update(dt);
    this.fog.update(this.ownEntities(), this.state.map.tileSize);

    this.renderer.render(this.state, this.camera, this.fog, alpha);
    this.hud.update();
    this.minimap.render();

    this.drainNotices();

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
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    window.removeEventListener("resize", this.onResize);
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

// --- Entry point ---------------------------------------------------------
const app = new App();
app.start();

// Debug/introspection handle. Harmless in production; lets dev tooling and the
// integration tests inspect live match state (e.g. `__rts.match.state.selection`).
if (typeof window !== "undefined") window.__rts = app;
