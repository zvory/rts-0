import {
  createCaptureMessage,
  createDestroyMessage,
  createDurableDecalMessage,
  createEditorFrameMessage,
  createFrameMessages,
  createInitializeMessage,
  createMapGenerationMessage,
  createRenderWorkerWireState,
  createResetGenerationMessage,
  createResizeMessage,
  RENDER_WORKER_MESSAGE,
  RENDER_WORKER_RESPONSE,
  validateRenderWorkerResponse,
} from "./worker_messages.js";
import {
  createPresentationSubmission,
  immediatePresentationSubmission,
  outcomeRecord,
  PRESENTATION_OUTCOME,
} from "../presentation/submission.js";

const READY_TIMEOUT_MS = 20_000;
const MAX_TIMING_SAMPLES = 2048;

export class PixiWorkerPresentationAdapter {
  static async create(canvasParent, sources, options = {}) {
    assertWorkerCapabilities();
    const canvas = document.createElement("canvas");
    canvas.className = "rts-pixi-worker-canvas";
    canvas.style.imageRendering = "pixelated";
    canvas.style.width = "100%";
    canvas.style.height = "100%";
    const widthCssPx = positiveSize(canvasParent.clientWidth || globalThis.innerWidth);
    const heightCssPx = positiveSize(canvasParent.clientHeight || globalThis.innerHeight);
    const dpr = currentDpr();
    canvas.width = Math.ceil(widthCssPx * dpr);
    canvas.height = Math.ceil(heightCssPx * dpr);
    canvasParent.appendChild(canvas);
    const offscreen = canvas.transferControlToOffscreen();
    const worker = new Worker(new URL("./pixi_render_worker.js", import.meta.url), { type: "module" });
    const adapter = new PixiWorkerPresentationAdapter(canvasParent, canvas, worker, sources, options);
    try {
      await adapter._initialize(offscreen, { widthCssPx, heightCssPx, dpr });
      return adapter;
    } catch (error) {
      adapter.destroy();
      throw error;
    }
  }

  constructor(canvasParent, canvas, worker, sources, { surface = "match" } = {}) {
    this.id = "pixi";
    this.surface = surface === "mapEditor" ? "mapEditor" : "match";
    this._parent = canvasParent;
    this._canvas = canvas;
    this._worker = worker;
    this._sources = sources || {};
    this._wireState = createRenderWorkerWireState();
    this._generation = 1;
    this._staticMapRevision = null;
    this._inFlight = null;
    this._pending = null;
    this._pendingResize = null;
    this._destroyed = false;
    this._fatal = null;
    this._workerTerminated = false;
    this._captureMode = false;
    this._captureId = 0;
    this._captureRequests = new Map();
    this._decalWaiters = new Map();
    this._sentDecalRevision = 0;
    this._retainedDecalRevision = 0;
    this._lastPresentedFrameId = 0;
    this._lastCapturedPixels = null;
    this._renderFrameCount = 0;
    this._lastReadiness = { frame: 0, assets: [], ready: false, failedAssets: [], pendingAssets: [] };
    this._backendInfo = null;
    this._stats = freshStats();
    this.app = {
      canvas,
      renderer: { width: canvas.width, height: canvas.height, gl: null, type: "webgl-worker" },
    };
    this._onMessage = (event) => this._handleMessage(event.data);
    this._onError = (event) => this._failFatal(event.error || new Error(event.message || "Pixi render worker failed."));
    worker.addEventListener("message", this._onMessage);
    worker.addEventListener("error", this._onError);
    this._control = {
      reset: () => this._resetStats(),
      snapshot: () => this.diagnostics(),
    };
    globalThis.__rtsRenderWorkerControl = this._control;
    this._publishStats();
  }

  render(frame) {
    const identity = { generation: frame?.generation, frameId: frame?.frameId };
    if (this._destroyed) {
      return immediatePresentationSubmission({ ...identity, status: PRESENTATION_OUTCOME.DESTROYED });
    }
    if (this._fatal) {
      return immediatePresentationSubmission({ ...identity, status: PRESENTATION_OUTCOME.FAILED, error: this._fatal });
    }
    if (!frame || frame.version !== 2) throw new TypeError("Pixi worker requires PresentationFrameV2.");
    this._ensureGeneration(frame.generation);
    const job = createJob(frame);
    this._retainDurableDecals(job);
    this._schedule(job);
    return job.submission;
  }

