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
    this.onPointerLockToggle = this.togglePointerLock.bind(this);
    this.onPointerLockChange = this.handlePointerLockChange.bind(this);
    this.onPointerLockError = this.handlePointerLockError.bind(this);
    this.input.onPointerLockChange = this.onPointerLockChange;
    this.input.onPointerLockError = this.onPointerLockError;
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    window.addEventListener("resize", this.onResize);
    window.addEventListener("keydown", this.onMenuKeyDown, true);
    dom.settingsButton?.addEventListener("click", this.onSettingsClick);
    dom.pointerLockToggle?.addEventListener("click", this.onPointerLockToggle);
    dom.giveUpOpen?.addEventListener("click", this.onGiveUpOpen);
    dom.giveUpCancel?.addEventListener("click", this.onGiveUpCancel);
    dom.giveUpConfirmButton?.addEventListener("click", this.onGiveUpConfirm);
    this.syncPointerLockUi();

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
    this.applySpectatorUi();
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
    if (!this.input.pointerLocked) this.closeSettingsMenu();
    void this.input.togglePointerLock();
  }

  handlePointerLockChange(locked) {
    if (locked) {
      this.closeSettingsMenu();
      this.toast("Cursor locked. Press Esc to unlock.");
    }
    this.syncPointerLockUi();
  }

  handlePointerLockError() {
    this.toast("Cursor lock was blocked. Click the game view and try again.");
    this.syncPointerLockUi();
  }

  syncPointerLockUi() {
    const btn = dom.pointerLockToggle;
    if (!btn || !this.input) return;
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
    this.stopAllMachineGunSounds();
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    window.removeEventListener("resize", this.onResize);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    dom.settingsButton?.removeEventListener("click", this.onSettingsClick);
    dom.pointerLockToggle?.removeEventListener("click", this.onPointerLockToggle);
    dom.giveUpOpen?.removeEventListener("click", this.onGiveUpOpen);
    dom.giveUpCancel?.removeEventListener("click", this.onGiveUpCancel);
    dom.giveUpConfirmButton?.removeEventListener("click", this.onGiveUpConfirm);
    if (dom.replaySpeed && this.replaySpeedHandler) {
      dom.replaySpeed.removeEventListener("click", this.replaySpeedHandler);
      dom.replaySpeed.hidden = true;
    }
    if (dom.giveUpOpen) dom.giveUpOpen.hidden = false;
    if (dom.commandCard) dom.commandCard.hidden = false;
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
