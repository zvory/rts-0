import {
  RENDER_WORKER_MESSAGE,
  RENDER_WORKER_RESPONSE,
  RENDER_WORKER_MESSAGE_VERSION,
  validateRenderWorkerRequest,
  validateRenderWorkerResponse,
} from "./worker_messages.js";
import { PIXI_WORKER_URL, configurePixiForWorker, installPixiWorkerEnvironment } from "./worker_environment.js";
import { compatibilityState, createWorkerPresentationState } from "./worker_rehydration.js";

let renderer = null;
let adapter = null;
let surface = "match";
let generation = 0;
let visualTimeMs = 0;
let compatibility = null;
let visualProfile = null;
let destroyed = false;
const presentation = createWorkerPresentationState();

self.addEventListener("message", (event) => {
  void handleMessage(event.data).catch((error) => fatal(error, event.data));
});

async function handleMessage(candidate) {
  if (destroyed) return;
  const message = validateRenderWorkerRequest(candidate, {
    requireCanvas: candidate?.type === RENDER_WORKER_MESSAGE.INITIALIZE,
  });
  switch (message.type) {
    case RENDER_WORKER_MESSAGE.INITIALIZE:
      await initialize(message);
      break;
    case RENDER_WORKER_MESSAGE.RESET_GENERATION:
      generation = message.generation;
      presentation.reset(generation);
      break;
    case RENDER_WORKER_MESSAGE.MAP_GENERATION:
      presentation.map(message);
      break;
    case RENDER_WORKER_MESSAGE.REVISIONED_GRIDS:
      presentation.revisions(message);
      break;
    case RENDER_WORKER_MESSAGE.DURABLE_DECALS:
      if (presentation.retainDecals(message)) {
        respond(RENDER_WORKER_RESPONSE.RETAINED, message.generation, {
          revision: message.payload.revision,
          frameId: message.payload.frameId || 0,
        });
      }
      break;
    case RENDER_WORKER_MESSAGE.FRAME:
      await presentFrame(message);
      break;
    case RENDER_WORKER_MESSAGE.RESIZE:
      adapter?.resize?.(message.payload.widthCssPx, message.payload.heightCssPx, message.payload.dpr);
      break;
    case RENDER_WORKER_MESSAGE.CAPTURE:
      capture(message);
      break;
    case RENDER_WORKER_MESSAGE.DESTROY:
      destroy(message.generation);
      break;
    default:
      throw new Error(`Unhandled render-worker message ${message.type}.`);
  }
}

async function initialize(message) {
  if (adapter) throw new Error("Pixi render worker was initialized twice.");
  generation = message.generation;
  presentation.reset(generation);
  surface = message.payload.configuration?.surface === "mapEditor" ? "mapEditor" : "match";
  installPixiWorkerEnvironment(message.payload.canvas, message.payload);
  const pixi = await import(PIXI_WORKER_URL);
  configurePixiForWorker(pixi);
  const [{ Renderer }, { PixiPresentationAdapter }, { MapEditorWorkerRenderer }] = await Promise.all([
    import("./index.js"),
    import("./pixi_compatibility_adapter.js"),
    import("./map_editor_worker_renderer.js"),
  ]);
  const parent = {
    clientWidth: message.payload.widthCssPx,
    clientHeight: message.payload.heightCssPx,
    appendChild() {},
  };
  renderer = await Renderer.create(parent, {
    canvas: message.payload.canvas,
    width: message.payload.widthCssPx,
    height: message.payload.heightCssPx,
    resolution: message.payload.dpr,
    autoDensity: true,
    renderClock: { now: () => visualTimeMs },
  });
  if (renderer.app?.renderer?.type !== pixi.RendererType.WEBGL) {
    throw new Error("Pixi worker initialized a non-WebGL backend.");
  }
  if (surface === "mapEditor") {
    adapter = new MapEditorWorkerRenderer(renderer);
  } else {
    adapter = new PixiPresentationAdapter(parent, {
      state: () => compatibility,
      profiler: () => null,
      visualProfile: () => visualProfile,
      staticMap: () => presentation.staticMap,
    }, { renderer });
    await waitForRendererAssets();
  }
  respond(RENDER_WORKER_RESPONSE.READY, generation, {
    backend: "webgl",
    pixiVersion: pixi.VERSION,
    contextAttributes: renderer.app.renderer.gl?.getContextAttributes?.() || null,
    resolution: renderer.app.renderer.resolution,
    width: renderer.app.renderer.width,
    height: renderer.app.renderer.height,
    assets: surface === "match" ? renderer.captureReadiness({}) : { ready: true },
  });
}

