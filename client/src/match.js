import { Audio, noticeSoundId } from "./audio.js";
import { Camera } from "./camera.js";
import {
  attackKindHasCombatSound,
  machineGunnerHasAudibleTarget,
  machineGunSoundKey,
} from "./combat_audio.js";
import {
  clientPerfReportFields,
  createSnapshotProcessingReport,
  recordSnapshotProcessing,
  snapshotReportFields,
} from "./client_perf_report.js";
import { Fog } from "./fog.js";
import { createFrameErrorState, runMatchFrameSafely } from "./frame_recovery.js";
import { FrameProfiler } from "./frame_profiler.js";
import { HUD } from "./hud.js";
import { Input } from "./input/index.js";
import { DomClickInputZone, MatchInputRouter } from "./input/router.js";
import { Minimap } from "./minimap.js";
import { MatchHealth } from "./match_health.js";
import { PredictionController } from "./prediction_controller.js";
import { Renderer } from "./renderer/index.js";
import { LivePauseOverlay } from "./live_pause_overlay.js";
import { ObserverAnalysisOverlay, shouldMountObserverAnalysisOverlay } from "./observer_analysis_overlay.js";
import { ReplayCameraInput } from "./replay_camera_input.js";
import { ReplayControls } from "./replay_controls.js";
import { createRoomCapabilities } from "./room_capabilities.js";
import { predictionBlockedReason, predictionCompatibility } from "./prediction_compatibility.js";
import { SimWasmPredictionAdapter } from "./sim_wasm_adapter.js";
import { GameState } from "./state.js";
import { ClientIntent } from "./client_intent.js";
import { INTERP_DELAY_MS, SNAPSHOT_MS } from "./config.js";
import { EVENT, KIND, MOVEMENT_PATH_DIAGNOSTICS, NOTICE_SEVERITY, S } from "./protocol.js";
import {
  UNDER_ATTACK_ID,
  VIEWPORT_ALERT_MARGIN_PX,
  noticeAlertId,
  noticeDisplayText,
} from "./alerts.js";
import { dom, isTextEntry } from "./bootstrap.js";
import { buildGiveUpAction, buildPauseAction, buildSettingsTabs } from "./settings_panels.js";
import { COMMAND_BUDGET_OVERFLOW_NOTICE, commandWithinBudget } from "./command_budget.js";