  presentEditor(record) {
    if (this.surface !== "mapEditor") throw new Error("Editor records require the Map Editor worker surface.");
    if (this._destroyed || this._fatal) return Promise.resolve({ status: "destroyed", frameId: record?.frameId || 0 });
    const job = createEditorJob(record, this._generation);
    this._schedule(job);
    return job.settled.promise;
  }

  resize(widthCssPx, heightCssPx) {
    if (this._destroyed || this._fatal) return;
    const width = positiveSize(widthCssPx);
    const height = positiveSize(heightCssPx);
    const dpr = currentDpr();
    this._canvas.style.width = `${width}px`;
    this._canvas.style.height = `${height}px`;
    this.app.renderer.width = Math.ceil(width * dpr);
    this.app.renderer.height = Math.ceil(height * dpr);
    if (this._pending && !this._captureMode) this._settleSuperseded(this._takePending());
    this._pendingResize = {
      generation: this._generation,
      frameId: this._lastPresentedFrameId,
      widthCssPx: width,
      heightCssPx: height,
      dpr,
    };
    if (!this._inFlight) this._flushResize();
  }

  setRenderClock() {}

  enterFixedCapture() {
    this._captureMode = true;
    if (this._pending) this._settleSuperseded(this._takePending());
  }

  exitFixedCapture() {
    this._captureMode = false;
  }

  async readPresentedPixels(frameId = this._lastPresentedFrameId) {
    if (this._destroyed || this._fatal) throw this._fatal || new Error("Pixi render worker is destroyed.");
    if (frameId !== this._lastPresentedFrameId) {
      throw new Error(`Cannot capture worker frame ${frameId}; visible frame is ${this._lastPresentedFrameId}.`);
    }
    if (this._lastCapturedPixels?.frameId === frameId) return this._lastCapturedPixels;
    this._captureId += 1;
    const request = deferred();
    this._captureRequests.set(this._captureId, request);
    this._post(createCaptureMessage({
      generation: this._generation,
      frameId,
      captureId: this._captureId,
      readPixels: true,
    }));
    return request.promise;
  }

  captureReadiness() {
    return this._lastReadiness;
  }

  groundDecalDiagnostics() {
    return this._lastReadiness.groundDecals || { assetStatus: "idle" };
  }

  trenchDiagnostics() {
    return this._lastReadiness.trenches || {};
  }

  visualSampleDiagnostics() {
    return this._lastReadiness.visualSamples || {};
  }

  visualUnitOverrideDiagnostics() {
    return this._lastReadiness.visualUnitOverrides || {};
  }

  diagnostics() {
    return Object.freeze(this._publicStats());
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    const error = new Error("Pixi render worker was destroyed.");
    this._settleJob(this._inFlight, PRESENTATION_OUTCOME.DESTROYED, error);
    this._settleJob(this._pending, PRESENTATION_OUTCOME.DESTROYED, error);
    this._inFlight = null;
    this._pending = null;
    this._pendingResize = null;
    this._settleDecalWaiters(null);
    for (const request of this._captureRequests.values()) request.reject(error);
    this._captureRequests.clear();
    if (!this._workerTerminated) {
      try { this._post(createDestroyMessage(this._generation)); } catch {}
    }
    this._teardownWorker();
    this._stats.destroyed = true;
    if (globalThis.__rtsRenderWorkerControl === this._control) delete globalThis.__rtsRenderWorkerControl;
    this._publishStats();
  }

