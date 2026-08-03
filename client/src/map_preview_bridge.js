import { captureMinimapPng } from "./minimap_capture.js";

export const MAP_PREVIEW_LIMITS = Object.freeze({
  minDimension: 64,
  maxDimension: 4096,
  maxPixels: 16_777_216,
  assetTimeoutMs: 15_000,
  captureTimeoutMs: 45_000,
});

export class MapPreviewBridge {
  constructor({
    app,
    match,
    documentObj = document,
    root = documentObj.getElementById("game-screen"),
    now = () => performance.now(),
    captureTimeoutMs = MAP_PREVIEW_LIMITS.captureTimeoutMs,
  }) {
    if (!app || !match?.state?.map || !match?.renderer || !match?.minimap || !root) {
      throw new TypeError("Map preview requires an authoritative live Match.");
    }
    this.app = app;
    this.match = match;
    this.documentObj = documentObj;
    this.root = root;
    this.now = now;
    this.captureTimeoutMs = boundedTimeout(captureTimeoutMs);
    this.captureActive = false;
    this.destroyed = false;
    this.originalRevealAll = !!match.fog?.revealAll;
    this.controls = null;
  }

  async initialize() {
    this.documentObj.body.classList.add("map-preview-mode");
    this.documentObj.title = "Map Preview · Bewegungskrieg";
    this.app.setCleanPresentation(true);
    this.match.fog?.setRevealAll?.(true);
    this.controls = createPreviewControls({
      documentObj: this.documentObj,
      root: this.root,
      mapName: this.match.state.map?.name || "Map",
      capture: (kind) => this.download(kind),
    });
    globalThis.__rtsMapPreview = this;
    await this.restoreInitialPreview();
    return this.status();
  }

  status() {
    const map = this.match?.state?.map;
    const entities = this.match?.state?.entitiesInterpolated?.(1) || [];
    return Object.freeze({
      version: 2,
      state: this.destroyed ? "destroyed" : "ready",
      authoritative: true,
      map: map ? Object.freeze({
        name: String(map.name || "Map"),
        width: map.width,
        height: map.height,
        resources: Array.isArray(map.resources) ? map.resources.length : 0,
        entities: entities.length,
      }) : null,
      limits: MAP_PREVIEW_LIMITS,
    });
  }

  async call(method, input = {}) {
    if (method !== "capture") throw new RangeError("Map preview bridge supports only capture.");
    if (this.destroyed) throw new Error("Map preview bridge is destroyed.");
    if (this.captureActive) throw new Error("A map preview capture is already active.");
    const request = normalizeCaptureRequest(input);
    this.captureActive = true;
    try {
      return await runWithDeadline(
        (signal) => request.kind === "minimap"
          ? this._captureMinimap(request, signal)
          : this._captureWorld(request, signal),
        this.captureTimeoutMs,
      );
    } finally {
      this.captureActive = false;
      if (!this.destroyed) await this.restoreInitialPreview();
    }
  }

  async _captureWorld(request, signal) {
    const match = this.match;
    const cameraBefore = match.camera.snapshot();
    let fixedCapture = false;
    try {
      signal.throwIfAborted();
      match.fog?.setRevealAll?.(true);
      match.renderer.resize(request.width, request.height, 1);
      match.camera.resize(request.width, request.height);
      fitMap(match, request.padding);
      await this._waitForAssets(signal);
      signal.throwIfAborted();
      const capture = match.enterFixedCapture();
      fixedCapture = true;
      const frame = await match.renderFixedCaptureFrame(capture.visualStartMs + 16);
      signal.throwIfAborted();
      const pixels = await match.renderer.readPresentedPixels(frame.rendererFrame, { signal });
      signal.throwIfAborted();
      if (pixels.width !== request.width || pixels.height !== request.height) {
        throw new Error(`Map preview renderer returned ${pixels.width}×${pixels.height}, expected ${request.width}×${request.height}.`);
      }
      const content = analyzeRgba(pixels.rgba);
      assertNonblank(content);
      const canvas = rgbaCanvas(this.documentObj, pixels);
      return captureResult(request, canvas.toDataURL("image/png"), match.renderer.captureReadiness({}), content);
    } finally {
      if (fixedCapture) match.exitFixedCapture();
      match.handleResize();
      match.camera.restore(cameraBefore);
    }
  }

