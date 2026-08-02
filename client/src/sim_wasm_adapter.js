const WASM_GLUE_PATH = "../vendor/sim-wasm/rts_sim_wasm.js";
const SNAP_CORRECTION_PX = 96;
const DEFAULT_REPLAY_BUDGET_MS = 4;
const TICK_MS = 1000 / 30;

export class SimWasmPredictionAdapter {
  constructor({
    startInfo,
    playerId,
    now = () => performance.now(),
    visualNow = now,
    importModule = (path) => import(path),
    replayBudgetMs = DEFAULT_REPLAY_BUDGET_MS,
  } = {}) {
    this.startInfo = startInfo;
    this.playerId = playerId;
    this.now = now;
    this.visualNow = visualNow;
    this.importModule = importModule;
    this.ready = false;
    this.disabledReason = null;
    this.loading = false;
    this.module = null;
    this.predictor = null;
    this.displayValid = false;
    this.lastPredictedTick = null;
    this.lastAdvanceAt = null;
    this.visualPaused = false;
    this.frozenFrame = null;
    this.maxCorrectionDistance = 0;
    this.snapCorrectionCount = 0;
    this.startupMs = null;
    this.lastTickMs = 0;
    this.maxTickMs = 0;
    this.lastReplayTicks = 0;
    this.maxReplayTicks = 0;
    this.replayBudgetMs = replayBudgetMs;
    this.budgetExceededCount = 0;
    this.memoryBytes = 0;
    this.progressCorrectionCount = 0;
    this.progressCorrectionTotal = 0;
    this.progressLastCorrection = 0;
    this.progressMaxCorrection = 0;
    this.resetReportStats();
  }

  async init() {
    if (this.ready || this.loading || this.disabledReason) return this.ready;
    this.loading = true;
    const startedAt = this.now();
    try {
      await assertModuleAvailable(WASM_GLUE_PATH);
      const module = await this.importModule(WASM_GLUE_PATH);
      await module.default();
      this.module = module;
      this.predictor = module.WasmPredictor.fromStartJson(
        JSON.stringify(this.startInfo),
        this.playerId,
      );
      this.ready = true;
      this.startupMs = this.now() - startedAt;
      this.lastAdvanceAt = this.visualNow();
      this.refreshMemoryBytes();
      return true;
    } catch (err) {
      this.disabledReason = errorMessage(err);
      return false;
    } finally {
      this.loading = false;
    }
  }

  destroy() {
    if (this.predictor && typeof this.predictor.free === "function") {
      this.predictor.free();
    }
    this.predictor = null;
    this.ready = false;
    this.displayValid = false;
    this.visualPaused = false;
    this.frozenFrame = null;
    this.lastAdvanceAt = null;
  }

  enqueueCommand(clientSeq, command) {
    if (!this.ready || !this.predictor) return false;
    this.predictor.enqueueCommandJson(clientSeq, JSON.stringify(command));
    this.lastPredictedTick = this.renderPredictionFrame()?.tick ?? this.lastPredictedTick;
    return true;
  }

  reconcile(authoritativeSnapshot, pendingCommands = []) {
    if (!this.ready || !this.predictor || !this.module || !authoritativeSnapshot) return null;
    this.displayValid = false;
    const replayTicks = Math.max(0, pendingCommands?.length || 0);
    const elapsed = this.measureTicks(() => {
      const baselineJson = this.module.WasmPredictor.baselineFromSnapshotJson(
        JSON.stringify(authoritativeSnapshot),
        this.playerId,
      );
      this.predictor.importBaselineJson(baselineJson);
      for (const pending of pendingCommands) {
        this.predictor.enqueueCommandJson(pending.clientSeq, JSON.stringify(pending.cmd));
      }
    }, replayTicks);
    this.displayValid = true;
    const replayBudgetExceeded = elapsed > this.replayBudgetMs;
    this.recordReplayReport(elapsed, replayTicks, replayBudgetExceeded);
    if (replayBudgetExceeded) this.budgetExceededCount += 1;
    let diagnostics = this.diagnostics();
    const correction = Number(diagnostics?.correctionMagnitude) || 0;
    const progressCorrection = Number(diagnostics?.progressCorrectionMagnitude) || 0;
    this.progressLastCorrection = progressCorrection;
    if (progressCorrection > 0) {
      this.progressCorrectionCount += 1;
      this.progressCorrectionTotal += progressCorrection;
      this.progressMaxCorrection = Math.max(this.progressMaxCorrection, progressCorrection);
    }
    diagnostics = this.diagnostics();
    this.maxCorrectionDistance = Math.max(this.maxCorrectionDistance, correction);
    if (correction > SNAP_CORRECTION_PX) this.snapCorrectionCount += 1;
    this.lastAdvanceAt = this.visualNow();
    this.frozenFrame = null;
    const frame = this.renderPredictionFrame(this.lastAdvanceAt);
    if (this.visualPaused) this.frozenFrame = frame;
    this.lastPredictedTick = frame?.tick ?? authoritativeSnapshot.tick ?? null;
    return {
      diagnostics,
      correctionDistance: correction,
      snapCorrection: correction > SNAP_CORRECTION_PX,
      maxCorrectionDistance: this.maxCorrectionDistance,
      snapCorrectionCount: this.snapCorrectionCount,
      replayBudgetExceeded,
    };
  }