  _initialize(canvas, size) {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        cleanup();
        reject(new Error("Pixi render worker did not become ready within 20 seconds."));
      }, READY_TIMEOUT_MS);
      const onMessage = (event) => {
        let message;
        try { message = validateRenderWorkerResponse(event.data); } catch { return; }
        if (message.type === RENDER_WORKER_RESPONSE.READY) {
          cleanup();
          if (message.payload.backend !== "webgl") {
            reject(new Error("Pixi render worker did not initialize WebGL."));
          } else {
            this._lastReadiness = message.payload.assets || this._lastReadiness;
            this._backendInfo = message.payload;
            resolve();
          }
        } else if (message.type === RENDER_WORKER_RESPONSE.FAILED) {
          cleanup();
          reject(new Error(message.payload.message || "Pixi render worker failed to initialize."));
        }
      };
      const onError = (event) => {
        cleanup();
        reject(event.error || new Error(event.message || "Pixi render worker failed during startup."));
      };
      const cleanup = () => {
        clearTimeout(timeout);
        this._worker.removeEventListener("message", onMessage);
        this._worker.removeEventListener("error", onError);
      };
      this._worker.addEventListener("message", onMessage);
      this._worker.addEventListener("error", onError);
      this._post(createInitializeMessage({
        canvas,
        ...size,
        configuration: { surface: this.surface },
      }));
    });
  }

  _ensureGeneration(nextGeneration) {
    if (nextGeneration === this._generation) return;
    if (nextGeneration < this._generation) throw new Error("Pixi worker rejected a stale generation.");
    this._settleJob(this._inFlight, PRESENTATION_OUTCOME.SUPERSEDED);
    this._settleJob(this._pending, PRESENTATION_OUTCOME.SUPERSEDED);
    this._inFlight = null;
    this._pending = null;
    this._pendingResize = null;
    this._generation = nextGeneration;
    this._staticMapRevision = null;
    this._wireState = createRenderWorkerWireState();
    this._sentDecalRevision = 0;
    this._retainedDecalRevision = 0;
    this._settleDecalWaiters(null);
    const resetError = new Error("Pixi render worker generation changed during capture.");
    for (const request of this._captureRequests.values()) request.reject(resetError);
    this._captureRequests.clear();
    this._lastPresentedFrameId = 0;
    this._lastCapturedPixels = null;
    this._post(createResetGenerationMessage(nextGeneration));
  }

  _retainDurableDecals(job) {
    const revision = job.frame.groundDecalRevision || 0;
    if (revision <= 0) {
      job.retained.resolve(null);
      return;
    }
    if (revision <= this._retainedDecalRevision) {
      job.retained.resolve(outcomeRecord(PRESENTATION_OUTCOME.RETAINED, job, { groundDecalRevision: revision }));
      return;
    }
    const waiters = this._decalWaiters.get(revision) || [];
    waiters.push(job);
    this._decalWaiters.set(revision, waiters);
    if (revision <= this._sentDecalRevision) return;
    const durable = createDurableDecalMessage(job.frame);
    if (!durable) {
      job.retained.resolve(null);
      return;
    }
    durable.message.payload.frameId = job.frame.frameId;
    this._sentDecalRevision = revision;
    this._post(durable);
  }

  _schedule(job) {
    this._stats.submitted += 1;
    this._recordCounter("renderWorker.frames.submitted");
    if (!this._inFlight) {
      this._submit(job);
      return;
    }
    if (this._pending) this._settleSuperseded(this._takePending());
    this._pending = job;
    this._stats.retainedPending += 1;
    this._recordCounter("renderWorker.frames.retainedPending");
    this._publishStats();
  }

  _submit(job) {
    if (this._destroyed || this._fatal) {
      this._settleJob(job, this._destroyed ? PRESENTATION_OUTCOME.DESTROYED : PRESENTATION_OUTCOME.FAILED, this._fatal);
      return;
    }
    const startedAt = performance.now();
    try {
      const packets = job.editor
        ? [createEditorFrameMessage(job.editor, job.generation)]
        : this._framePackets(job.frame);
      const submittedAtMs = epochNow();
      for (const packet of packets) {
        if (packet.message.type === RENDER_WORKER_MESSAGE.FRAME) {
          packet.message.payload.submittedAtMs = submittedAtMs;
          packet.message.payload.capturePixels = this._captureMode;
          if (packet.message.payload.frame) {
            packet.message.payload.frame.pixiCompatibility = compatibilitySnapshot(job.frame, this._sources?.state?.());
            packet.message.payload.frame.visualProfile = cloneOptional(this._sources?.visualProfile?.());
          }
        }
        this._stats.clonedBytes += packet.transfer.reduce((sum, item) => sum + (item?.byteLength || 0), 0);
        this._post(packet);
      }
      const mainSubmitMs = performance.now() - startedAt;
      pushTiming(this._stats.mainSubmitMs, mainSubmitMs);
      job.submittedAtMs = submittedAtMs;
      this._inFlight = job;
      this._stats.dispatched += 1;
      this._publishStats();
    } catch (error) {
      this._settleJob(job, PRESENTATION_OUTCOME.FAILED, error);
      this._failFatal(error);
    }
  }

  _framePackets(frame) {
    const packets = [];
    if (this._staticMapRevision !== frame.staticMapRevision) {
      const staticMap = this._sources?.staticMap?.();
      if (!staticMap || staticMap.revision !== frame.staticMapRevision || staticMap.generation !== frame.generation) {
        throw new Error("Pixi worker static-map revision is unavailable.");
      }
      packets.push(createMapGenerationMessage(staticMap));
      this._staticMapRevision = staticMap.revision;
    }
    packets.push(...createFrameMessages(frame, this._wireState)
      .filter((packet) => packet.message.type !== RENDER_WORKER_MESSAGE.DURABLE_DECALS));
    return packets;
  }

  _handleMessage(candidate) {
    if (this._destroyed) return;
    let message;
    try {
      message = validateRenderWorkerResponse(candidate);
    } catch (error) {
      this._failFatal(error);
      return;
    }
    if (message.generation !== this._generation) {
      this._stats.staleResponses += 1;
      this._recordCounter("renderWorker.responses.stale");
      this._publishStats();
      return;
    }
    if (message.type === RENDER_WORKER_RESPONSE.READY) return;
    if (message.type === RENDER_WORKER_RESPONSE.RETAINED) {
      this._acceptRetained(message);
      return;
    }
    if (message.type === RENDER_WORKER_RESPONSE.PRESENTED && message.payload.captureId) {
      const request = this._captureRequests.get(message.payload.captureId);
      if (!request) return;
      this._captureRequests.delete(message.payload.captureId);
      request.resolve({
        frameId: message.payload.frameId,
        width: message.payload.width,
        height: message.payload.height,
        rgba: new Uint8Array(message.payload.rgba),
      });
      return;
    }
    if (message.type === RENDER_WORKER_RESPONSE.PRESENTED) {
      this._acceptPresented(message);
      return;
    }
    if (message.type === RENDER_WORKER_RESPONSE.FAILED) {
      this._failFatal(new Error(message.payload.message || "Pixi render worker failed."));
    }
  }

  _acceptRetained(message) {
    const revision = message.payload.revision;
    this._retainedDecalRevision = Math.max(this._retainedDecalRevision, revision);
    const waiters = this._decalWaiters.get(revision) || [];
    this._decalWaiters.delete(revision);
    for (const job of waiters) {
      job.retained.resolve(outcomeRecord(PRESENTATION_OUTCOME.RETAINED, job, { groundDecalRevision: revision }));
    }
    this._stats.retained += 1;
    this._recordCounter("renderWorker.frames.retained");
    this._publishStats();
  }

  _acceptPresented(message) {
    const job = this._inFlight;
    if (!job || job.frameId !== message.payload.frameId || job.generation !== message.generation) {
      this._failFatal(new Error(`Unexpected worker presentation ${message.generation}:${message.payload.frameId}.`));
      return;
    }
    queueMicrotask(() => this._commitPresented(message, job));
  }

  _commitPresented(message, job) {
    if (this._destroyed || this._fatal || this._inFlight !== job || message.generation !== this._generation) return;
    this._inFlight = null;
    this._lastPresentedFrameId = job.frameId;
    if (message.payload.rgba instanceof ArrayBuffer) {
      this._lastCapturedPixels = {
        frameId: job.frameId,
        width: message.payload.width,
        height: message.payload.height,
        rgba: new Uint8Array(message.payload.rgba),
      };
    } else {
      this._lastCapturedPixels = null;
    }
    this._renderFrameCount += 1;
    this._stats.presented += 1;
    this._stats.completed += 1;
    pushTiming(this._stats.workerUpdateMs, message.payload.workerUpdateMs);
    pushTiming(this._stats.workerPresentMs, message.payload.workerPresentMs);
    pushTiming(this._stats.queueAgeMs, message.payload.queueAgeMs);
    const displayAgeMs = Number.isFinite(job.submittedAtMs)
      ? Math.max(0, epochNow() - job.submittedAtMs)
      : message.payload.displayAgeMs;
    pushTiming(this._stats.displayAgeMs, displayAgeMs);
    if (message.payload.workerUpdateMs + message.payload.workerPresentMs > 16.67) this._stats.longFrames += 1;
    if (message.payload.readiness) this._lastReadiness = message.payload.readiness;
    const outcome = outcomeRecord(PRESENTATION_OUTCOME.PRESENTED, job, {
      workerUpdateMs: message.payload.workerUpdateMs,
      workerPresentMs: message.payload.workerPresentMs,
      queueAgeMs: message.payload.queueAgeMs,
      displayAgeMs,
    });
    job.settled.resolve(outcome);
    this._recordCounter("renderWorker.frames.presented");
    const next = this._takePending();
    this._flushResize();
    this._publishStats();
    if (next) this._submit(next);
  }

  _settleSuperseded(job) {
    if (!job) return;
    this._stats.superseded += 1;
    this._recordCounter("renderWorker.frames.superseded");
    this._settleJob(job, PRESENTATION_OUTCOME.SUPERSEDED);
  }

  _settleJob(job, status, error = null) {
    if (!job || job.terminalSettled) return;
    job.terminalSettled = true;
    if (!job.retained.settled && (status === PRESENTATION_OUTCOME.DESTROYED || status === PRESENTATION_OUTCOME.FAILED)) {
      job.retained.resolve(null);
    }
    job.settled.resolve(outcomeRecord(status, job, error ? { error: { name: error.name || "Error", message: error.message || String(error) } } : {}));
  }

  _takePending() {
    const pending = this._pending;
    this._pending = null;
    return pending;
  }

  _failFatal(error) {
    if (this._fatal || this._destroyed) return;
    this._fatal = error instanceof Error ? error : new Error(String(error));
    this._stats.failed += 1;
    this._stats.lastError = this._fatal.message.slice(0, 500);
    this._settleJob(this._inFlight, PRESENTATION_OUTCOME.FAILED, this._fatal);
    this._settleJob(this._pending, PRESENTATION_OUTCOME.FAILED, this._fatal);
    this._inFlight = null;
    this._pending = null;
    this._pendingResize = null;
    this._settleDecalWaiters(null);
    for (const request of this._captureRequests.values()) request.reject(this._fatal);
    this._captureRequests.clear();
    this._showFatal(this._fatal.message);
    this._teardownWorker();
    this._recordCounter("renderWorker.frames.failed");
    console.error("[RTS_RENDER_WORKER] fatal renderer error", this._fatal);
    this._publishStats();
  }

  _settleDecalWaiters(value) {
    for (const waiters of this._decalWaiters.values()) {
      for (const job of waiters) job.retained.resolve(value);
    }
    this._decalWaiters.clear();
  }

  _showFatal(message) {
    if (this._parent.querySelector?.(".renderer-fatal-error")) return;
    const error = document.createElement("div");
    error.className = "renderer-fatal-error";
    error.setAttribute("role", "alert");
    error.textContent = `Renderer stopped: ${message}`;
    this._parent.appendChild(error);
  }

  _flushResize() {
    if (!this._pendingResize || this._destroyed || this._fatal) return;
    const resize = this._pendingResize;
    this._pendingResize = null;
    this._post(createResizeMessage(resize));
  }

  _teardownWorker() {
    if (this._workerTerminated) return;
    this._workerTerminated = true;
    this._worker.removeEventListener("message", this._onMessage);
    this._worker.removeEventListener("error", this._onError);
    this._worker.terminate();
    this._canvas.remove();
  }

  _post(packet) {
    const message = packet?.message || packet;
    const transfer = packet?.transfer || [];
    this._worker.postMessage(message, transfer);
  }

  _recordCounter(label, amount = 1) {
    this._sources?.profiler?.()?.recordDiagnosticCounter?.(label, amount);
  }

  _publishStats() {
    if (globalThis.__rtsRenderWorkerControl !== this._control) return;
    globalThis.__rtsRenderWorkerStats = this._publicStats();
  }

  _resetStats() {
    const active = {
      lastPresentedFrameId: this._lastPresentedFrameId,
      destroyed: this._destroyed,
      backendInfo: this._backendInfo,
      lastError: this._stats.lastError,
    };
    this._stats = freshStats();
    Object.assign(this._stats, active);
    this._publishStats();
  }

  _publicStats() {
    return {
      mode: "pixi-webgl-module-worker",
      surface: this.surface,
      submitted: this._stats.submitted,
      dispatched: this._stats.dispatched,
      retained: this._stats.retained,
      presented: this._stats.presented,
      completed: this._stats.completed,
      superseded: this._stats.superseded,
      failed: this._stats.failed,
      longFrames: this._stats.longFrames,
      staleResponses: this._stats.staleResponses,
      lastError: this._stats.lastError,
      inFlight: !!this._inFlight,
      pending: !!this._pending,
      captureMode: this._captureMode,
      clonedBytes: this._stats.clonedBytes,
      lastPresentedFrameId: this._lastPresentedFrameId,
      destroyed: this._destroyed,
      backendInfo: this._backendInfo,
      mainSubmitMs: summarize(this._stats.mainSubmitMs),
      queueAgeMs: summarize(this._stats.queueAgeMs),
      displayAgeMs: summarize(this._stats.displayAgeMs),
      workerUpdateMs: summarize(this._stats.workerUpdateMs),
      workerPresentMs: summarize(this._stats.workerPresentMs),
    };
  }
}