  async _captureMinimap(request, signal) {
    const match = this.match;
    signal.throwIfAborted();
    match.fog?.setRevealAll?.(true);
    const pixels = captureMinimapPng(match.minimap, { width: request.width, height: request.height });
    signal.throwIfAborted();
    const content = analyzeRgba(pixels.rgba);
    assertNonblank(content);
    return captureResult(request, pixels.pngDataUrl, {
      ready: true,
      authoritativeEntities: (match.state.entitiesInterpolated?.(1) || []).length,
      fog: "revealAll",
    }, content);
  }

  async _waitForAssets(signal) {
    const deadline = this.now() + MAP_PREVIEW_LIMITS.assetTimeoutMs;
    while (true) {
      signal.throwIfAborted();
      const entities = (this.match.state.entitiesInterpolated?.(1) || [])
        .filter((entity) => Number.isSafeInteger(entity.id))
        .slice(0, 4096);
      const readiness = this.match.renderer.captureReadiness({
        subjectIds: entities.map((entity) => entity.id),
        subjectKinds: entities.map((entity) => entity.kind),
      });
      if (readiness.failedAssets?.length || readiness.renderErrors?.length || readiness.missingTextureSubjectIds?.length) {
        throw new Error(firstReadinessFailure(readiness));
      }
      if (readiness.ready) return readiness;
      if (this.now() >= deadline) {
        const pending = (readiness.pendingAssets || []).map((asset) => asset.id).join(", ");
        throw new Error(`Map preview assets did not become ready${pending ? `: ${pending}` : "."}`);
      }
      await animationFrames(1, signal);
    }
  }

  async restoreInitialPreview() {
    if (this.destroyed) return;
    this.app.setCleanPresentation(true);
    this.match.fog?.setRevealAll?.(true);
    this.match.handleResize();
    fitMap(this.match, 24);
    await animationFrames(2);
  }

  async download(kind) {
    if (!this.controls) return;
    const status = this.controls.querySelector("[data-map-preview-status]");
    const buttons = [...this.controls.querySelectorAll("button")];
    for (const button of buttons) button.disabled = true;
    if (status) status.textContent = `Rendering ${kind} PNG…`;
    try {
      const result = await this.call("capture", { kind, width: 2048, height: 2048, padding: 32 });
      const anchor = this.documentObj.createElement("a");
      anchor.href = result.pngDataUrl;
      anchor.download = `${slug(this.match.state.map?.name)}-${kind}-2048.png`;
      this.documentObj.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      if (status) status.textContent = `Downloaded ${anchor.download}.`;
    } catch (error) {
      if (status) status.textContent = `Error: ${error?.message || String(error)}`;
    } finally {
      for (const button of buttons) button.disabled = false;
    }
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.match?.fog?.setRevealAll?.(this.originalRevealAll);
    this.controls?.remove();
    this.controls = null;
    this.documentObj.body.classList.remove("map-preview-mode");
    if (globalThis.__rtsMapPreview === this) delete globalThis.__rtsMapPreview;
  }
}

export function normalizeCaptureRequest(input) {
  const kind = input?.kind === "minimap" ? "minimap" : input?.kind === "world" ? "world" : "";
  if (!kind) throw new RangeError("Map preview kind must be world or minimap.");
  const width = boundedDimension(input?.width, "width");
  const height = boundedDimension(input?.height, "height");
  if (width * height > MAP_PREVIEW_LIMITS.maxPixels) throw new RangeError("Map preview pixel count exceeds the capture limit.");
  if (kind === "minimap" && width !== height) throw new RangeError("Authoritative minimap captures must be square.");
  const padding = kind === "world" ? boundedPadding(input?.padding ?? 24, width, height) : 0;
  return Object.freeze({ kind, width, height, padding });
}

export function analyzeRgba(rgba) {
  if (!(rgba instanceof Uint8Array || rgba instanceof Uint8ClampedArray) || rgba.length < 4 || rgba.length % 4 !== 0) {
    throw new TypeError("Map preview content analysis requires RGBA pixels.");
  }
  const colors = new Map();
  for (let index = 0; index < rgba.length; index += 4) {
    const color = (((rgba[index] * 256 + rgba[index + 1]) * 256 + rgba[index + 2]) * 256) + rgba[index + 3];
    colors.set(color, (colors.get(color) || 0) + 1);
  }
  let dominantColorPixels = 0;
  for (const count of colors.values()) dominantColorPixels = Math.max(dominantColorPixels, count);
  const pixelCount = rgba.length / 4;
  return Object.freeze({ pixelCount, uniqueColors: colors.size, dominantColorPixels, nonDominantPixels: pixelCount - dominantColorPixels });
}

