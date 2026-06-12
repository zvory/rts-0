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
import { MatchHealth } from "./match_health.js";
import { PredictionController } from "./prediction_controller.js";
import { Renderer } from "./renderer/index.js";
import { ReplayCameraInput } from "./replay_camera_input.js";
import { ReplayControls } from "./replay_controls.js";
import { SimWasmPredictionAdapter } from "./sim_wasm_adapter.js";
import { GameState } from "./state.js";
import { INTERP_DELAY_MS, SNAPSHOT_MS } from "./config.js";
import { EVENT, KIND, NOTICE_SEVERITY, PREDICTION_PROTOCOL_VERSION, S } from "./protocol.js";
import {
  UNDER_ATTACK_ID,
  VIEWPORT_ALERT_MARGIN_PX,
  noticeAlertId,
  noticeDisplayText,
} from "./alerts.js";
import { dom, isTextEntry } from "./bootstrap.js";
import { buildGiveUpAction, buildSettingsTabs } from "./settings_panels.js";

const KAR98K_GAIN = 0.25;
const MG_BURST_GAIN = 0.7;
const MORTAR_LAUNCH_GAIN = 0.85;
const ARTILLERY_FIRE_GAIN = 1.2;
const MATCH_PING_MS = 2000;
const NET_REPORT_MS = 10000;
const PREDICTION_REPLAY_BUDGET_MS = 4;
const AUTO_POINTER_LOCK_SUPPRESS_MS = 1200;
const INSTALLED_APP_POINTER_LOCK_RETRY_ATTEMPTS = 4;
const INSTALLED_APP_POINTER_LOCK_RETRY_DELAY_MS = 120;
const POINTER_LOCK_PAN_STORAGE_KEY = "rts.lockCursorPan";

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

const POINT_FIRE_SOUNDS = Object.freeze({
  [EVENT.MORTAR_LAUNCH]: {
    id: "combat_mortar_launch_04",
    priority: 3.5,
    gain: MORTAR_LAUNCH_GAIN,
  },
  [EVENT.ARTILLERY_TARGET]: {
    id: "combat_artillery_fire_05",
    priority: 4.5,
    gain: ARTILLERY_FIRE_GAIN,
  },
});