function createJob(frame) {
  const retained = trackedDeferred();
  const settled = trackedDeferred();
  const job = {
    generation: frame.generation,
    frameId: frame.frameId,
    frame,
    editor: null,
    retained,
    settled,
    terminalSettled: false,
  };
  job.submission = createPresentationSubmission({
    generation: job.generation,
    frameId: job.frameId,
    retained: retained.promise,
    settled: settled.promise,
  });
  return job;
}

function createEditorJob(record, generation) {
  const retained = trackedDeferred();
  retained.resolve(null);
  const settled = trackedDeferred();
  return {
    generation,
    frameId: record.frameId,
    frame: null,
    editor: record,
    retained,
    settled,
    terminalSettled: false,
  };
}

function compatibilitySnapshot(frame, state) {
  const entities = [];
  for (const records of Object.values(frame.layers || {})) {
    for (const record of records || []) {
      if (["entity", "intelEntity", "shotRevealEntity"].includes(record?.type)) entities.push(record);
    }
  }
  const poses = [];
  for (const entity of entities) {
    const current = finitePose(state?._curById?.get?.(entity.id));
    const previous = finitePose(state?._prevById?.get?.(entity.id));
    let recoil = 0;
    let recoilPhase = 0;
    let recoilKind = null;
    try {
      recoil = finiteOrZero(state?.weaponRecoil?.(entity.id, entity.kind, frame.visualTimeMs));
      recoilPhase = finiteOrZero(state?.weaponRecoilPhase?.(entity.id, entity.kind, frame.visualTimeMs));
      if (recoil > 0) recoilKind = state?.weaponRecoilKind?.(entity.id) || null;
    } catch {}
    if (current || previous || recoil || recoilPhase || recoilKind) {
      poses.push({ id: entity.id, current, previous, recoil, recoilPhase, recoilKind });
    }
  }
  return {
    oil: Number.isFinite(state?.resources?.oil) ? state.resources.oil : null,
    poses,
  };
}