async function waitForRendererAssets() {
  const deadline = performance.now() + 15_000;
  while (true) {
    const readiness = renderer.captureReadiness({});
    if (readiness.failedAssets.length > 0) {
      const first = readiness.failedAssets[0];
      throw new Error(`Pixi worker asset ${first.id} failed: ${first.message || "unknown error"}`);
    }
    if (readiness.ready) return;
    if (performance.now() >= deadline) {
      throw new Error(`Pixi worker assets did not become ready: ${readiness.pendingAssets.map((asset) => asset.id).join(", ")}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
}

async function presentFrame(message) {
  if (!adapter) throw new Error("Pixi render worker received a frame before initialization.");
  const startedAt = performance.now();
  const startedAtEpochMs = epochNow();
  let frameId;
  let submission = null;
  if (message.payload.editor) {
    frameId = message.payload.editor.frameId;
    adapter.present(message.payload.editor);
  } else {
    const frame = presentation.frame(message);
    frameId = frame.frameId;
    compatibility = compatibilityState(frame.pixiCompatibility);
    visualProfile = frame.visualProfile || null;
    visualTimeMs = frame.visualTimeMs;
    submission = adapter.render(frame);
    const retained = await submission.retained;
    if (retained?.status === "retained") presentation.decalsPresented(frame.groundDecalRevision);
    const terminal = await submission.settled;
    if (terminal?.status !== "presented") {
      throw new Error(terminal?.error?.message || `Pixi frame ${frameId} was not presented.`);
    }
  }
  const timing = adapter.lastTiming || { workerUpdateMs: performance.now() - startedAt, workerPresentMs: 0 };
  const presentedAtEpochMs = epochNow();
  const submittedAtMs = Number(message.payload.submittedAtMs);
  const response = {
    frameId,
    workerUpdateMs: timing.workerUpdateMs,
    workerPresentMs: timing.workerPresentMs,
    queueAgeMs: Number.isFinite(submittedAtMs) ? Math.max(0, startedAtEpochMs - submittedAtMs) : 0,
    displayAgeMs: Number.isFinite(submittedAtMs) ? Math.max(0, presentedAtEpochMs - submittedAtMs) : 0,
    presentedAtMs: presentedAtEpochMs,
    readiness: readinessSnapshot(),
  };
  const transfer = [];
  if (message.payload.capturePixels) {
    const pixels = readPresentedPixels();
    response.rgba = pixels.rgba.buffer;
    response.width = pixels.width;
    response.height = pixels.height;
    transfer.push(pixels.rgba.buffer);
  }
  respond(RENDER_WORKER_RESPONSE.PRESENTED, message.generation, response, transfer);
}

function capture(message) {
  const payload = {
    frameId: message.payload.frameId,
    captureId: message.payload.captureId,
    workerUpdateMs: 0,
    workerPresentMs: 0,
    queueAgeMs: 0,
    displayAgeMs: 0,
  };
  let transfer = [];
  if (message.payload.readPixels) {
    const { rgba, width, height } = readPresentedPixels();
    payload.rgba = rgba.buffer;
    payload.width = width;
    payload.height = height;
    transfer = [rgba.buffer];
  }
  respond(RENDER_WORKER_RESPONSE.PRESENTED, message.generation, payload, transfer);
}

function readPresentedPixels() {
  const gl = renderer?.app?.renderer?.gl;
  const width = renderer?.app?.renderer?.width || 0;
  const height = renderer?.app?.renderer?.height || 0;
  if (!gl || width <= 0 || height <= 0) throw new Error("Pixi worker capture has no readable WebGL surface.");
  const bottomUp = new Uint8Array(width * height * 4);
  gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, bottomUp);
  const rgba = new Uint8Array(bottomUp.length);
  const stride = width * 4;
  for (let y = 0; y < height; y += 1) {
    rgba.set(bottomUp.subarray(y * stride, (y + 1) * stride), (height - y - 1) * stride);
  }
  return { rgba, width, height };
}

function epochNow() {
  return performance.timeOrigin + performance.now();
}

function readinessSnapshot() {
  if (surface === "mapEditor") return { ready: true };
  return {
    ...renderer.captureReadiness({}),
    groundDecals: renderer.groundDecalDiagnostics(),
    trenches: renderer.trenchDiagnostics(),
    visualSamples: renderer.visualSampleDiagnostics(),
    visualUnitOverrides: renderer.visualUnitOverrideDiagnostics(),
  };
}

function fatal(error, candidate) {
  if (destroyed) return;
  const message = String(error?.message || error || "Unknown Pixi worker failure").slice(0, 500);
  try {
    respond(RENDER_WORKER_RESPONSE.FAILED, candidate?.generation || generation || 1, {
      frameId: candidate?.payload?.frame?.frameId || candidate?.payload?.editor?.frameId || 0,
      code: "renderWorkerFailure",
      message,
    });
  } finally {
    destroy(candidate?.generation || generation || 1);
  }
}

function destroy(responseGeneration) {
  if (destroyed) return;
  destroyed = true;
  try { adapter?.destroy?.(); } catch {}
  adapter = null;
  renderer = null;
  respond(RENDER_WORKER_RESPONSE.DESTROYED, responseGeneration || generation || 1, {});
  self.close();
}

function respond(type, responseGeneration, payload, transfer = []) {
  const message = {
    version: RENDER_WORKER_MESSAGE_VERSION,
    type,
    generation: responseGeneration,
    payload,
  };
  validateRenderWorkerResponse(message);
  self.postMessage(message, transfer);
}
