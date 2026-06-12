const WASM_GLUE_PATH = "../vendor/sim-wasm/rts_sim_wasm.js";
const SNAP_CORRECTION_PX = 96;

export class SimWasmPredictionAdapter {
  constructor({
    startInfo,
    playerId,
    now = () => performance.now(),
    importModule = (path) => import(path),
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
  }

  async init() {
    if (this.ready || this.loading || this.disabledReason) return this.ready;
    this.loading = true;
    try {
      const module = await this.importModule(WASM_GLUE_PATH);
      await module.default();
      this.module = module;
      this.predictor = module.WasmPredictor.fromStartJson(
        JSON.stringify(this.startInfo),
        this.playerId,
      );
      this.ready = true;
      this.lastAdvanceAt = this.now();
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
    this.predictor.advanceTicks(1);
    this.lastPredictedTick = this.renderSnapshot()?.tick ?? this.lastPredictedTick;
    return true;
  }

  reconcile(authoritativeSnapshot, pendingCommands = []) {
    if (!this.ready || !this.predictor || !this.module || !authoritativeSnapshot) return null;
    const baselineJson = this.module.WasmPredictor.baselineFromSnapshotJson(
      JSON.stringify(authoritativeSnapshot),
      this.playerId,
    );
    this.predictor.importBaselineJson(baselineJson);
    for (const pending of pendingCommands) {
      this.predictor.enqueueCommandJson(pending.clientSeq, JSON.stringify(pending.cmd));
    }
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
    };
  }

  advanceVisual() {
    if (!this.ready || !this.predictor) return null;
    const now = this.now();
    if (this.lastAdvanceAt == null) this.lastAdvanceAt = now;
    const elapsedMs = Math.max(0, now - this.lastAdvanceAt);
    const ticks = Math.min(8, Math.floor(elapsedMs / (1000 / 30)));
    if (ticks > 0) {
      this.predictor.advanceTicks(ticks);
      this.lastAdvanceAt += ticks * (1000 / 30);
    }
    const snapshot = this.renderSnapshot();
    if (snapshot) this.lastPredictedTick = snapshot.tick;
    return snapshot;
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
    };
  }
}

function errorMessage(err) {
  if (err instanceof Error) return err.message;
  return String(err);
}