const KAR98K_GAIN = 0.25;
const MG_BURST_GAIN = 0.7;
const MORTAR_LAUNCH_GAIN = 0.85;
const ARTILLERY_FIRE_GAIN = 1.2;
const MATCH_PING_MS = 2000;
const NET_REPORT_MS = 10000;
const PREDICTION_REPLAY_BUDGET_MS = 4;

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
  [KIND.ANTI_TANK_GUN]: {
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
    this.labMetadata = options.labMetadata || null;
    this.labClient = options.labClient || null;
    this.labControlPolicy = options.labControlPolicy || null;
    this.onLabToolChange = options.onLabToolChange || null;
    this.labToolWorldClickHandler = null;
    this.replayViewer = !!options.replayViewer;
    this.capabilities = options.capabilities || createRoomCapabilities({ startPayload: payload });
    this.observerAnalysisOverlayPreferences = options.observerAnalysisOverlayPreferences || null;
    this.predictionStateMismatchLogged = false;
    this.missingCombatSoundKinds = new Set();
    this.activeMachineGunSoundKeys = new Map();
    this.replayControls = null;
    this.observerAnalysisOverlay = null;
    this.livePauseOverlay = null;
    this.livePauseState = {
      paused: false,
      pausesRemaining: null,
      pauseLimit: null,
      canPause: false,
      canUnpause: false,
    };
    this.giveUpSent = false;
    this.matchPingTimer = undefined;
    this.netReportTimer = undefined;
    this.skipFinalNetReport = false;
    this.lastSnapshotTick = 0;
    this.health = new MatchHealth({ net: this.net, statusBadge: this.statusBadge, snapshotMs: SNAPSHOT_MS });
    this.frameProfiler = new FrameProfiler();
    this.snapshotProcessingReport = createSnapshotProcessingReport();
    this.frameProfilerSurface = this.frameProfiler.debugSurface();
    if (typeof window !== "undefined") window.__rtsPerf = this.frameProfilerSurface;
    this.predictionStartInfo = payload;
    this.predictionPlayerId = payload?.playerId;
    this.matchRunId = typeof payload?.matchRunId === "string" ? payload.matchRunId : "";
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
        if (this.labControlPolicy?.kind === "lab") {
          return this.labControlPolicy.issueCommand(command, {
            state: this.state,
            toast: this.toast,
          });
        }
        const budget = commandWithinBudget(this.state, command);
        if (!budget.ok) {
          this.toast?.(COMMAND_BUDGET_OVERFLOW_NOTICE);
          return { clientSeq: null, sent: false, predicted: false, blocked: "commandBudget", budget };
        }
        const issued = this.prediction.issueCommand(command, options);
        this.applyPredictionDisplayOverlay(this.prediction.predictionDisplayOverlay());
        return issued;
      },
    };
    this.pointerLockDiagnosticShown = false;

    // --- Build the module graph from the static start payload (docs/design/client-ui.md §4.1). ---
    this.state = this._timeInit("match.state", () => new GameState(payload));
    this.state.controlPolicy = this.labControlPolicy;
    this.clientIntent = this._timeInit("match.clientIntent", () => new ClientIntent());
    this.camera = this._timeInit("match.camera", () => new Camera());
    this.renderer = this._timeInit("match.renderer", () => new Renderer(dom.viewport));
    this.fog = this._timeInit(
      "match.fog",
      () => new Fog(this.state.map.width, this.state.map.height, this.state.map.terrain),
    );
    this.fog.setRevealAll(!!this.devWatch?.noFog);
    this.hud = this._timeInit(
      "match.hud",
      () => new HUD(
        dom.gameScreen,
        this.state,
        this.commandIssuer,
        this.audio,
        this.hotkeyProfiles,
        this.clientIntent,
        this.labControlPolicy,
      ),
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
        commandsEnabled: !!this.capabilities.commands.gameplay,
        clientIntent: this.clientIntent,
      }),
    );
    this.input = this._timeInit(
      "match.input",
      () => this.replayViewer
        ? new ReplayCameraInput(dom.viewport, this.camera)
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
          this.clientIntent,
          {
            consumeWorldClick: (event) => this.consumeLabToolWorldClick(event),
            cancel: (reason) => this.cancelLabTool(reason),
          },
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
    this.frameErrors = createFrameErrorState();

    // --- Listeners (bound so they can be removed on destroy). ---
    this.onSnapshot = (m) => {
      const now = performance.now();
      const ackSeq = Number.isFinite(m?.netStatus?.lastSimConsumedClientSeq)
        ? m.netStatus.lastSimConsumedClientSeq
        : null;
      this.health.noteSnapshotArrival(now, document.hidden, m?.tick);
      recordSnapshotProcessing(
        this.snapshotProcessingReport,
        () => this.prediction.applyAuthoritativeSnapshot(m),
        () => this.state.applySnapshot(m),
        () => {
          this.applyPredictionDisplayOverlay(this.prediction.predictionDisplayOverlay());
          this.applyPredictedSnapshot();
        },
      );
      if (ackSeq != null) this.prediction.recordAckSnapshotApplied(ackSeq, now);
      this.lastSnapshotTick = Number.isFinite(m?.tick) ? m.tick : this.lastSnapshotTick;
      this.replayControls?.noteSnapshotTick(m?.tick);
      this.health.applyServerNetStatus(m?.netStatus || null);
      this.stopInactiveMachineGunSounds();
      this.handleSnapshotEvents(m.events || []);
    };
    this.onCommandReceipt = (m) => this.handleCommandReceipt(m);
    this.onRoomTimeState = (m) => this.applyRoomTimeState(m);
    this.onLivePauseState = (m) => this.applyLivePauseState(m);
    this.onObserverAnalysis = (m) => this.observerAnalysisOverlay?.applyObserverAnalysis(m);
    this.onResize = this.handleResize.bind(this);
    this.onMenuKeyDown = this.handleMenuKeyDown.bind(this);
    this.onGiveUpOpen = this.openGiveUpConfirm.bind(this);
    this.onGiveUpCancel = this.closeGiveUpConfirm.bind(this);
    this.onGiveUpConfirm = this.requestGiveUp.bind(this);
    this.onPauseGame = this.requestPauseGame.bind(this);
    this.onUnpauseGame = this.requestUnpauseGame.bind(this);
    this.onPointerLockToggle = this.togglePointerLock.bind(this);
    this.onDebugPathToggle = this.toggleDebugPathOverlays.bind(this);
    this.onPointerLockChange = this.handlePointerLockChange.bind(this);
    this.onPointerLockError = this.handlePointerLockError.bind(this);
    if (!this.replayViewer) {
      this.input.onPointerLockChange = this.onPointerLockChange;
      this.input.onPointerLockError = this.onPointerLockError;
    }
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    this.net.on(S.COMMAND_RECEIPT, this.onCommandReceipt);
    this.net.on(S.ROOM_TIME_STATE, this.onRoomTimeState);
    this.net.on(S.LIVE_PAUSE_STATE, this.onLivePauseState);
    this.net.on(S.REPLAY_ANALYSIS, this.onObserverAnalysis);
    window.addEventListener("resize", this.onResize);
    window.addEventListener("keydown", this.onMenuKeyDown, true);
    if (!this.replayViewer) {
      dom.giveUpCancel?.addEventListener("click", this.onGiveUpCancel);
      dom.giveUpConfirmButton?.addEventListener("click", this.onGiveUpConfirm);
    }
    this.mountSettings();

    this.rafId = requestAnimationFrame(this.tickFn);
    this.startMatchPings();
    this.startNetReports();
    this.health.publish();
    if (this.prediction.enabled) this.initPredictionAdapter();

    if (this.capabilities.roomTime.available && dom.replaySpeed) {
      this.replayControls = new ReplayControls({
        net: this.net,
        state: this.state,
        replayViewer: this.replayViewer,
        capabilities: this.capabilities,
      });
    }
    if (shouldMountObserverAnalysisOverlay({ capabilities: this.capabilities })) {
      this.observerAnalysisOverlay = new ObserverAnalysisOverlay({
        root: dom.gameScreen,
        preferences: this.observerAnalysisOverlayPreferences || undefined,
        getEntities: () => this.state.entitiesInterpolated(1, { includePrediction: false }),
        getCameraBounds: () => this.cameraWorldBounds(),
        getPlayers: () => this.state.players,
      });
    }
    if (this.capabilities.matchControls?.pause) {
      this.livePauseOverlay = new LivePauseOverlay({
        root: dom.gameScreen,
        onUnpause: this.onUnpauseGame,
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
    const transportStats = this.net.consumeSnapshotReportStats?.() || {};
    const elapsedMs = performance.now() - this.health.reportStartedAt;
    const avgFrameMs = stats.frameCount > 0 ? stats.frameTotalMs / stats.frameCount : 0;
    const report = {
      schemaVersion: 1,
      matchRunId: this.matchRunId,
      elapsedMs: clampU32(elapsedMs),
      matchTick: clampU32(this.lastSnapshotTick),
      rttMs: clampU16(metrics.latencyMs),
      rttMaxMs: clampU16(stats.rttMaxMs),
      badRttSamples: clampU32(stats.badRttSamples),
      snapshotJitterMs: clampU16(metrics.jitterMs),
      snapshotGapMaxMs: clampU16(stats.snapshotGapMaxMs),
      jitterSamples: clampU32(stats.jitterSamples),
      snapshots: clampU32(stats.snapshots),
      ...snapshotReportFields({
        reportStats: stats,
        transportStats,
        snapshotProcessing: this.snapshotProcessingReport,
      }),
      frameGapMaxMs: clampU16(stats.frameGapMaxMs),
      fpsEstimate: clampU16(avgFrameMs > 0 ? 1000 / avgFrameMs : 0),
      ...clientPerfReportFields(this.frameProfiler),
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
      frameWorkMaxMs: report.frameWorkMaxMs,
      rendererMaxMs: report.rendererMaxMs,
      worstFramePhase: report.worstFramePhase,
    });
    this.health.resetReportStats();
    this.frameProfiler?.resetReportWindow?.();
    this.snapshotProcessingReport.reset();
  }

  applyPredictionDisplayOverlay(overlay = null) {
    this.state?.applyPredictionDisplayOverlay?.(overlay);
  }

  applyPredictedSnapshot() {
    if (!this.predictionStateCompatible()) {
      this.disablePredictionForStateMismatch();
      return;
    }
    if (!this.prediction.enabled || !this.predictionAdapter.ready) {
      this.applyPredictionDisplayOverlay({ predictedSnapshot: null });
      this.publishPredictionDebug();
      return;
    }
    const snapshot = this.predictionAdapter.renderSnapshot();
    if (!snapshot) return;
    const diagnostics = this.predictionAdapter.diagnostics();
    if (this.disablePredictionForReplayBudget(diagnostics)) return;
    this.applyPredictionDisplayOverlay({
      predictedSnapshot: snapshot,
      diagnostics,
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
      this.applyPredictionDisplayOverlay({ predictedSnapshot: snapshot, diagnostics });
      this.publishPredictionDebug();
    }
  }

  disablePredictionForReplayBudget(diagnostics) {
    if (!this.prediction.enabled || !(diagnostics?.budgetExceededCount > 0)) return false;
    this.prediction.reset({ enabled: true, preserveClientSeq: true, reason: "replay-budget-exceeded" });
    this.resetPredictionAdapter();
    this.applyPredictionDisplayOverlay({ predictedSnapshot: null });
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
    return typeof this.state?.applyPredictionDisplayOverlay === "function";
  }

  disablePredictionForStateMismatch() {
    if (!this.prediction.enabled) return;
    this.prediction.reset({ enabled: false, preserveClientSeq: true, reason: "state-mismatch" });
    this.applyPredictionDisplayOverlay({ optimisticCommands: null, predictedSnapshot: null });
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
      this.applyPredictionDisplayOverlay({ optimisticCommands: null, predictedSnapshot: null });
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
    const commandReport = this.prediction.consumeCommandReportStats?.() || {};
    return {
      predictionMode: String(controller.mode || "disabled"),
      pendingCommandCount: clampU16(controller.commandDiagnosticPendingCount ?? controller.pendingCommandCount),
      acknowledgedCommandLatencyMs: clampU16(controller.ackLatencyMs),
      ...clampedCommandReportFields(commandReport),
      correctionDistancePx: clampU16(controller.maxCorrectionDistance),
      correctionCount: clampU32(controller.correctionCount),
      predictionDisableCount: clampU32(controller.disableCount),
      wasmTickMs: clampU16(wasm.lastTickMs),
      wasmMemoryBytes: clampU32(wasm.memoryBytes),
      predictionReplayTicks: clampU16(wasm.lastReplayTicks),
    };
  }

  handleCommandReceipt(message) {
    if (!message || !this.prediction) return;
    const detail = {
      serverTick: message.serverTick,
      accepted: message.accepted !== false,
      reason: typeof message.reason === "string" ? message.reason : null,
    };
    if (detail.accepted) this.prediction.recordSocketReceipt(message.clientSeq, detail);
    else this.prediction.recordCommandRejection(message.clientSeq, detail.reason, detail);
    this.publishPredictionDebug();
  }

  applySpectatorUi() {
    const hidden = this.replayViewer ||
      !((this.state?.controlPolicy || this.labControlPolicy)?.canUseCommandSurface?.(this.state) ?? !this.state?.spectator);
    if (dom.selectionArea) dom.selectionArea.hidden = hidden;
    if (dom.commandCard) dom.commandCard.hidden = hidden;
    if (dom.giveUpConfirm) dom.giveUpConfirm.hidden = true;
  }

  armLabTool(tool, callbacks = {}) {
    if (!this.clientIntent || typeof this.clientIntent.beginLabTool !== "function") return null;
    const onWorldClick = typeof callbacks === "function"
      ? callbacks
      : callbacks?.onWorldClick;
    this.labToolWorldClickHandler = typeof onWorldClick === "function" ? onWorldClick : null;
    const active = this.clientIntent.beginLabTool(tool);
    this.publishLabToolChange({ type: "armed", tool: active });
    return active;
  }

  cancelLabTool(reason = "cancelled") {
    this.labToolWorldClickHandler = null;
    const cancelled = this.clientIntent?.cancelLabTool?.(reason) || null;
    if (cancelled) this.publishLabToolChange({ type: "cancelled", reason, tool: cancelled });
    return cancelled;
  }

  consumeLabToolWorldClick(event) {
    const active = this.clientIntent?.activeLabTool || null;
    if (!active || event?.tool?.id !== active.id) return;
    const handler = this.labToolWorldClickHandler;
    try {
      const result = handler?.({ ...event, tool: active });
      if (result && typeof result.catch === "function") {
        result.catch((err) => this.handleLabToolActionError(err));
      }
    } catch (err) {
      this.handleLabToolActionError(err);
    } finally {
      this.cancelLabTool("worldClick");
    }
  }

  handleLabToolActionError(err) {
    console.error("Lab tool world-click handler failed", err);
    this.toast?.("Lab tool action failed.");
  }

  publishLabToolChange(change) {
    if (typeof this.onLabToolChange !== "function") return;
    this.onLabToolChange(change);
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

  requestPauseGame() {
    if (!this.capabilities.matchControls?.pause) return;
    if (this.livePauseState.paused || !this.livePauseState.canPause) {
      this.syncLivePauseUi();
      return;
    }
    this.closeSettingsMenu();
    this.net.pauseGame();
    this.livePauseState = { ...this.livePauseState, canPause: false };
    this.syncLivePauseUi();
  }

  requestUnpauseGame() {
    if (!this.capabilities.matchControls?.pause) return;
    if (!this.livePauseState.paused || !this.livePauseState.canUnpause) return;
    this.net.unpauseGame();
    this.livePauseState = { ...this.livePauseState, canUnpause: false };
    this.syncLivePauseUi();
  }

  applyLivePauseState(state) {
    this.livePauseState = {
      paused: state?.paused === true,
      pausedBy: Number.isInteger(state?.pausedBy) ? state.pausedBy : null,
      pausesRemaining: Number.isInteger(state?.pausesRemaining) ? state.pausesRemaining : null,
      pauseLimit: Number.isInteger(state?.pauseLimit) ? state.pauseLimit : null,
      canPause: state?.canPause === true,
      canUnpause: state?.canUnpause === true,
    };
    this.livePauseOverlay?.applyLivePauseState(this.livePauseState);
    this.syncLivePauseUi();
  }

  togglePointerLock() {
    if (!this.input?.pointerLockSupported()) {
      this.toast("Cursor lock is not supported by this browser.");
      this.syncPointerLockUi();
      return;
    }
    if (!this.input.pointerLocked) this.closeSettingsMenu();
    void this.input.togglePointerLock();
    this.syncPointerLockUi();
  }

  toggleDebugPathOverlays() {
    if (this.capabilities.diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.NONE) {
      this.syncDebugPathUi();
      return;
    }
    this.state.debugPathOverlaysEnabled = !this.state.debugPathOverlaysEnabled;
    this.syncDebugPathUi();
  }

  handlePointerLockChange(locked) {
    if (locked) {
      this.closeSettingsMenu();
      this.toast("Cursor locked. Press Esc to unlock.");
    }
    this.syncPointerLockUi();
  }

  handlePointerLockError(err) {
    this.recordPointerLockDiagnostic(err);
    this.toast("Cursor lock was blocked. Click the game view and try again.");
    this.syncPointerLockUi();
  }

  recordPointerLockDiagnostic(err = null) {
    if (!this.input?.installedAppRuntime()) return;
    const snapshot = {
      at: new Date().toISOString(),
      error: this.pointerLockErrorSummary(err),
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

  syncLivePauseUi() {
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
        buildPauseAction({
          visible: !spectator && this.capabilities.matchControls?.pause && !this.livePauseState.paused,
          disabled: !this.livePauseState.canPause,
          label: this.livePauseActionLabel(),
          title: this.livePauseActionTitle(),
          onPause: this.onPauseGame,
        }),
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
              enabled: !!this.input?.pointerLocked,
              locked: !!this.input?.pointerLocked,
            }),
            onToggle: this.onPointerLockToggle,
          },
        },
        debug: {
          available: this.capabilities.diagnostics.movementPaths !== MOVEMENT_PATH_DIAGNOSTICS.NONE,
          state: () => ({
            available: this.capabilities.diagnostics.movementPaths !== MOVEMENT_PATH_DIAGNOSTICS.NONE,
            enabled: !!this.state?.debugPathOverlaysEnabled,
          }),
          onToggle: this.onDebugPathToggle,
        },
      }),
    });
    if (wasOpen) this.settings.open({ focus: false });
  }

  livePauseActionLabel() {
    const remaining = this.livePauseState.pausesRemaining;
    if (Number.isInteger(remaining)) return `Pause (${remaining})`;
    return "Pause";
  }

  livePauseActionTitle() {
    const remaining = this.livePauseState.pausesRemaining;
    if (!Number.isInteger(remaining)) return "Pause the live match.";
    if (remaining <= 0) return "No pauses remaining.";
    return `${remaining} pause${remaining === 1 ? "" : "s"} remaining.`;
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

  cameraWorldBounds() {
    const zoom = this.camera?.zoom || 1;
    return {
      x: this.camera?.x || 0,
      y: this.camera?.y || 0,
      width: (this.camera?.viewW || 0) / zoom,
      height: (this.camera?.viewH || 0) / zoom,
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
   * Surface events once. Notices become toasts and alerts; combat/death drives
   * spatial sounds.
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

    if (this.replayViewer || this.state?.spectator || !this.audio) return;
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
    runMatchFrameSafely(this, now);
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
    this.net.off(S.COMMAND_RECEIPT, this.onCommandReceipt);
    this.net.off(S.ROOM_TIME_STATE, this.onRoomTimeState);
    this.net.off(S.LIVE_PAUSE_STATE, this.onLivePauseState);
    this.net.off(S.REPLAY_ANALYSIS, this.onObserverAnalysis);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    this.replayControls?.destroy();
    this.observerAnalysisOverlay?.destroy();
    this.livePauseOverlay?.destroy();
    this.cancelLabTool("freeze");
    this.predictionInitToken += 1;
    this.predictionAdapter?.destroy();
    if (typeof window !== "undefined" && window.__rtsPerf === this.frameProfilerSurface) {
      delete window.__rtsPerf;
    }
    this.replayControls = null;
    this.observerAnalysisOverlay = null;
    this.livePauseOverlay = null;
    if (this.input && typeof this.input.destroy === "function") {
      this.input.destroy();
      this.input = null;
    }
  }

  applyRoomTimeState(state) {
    this.replayControls?.applyRoomTimeState(state);
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
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    this.net.off(S.COMMAND_RECEIPT, this.onCommandReceipt);
    this.net.off(S.ROOM_TIME_STATE, this.onRoomTimeState);
    this.net.off(S.LIVE_PAUSE_STATE, this.onLivePauseState);
    this.net.off(S.REPLAY_ANALYSIS, this.onObserverAnalysis);
    window.removeEventListener("resize", this.onResize);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    if (!this.replayViewer) {
      dom.giveUpCancel?.removeEventListener("click", this.onGiveUpCancel);
      dom.giveUpConfirmButton?.removeEventListener("click", this.onGiveUpConfirm);
    }
    this.replayControls?.destroy();
    this.observerAnalysisOverlay?.destroy();
    this.livePauseOverlay?.destroy();
    this.cancelLabTool("destroy");
    this.predictionInitToken += 1;
    this.predictionAdapter?.destroy();
    this.replayControls = null;
    this.observerAnalysisOverlay = null;
    this.livePauseOverlay = null;
    if (dom.selectionArea) dom.selectionArea.hidden = false;
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

function clampU16(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(65535, Math.round(n));
}

function clampedCommandReportFields(report = {}) {
  return {
    commandsIssued: clampU32(report.commandsIssued),
    commandSocketSendAccepted: clampU32(report.commandSocketSendAccepted),
    commandServerReceived: clampU32(report.commandServerReceived),
    commandSimAcknowledged: clampU32(report.commandSimAcknowledged),
    commandRejected: clampU32(report.commandRejected),
    commandIssueToServerReceiptLatestMs: clampU16(report.commandIssueToServerReceiptLatestMs),
    commandIssueToServerReceiptMaxMs: clampU16(report.commandIssueToServerReceiptMaxMs),
    commandIssueToServerReceiptP95Ms: clampU16(report.commandIssueToServerReceiptP95Ms),
    commandServerReceiptToSimAckLatestMs: clampU16(report.commandServerReceiptToSimAckLatestMs),
    commandServerReceiptToSimAckMaxMs: clampU16(report.commandServerReceiptToSimAckMaxMs),
    commandServerReceiptToSimAckP95Ms: clampU16(report.commandServerReceiptToSimAckP95Ms),
    commandIssueToSimAckLatestMs: clampU16(report.commandIssueToSimAckLatestMs),
    commandIssueToSimAckMaxMs: clampU16(report.commandIssueToSimAckMaxMs),
    commandIssueToSimAckP95Ms: clampU16(report.commandIssueToSimAckP95Ms),
    commandAckSnapshotReceivedToAppliedLatestMs: clampU16(report.commandAckSnapshotReceivedToAppliedLatestMs),
    commandAckSnapshotReceivedToAppliedMaxMs: clampU16(report.commandAckSnapshotReceivedToAppliedMaxMs),
    commandAckSnapshotReceivedToAppliedP95Ms: clampU16(report.commandAckSnapshotReceivedToAppliedP95Ms),
    oldestPendingCommandAgeMs: clampU16(report.oldestPendingCommandAgeMs),
    maxPendingCommandCount: clampU16(report.maxPendingCommandCount),
  };
}

function clampU32(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return 0;
  return Math.min(4294967295, Math.round(n));
}
