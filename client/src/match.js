import { Audio, noticeSoundId } from "./audio.js";
import { Camera } from "./camera.js";
import {
  attackKindHasCombatSound,
  machineGunnerHasAudibleTarget,
  machineGunSoundKey,
} from "./combat_audio.js";
import { Fog } from "./fog.js";
import { HUD } from "./hud.js";
import { Input } from "./input/index.js";
import { automaticPointerLockDisabledForTests, shouldRequestPointerLock } from "./input/cursor_lock.js";
import { DomClickInputZone, MatchInputRouter } from "./input/router.js";
import { Minimap } from "./minimap.js";
import { Renderer } from "./renderer/index.js";
import { GameState } from "./state.js";
import { INTERP_DELAY_MS, SNAPSHOT_MS } from "./config.js";
import { EVENT, KIND, NOTICE_SEVERITY, S } from "./protocol.js";
import {
  UNDER_ATTACK_ID,
  VIEWPORT_ALERT_MARGIN_PX,
  noticeAlertId,
  noticeDisplayText,
} from "./alerts.js";
import { dom, isTextEntry } from "./bootstrap.js";

const KAR98K_GAIN = 0.25;
const MG_BURST_GAIN = 0.7;
const MATCH_PING_MS = 2000;
const LATENCY_ISSUE_MS = 180;
const JITTER_ISSUE_MS = 20;
const JITTER_WINDOW = 8;
const AUTO_POINTER_LOCK_SUPPRESS_MS = 1200;
const DESKTOP_POINTER_LOCK_RETRY_ATTEMPTS = 4;
const DESKTOP_POINTER_LOCK_RETRY_DELAY_MS = 120;

const COMBAT_SOUNDS = Object.freeze({
  [KIND.TANK]: {
    ids: ["combat_tank_01"],
    priority: 4,
    gain: 2,
  },
  [KIND.SCOUT_CAR]: {
    ids: ["combat_mg_burst_02", "combat_mg_burst_03"],
    priority: 2.5,
    gain: MG_BURST_GAIN,
  },
  [KIND.RIFLEMAN]: {
    ids: ["combat_rifle_02", "combat_rifle_03"],
    priority: 2,
    gain: KAR98K_GAIN,
  },
  [KIND.AT_TEAM]: {
    ids: ["combat_tank_01"],
    priority: 4,
    gain: 2,
  },
  [KIND.MACHINE_GUNNER]: {
    ids: ["combat_mg_burst_02", "combat_mg_burst_03"],
    priority: 2.5,
    gain: MG_BURST_GAIN,
  },
});