function cloneOptional(value) {
  return value == null ? null : structuredClone(value);
}

function finitePose(value) {
  return Number.isFinite(value?.x) && Number.isFinite(value?.y) ? { x: value.x, y: value.y } : null;
}

function finiteOrZero(value) {
  return Number.isFinite(value) ? value : 0;
}

function assertWorkerCapabilities() {
  if (typeof Worker !== "function" || typeof HTMLCanvasElement !== "function"
    || typeof HTMLCanvasElement.prototype.transferControlToOffscreen !== "function") {
    throw new Error("This browser requires module workers and OffscreenCanvas to render the game.");
  }
}

function currentDpr() {
  const value = Number(globalThis.devicePixelRatio || 1);
  return Math.max(0.25, Math.min(8, Number.isFinite(value) ? value : 1));
}

function positiveSize(value) {
  const number = Number(value);
  return Math.max(1, Number.isFinite(number) ? number : 1);
}

function epochNow() {
  return performance.timeOrigin + performance.now();
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function trackedDeferred() {
  const result = deferred();
  result.settled = false;
  const resolve = result.resolve;
  result.resolve = (value) => {
    if (result.settled) return;
    result.settled = true;
    resolve(value);
  };
  return result;
}

function pushTiming(values, value) {
  if (!Number.isFinite(value) || value < 0) return;
  values.push(value);
  if (values.length > MAX_TIMING_SAMPLES) values.shift();
}

function summarize(values) {
  if (!values.length) return { samples: 0, avg: 0, p50: 0, p95: 0, max: 0 };
  const sorted = [...values].sort((a, b) => a - b);
  return {
    samples: values.length,
    avg: round(values.reduce((sum, value) => sum + value, 0) / values.length),
    p50: round(sorted[Math.floor((sorted.length - 1) * 0.5)]),
    p95: round(sorted[Math.floor((sorted.length - 1) * 0.95)]),
    max: round(sorted[sorted.length - 1]),
  };
}

function round(value) {
  return Math.round(value * 100) / 100;
}

function freshStats() {
  return {
    submitted: 0,
    dispatched: 0,
    retainedPending: 0,
    retained: 0,
    presented: 0,
    completed: 0,
    superseded: 0,
    failed: 0,
    longFrames: 0,
    staleResponses: 0,
    clonedBytes: 0,
    lastError: "",
    mainSubmitMs: [],
    queueAgeMs: [],
    displayAgeMs: [],
    workerUpdateMs: [],
    workerPresentMs: [],
  };
}
