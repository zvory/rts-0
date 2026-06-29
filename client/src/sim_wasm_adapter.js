const WASM_GLUE_PATH = "../vendor/sim-wasm/rts_sim_wasm.js";
const SNAP_CORRECTION_PX = 96;
const DEFAULT_REPLAY_BUDGET_MS = 4;

export class SimWasmPredictionAdapter {
  constructor({
    startInfo,
    playerId,
    now = () => performance.now(),
    importModule = (path) => import(path),
    replayBudgetMs = DEFAULT_REPLAY_BUDGET_MS,
  } = {}) {
    this.startInfo = startInfo;
    this.playerId = playerId;
    this.now = now;
    this.importModule = importModule;
    this.ready = false;
    this.disabledReason = null;
    this.loading = false;
    this.module = null;
    this.predictor = null;
    this.lastPredictedTick = null;
    this.lastAdvanceAt = null;
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
      this.lastAdvanceAt = this.now();
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
  }

  enqueueCommand(clientSeq, command) {
    if (!this.ready || !this.predictor) return false;
    this.predictor.enqueueCommandJson(clientSeq, JSON.stringify(command));
    this.measureTicks(() => this.predictor.advanceTicks(1), 1);
    this.lastPredictedTick = this.renderSnapshot()?.tick ?? this.lastPredictedTick;
    return true;
  }

  reconcile(authoritativeSnapshot, pendingCommands = []) {
    if (!this.ready || !this.predictor || !this.module || !authoritativeSnapshot) return null;
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
    const replayBudgetExceeded = elapsed > this.replayBudgetMs;
    this.recordReplayReport(elapsed, replayTicks, replayBudgetExceeded);
    if (replayBudgetExceeded) this.budgetExceededCount += 1;
    const diagnostics = this.diagnostics();
    const correction = Number(diagnostics?.correctionMagnitude) || 0;
    this.maxCorrectionDistance = Math.max(this.maxCorrectionDistance, correction);
    if (correction > SNAP_CORRECTION_PX) this.snapCorrectionCount += 1;
    this.lastPredictedTick = this.renderSnapshot()?.tick ?? authoritativeSnapshot.tick ?? null;
    this.lastAdvanceAt = this.now();
    return {
      diagnostics,
      correctionDistance: correction,
      snapCorrection: correction > SNAP_CORRECTION_PX,
      maxCorrectionDistance: this.maxCorrectionDistance,
      snapCorrectionCount: this.snapCorrectionCount,
      replayBudgetExceeded,
    };
  }

  advanceVisual() {
    if (!this.ready || !this.predictor) return null;
    const now = this.now();
    if (this.lastAdvanceAt == null) this.lastAdvanceAt = now;
    const elapsedMs = Math.max(0, now - this.lastAdvanceAt);
    const ticks = Math.min(8, Math.floor(elapsedMs / (1000 / 30)));
    if (ticks > 0) {
      this.measureTicks(() => this.predictor.advanceTicks(ticks), ticks);
      this.lastAdvanceAt += ticks * (1000 / 30);
    }
    const snapshot = this.renderSnapshot();
    if (snapshot) this.lastPredictedTick = snapshot.tick;
    return snapshot;
  }

  pauseVisualClock() {
    const now = this.now();
    if (Number.isFinite(now)) this.lastAdvanceAt = now;
  }

  renderSnapshot() {
    if (!this.ready || !this.predictor) return null;
    return JSON.parse(this.predictor.renderSnapshotJson());
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