  advanceVisual(visualNow = this.visualNow()) {
    if (!this.ready || !this.predictor) return null;
    if (this.visualPaused) return this.frozenFrame || this.renderPredictionFrame(visualNow);
    const now = finiteVisualTime(visualNow, this.visualNow());
    if (this.lastAdvanceAt == null) this.lastAdvanceAt = now;
    const elapsedMs = Math.max(0, now - this.lastAdvanceAt);
    const ticks = Math.min(8, Math.floor(elapsedMs / TICK_MS));
    if (ticks > 0) {
      this.measureTicks(() => this.predictor.advanceTicks(ticks), ticks);
      this.lastAdvanceAt += ticks * TICK_MS;
    }
    const frame = this.renderPredictionFrame(now);
    if (frame) this.lastPredictedTick = frame.tick;
    return frame;
  }

  pauseVisualClock(visualNow = this.visualNow()) {
    if (this.visualPaused) return this.frozenFrame;
    const now = finiteVisualTime(visualNow, this.visualNow());
    this.frozenFrame = this.advanceVisual(now);
    this.visualPaused = true;
    return this.frozenFrame;
  }

  resumeVisualClock(visualNow = this.visualNow()) {
    const now = finiteVisualTime(visualNow, this.visualNow());
    this.visualPaused = false;
    this.frozenFrame = null;
    this.lastAdvanceAt = now;
  }

  renderPredictionFrame(visualNow = this.visualNow()) {
    if (!this.ready || !this.predictor || !this.displayValid) return null;
    if (this.visualPaused && this.frozenFrame) return this.frozenFrame;
    const now = finiteVisualTime(visualNow, this.visualNow());
    const elapsedMs = this.lastAdvanceAt == null ? 0 : Math.max(0, now - this.lastAdvanceAt);
    const visualTickFraction = Math.min(0.999999, (elapsedMs % TICK_MS) / TICK_MS);
    return JSON.parse(this.predictor.renderPredictionFrameJson(visualTickFraction));
  }

  diagnostics() {
    if (!this.ready || !this.predictor) {
      return {
        ready: false,
        loading: this.loading,
        disabledReason: this.disabledReason,
      };
    }
    return {
      ready: true,
      ...JSON.parse(this.predictor.diagnosticsJson()),
      maxCorrectionDistance: this.maxCorrectionDistance,
      snapCorrectionCount: this.snapCorrectionCount,
      startupMs: this.startupMs,
      lastTickMs: this.lastTickMs,
      maxTickMs: this.maxTickMs,
      lastReplayTicks: this.lastReplayTicks,
      maxReplayTicks: this.maxReplayTicks,
      replayBudgetMs: this.replayBudgetMs,
      budgetExceededCount: this.budgetExceededCount,
      memoryBytes: this.refreshMemoryBytes(),
      progressCorrectionCount: this.progressCorrectionCount,
      progressLastCorrection: this.progressLastCorrection,
      progressMaxCorrection: this.progressMaxCorrection,
      progressAverageCorrection: this.progressCorrectionCount > 0
        ? this.progressCorrectionTotal / this.progressCorrectionCount
        : 0,
    };
  }

  consumeReportStats() {
    const out = {
      predictionReplayMaxMs: this.reportReplayMaxMs,
      predictionReplayMaxTicks: this.reportReplayMaxTicks,
      predictionReplayBudgetExceededCount: this.reportReplayBudgetExceededCount,
    };
    this.resetReportStats();
    return out;
  }

  resetReportStats() {
    this.reportReplayMaxMs = 0;
    this.reportReplayMaxTicks = 0;
    this.reportReplayBudgetExceededCount = 0;
  }

  recordReplayReport(elapsedMs, replayTicks, replayBudgetExceeded) {
    const elapsed = Number(elapsedMs);
    if (Number.isFinite(elapsed) && elapsed >= 0) {
      this.reportReplayMaxMs = Math.max(this.reportReplayMaxMs, elapsed);
    }
    const ticks = Number(replayTicks);
    if (Number.isFinite(ticks) && ticks >= 0) {
      this.reportReplayMaxTicks = Math.max(this.reportReplayMaxTicks, Math.trunc(ticks));
    }
    if (replayBudgetExceeded) {
      this.reportReplayBudgetExceededCount += 1;
    }
  }

  measureTicks(fn, ticks) {
    const startedAt = this.now();
    fn();
    const elapsed = this.now() - startedAt;
    this.lastTickMs = elapsed;
    this.maxTickMs = Math.max(this.maxTickMs, elapsed);
    this.lastReplayTicks = ticks;
    this.maxReplayTicks = Math.max(this.maxReplayTicks, ticks);
    this.refreshMemoryBytes();
    return elapsed;
  }

  refreshMemoryBytes() {
    const memory = this.module?.memory || this.module?.wasm?.memory;
    const bytes = memory?.buffer?.byteLength;
    if (Number.isFinite(bytes)) this.memoryBytes = bytes;
    return this.memoryBytes;
  }
}

function finiteVisualTime(value, fallback) {
  if (Number.isFinite(value)) return value;
  return Number.isFinite(fallback) ? fallback : 0;
}

function errorMessage(err) {
  if (err instanceof Error) return err.message;
  return String(err);
}

async function assertModuleAvailable(path) {
  if (typeof fetch !== "function") return;
  const response = await fetch(path, { method: "GET", cache: "no-store" });
  const contentType = response.headers?.get?.("content-type") || "";
  if (!response.ok || !/\bjavascript\b|\becmascript\b|\btext\/plain\b/.test(contentType)) {
    throw new Error("prediction WASM glue is not available; run scripts/build-sim-wasm.sh");
  }
}