export class Match {
  /**
   * @param {Net} net live connection (shared, not owned)
   * @param {object} payload §2.3 start payload
   * @param {(msg: string) => void} toast surface a notice in the App's toast
   */
  constructor(net, payload, toast, devWatch, audio, statusBadge, diagnostics = null) {
    this.net = net;
    this.toast = toast;
    this.devWatch = devWatch;
    this.audio = audio;
    this.statusBadge = statusBadge;
    this.diagnostics = diagnostics;
    this.missingCombatSoundKinds = new Set();
    this.activeMachineGunSoundKeys = new Map();
    this.replaySpeedHandler = null;
    this.giveUpSent = false;
    this.matchPingTimer = undefined;
    this.lastLatencySampleAt = 0;
    this.snapshotJitterDeltas = [];
    this.lastSnapshotArrivedAt = null;
    this.lastServerNetStatus = null;
    this.autoPointerLockUntil = 0;
    this.pointerLockDiagnosticShown = false;
    this.pointerLockRetryToken = 0;
    this.pointerLockRetry = null;
    this.health = {
      latencyMs: null,
      serverTickMs: null,
      serverLagMs: null,
      jitterMs: null,
      issues: {
        latency: { active: false, count: 0 },
        slowTick: { active: false, count: 0 },
        headOfLine: { active: false, count: 0 },
        jitter: { active: false, count: 0 },
      },
    };

    // --- Build the module graph from the static start payload (docs/design/client-ui.md §4.1). ---
    this.state = this._timeInit("match.state", () => new GameState(payload));
    this.state.showAllDebugPathOverlays = this.devWatch?.kind === "scenario";
    this.camera = this._timeInit("match.camera", () => new Camera());
    this.renderer = this._timeInit("match.renderer", () => new Renderer(dom.viewport));
    this.fog = this._timeInit(
      "match.fog",
      () => new Fog(this.state.map.width, this.state.map.height, this.state.map.terrain),
    );
    this.fog.setRevealAll(!!this.devWatch?.noFog);
    this.hud = this._timeInit("match.hud", () => new HUD(dom.gameScreen, this.state, this.net));
    this.inputRouter = this._timeInit("match.inputRouter", () => new MatchInputRouter(dom.viewport));
    this.hudInputZone = this._timeInit(
      "match.hudInputZone",
      () => new DomClickInputZone([dom.gameMenu, dom.commandCard]),
    );
    this.unregisterHudInputZone = this.inputRouter.registerZone(this.hudInputZone);
    this.minimap = this._timeInit(
      "match.minimap",
      () => new Minimap(dom.minimap, this.state, this.camera, this.fog, this.net, this.inputRouter),
    );
    this.input = this._timeInit(
      "match.input",
      () => new Input(
        dom.viewport,
        this.camera,
        this.state,
        this.net,
        this.renderer,
        this.fog,
        this.audio,
        this.inputRouter,
      ),
    );

    // Draw the static terrain once into the renderer's cached layer.
    this._timeInit("match.staticMap", () => this.renderer.buildStaticMap(this.state.map));

    // Size the camera to the map and the current viewport, then center on home.
    this._timeInit("match.bounds", () => {
      this.applyBounds();
      this.centerOnHome();
    });

    // --- Render loop state. ---
    this.running = true;
    this.lastFrame = performance.now();
    this.tickFn = this.frame.bind(this);
    this.rafId = undefined;

    // --- Listeners (bound so they can be removed on destroy). ---
    this.onSnapshot = (m) => {
      const now = performance.now();
      this.noteSnapshotArrival(now);
      this.state.applySnapshot(m);
      this.lastServerNetStatus = m?.netStatus || null;
      this.applyServerNetStatus();
      this.stopInactiveMachineGunSounds();
      this.handleSnapshotEvents(m.events || []);
    };
    this.onResize = this.handleResize.bind(this);
    this.onMenuKeyDown = this.handleMenuKeyDown.bind(this);
    this.onWindowFocus = this.handleWindowFocus.bind(this);
    this.onVisibilityChange = this.handleVisibilityChange.bind(this);
    this.onPointerLockGesture = this.handlePointerLockGesture.bind(this);
    this.onSettingsClick = this.toggleSettingsMenu.bind(this);
    this.onGiveUpOpen = this.openGiveUpConfirm.bind(this);
    this.onGiveUpCancel = this.closeGiveUpConfirm.bind(this);
    this.onGiveUpConfirm = this.requestGiveUp.bind(this);
    this.onPointerLockToggle = this.togglePointerLock.bind(this);
    this.onPointerLockChange = this.handlePointerLockChange.bind(this);
    this.onPointerLockError = this.handlePointerLockError.bind(this);
    this.input.onPointerLockChange = this.onPointerLockChange;
    this.input.onPointerLockError = this.onPointerLockError;
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    window.addEventListener("resize", this.onResize);
    window.addEventListener("focus", this.onWindowFocus);
    window.addEventListener("keydown", this.onMenuKeyDown, true);
    window.addEventListener("keydown", this.onPointerLockGesture, true);
    window.addEventListener("click", this.onPointerLockGesture, true);
    document.addEventListener("visibilitychange", this.onVisibilityChange);
    dom.settingsButton?.addEventListener("click", this.onSettingsClick);
    dom.pointerLockToggle?.addEventListener("click", this.onPointerLockToggle);
    dom.giveUpOpen?.addEventListener("click", this.onGiveUpOpen);
    dom.giveUpCancel?.addEventListener("click", this.onGiveUpCancel);
    dom.giveUpConfirmButton?.addEventListener("click", this.onGiveUpConfirm);
    this.syncPointerLockUi();

    this.rafId = requestAnimationFrame(this.tickFn);
    this.startMatchPings();
    this.publishStatusBadge();
    this.requestAutomaticPointerLock({ requireGesture: false });

    // Show speed controls for replay and scenario dev-watch rooms.
    const isReplay = this.devWatch?.kind === "replay";
    const isScenario = this.devWatch?.kind === "scenario";
    if ((isReplay || isScenario) && dom.replaySpeed) {
      dom.replaySpeed.hidden = false;
      for (const btn of dom.replaySpeed.querySelectorAll(".seek-btn")) {
        btn.hidden = isScenario;
      }
      this.replaySpeedHandler = (e) => {
        const btn = e.target.closest(".spd-btn");
        if (!btn) return;
        if (btn.dataset.seekBack !== undefined) {
          if (!isReplay) return;
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
    this.applySpectatorUi();
  }

  _timeInit(label, fn) {
    return this.diagnostics?.time(label, undefined, fn) ?? fn();
  }

  startMatchPings() {
    this.stopMatchPings();
    this.net.ping();
    this.matchPingTimer = window.setInterval(() => this.net.ping(), MATCH_PING_MS);
  }

  stopMatchPings() {
    if (this.matchPingTimer !== undefined) {
      clearInterval(this.matchPingTimer);
      this.matchPingTimer = undefined;
    }
  }

  noteSnapshotArrival(now) {
    if (!document.hidden && this.lastSnapshotArrivedAt != null) {
      const delta = Math.abs((now - this.lastSnapshotArrivedAt) - SNAPSHOT_MS);
      this.snapshotJitterDeltas.push(delta);
      if (this.snapshotJitterDeltas.length > JITTER_WINDOW) {
        this.snapshotJitterDeltas.splice(0, this.snapshotJitterDeltas.length - JITTER_WINDOW);
      }
      this.health.jitterMs = Math.max(...this.snapshotJitterDeltas, 0);
      const jitterActive = delta >= JITTER_ISSUE_MS;
      this.health.issues.jitter.active = jitterActive;
      if (jitterActive) {
        this.health.issues.jitter.count += 1;
      }
    }
    this.lastSnapshotArrivedAt = now;
  }

  applyServerNetStatus() {
    const status = this.lastServerNetStatus;
    if (!status) return;
    this.health.serverTickMs = status.tickMs;
    this.health.serverLagMs = status.serverLagMs;
    this.health.issues.slowTick.active = !!status.slowTick;
    this.health.issues.slowTick.count = status.slowTickCount || 0;
    this.health.issues.headOfLine.active = !!status.headOfLine;
    this.health.issues.headOfLine.count = status.headOfLineCount || 0;
  }

  refreshLatencyHealth() {
    if (this.net.latencyUpdatedAt === this.lastLatencySampleAt) return;
    this.lastLatencySampleAt = this.net.latencyUpdatedAt;
    this.health.latencyMs = this.net.latency;
    const latencyActive = Number.isFinite(this.net.latency) && this.net.latency >= LATENCY_ISSUE_MS;
    this.health.issues.latency.active = latencyActive;
    if (latencyActive) {
      this.health.issues.latency.count += 1;
    }
  }

  publishStatusBadge() {
    this.statusBadge?.setMatchMetrics({
      latencyMs: this.health.latencyMs,
      serverTickMs: this.health.serverTickMs,
      serverLagMs: this.health.serverLagMs,
      jitterMs: this.health.jitterMs,
      issues: this.health.issues,
    });
  }

  applySpectatorUi() {
    const spectator = !!this.state?.spectator;
    if (dom.giveUpOpen) dom.giveUpOpen.hidden = spectator;
    if (dom.commandCard) dom.commandCard.hidden = spectator;
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

  handleWindowFocus() {
    this.requestAutomaticPointerLock({ requireGesture: false });
  }

  handleVisibilityChange() {
    if (!document.hidden) this.requestAutomaticPointerLock({ requireGesture: false });
  }

  handlePointerLockGesture(ev) {
    if (ev.code === "Escape" || isTextEntry(ev.target)) return;
    if (ev.type === "click" && !dom.viewport?.contains(ev.target)) return;
    this.requestAutomaticPointerLock({ requireGesture: true });
  }

  requestAutomaticPointerLock({ requireGesture = false } = {}) {
    if (!this.input || !this.input.pointerLockSupported()) return;
    if (automaticPointerLockDisabledForTests()) return;
    const isDesktop = this.input.desktopRuntime();
    if (!shouldRequestPointerLock({ desktopRuntime: isDesktop, requireGesture })) return;
    if (this.input.pointerLocked) return;
    this.autoPointerLockUntil = performance.now() + AUTO_POINTER_LOCK_SUPPRESS_MS;
    const maxAttempts = isDesktop && requireGesture ? DESKTOP_POINTER_LOCK_RETRY_ATTEMPTS : 1;
    this.pointerLockRetryToken += 1;
    void this.runPointerLockRetryBurst(this.pointerLockRetryToken, maxAttempts);
  }

  async runPointerLockRetryBurst(token, maxAttempts) {
    this.pointerLockRetry = {
      startedAt: new Date().toISOString(),
      attempts: 0,
      maxAttempts,
      lastResult: null,
      stopped: null,
    };

    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      if (token !== this.pointerLockRetryToken || !this.running) {
        this.pointerLockRetry.stopped = "superseded";
        break;
      }
      if (!this.input || !this.input.pointerLockSupported()) {
        this.pointerLockRetry.stopped = "unavailable";
        break;
      }
      if (this.input.pointerLocked) {
        this.pointerLockRetry.stopped = "already-locked";
        break;
      }
      if (this.input.desktopRuntime() && typeof document.hasFocus === "function" && !document.hasFocus()) {
        this.pointerLockRetry.stopped = "document-not-focused";
        break;
      }

      this.pointerLockRetry.attempts = attempt;
      const locked = await this.input.requestPointerLock();
      this.pointerLockRetry.lastResult = locked ? "locked" : "not-locked";
      if (locked || this.input.pointerLocked) {
        this.pointerLockRetry.stopped = "locked";
        break;
      }
      if (attempt < maxAttempts) await this.waitPointerLockRetryDelay();
    }

    if (!this.pointerLockRetry.stopped) this.pointerLockRetry.stopped = "exhausted";
    window.setTimeout(() => {
      if (performance.now() >= this.autoPointerLockUntil) this.autoPointerLockUntil = 0;
    }, AUTO_POINTER_LOCK_SUPPRESS_MS);
  }

  waitPointerLockRetryDelay() {
    return new Promise((resolve) => window.setTimeout(resolve, DESKTOP_POINTER_LOCK_RETRY_DELAY_MS));
  }

  automaticPointerLockActive() {
    return performance.now() <= this.autoPointerLockUntil;
  }

  toggleSettingsMenu() {
    if (!dom.settingsMenu || this.giveUpSent) return;
    if (dom.giveUpConfirm && !dom.giveUpConfirm.hidden) this.closeGiveUpConfirm();
    this.syncPointerLockUi();
    dom.settingsMenu.hidden = !dom.settingsMenu.hidden;
    dom.settingsButton?.setAttribute("aria-expanded", String(!dom.settingsMenu.hidden));
  }

  closeSettingsMenu() {
    if (!dom.settingsMenu) return;
    dom.settingsMenu.hidden = true;
    dom.settingsButton?.setAttribute("aria-expanded", "false");
  }

  openGiveUpConfirm() {
    if (this.state?.spectator) return;
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
    if (this.state?.spectator) return;
    if (this.giveUpSent) return;
    this.giveUpSent = true;
    if (dom.giveUpConfirmButton) {
      dom.giveUpConfirmButton.disabled = true;
      dom.giveUpConfirmButton.textContent = "Giving up...";
    }
    this.net.giveUp();
  }

  togglePointerLock() {
    if (!this.input?.pointerLockSupported()) {
      this.toast("Cursor lock is not supported by this browser.");
      this.syncPointerLockUi();
      return;
    }
    this.autoPointerLockUntil = 0;
    if (!this.input.pointerLocked) this.closeSettingsMenu();
    void this.input.togglePointerLock();
  }

  handlePointerLockChange(locked) {
    if (locked) {
      this.closeSettingsMenu();
      if (!this.automaticPointerLockActive()) this.toast("Cursor locked. Press Esc to unlock.");
    }
    this.syncPointerLockUi();
  }

  handlePointerLockError(err) {
    this.recordPointerLockDiagnostic(err);
    if (this.automaticPointerLockActive()) return;
    this.toast("Cursor lock was blocked. Click the game view and try again.");
    this.syncPointerLockUi();
  }

  recordPointerLockDiagnostic(err = null) {
    if (!this.input?.desktopRuntime()) return;
    const snapshot = {
      at: new Date().toISOString(),
      error: this.pointerLockErrorSummary(err),
      retry: this.pointerLockRetry,
      support: this.input.pointerLockDebugSnapshot(),
    };
    if (typeof window !== "undefined") window.__rtsPointerLockDebug = snapshot;
    if (this.pointerLockDiagnosticShown) return;
    this.pointerLockDiagnosticShown = true;
    console.warn("[RTS_POINTER_LOCK_DESKTOP]", snapshot);
    this.toast("Desktop cursor lock failed. Inspect window.__rtsPointerLockDebug.");
  }

  pointerLockErrorSummary(err) {
    if (!err) return null;
    if (err instanceof Error) return { name: err.name, message: err.message };
    if (typeof err === "object") {
      return {
        type: err.type || null,
        name: err.name || null,
        message: err.message || null,
      };
    }
    return { message: String(err) };
  }

  syncPointerLockUi() {
    const btn = dom.pointerLockToggle;
    if (!btn || !this.input) return;
    btn.hidden = false;
    const supported = this.input.pointerLockSupported();
    const locked = this.input.pointerLocked;
    btn.disabled = !supported;
    btn.setAttribute("aria-checked", String(locked));
    btn.textContent = locked ? "Cursor locked (Esc)" : "Lock cursor pan";
    btn.title = supported
      ? "Trap the cursor in the game view for multi-monitor edge panning."
      : "Cursor lock is not supported by this browser.";
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

  /** Center the camera on this player's own starting tile (City Centre location). */
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
        this.handleNotice(ev);
      } else if (ev && ev.e === EVENT.ATTACK) {
        this.playAttackSound(ev);
      }
    }
  }

  handleNotice(ev) {
    const alertId = noticeAlertId(ev.msg);
    const severity = ev.severity || (alertId ? NOTICE_SEVERITY.ALERT : NOTICE_SEVERITY.INFO);
    this.toast(noticeDisplayText(ev.msg));

    const hasPos = Number.isFinite(ev.x) && Number.isFinite(ev.y);
    const isAlert = severity === NOTICE_SEVERITY.ALERT || !!alertId;
    if (isAlert) {
      if (hasPos) this.minimap?.ping(ev.x, ev.y, severity);
      else this.minimap?.pulseBorder();
    }

    if (!this.audio) return;
    if (alertId === UNDER_ATTACK_ID && hasPos && this.pointInViewport(ev.x, ev.y, VIEWPORT_ALERT_MARGIN_PX)) {
      return;
    }
    const opts = {
      category: isAlert ? "alert" : "ui",
      priority: isAlert ? 3 : 1,
      alertId,
    };
    if (hasPos) {
      opts.alertX = ev.x;
      opts.alertY = ev.y;
    }
    const soundId = noticeSoundId(ev.msg);
    if (soundId) this.audio.play(soundId, opts);
  }

  pointInViewport(x, y, marginPx = 0) {
    const zoom = this.camera.zoom || 1;
    const margin = marginPx / zoom;
    const left = this.camera.x - margin;
    const top = this.camera.y - margin;
    const right = this.camera.x + this.camera.viewW / zoom + margin;
    const bottom = this.camera.y + this.camera.viewH / zoom + margin;
    return x >= left && x <= right && y >= top && y <= bottom;
  }

  playAttackSound(ev) {
    if (!this.audio) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const to = typeof ev.to === "number" ? this.state.entityById(ev.to) : null;
    const pos = from || to;
    if (!pos || typeof pos.x !== "number" || typeof pos.y !== "number") return;

    const kind = from?.kind || KIND.RIFLEMAN;
    if (!attackKindHasCombatSound(kind)) return;
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
   * Order matches docs/design/client-ui.md §4.1 main.js loop description.
   * @param {number} now high-res timestamp from rAF
   */
  frame(now) {
    if (!this.running) return;

    const dt = (now - this.lastFrame) / 1000; // seconds since last frame
    this.lastFrame = now;
    this.refreshLatencyHealth();

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
    this.fog.update(this.ownEntities(), this.state.map.tileSize, this.state.visibleTiles);

    this.renderer.render(this.state, this.camera, this.fog, alpha);
    this.hud.update();
    this.minimap.render();
    this.publishStatusBadge();

    this.rafId = requestAnimationFrame(this.tickFn);
  }

  /**
   * Entities used to drive the local fog overlay.
   * Spectators receive the server-filtered union of all players' visible entities, so every
   * non-resource entity in their snapshot contributes to the local overlay.
   * Resource nodes (owner 0) never grant vision.
   * @returns {object[]}
   */
  ownEntities() {
    const all = this.state
      .entitiesInterpolated(1)
      .filter((e) => !e.shotReveal && !e.visionOnly);
    if (this.state.spectator) {
      return all.filter((e) => e.owner !== 0);
    }
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
    this.stopMatchPings();
    this.stopAllMachineGunSounds();
    this.pointerLockRetryToken += 1;
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    window.removeEventListener("resize", this.onResize);
    window.removeEventListener("focus", this.onWindowFocus);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    window.removeEventListener("keydown", this.onPointerLockGesture, true);
    window.removeEventListener("click", this.onPointerLockGesture, true);
    document.removeEventListener("visibilitychange", this.onVisibilityChange);
    dom.settingsButton?.removeEventListener("click", this.onSettingsClick);
    dom.pointerLockToggle?.removeEventListener("click", this.onPointerLockToggle);
    dom.giveUpOpen?.removeEventListener("click", this.onGiveUpOpen);
    dom.giveUpCancel?.removeEventListener("click", this.onGiveUpCancel);
    dom.giveUpConfirmButton?.removeEventListener("click", this.onGiveUpConfirm);
    if (dom.replaySpeed && this.replaySpeedHandler) {
      dom.replaySpeed.removeEventListener("click", this.replaySpeedHandler);
      dom.replaySpeed.hidden = true;
      for (const btn of dom.replaySpeed.querySelectorAll(".seek-btn")) btn.hidden = false;
    }
    if (dom.giveUpOpen) dom.giveUpOpen.hidden = false;
    if (dom.commandCard) dom.commandCard.hidden = false;
    if (this.unregisterHudInputZone) {
      this.unregisterHudInputZone();
      this.unregisterHudInputZone = null;
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