function fitMap(match, paddingCssPx) {
  const map = match.state.map;
  const maxX = map.width * map.tileSize;
  const maxY = map.height * map.tileSize;
  match.camera.fitWorldPoints([
    { x: 0, y: 0 },
    { x: maxX, y: 0 },
    { x: maxX, y: maxY },
    { x: 0, y: maxY },
  ], { paddingCssPx });
}

function rgbaCanvas(documentObj, pixels) {
  const canvas = documentObj.createElement("canvas");
  canvas.width = pixels.width;
  canvas.height = pixels.height;
  const ctx = canvas.getContext("2d");
  if (!ctx || pixels.rgba?.length !== pixels.width * pixels.height * 4) throw new Error("Map preview renderer returned malformed pixels.");
  ctx.putImageData(new ImageData(new Uint8ClampedArray(pixels.rgba), pixels.width, pixels.height), 0, 0);
  return canvas;
}

function captureResult(request, pngDataUrl, readiness, content) {
  if (!String(pngDataUrl).startsWith("data:image/png;base64,")) throw new Error("Map preview PNG encoding failed.");
  return Object.freeze({ version: 2, authoritative: true, ...request, pngDataUrl, readiness: structuredClone(readiness), content });
}

function assertNonblank(content) {
  if (content.uniqueColors < 2 || content.nonDominantPixels < 64) {
    throw new Error(`Map preview renderer returned a visually empty frame (${content.uniqueColors} color).`);
  }
}

function firstReadinessFailure(readiness) {
  const asset = readiness.failedAssets?.[0];
  if (asset) return `Map preview asset ${asset.id || "unknown"} failed: ${asset.message || "unknown error"}`;
  const render = readiness.renderErrors?.[0];
  if (render) return `Map preview render failed: ${render.message || render.label || "unknown error"}`;
  return `Map preview has missing textures for ${(readiness.missingTextureSubjectIds || []).join(", ")}.`;
}

function boundedDimension(value, label) {
  const dimension = Number(value);
  if (!Number.isSafeInteger(dimension) || dimension < MAP_PREVIEW_LIMITS.minDimension || dimension > MAP_PREVIEW_LIMITS.maxDimension) {
    throw new RangeError(`Map preview ${label} must be ${MAP_PREVIEW_LIMITS.minDimension}–${MAP_PREVIEW_LIMITS.maxDimension} pixels.`);
  }
  return dimension;
}

function boundedPadding(value, width, height) {
  const padding = Number(value);
  const maximum = Math.floor(Math.min(width, height) / 4);
  if (!Number.isSafeInteger(padding) || padding < 0 || padding > maximum) throw new RangeError(`Map preview padding must be a whole number from 0 to ${maximum}.`);
  return padding;
}

function animationFrames(count, signal = null) {
  return new Promise((resolve, reject) => {
    const step = () => {
      if (signal?.aborted) return reject(signal.reason);
      return count-- <= 0 ? resolve() : requestAnimationFrame(step);
    };
    step();
  });
}

async function runWithDeadline(operation, timeoutMs) {
  const controller = new AbortController();
  const timeoutError = new Error("Map preview capture timed out.");
  const timer = setTimeout(() => controller.abort(timeoutError), timeoutMs);
  try {
    return await operation(controller.signal);
  } finally {
    clearTimeout(timer);
  }
}

function boundedTimeout(value) {
  const timeout = Number(value);
  if (!Number.isFinite(timeout) || timeout < 1 || timeout > 60_000) {
    throw new RangeError("Map preview capture timeout must be 1–60000 ms.");
  }
  return timeout;
}

function createPreviewControls({ documentObj, root, mapName, capture }) {
  const controls = documentObj.createElement("aside");
  controls.className = "map-preview-controls";
  controls.setAttribute("aria-label", "Map PNG preview controls");
  const title = documentObj.createElement("strong");
  title.textContent = String(mapName || "Map");
  const summary = documentObj.createElement("p");
  summary.textContent = "Authoritative server-materialized map preview.";
  const world = documentObj.createElement("button");
  world.type = "button";
  world.textContent = "Download world PNG (2048 px)";
  world.addEventListener("click", () => void capture("world"));
  const minimap = documentObj.createElement("button");
  minimap.type = "button";
  minimap.textContent = "Download minimap PNG (2048 px)";
  minimap.addEventListener("click", () => void capture("minimap"));
  const status = documentObj.createElement("p");
  status.dataset.mapPreviewStatus = "";
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.textContent = "Ready.";
  controls.append(title, summary, world, minimap, status);
  root.appendChild(controls);
  return controls;
}

function slug(value) {
  return String(value || "map").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "map";
}
