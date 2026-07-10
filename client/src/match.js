import { Audio, noticeSoundId } from "./audio.js";
import { Camera } from "./camera.js";
import {
  createSnapshotProcessingReport,
  recordSnapshotProcessing,
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
import { ARTILLERY_RIG_SVG } from "./renderer/rigs/support_svg.js";
import { LivePauseOverlay } from "./live_pause_overlay.js";
import { MatchObserverDiagnostics } from "./match_observer_diagnostics.js";
import { ReplayCameraInput } from "./replay_camera_input.js";
import { RoomTimeControls } from "./replay_controls.js";
import { createRoomCapabilities } from "./room_capabilities.js";
import { predictionBlockedReason, predictionCompatibility } from "./prediction_compatibility.js";
import { SimWasmPredictionAdapter } from "./sim_wasm_adapter.js";
import { GameState } from "./state.js";
import { ClientIntent } from "./client_intent.js";
import { INTERP_DELAY_MS, SNAPSHOT_MS } from "./config.js";
import { EVENT, NOTICE_SEVERITY, S } from "./protocol.js";
import {
  UNDER_ATTACK_ID,
  VIEWPORT_ALERT_MARGIN_PX,
  noticeAlertId,
  noticeDisplayText,
} from "./alerts.js";
import { dom, isTextEntry } from "./bootstrap.js";
import { COMMAND_BUDGET_OVERFLOW_NOTICE, commandWithinBudget } from "./command_budget.js";
import { MatchCombatAudio } from "./match_combat_audio.js";
import {
  applyLivePauseState as applyLivePauseStateModel,
  clearPredictedMovementOverlay,
  livePauseActionLabel as livePauseActionLabelModel,
  livePauseActionTitle as livePauseActionTitleModel,
  notePredictionAuthoritativeSnapshot,
  pausePredictionVisualClock as pausePredictionVisualClockModel,
  predictionVisualsPaused as predictionVisualsPausedModel,
  requestPauseGame as requestPauseGameModel,
  requestUnpauseGame as requestUnpauseGameModel,
  suspendPredictionVisuals as suspendPredictionVisualsModel,
} from "./match_live_pause.js";
import { MatchNetReporter, predictionReportFields as buildPredictionReportFields } from "./match_net_reporter.js";
import { buildMatchSettingsContext } from "./match_settings_context.js";
import {
  applyInitialUnitRanges,
  toggleDebugPaths,
  toggleUnitRanges,
} from "./match_settings_toggles.js";

const PREDICTION_REPLAY_BUDGET_MS = 4;
const DESKTOP_CURSOR_AUTOLOCK_INITIAL_DELAY_MS = 250;
const DESKTOP_CURSOR_AUTOLOCK_FOCUS_DELAY_MS = 120;
const DESKTOP_CURSOR_AUTOLOCK_RETRY_BASE_MS = 750;
const DESKTOP_CURSOR_AUTOLOCK_RETRY_MAX_MS = 5000;

function desktopRuntime(root = globalThis) {
  return root?.__RTS_DESKTOP_RUNTIME || root?.window?.__RTS_DESKTOP_RUNTIME || null;
}

function desktopCursorAutoLockOptedOut(root = globalThis) {
  const search = root?.location?.search || root?.window?.location?.search || "";
  if (!search) return false;
  try {
    return new URLSearchParams(search).get("rtsNoAutoPointerLock") === "1";
  } catch {
    return false;
  }
}

function desktopCursorAggressiveLockEnabled(root = globalThis) {
  const runtime = desktopRuntime(root);
  return runtime?.shell === "tauri" &&
    runtime?.nativeCursorCapture === true &&
    runtime?.aggressiveCursorLock !== false &&
    !desktopCursorAutoLockOptedOut(root);
}

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
    this.backToLobbyHandler = options.onBackToLobby || null;
    this.onPredictionEnabledChange = options.onPredictionEnabledChange || null;
    this.onUnitRangesEnabledChange = options.onUnitRangesEnabledChange;
    this.labMetadata = options.labMetadata || null;
    this.labClient = options.labClient || null;
    this.labControlPolicy = options.labControlPolicy || null;
    this.visualProfile = options.visualProfile || null;
    this.visualProfileError = options.visualProfileError || null;
    if (typeof window !== "undefined") {
      window.__rtsVisualProfile = this.visualProfile || this.visualProfileError
        ? { profile: this.visualProfile, error: this.visualProfileError }
        : null;
    }
    this.onLabToolChange = options.onLabToolChange || null;
    this.labToolWorldClickHandler = null;
    this.labToolBoxSelectionHandler = null;
    this.replayViewer = !!options.replayViewer;
    this.capabilities = options.capabilities || createRoomCapabilities({ startPayload: payload });
    this.predictionStateMismatchLogged = false;
    this.roomTimeControls = null;
    this.observerDiagnostics = null;
    this.livePauseOverlay = null;
    this.livePauseState = {
      paused: false,
      pausesRemaining: null,
      pauseLimit: null,
      canPause: false,
      canUnpause: false,
    };
    this.predictionVisualSuspended = false;
    this.giveUpSent = false;
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
    this.netReporter = new MatchNetReporter({
      net: this.net,
      health: this.health,
      frameProfiler: this.frameProfiler,
      snapshotProcessingReport: this.snapshotProcessingReport,
      diagnostics: this.diagnostics,
      matchRunId: this.matchRunId,
      getLastSnapshotTick: () => this.lastSnapshotTick,
      getPredictionReportFields: () => this.predictionReportFields(),
    });
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
        const predictionOptions = this.predictionVisualsPaused()
          ? { ...options, predictMovement: false }
          : options;
        const issued = this.prediction.issueCommand(command, predictionOptions);
        if (issued?.sent) {
          const issuedAt = this.frameProfiler?.now?.() ?? performance.now();
          this.health.noteCommandIssued(issuedAt);
          this.frameProfiler?.recordDiagnosticCounter?.("commands.issued");
        }
        this.applyPredictionDisplayOverlay(this.prediction.predictionDisplayOverlay());
        return issued;
      },
    };
    this.pointerLockDiagnosticShown = false;
    this.desktopCursorAutoLockEnabled = false;
    this.desktopCursorAutoLockTimer = null;
    this.desktopCursorAutoLockInFlight = false;
    this.desktopCursorAutoLockFailures = 0;
    this.onDesktopCursorAutoLockSignal = this.handleDesktopCursorAutoLockSignal.bind(this);

    // --- Build the module graph. ---
    this.state = this._timeInit("match.state", () => new GameState(payload));
    applyInitialUnitRanges(this.state, options.unitRangesEnabled);
    this.state.controlPolicy = this.labControlPolicy;
    this.combatAudio = this._timeInit(
      "match.combatAudio",
      () => new MatchCombatAudio({ audio: this.audio, state: this.state }),
    );
    this.clientIntent = this._timeInit("match.clientIntent", () => new ClientIntent());
    this.camera = this._timeInit("match.camera", () => new Camera(0, 0, {
      maxZoom: options.cameraMaxZoom,
    }));
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
        this.camera,
      ),
    );
    this.inputRouter = this._timeInit("match.inputRouter", () => new MatchInputRouter(dom.viewport));
    this.domInputZone = this._timeInit(
      "match.domInputZone",
      () => new DomClickInputZone([dom.gameScreen, dom.gameMenu], {
        priority: 20,
        ignoreRoots: [dom.viewport],
      }),
    );
    this.unregisterDomInputZone = this.inputRouter.registerZone(this.domInputZone);
    this.minimap = this._timeInit(
      "match.minimap",
      () => new Minimap(dom.minimap, this.state, this.camera, this.fog, this.commandIssuer, this.inputRouter, {
        commandsEnabled: !!this.capabilities.commands.gameplay,
        clientIntent: this.clientIntent,
        artilleryIconSvg: ARTILLERY_RIG_SVG,
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
            consumeBoxSelection: (event) => this.consumeLabToolBoxSelection(event),
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
          notePredictionAuthoritativeSnapshot(this);
          this.applyPredictionDisplayOverlay(this.prediction.predictionDisplayOverlay());
          this.applyPredictedSnapshot();
        },
      );
      this.clientIntent?.reconcilePlannedOrders?.(this.state.selectedEntities(), {
        acknowledgedClientSeq: ackSeq,
      });
      if (ackSeq != null) this.prediction.recordAckSnapshotApplied(ackSeq, now);
      this.lastSnapshotTick = Number.isFinite(m?.tick) ? m.tick : this.lastSnapshotTick;
      this.roomTimeControls?.noteSnapshotTick(m?.tick);
      this.health.applyServerNetStatus(m?.netStatus || null);
      this.stopInactiveMachineGunSounds();
      this.handleSnapshotEvents(m.events || []);
    };
    this.onCommandReceipt = (m) => this.handleCommandReceipt(m);
    this.onRoomTimeState = (m) => this.applyRoomTimeState(m);
    this.onLivePauseState = (m) => this.applyLivePauseState(m);
    this.onObserverAnalysis = (m) => this.observerDiagnostics?.applyObserverAnalysis(m);
    this.onResize = this.handleResize.bind(this);
    this.onMenuKeyDown = this.handleMenuKeyDown.bind(this);
    this.onGiveUpOpen = this.openGiveUpConfirm.bind(this);
    this.onGiveUpCancel = this.closeGiveUpConfirm.bind(this);
    this.onGiveUpConfirm = this.requestGiveUp.bind(this);
    this.onBackToLobby = this.requestBackToLobby.bind(this);
    this.onPauseGame = this.requestPauseGame.bind(this);
    this.onUnpauseGame = this.requestUnpauseGame.bind(this);
    this.onPointerLockToggle = this.togglePointerLock.bind(this);
    this.onDebugPathToggle = this.toggleDebugPathOverlays.bind(this);
    this.onUnitRangeToggle = this.toggleUnitRangeOverlays.bind(this);
    this.onPointerLockChange = this.handlePointerLockChange.bind(this);
    this.onPointerLockError = this.handlePointerLockError.bind(this);
    if (!this.replayViewer) {
      this.input.onPointerLockChange = this.onPointerLockChange;
      this.input.onPointerLockError = this.onPointerLockError;
    }
    this.desktopCursorAutoLockEnabled = this.shouldUseDesktopCursorAutoLock();
    this.net.on(S.SNAPSHOT, this.onSnapshot);
    this.net.on(S.COMMAND_RECEIPT, this.onCommandReceipt);
    this.net.on(S.ROOM_TIME_STATE, this.onRoomTimeState);
    this.net.on(S.LIVE_PAUSE_STATE, this.onLivePauseState);
    this.net.on(S.OBSERVER_ANALYSIS, this.onObserverAnalysis);
    window.addEventListener("resize", this.onResize);
    window.addEventListener("keydown", this.onMenuKeyDown, true);
    if (!this.replayViewer) {
      dom.giveUpCancel?.addEventListener("click", this.onGiveUpCancel);
      dom.giveUpConfirmButton?.addEventListener("click", this.onGiveUpConfirm);
    }
    this.mountSettings();
    this.installDesktopCursorAutoLock();

    this.rafId = requestAnimationFrame(this.tickFn);
    this.startMatchPings();
    this.startNetReports();
    this.health.publish();
    if (this.prediction.enabled) this.initPredictionAdapter();

    if (this.capabilities.roomTime.available && dom.roomTimeControls) {
      this.roomTimeControls = new RoomTimeControls({
        net: this.net,
        state: this.state,
        replayViewer: this.replayViewer,
        capabilities: this.capabilities,
      });
    }
    this.observerDiagnostics = new MatchObserverDiagnostics({
      root: dom.gameScreen,
      capabilities: this.capabilities,
      observerAnalysisOverlayPreferences: options.observerAnalysisOverlayPreferences || null,
      aiDiagnosticsPanelPreferences: options.aiDiagnosticsPanelPreferences || null,
      getEntities: () => this.state.entitiesInterpolated(1, { includePrediction: false }),
      getCameraBounds: () => this.cameraWorldBounds(),
      getPlayers: () => this.state.players,
    });
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
    this.netReporter.startMatchPings();
  }

  stopMatchPings() {
    this.netReporter.stopMatchPings();
  }

  startNetReports() {
    this.netReporter.startNetReports();
  }

  stopNetReports() {
    this.netReporter.stopNetReports();
  }

  sendNetReport() {
    this.netReporter.sendNetReport();
  }

  applyPredictionDisplayOverlay(overlay = null) {
    this.state?.applyPredictionDisplayOverlay?.(overlay);
  }

  applyPredictedSnapshot() {
    if (!this.predictionStateCompatible()) {
      this.disablePredictionForStateMismatch();
      return;
    }
    if (this.predictionVisualsPaused()) {
      this.pausePredictionVisualClock();
      clearPredictedMovementOverlay(this);
      this.publishPredictionDebug();
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
    if (this.predictionVisualsPaused()) {
      this.pausePredictionVisualClock();
      clearPredictedMovementOverlay(this);
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
    this.prediction.recordReplayBudgetExceeded({
      elapsedMs: diagnostics.lastTickMs,
      replayTicks: diagnostics.lastReplayTicks,
    });
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

  predictionVisualsPaused() {
    return predictionVisualsPausedModel(this);
  }

  pausePredictionVisualClock() {
    pausePredictionVisualClockModel(this);
  }

  suspendPredictionVisuals() {
    suspendPredictionVisualsModel(this);
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
        this.prediction.recordDisableReason("wasm-unavailable");
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
    return buildPredictionReportFields({
      prediction: this.prediction,
      predictionAdapter: this.predictionAdapter,
    });
  }

  handleCommandReceipt(message) {
    if (!message || !this.prediction) return;
    const detail = {
      serverTick: message.serverTick,
      accepted: message.accepted !== false,
      reason: typeof message.reason === "string" ? message.reason : null,
    };
    if (detail.accepted) this.prediction.recordSocketReceipt(message.clientSeq, detail);
    else {
      this.clientIntent?.clearPlannedOrdersForClientSeq?.(message.clientSeq);
      this.prediction.recordCommandRejection(message.clientSeq, detail.reason, detail);
    }
    this.publishPredictionDebug();
  }

  applySpectatorUi() {
    const hidden = this.replayViewer ||
      !((this.state?.controlPolicy || this.labControlPolicy)?.canUseCommandSurface?.(this.state) ?? !this.state?.spectator);
    if (dom.selectionArea) dom.selectionArea.hidden = hidden;
    if (dom.commandCard) dom.commandCard.hidden = hidden;
    this.closeGiveUpConfirm();
  }

  armLabTool(tool, callbacks = {}) {
    if (!this.clientIntent || typeof this.clientIntent.beginLabTool !== "function") return null;
    const onWorldClick = typeof callbacks === "function"
      ? callbacks
      : callbacks?.onWorldClick;
    const onBoxSelection = typeof callbacks === "object" ? callbacks?.onBoxSelection : null;
    this.labToolWorldClickHandler = typeof onWorldClick === "function" ? onWorldClick : null;
    this.labToolBoxSelectionHandler = typeof onBoxSelection === "function" ? onBoxSelection : null;
    const active = this.clientIntent.beginLabTool(tool);
    this.publishLabToolChange({ type: "armed", tool: active });
    return active;
  }

  cancelLabTool(reason = "cancelled") {
    this.labToolWorldClickHandler = null;
    this.labToolBoxSelectionHandler = null;
    const cancelled = this.clientIntent?.cancelLabTool?.(reason) || null;
    if (cancelled) this.publishLabToolChange({ type: "cancelled", reason, tool: cancelled });
    return cancelled;
  }

  consumeLabToolWorldClick(event) {
    const tool = this.clientIntent?.activeLabTool || null;
    if (!tool || event?.tool?.id !== tool.id) return;
    const h = this.labToolWorldClickHandler;
    try {
      const r = h?.({ ...event, tool });
      if (r && typeof r.catch === "function") {
        r.catch((err) => this.handleLabToolActionError(err));
      }
    } catch (err) {
      this.handleLabToolActionError(err);
    } finally {
      if (!tool.keepArmedOnWorldClick) this.cancelLabTool("worldClick");
    }
  }

  consumeLabToolBoxSelection(event) {
    const tool = this.clientIntent?.activeLabTool || null;
    if (!tool || event?.tool?.id !== tool.id) return;
    const h = this.labToolBoxSelectionHandler;
    try {
      const r = h?.({ ...event, tool });
      if (r && typeof r.catch === "function") {
        r.catch((err) => this.handleLabToolActionError(err));
      }
    } catch (err) {
      this.handleLabToolActionError(err);
    } finally {
      if (!tool.keepArmedOnBoxSelection) this.cancelLabTool("boxSelect");
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
    this.resetGiveUpConfirmButton();
    dom.giveUpConfirm.hidden = false;
    dom.giveUpConfirmButton?.focus();
  }

  resetGiveUpConfirmButton() {
    if (dom.giveUpConfirmButton) {
      dom.giveUpConfirmButton.disabled = false;
      dom.giveUpConfirmButton.textContent = "Give up";
    }
  }

  closeGiveUpConfirm() {
    if (dom.giveUpConfirm) dom.giveUpConfirm.hidden = true;
    this.resetGiveUpConfirmButton();
  }

  closeMenus() {
    this.closeSettingsMenu();
    this.closeGiveUpConfirm();
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

  requestBackToLobby() {
    this.settings?.close();
    this.backToLobbyHandler?.();
  }

  requestPauseGame() {
    requestPauseGameModel(this);
  }

  requestUnpauseGame() {
    requestUnpauseGameModel(this);
  }

  applyLivePauseState(state) {
    applyLivePauseStateModel(this, state);
  }

  shouldUseDesktopCursorAutoLock() {
    return !this.replayViewer &&
      desktopCursorAggressiveLockEnabled() &&
      typeof this.input?.requestPointerLock === "function";
  }

  desktopCursorAutoLockCanRun() {
    if (!this.desktopCursorAutoLockEnabled) return false;
    if (!this.input || this.input.pointerLocked) return false;
    const doc = globalThis.document;
    if (doc?.hidden) return false;
    if (typeof doc?.hasFocus === "function" && !doc.hasFocus()) return false;
    return true;
  }

  installDesktopCursorAutoLock() {
    if (!this.desktopCursorAutoLockEnabled) return;
    const win = globalThis.window;
    const doc = globalThis.document;
    win?.addEventListener?.("focus", this.onDesktopCursorAutoLockSignal);
    win?.addEventListener?.("pageshow", this.onDesktopCursorAutoLockSignal);
    doc?.addEventListener?.("visibilitychange", this.onDesktopCursorAutoLockSignal);
    this.scheduleDesktopCursorAutoLock("match-start", DESKTOP_CURSOR_AUTOLOCK_INITIAL_DELAY_MS);
  }

  teardownDesktopCursorAutoLock() {
    const win = globalThis.window;
    const doc = globalThis.document;
    win?.removeEventListener?.("focus", this.onDesktopCursorAutoLockSignal);
    win?.removeEventListener?.("pageshow", this.onDesktopCursorAutoLockSignal);
    doc?.removeEventListener?.("visibilitychange", this.onDesktopCursorAutoLockSignal);
    this.clearDesktopCursorAutoLockTimer();
    this.desktopCursorAutoLockEnabled = false;
    this.desktopCursorAutoLockInFlight = false;
  }

  handleDesktopCursorAutoLockSignal() {
    this.scheduleDesktopCursorAutoLock("focus", DESKTOP_CURSOR_AUTOLOCK_FOCUS_DELAY_MS);
  }

  scheduleDesktopCursorAutoLock(reason, delayMs = DESKTOP_CURSOR_AUTOLOCK_FOCUS_DELAY_MS) {
    if (!this.desktopCursorAutoLockEnabled) return;
    if (this.desktopCursorAutoLockTimer != null || this.desktopCursorAutoLockInFlight) return;
    if (!this.input || this.input.pointerLocked) return;
    const setTimer = globalThis.window?.setTimeout || globalThis.setTimeout;
    if (typeof setTimer !== "function") {
      void this.requestDesktopCursorAutoLock(reason);
      return;
    }
    this.desktopCursorAutoLockTimer = setTimer(() => {
      this.desktopCursorAutoLockTimer = null;
      void this.requestDesktopCursorAutoLock(reason);
    }, Math.max(0, delayMs));
  }

  clearDesktopCursorAutoLockTimer() {
    if (this.desktopCursorAutoLockTimer == null) return;
    const clearTimer = globalThis.window?.clearTimeout || globalThis.clearTimeout;
    if (typeof clearTimer === "function") clearTimer(this.desktopCursorAutoLockTimer);
    this.desktopCursorAutoLockTimer = null;
  }

  async requestDesktopCursorAutoLock(reason = "auto") {
    if (!this.desktopCursorAutoLockCanRun() || this.desktopCursorAutoLockInFlight) return false;
    if (!this.input?.pointerLockSupported?.()) {
      this.syncPointerLockUi();
      return false;
    }
    this.desktopCursorAutoLockInFlight = true;
    let locked = false;
    try {
      locked = await this.input.requestPointerLock();
    } catch (err) {
      this.handlePointerLockError(err);
    } finally {
      this.desktopCursorAutoLockInFlight = false;
    }
    this.syncPointerLockUi();
    if (locked) {
      this.desktopCursorAutoLockFailures = 0;
      return true;
    }
    this.desktopCursorAutoLockFailures += 1;
    if (this.desktopCursorAutoLockCanRun()) {
      const retryDelay = Math.min(
        DESKTOP_CURSOR_AUTOLOCK_RETRY_MAX_MS,
        DESKTOP_CURSOR_AUTOLOCK_RETRY_BASE_MS * (2 ** Math.min(this.desktopCursorAutoLockFailures - 1, 3)),
      );
      this.scheduleDesktopCursorAutoLock(`retry:${reason}`, retryDelay);
    }
    return false;
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
    toggleDebugPaths(this);
  }

  toggleUnitRangeOverlays() {
    toggleUnitRanges(this);
  }

  handlePointerLockChange(locked) {
    if (locked) {
      this.closeSettingsMenu();
      this.desktopCursorAutoLockFailures = 0;
      this.toast(
        this.desktopCursorAutoLockEnabled
          ? "Cursor locked. Alt-Tab to leave the game."
          : "Cursor locked. Toggle cursor lock in settings to unlock.",
      );
    } else {
      this.scheduleDesktopCursorAutoLock("cursor-unlocked", DESKTOP_CURSOR_AUTOLOCK_FOCUS_DELAY_MS);
    }
    this.syncPointerLockUi();
  }

  handlePointerLockError(err) {
    if (this.input?.installedAppRuntime()) {
      this.recordPointerLockDiagnostic(err);
    } else {
      this.toast("Cursor lock was blocked. Click the game view and try again.");
    }
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
    this.showPointerLockDiagnostic(snapshot);
    if (this.pointerLockDiagnosticShown) return;
    this.pointerLockDiagnosticShown = true;
    console.warn("[RTS_POINTER_LOCK_INSTALLED_APP]", snapshot);
    this.toast(this.pointerLockDiagnosticToast(snapshot));
  }

  showPointerLockDiagnostic(snapshot) {
    if (typeof document === "undefined") return;
    let panel = document.getElementById("desktop-cursor-diagnostic");
    if (!panel) {
      panel = document.createElement("pre");
      panel.id = "desktop-cursor-diagnostic";
      panel.style.position = "fixed";
      panel.style.left = "12px";
      panel.style.bottom = "12px";
      panel.style.zIndex = "99999";
      panel.style.maxWidth = "760px";
      panel.style.maxHeight = "240px";
      panel.style.overflow = "auto";
      panel.style.padding = "10px 12px";
      panel.style.margin = "0";
      panel.style.border = "1px solid rgba(255,255,255,0.35)";
      panel.style.background = "rgba(18, 22, 28, 0.94)";
      panel.style.color = "#f4f7fb";
      panel.style.font = "12px ui-monospace, SFMono-Regular, Menlo, monospace";
      panel.style.whiteSpace = "pre-wrap";
      panel.style.pointerEvents = "none";
      document.body.appendChild(panel);
    }
    const native = snapshot?.support?.nativeCursor || {};
    panel.textContent = [
      "Installed-app cursor lock diagnostic",
      `error: ${this.pointerLockDiagnosticToast(snapshot)}`,
      `nativeBridgePresent: ${!!snapshot?.support?.nativeCursorBridgePresent}`,
      `nativeSupported: ${native.supported !== false}`,
      `nativeBackend: ${native.backend || "none"}`,
      `nativeLastError: ${native.lastError || "none"}`,
      `tauriGlobals: ${(snapshot?.support?.tauriGlobals || []).join(", ") || "none"}`,
      `desktopRuntime: ${JSON.stringify(snapshot?.support?.desktopRuntime || null)}`,
    ].join("\n");
  }

  pointerLockDiagnosticToast(snapshot) {
    const native = snapshot?.support?.nativeCursor;
    const nativeError = native?.lastError || null;
    const error = snapshot?.error?.message || snapshot?.error?.name || null;
    if (nativeError) return `Installed-app cursor lock failed: ${nativeError}`;
    if (native?.supported === false) return "Installed-app cursor lock failed: native bridge missing.";
    if (error) return `Installed-app cursor lock failed: ${error}`;
    return "Installed-app cursor lock failed.";
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

  syncSettingsToggleUi() {
    if (this.settings?.isOpen()) this.mountSettings({ keepOpen: true });
  }

  syncLivePauseUi() {
    this.mountSettings({ keepOpen: true });
  }

  mountSettings({ keepOpen = false } = {}) {
    if (!this.settings) return;
    const wasOpen = keepOpen && this.settings.isOpen();
    this.settings.setContext(buildMatchSettingsContext({
      replayViewer: this.replayViewer,
      labMetadata: this.labMetadata,
      state: this.state,
      capabilities: this.capabilities,
      livePauseState: this.livePauseState,
      giveUpSent: this.giveUpSent,
      audio: this.audio,
      hotkeyProfiles: this.hotkeyProfiles,
      prediction: this.prediction,
      predictionAdapter: this.predictionAdapter,
      input: this.input,
      onPauseGame: this.onPauseGame,
      onGiveUpOpen: this.onGiveUpOpen,
      onBackToLobby: this.onBackToLobby,
      onPredictionEnabledChange: this.onPredictionEnabledChange,
      onPointerLockToggle: this.onPointerLockToggle,
      onDebugPathToggle: this.onDebugPathToggle,
      onUnitRangeToggle: this.onUnitRangeToggle,
      livePauseActionLabel: () => this.livePauseActionLabel(),
      livePauseActionTitle: () => this.livePauseActionTitle(),
    }));
    if (wasOpen) this.settings.open({ focus: false });
  }

  livePauseActionLabel() {
    return livePauseActionLabelModel(this);
  }

  livePauseActionTitle() {
    return livePauseActionTitleModel(this);
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
    this.input?.configureNativeCursorBounds?.();
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
      } else if (ev && ev.e === EVENT.ARTILLERY_FIRING) {
        this.minimap?.markArtilleryFiring(ev);
      } else if (ev && this.combatAudio?.hasPointFireSound(ev.e)) {
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
    this.combatAudio?.playAttackSound(ev);
  }

  playPointFireSound(ev) {
    this.combatAudio?.playPointFireSound(ev);
  }

  stopInactiveMachineGunSounds() {
    this.combatAudio?.stopInactiveMachineGunSounds();
  }

  stopAllMachineGunSounds() {
    this.combatAudio?.stopAllMachineGunSounds();
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
    this.teardownDesktopCursorAutoLock();
    this.stopMatchPings();
    this.stopNetReports();
    this.stopAllMachineGunSounds();
    this.combatAudio?.destroy();
    this.combatAudio = null;
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    this.net.off(S.COMMAND_RECEIPT, this.onCommandReceipt);
    this.net.off(S.ROOM_TIME_STATE, this.onRoomTimeState);
    this.net.off(S.LIVE_PAUSE_STATE, this.onLivePauseState);
    this.net.off(S.OBSERVER_ANALYSIS, this.onObserverAnalysis);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    this.roomTimeControls?.destroy();
    this.observerDiagnostics?.destroy();
    this.livePauseOverlay?.destroy();
    this.cancelLabTool("freeze");
    this.predictionInitToken += 1;
    this.predictionAdapter?.destroy();
    if (typeof window !== "undefined" && window.__rtsPerf === this.frameProfilerSurface) {
      delete window.__rtsPerf;
    }
    this.roomTimeControls = null;
    this.observerDiagnostics = null;
    this.livePauseOverlay = null;
    if (this.input && typeof this.input.destroy === "function") {
      this.input.destroy();
      this.input = null;
    }
  }

  applyRoomTimeState(state) {
    this.roomTimeControls?.applyRoomTimeState(state);
  }

  /**
   * Fully dispose of the match: stop the loop, drop listeners, and destroy any
   * module that exposes a destroy()/teardown() hook. After this the App can
   * build a fresh Match on the next `start`. Best-effort and idempotent.
   */
  destroy() {
    if (!this.skipFinalNetReport) this.sendNetReport();
    this.stop();
    this.teardownDesktopCursorAutoLock();
    this.stopMatchPings();
    this.stopNetReports();
    this.stopAllMachineGunSounds();
    this.combatAudio?.destroy();
    this.combatAudio = null;
    this.net.off(S.SNAPSHOT, this.onSnapshot);
    this.net.off(S.COMMAND_RECEIPT, this.onCommandReceipt);
    this.net.off(S.ROOM_TIME_STATE, this.onRoomTimeState);
    this.net.off(S.LIVE_PAUSE_STATE, this.onLivePauseState);
    this.net.off(S.OBSERVER_ANALYSIS, this.onObserverAnalysis);
    window.removeEventListener("resize", this.onResize);
    window.removeEventListener("keydown", this.onMenuKeyDown, true);
    if (!this.replayViewer) {
      dom.giveUpCancel?.removeEventListener("click", this.onGiveUpCancel);
      dom.giveUpConfirmButton?.removeEventListener("click", this.onGiveUpConfirm);
    }
    this.roomTimeControls?.destroy();
    this.observerDiagnostics?.destroy();
    this.livePauseOverlay?.destroy();
    this.cancelLabTool("destroy");
    this.predictionInitToken += 1;
    this.predictionAdapter?.destroy();
    this.roomTimeControls = null;
    this.observerDiagnostics = null;
    this.livePauseOverlay = null;
    if (dom.selectionArea) dom.selectionArea.hidden = false;
    if (dom.commandCard) dom.commandCard.hidden = false;
    if (this.unregisterDomInputZone) {
      this.unregisterDomInputZone();
      this.unregisterDomInputZone = null;
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