export class Match {
  /**
   * @param {Net} net live connection (shared, not owned)
   * @param {object} payload §2.3 start payload
   * @param {(msg: string) => void} toast surface a notice in the App's toast
   */
  constructor(net, payload, toast, devWatch, audio, statusBadge, diagnostics = null, options = {}) {
    this.net = net;
    this.toast = toast;
    this.devWatch = devWatch;
    this.audio = audio;
    this.statusBadge = statusBadge;
    this.diagnostics = diagnostics;
    this.hotkeyProfiles = options.hotkeyProfiles || null;
    this.settings = options.settings || null;
    this.onPredictionEnabledChange = options.onPredictionEnabledChange || null;
    this.replayViewer = !!options.replayViewer;
    this.predictionStateMismatchLogged = false;
    this.missingCombatSoundKinds = new Set();
    this.activeMachineGunSoundKeys = new Map();
    this.replayControls = null;
    this.giveUpSent = false;
    this.matchPingTimer = undefined;
    this.netReportTimer = undefined;
    this.skipFinalNetReport = false;
    this.lastSnapshotTick = 0;
    this.health = new MatchHealth({ net: this.net, statusBadge: this.statusBadge, snapshotMs: SNAPSHOT_MS });
    this.predictionStartInfo = payload;
    this.predictionPlayerId = payload?.playerId;
    this.predictionCompatibility = predictionCompatibility(payload);
    this.predictionAdapter = this.createPredictionAdapter();
    this.predictionInitToken = 0;
    this.prediction = new PredictionController({
      enabled: !!options.predictionEnabled && !this.replayViewer && !payload?.spectator && this.predictionCompatibility.ok,
      predictor: this.predictionAdapter,
      sendCommand: (command, clientSeq) => this.net.command(command, clientSeq),
    });
    if (!!options.predictionEnabled && !this.prediction.enabled && this.predictionCompatibility.reason) {
      this.prediction.recordDisableReason(this.predictionCompatibility.reason);
    }
    this.commandIssuer = {
      issueCommand: (command, options = {}) => {
        const issued = this.prediction.issueCommand(command, options);
        this.state?.setOptimisticCommandState(this.prediction.optimisticUiState());
        return issued;
      },
    };
    this.autoPointerLockUntil = 0;
    this.pointerLockPanEnabled = this.readPointerLockPanEnabled();
    this.pointerLockDiagnosticShown = false;
    this.pointerLockRetryToken = 0;
    this.pointerLockRetry = null;

    // --- Build the module graph from the static start payload (docs/design/client-ui.md §4.1). ---
    this.state = this._timeInit("match.state", () => new GameState(payload));
    this.state.debugPathOverlaysAvailable =
      this.state.debugPathOverlaysAvailable || this.devWatch?.kind === "scenario";
    this.state.debugPathOverlaysEnabled = this.state.debugPathOverlaysAvailable;
    this.state.showAllDebugPathOverlays = this.devWatch?.kind === "scenario";
    this.camera = this._timeInit("match.camera", () => new Camera());
    this.renderer = this._timeInit("match.renderer", () => new Renderer(dom.viewport));
    this.fog = this._timeInit(
      "match.fog",
      () => new Fog(this.state.map.width, this.state.map.height, this.state.map.terrain),
    );
    this.fog.setRevealAll(!!this.devWatch?.noFog);
    this.hud = this._timeInit(
      "match.hud",
      () => new HUD(dom.gameScreen, this.state, this.commandIssuer, this.audio, this.hotkeyProfiles),
    );
    this.inputRouter = this._timeInit("match.inputRouter", () => new MatchInputRouter(dom.viewport));
    this.hudInputZone = this._timeInit(
      "match.hudInputZone",
      () => new DomClickInputZone([dom.gameMenu, dom.commandCard]),
    );
    this.unregisterHudInputZone = this.inputRouter.registerZone(this.hudInputZone);
    this.minimap = this._timeInit(
      "match.minimap",
      () => new Minimap(dom.minimap, this.state, this.camera, this.fog, this.commandIssuer, this.inputRouter, {
        commandsEnabled: !this.replayViewer,
      }),
    );
    this.input = this._timeInit(
      "match.input",
      () => this.replayViewer
        ? new ReplayCameraInput(dom.viewport)
        : new Input(
          dom.viewport,
          this.camera,
          this.state,
          this.commandIssuer,
          this.renderer,
          this.fog,
          this.audio,
          this.inputRouter,
          this.hotkeyProfiles,
        ),
    );

    // Draw the static terrain once into the renderer's cached layer.
    this._timeInit("match.staticMap", () => this.renderer.buildStaticMap(this.state.map));

    // Size the camera to the map and the current viewport, then restore a carried view or center on home.
    this._timeInit("match.bounds", () => {
      this.applyBounds();
      if (options.initialCamera) this.camera.setView(options.initialCamera);
      else this.centerOnHome();
    });

    // --- Render loop state. ---
    this.running = true;
    this.lastFrame = performance.now();
    this.tickFn = this.frame.bind(this);
    this.rafId = undefined;

    // --- Listeners (bound so they can be removed on destroy). ---
    this.onSnapshot = (m) => {
      const now = performance.now();
      this.health.noteSnapshotArrival(now, document.hidden);
      this.prediction.applyAuthoritativeSnapshot(m);
      this.state.applySnapshot(m);
      this.state.setOptimisticCommandState(this.prediction.optimisticUiState());
      this.applyPredictedSnapshot();
      this.lastSnapshotTick = Number.isFinite(m?.tick) ? m.tick : this.lastSnapshotTick;
      this.replayControls?.noteSnapshotTick(m?.tick);
      this.health.applyServerNetStatus(m?.netStatus || null);
      this.stopInactiveMachineGunSounds();
      this.handleSnapshotEvents(m.events || []);
    };
    this.onReplayState = (m) => this.applyReplayState(m);
    this.onResize = this.handleResize.bind(this);
    this.onMenuKeyDown = this.handleMenuKeyDown.bind(this);
    this.onWindowFocus = this.handleWindowFocus.bind(this);
    this.onVisibilityChange = this.handleVisibilityChange.bind(this);
    this.onPointerLockGesture = this.handlePointerLockGesture.bind(this);
    this.onGiveUpOpen = this.openGiveUpConfirm.bind(this);
    this.onGiveUpCancel = this.closeGiveUpConfirm.bind(this);
    this.onGiveUpConfirm = this.requestGiveUp.bind(this);
    this.onPointerLockToggle = this.togglePointerLock.bind(this);
    this.onDebugPathToggle = this.toggleDebugPathOverlays.bind(this);
    this.onPointerLockChange = this.handlePointerLockChange.bind(this);
    this.onPointerLockError = this.handlePointerLockError.bind(this);
    if (!this.replayViewer) {
      this.input.onPointerLockChange = this.onPointerLockChange;
      this.input.onPointerLockError = this.onPointerLockError;
    }
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    this.net.on(S.REPLAY_STATE, this.onReplayState);
    window.addEventListener("resize", this.onResize);
    window.addEventListener("focus", this.onWindowFocus);
    window.addEventListener("keydown", this.onMenuKeyDown, true);
    if (!this.replayViewer) {
      window.addEventListener("keydown", this.onPointerLockGesture, true);
      window.addEventListener("click", this.onPointerLockGesture, true);
    }
    document.addEventListener("visibilitychange", this.onVisibilityChange);
    if (!this.replayViewer) {
      dom.giveUpCancel?.addEventListener("click", this.onGiveUpCancel);
      dom.giveUpConfirmButton?.addEventListener("click", this.onGiveUpConfirm);
    }
    this.mountSettings();

    this.rafId = requestAnimationFrame(this.tickFn);
    this.startMatchPings();
    this.startNetReports();
    this.health.publish();
    this.requestAutomaticPointerLock({ requireGesture: false });
    if (this.prediction.enabled) this.initPredictionAdapter();

    // Show speed controls for replay and scenario dev-watch rooms.
    const isReplay = !!payload?.replay;
    const isScenario = this.devWatch?.kind === "scenario";
    if ((isReplay || isScenario) && dom.replaySpeed) {
      this.replayControls = new ReplayControls({
        net: this.net,
        state: this.state,
        replayViewer: this.replayViewer,
        isReplay,
        isScenario,
      });
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

  startNetReports() {
    this.stopNetReports();
    this.netReportTimer = window.setInterval(() => this.sendNetReport(), NET_REPORT_MS);
  }

  stopNetReports() {
    if (this.netReportTimer !== undefined) {
      clearInterval(this.netReportTimer);
      this.netReportTimer = undefined;
    }
  }

  sendNetReport() {
    const stats = this.health.reportStats;
    const metrics = this.health.metrics();
    const elapsedMs = performance.now() - this.health.reportStartedAt;
    const avgFrameMs = stats.frameCount > 0 ? stats.frameTotalMs / stats.frameCount : 0;
    const report = {
      schemaVersion: 1,
      elapsedMs: clampU32(elapsedMs),
      matchTick: clampU32(this.lastSnapshotTick),
      rttMs: clampU16(metrics.latencyMs),
      rttMaxMs: clampU16(stats.rttMaxMs),
      badRttSamples: clampU32(stats.badRttSamples),
      snapshotJitterMs: clampU16(metrics.jitterMs),
      snapshotGapMaxMs: clampU16(stats.snapshotGapMaxMs),
      jitterSamples: clampU32(stats.jitterSamples),
      snapshots: clampU32(stats.snapshots),
      frameGapMaxMs: clampU16(stats.frameGapMaxMs),
      fpsEstimate: clampU16(avgFrameMs > 0 ? 1000 / avgFrameMs : 0),
      hidden: !!document.hidden,
      focused: typeof document.hasFocus === "function" ? document.hasFocus() : true,
      wsBufferedBytes: clampU32(this.net.bufferedAmount),
      serverTickMs: clampU16(metrics.serverTickMs),
      serverLagMs: clampU16(metrics.serverLagMs),
      slowTickCount: clampU32(metrics.issues.slowTick.count),
      headOfLineCount: clampU32(metrics.issues.headOfLine.count),
      ...this.predictionReportFields(),
    };
    this.net.netReport(report);
    this.diagnostics?.count("client.send.netReport", {
      rttMs: report.rttMs,
      rttMaxMs: report.rttMaxMs,
      snapshotGapMaxMs: report.snapshotGapMaxMs,
      jitterSamples: report.jitterSamples,
      wsBufferedBytes: report.wsBufferedBytes,
      predictionMode: report.predictionMode,
      pendingCommandCount: report.pendingCommandCount,
      correctionDistancePx: report.correctionDistancePx,
    });
    this.health.resetReportStats();
  }

  applyPredictedSnapshot() {
    if (!this.predictionStateCompatible()) {
      this.disablePredictionForStateMismatch();
      return;
    }
    if (!this.prediction.enabled || !this.predictionAdapter.ready) {
      this.state.clearPredictedSnapshot?.();
      this.publishPredictionDebug();
      return;
    }
    const snapshot = this.predictionAdapter.renderSnapshot();
    if (!snapshot) return;
    const diagnostics = this.predictionAdapter.diagnostics();
    if (this.disablePredictionForReplayBudget(diagnostics)) return;
    this.state.setPredictedSnapshot(snapshot, diagnostics, {
      smoothCorrections: true,
    });
    this.publishPredictionDebug();
  }

  advancePredictionVisual() {
    if (!this.predictionStateCompatible()) {
      this.disablePredictionForStateMismatch();
      return;
    }
    if (!this.prediction.enabled || !this.predictionAdapter.ready) return;
    const snapshot = this.predictionAdapter.advanceVisual();
    if (snapshot) {
      const diagnostics = this.predictionAdapter.diagnostics();
      if (this.disablePredictionForReplayBudget(diagnostics)) return;
      this.state.setPredictedSnapshot(snapshot, diagnostics);
      this.publishPredictionDebug();
    }
  }

  disablePredictionForReplayBudget(diagnostics) {
    if (!this.prediction.enabled || !(diagnostics?.budgetExceededCount > 0)) return false;
    this.prediction.reset({ enabled: true, preserveClientSeq: true, reason: "replay-budget-exceeded" });
    this.resetPredictionAdapter();
    this.state?.clearPredictedSnapshot?.();
    this.publishPredictionDebug();
    this.logPredictionStatus("tracking-replay-budget-exceeded");
    return true;
  }

  publishPredictionDebug() {
    if (typeof window === "undefined") return;
    window.__rtsPredictionDebug = {
      compatibility: this.predictionCompatibility,
      controller: this.prediction.debugSummary(),
      wasm: this.predictionAdapter.diagnostics(),
    };
  }

  logPredictionStatus(status) {
    const debug = {
      status,
      compatibility: this.predictionCompatibility,
      controller: this.prediction.debugSummary(),
      wasm: this.predictionAdapter.diagnostics(),
    };
    if (typeof window !== "undefined") window.__rtsPredictionDebug = debug;
    console.info("[RTS_PREDICTION]", debug);
  }

  predictionStateCompatible() {
    return typeof this.state?.setPredictedSnapshot === "function";
  }

  disablePredictionForStateMismatch() {
    if (!this.prediction.enabled) return;
    this.prediction.reset({ enabled: false, preserveClientSeq: true, reason: "state-mismatch" });
    this.state?.setOptimisticCommandState?.(null);
    if (!this.predictionStateMismatchLogged) {
      this.predictionStateMismatchLogged = true;
      this.logPredictionStatus("disabled-state-mismatch");
    }
  }

  setPredictionEnabled(enabled) {
    const blockedReason = predictionBlockedReason({
      enabled,
      replayViewer: this.replayViewer,
      spectator: this.state?.spectator,
      compatibility: this.predictionCompatibility,
    });
    const allowed = !blockedReason;
    this.prediction.reset({ enabled: allowed, preserveClientSeq: true, reason: blockedReason });
    if (!allowed) {
      this.predictionInitToken += 1;
      this.resetPredictionAdapter();
      this.state?.clearPredictedSnapshot?.();
      this.state?.setOptimisticCommandState?.(null);
      this.publishPredictionDebug();
      this.mountSettings({ keepOpen: true });
      return;
    }
    this.initPredictionAdapter({ remountSettings: true });
  }

  initPredictionAdapter({ remountSettings = false } = {}) {
    const token = ++this.predictionInitToken;
    const adapter = this.predictionAdapter;
    void adapter.init().then((ready) => {
      if (token !== this.predictionInitToken) {
        adapter.destroy();
        return;
      }
      if (!this.prediction.enabled) {
        adapter.destroy();
        this.publishPredictionDebug();
        if (remountSettings) this.mountSettings({ keepOpen: true });
        return;
      }
      if (ready) this.logPredictionStatus("ready");
      else {
        this.prediction.recordDisableReason(adapter.disabledReason || "wasm-unavailable");
        this.logPredictionStatus("disabled");
      }
      if (remountSettings) this.mountSettings({ keepOpen: true });
    });
  }

  createPredictionAdapter() {
    return new SimWasmPredictionAdapter({
      startInfo: this.predictionStartInfo,
      playerId: this.predictionPlayerId,
      replayBudgetMs: PREDICTION_REPLAY_BUDGET_MS,
    });
  }

  resetPredictionAdapter() {
    this.predictionAdapter?.destroy();
    this.predictionAdapter = this.createPredictionAdapter();
    if (this.prediction) this.prediction.predictor = this.predictionAdapter;
  }

  predictionReportFields() {
    const controller = this.prediction.debugSummary();
    const wasm = this.predictionAdapter.diagnostics();
    return {
      predictionMode: String(controller.mode || "disabled"),
      pendingCommandCount: clampU16(controller.pendingCommandCount),
      acknowledgedCommandLatencyMs: clampU16(controller.ackLatencyMs),
      correctionDistancePx: clampU16(controller.maxCorrectionDistance),
      correctionCount: clampU32(controller.correctionCount),
      predictionDisableCount: clampU32(controller.disableCount),
      wasmTickMs: clampU16(wasm.lastTickMs),
      wasmMemoryBytes: clampU32(wasm.memoryBytes),
      predictionReplayTicks: clampU16(wasm.lastReplayTicks),
    };
  }

  applySpectatorUi() {
    const spectator = !!this.state?.spectator || this.replayViewer;
    if (dom.commandCard) dom.commandCard.hidden = spectator;
    if (dom.giveUpConfirm) dom.giveUpConfirm.hidden = true;
  }

  handleMenuKeyDown(ev) {
    if (ev.code !== "Escape" || ev.repeat || isTextEntry(ev.target)) return;
    if (dom.giveUpConfirm && !dom.giveUpConfirm.hidden) {
      ev.preventDefault();
      ev.stopPropagation();
      this.closeGiveUpConfirm();
      return;
    }
    if (this.settings?.isOpen()) {
      ev.preventDefault();
      ev.stopPropagation();
      this.settings.close({ restoreFocus: true });
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
    if (!this.pointerLockPanEnabled) return;
    if (!this.input || !this.input.pointerLockSupported()) return;
    if (automaticPointerLockDisabledForTests()) return;
    const isInstalledApp = this.input.installedAppRuntime();
    if (!shouldRequestPointerLock({ installedAppRuntime: isInstalledApp, requireGesture })) return;
    if (this.input.pointerLocked) return;
    this.autoPointerLockUntil = performance.now() + AUTO_POINTER_LOCK_SUPPRESS_MS;
    const maxAttempts = isInstalledApp && requireGesture ? INSTALLED_APP_POINTER_LOCK_RETRY_ATTEMPTS : 1;
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
      if (this.input.installedAppRuntime() && typeof document.hasFocus === "function" && !document.hasFocus()) {
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
    return new Promise((resolve) => window.setTimeout(resolve, INSTALLED_APP_POINTER_LOCK_RETRY_DELAY_MS));
  }

  automaticPointerLockActive() {
    return performance.now() <= this.autoPointerLockUntil;
  }

  closeSettingsMenu() {
    this.settings?.close();
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
    this.pointerLockPanEnabled = !this.pointerLockPanEnabled;
    this.writePointerLockPanEnabled(this.pointerLockPanEnabled);
    this.pointerLockRetryToken += 1;
    if (this.pointerLockPanEnabled) {
      this.closeSettingsMenu();
      void this.input.requestPointerLock();
    } else if (this.input.pointerLocked) {
      void this.input.exitPointerLock();
    }
    this.syncPointerLockUi();
  }

  toggleDebugPathOverlays() {
    if (!this.state?.debugPathOverlaysAvailable) {
      this.syncDebugPathUi();
      return;
    }
    this.state.debugPathOverlaysEnabled = !this.state.debugPathOverlaysEnabled;
    this.syncDebugPathUi();
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
    if (!this.input?.installedAppRuntime()) return;
    const snapshot = {
      at: new Date().toISOString(),
      error: this.pointerLockErrorSummary(err),
      retry: this.pointerLockRetry,
      support: this.input.pointerLockDebugSnapshot(),
    };
    if (typeof window !== "undefined") window.__rtsPointerLockDebug = snapshot;
    if (this.pointerLockDiagnosticShown) return;
    this.pointerLockDiagnosticShown = true;
    console.warn("[RTS_POINTER_LOCK_INSTALLED_APP]", snapshot);
    this.toast("Installed-app cursor lock failed. Inspect window.__rtsPointerLockDebug.");
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
    if (this.settings?.isOpen()) this.mountSettings({ keepOpen: true });
  }

  syncDebugPathUi() {
    if (this.settings?.isOpen()) this.mountSettings({ keepOpen: true });
  }

  mountSettings({ keepOpen = false } = {}) {
    if (!this.settings) return;
    const spectator = !!this.state?.spectator || this.replayViewer;
    const kind = this.replayViewer ? "replay" : spectator ? "spectator" : "match";
    const wasOpen = keepOpen && this.settings.isOpen();
    this.settings.setContext({
      kind,
      spectator,
      replay: this.replayViewer,
      actions: [
        buildGiveUpAction({
          visible: !spectator && !this.giveUpSent,
          onOpen: this.onGiveUpOpen,
        }),
      ],
      tabs: buildSettingsTabs({
        audio: this.audio,
        hotkeyProfiles: this.hotkeyProfiles,
        game: {
          kind,
          spectator,
          prediction: {
            state: () => ({
              hidden: spectator || this.replayViewer,
              enabled: !!this.prediction.enabled,
              active: !!this.prediction.enabled && !!this.predictionAdapter?.ready,
              pending: !!this.prediction.enabled && !!this.predictionAdapter?.loading,
              available: !this.replayViewer && !this.state?.spectator,
            }),
            onToggle: () => this.onPredictionEnabledChange?.(!this.prediction.enabled),
          },
          pointerLock: this.replayViewer ? null : {
            state: () => ({
              hidden: false,
              supported: !!this.input?.pointerLockSupported(),
              enabled: this.pointerLockPanEnabled,
              locked: !!this.input?.pointerLocked,
            }),
            onToggle: this.onPointerLockToggle,
          },
        },
        debug: {
          available: !!this.state?.debugPathOverlaysAvailable,
          state: () => ({
            available: !!this.state?.debugPathOverlaysAvailable,
            enabled: !!this.state?.debugPathOverlaysEnabled,
          }),
          onToggle: this.onDebugPathToggle,
        },
      }),
    });
    if (wasOpen) this.settings.open({ focus: false });
  }

  readPointerLockPanEnabled() {
    try {
      return window.localStorage.getItem(POINTER_LOCK_PAN_STORAGE_KEY) === "1";
    } catch {
      return false;
    }
  }

  writePointerLockPanEnabled(enabled) {
    try {
      if (enabled) window.localStorage.setItem(POINTER_LOCK_PAN_STORAGE_KEY, "1");
      else window.localStorage.removeItem(POINTER_LOCK_PAN_STORAGE_KEY);
    } catch {
      // Private browsing or storage policy failures should only make the setting session-local.
    }
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

  cameraView() {
    return {
      x: this.camera?.x,
      y: this.camera?.y,
      zoom: this.camera?.zoom,
    };
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
      } else if (ev && (ev.e === EVENT.MORTAR_LAUNCH || ev.e === EVENT.ARTILLERY_TARGET)) {
        this.playPointFireSound(ev);
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

    if (this.replayViewer || !this.audio) return;
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

  playPointFireSound(ev) {
    if (!this.audio) return;
    const spec = POINT_FIRE_SOUNDS[ev.e];
    if (!spec) return;
    let pos = null;
    if (ev.e === EVENT.MORTAR_LAUNCH && Number.isFinite(ev.fromX) && Number.isFinite(ev.fromY)) {
      pos = { x: ev.fromX, y: ev.fromY };
    } else if (ev.e === EVENT.ARTILLERY_TARGET && typeof ev.from === "number") {
      const from = this.state.entityById(ev.from);
      if (from && Number.isFinite(from.x) && Number.isFinite(from.y)) pos = from;
    }
    if (!pos) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const category = from && from.owner === this.state.playerId ? "combat_self" : "combat_other";
    this.audio.play(spec.id, {
      x: pos.x,
      y: pos.y,
      category,
      priority: spec.priority,
      gain: spec.gain,
    });
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
    const frameGapMs = now - this.lastFrame;
    this.lastFrame = now;
    if (Number.isFinite(frameGapMs) && frameGapMs >= 0) {
      this.health.noteFrameGap(frameGapMs);
    }
    this.health.refreshLatency();

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
    this.advancePredictionVisual();
    this.fog.update(this.ownEntities(), this.state.map.tileSize, this.state.visibleTiles);

    this.renderer.render(this.state, this.camera, this.fog, alpha);
    this.hud.update();
    this.minimap.render();
    this.health.publish();

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
      .entitiesInterpolated(1, { includePrediction: false })
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
   * Freeze the current rendered frame for a non-interactive overlay. This keeps
   * Pixi resources alive so App can show the map behind branch seat claiming,
   * but detaches replay/live message handlers before the socket joins another
   * room. A later destroy() still owns the full resource teardown.
   */
  freezeForBranchStagingBackground() {
    this.sendNetReport();
    this.skipFinalNetReport = true;
    this.stop();
    this.stopMatchPings();
    this.stopNetReports();
    this.stopAllMachineGunSounds();
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    this.net.off(S.REPLAY_STATE, this.onReplayState);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    document.removeEventListener("visibilitychange", this.onVisibilityChange);
    this.replayControls?.destroy();
    this.predictionInitToken += 1;
    this.predictionAdapter?.destroy();
    this.replayControls = null;
    if (this.input && typeof this.input.destroy === "function") {
      this.input.destroy();
      this.input = null;
    }
  }

  applyReplayState(state) {
    this.replayControls?.applyReplayState(state);
  }

  /**
   * Fully dispose of the match: stop the loop, drop listeners, and destroy any
   * module that exposes a destroy()/teardown() hook. After this the App can
   * build a fresh Match on the next `start`. Best-effort and idempotent.
   */
  destroy() {
    if (!this.skipFinalNetReport) this.sendNetReport();
    this.stop();
    this.stopMatchPings();
    this.stopNetReports();
    this.stopAllMachineGunSounds();
    this.pointerLockRetryToken += 1;
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    this.net.off(S.REPLAY_STATE, this.onReplayState);
    window.removeEventListener("resize", this.onResize);
    window.removeEventListener("focus", this.onWindowFocus);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    if (!this.replayViewer) {
      window.removeEventListener("keydown", this.onPointerLockGesture, true);
      window.removeEventListener("click", this.onPointerLockGesture, true);
    }
    document.removeEventListener("visibilitychange", this.onVisibilityChange);
    if (!this.replayViewer) {
      dom.giveUpCancel?.removeEventListener("click", this.onGiveUpCancel);
      dom.giveUpConfirmButton?.removeEventListener("click", this.onGiveUpConfirm);
    }
    this.replayControls?.destroy();
    this.predictionInitToken += 1;
    this.predictionAdapter?.destroy();
    this.replayControls = null;
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

function predictionCompatibility(payload) {
  const serverVersion = Number(payload?.predictionVersion) || 0;
  if (serverVersion !== PREDICTION_PROTOCOL_VERSION) {
    return {
      ok: false,
      reason: serverVersion ? "prediction-version-mismatch" : "prediction-unavailable",
      clientVersion: PREDICTION_PROTOCOL_VERSION,
      serverVersion,
      clientBuildId: clientBuildId(),
      serverBuildId: payload?.predictionBuildId || null,
    };
  }
  const client = clientBuildId();
  const server = typeof payload?.predictionBuildId === "string" ? payload.predictionBuildId : "";
  if (client && server && client !== server) {
    return {
      ok: false,
      reason: "prediction-build-mismatch",
      clientVersion: PREDICTION_PROTOCOL_VERSION,
      serverVersion,
      clientBuildId: client,
      serverBuildId: server,
    };
  }
  return {
    ok: true,
    reason: null,
    clientVersion: PREDICTION_PROTOCOL_VERSION,
    serverVersion,
    clientBuildId: client || null,
    serverBuildId: server || null,
  };
}

function predictionBlockedReason({ enabled, replayViewer, spectator, compatibility }) {
  if (!enabled) return "user-disabled";
  if (replayViewer) return "replay-viewer";
  if (spectator) return "spectator";
  if (compatibility && !compatibility.ok) return compatibility.reason || "compatibility-mismatch";
  return null;
}

function clientBuildId() {
  if (typeof globalThis.__RTS_BUILD__ === "string" && globalThis.__RTS_BUILD__ !== "unknown") {
    return globalThis.__RTS_BUILD__;
  }
  const scripts = typeof document !== "undefined" ? Array.from(document.scripts || []) : [];
  for (const script of scripts) {
    const src = script?.src || "";
    if (!src.includes("/src/main.js")) continue;
    try {
      const version = new URL(src, window.location.href).searchParams.get("v");
      if (version) return version;
    } catch {
      // Ignore malformed script URLs and fall through to unknown build compatibility.
    }
  }
  return "";
}

function clampU16(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(65535, Math.round(n));
}

function clampU32(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(4294967295, Math.round(n));
}
