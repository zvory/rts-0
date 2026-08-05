import { captureMinimapPng } from "./minimap_capture.js";

export const MAP_PREVIEW_LIMITS = Object.freeze({
  minDimension: 64,
  maxDimension: 4096,
  maxPixels: 16_777_216,
  captureTimeoutMs: 45_000,
});

/** Expose bounded startup progress before an authoritative Match exists. */
export function installMapPreviewStartupStatus(initialError = "") {
  let error = String(initialError || "");
  const bridge = Object.freeze({
    status: () => Object.freeze({
      version: 3,
      state: error ? "failed" : "starting",
      authoritative: false,
      error,
      map: null,
      limits: MAP_PREVIEW_LIMITS,
    }),
    fail(reason) {
      error = boundedErrorMessage(reason);
      globalThis.__rtsMapPreview = bridge;
    },
  });
  globalThis.__rtsMapPreview = bridge;
  return bridge;
}

export class MapPreviewBridge {
  constructor({
    app,
    match,
    documentObj = document,
    root = documentObj.getElementById("game-screen"),
    captureTimeoutMs = MAP_PREVIEW_LIMITS.captureTimeoutMs,
  }) {
    if (!app || !match?.state?.map || !match?.minimap || !root) {
      throw new TypeError("Map preview requires an authoritative live Match.");
    }
    this.app = app;
    this.match = match;
    this.documentObj = documentObj;
    this.root = root;
    this.captureTimeoutMs = boundedTimeout(captureTimeoutMs);
    this.captureActive = false;
    this.destroyed = false;
    this.startupState = "starting";
    this.startupError = "";
    this.originalRevealAll = !!match.fog?.revealAll;
    this.controls = null;
  }

  async initialize() {
    globalThis.__rtsMapPreview = this;
    try {
      this.documentObj.body.classList.add("map-preview-mode");
      this.documentObj.title = "Minimap Preview · Bewegungskrieg";
      this.app.setCleanPresentation(true);
      this.match.fog?.setRevealAll?.(true);
      this.controls = createPreviewControls({
        documentObj: this.documentObj,
        root: this.root,
        mapName: this.match.state.map?.name || "Map",
        capture: () => this.download(),
      });
      await this.restoreInitialPreview();
      this.refreshPreviewImage();
      this.startupState = "ready";
      return this.status();
    } catch (error) {
      this.startupState = "failed";
      this.startupError = boundedErrorMessage(error);
      throw error;
    }
  }

  status() {
    const map = this.match?.state?.map;
    const entities = this.match?.state?.entitiesInterpolated?.(1) || [];
    return Object.freeze({
      version: 3,
      state: this.destroyed ? "destroyed" : this.startupState,
      authoritative: true,
      error: this.startupError,
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
    if (this.startupState !== "ready") {
      throw new Error(this.startupError || "Map preview is still starting.");
    }
    if (this.captureActive) throw new Error("A map preview capture is already active.");
    const request = normalizeCaptureRequest(input);
    this.captureActive = true;
    try {
      return await runWithDeadline(
        (signal) => this._captureMinimap(request, signal),
        this.captureTimeoutMs,
      );
    } finally {
      this.captureActive = false;
      if (!this.destroyed) await this.restoreInitialPreview();
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

  async restoreInitialPreview() {
    if (this.destroyed) return;
    this.app.setCleanPresentation(true);
    this.match.fog?.setRevealAll?.(true);
    this.match.handleResize();
    await animationFrames(2);
  }

  refreshPreviewImage() {
    const image = this.controls?.querySelector?.("[data-map-preview-image]");
    if (!image) return;
    image.src = captureMinimapPng(this.match.minimap, { width: 512, height: 512 }).pngDataUrl;
  }

  async download() {
    if (!this.controls) return;
    const status = this.controls.querySelector("[data-map-preview-status]");
    const buttons = [...this.controls.querySelectorAll("button")];
    for (const button of buttons) button.disabled = true;
    if (status) status.textContent = "Rendering minimap PNG…";
    try {
      const result = await this.call("capture", { width: 2048, height: 2048 });
      const anchor = this.documentObj.createElement("a");
      anchor.href = result.pngDataUrl;
      anchor.download = `${slug(this.match.state.map?.name)}-minimap-2048.png`;
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
  const width = boundedDimension(input?.width, "width");
  const height = boundedDimension(input?.height, "height");
  if (width * height > MAP_PREVIEW_LIMITS.maxPixels) throw new RangeError("Map preview pixel count exceeds the capture limit.");
  if (width !== height) throw new RangeError("Authoritative minimap captures must be square.");
  return Object.freeze({ kind: "minimap", width, height });
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

function captureResult(request, pngDataUrl, readiness, content) {
  if (!String(pngDataUrl).startsWith("data:image/png;base64,")) throw new Error("Map preview PNG encoding failed.");
  return Object.freeze({ version: 3, authoritative: true, ...request, pngDataUrl, readiness: structuredClone(readiness), content });
}

function assertNonblank(content) {
  if (content.uniqueColors < 2 || content.nonDominantPixels < 64) {
    throw new Error(`Map preview renderer returned a visually empty frame (${content.uniqueColors} color).`);
  }
}

function boundedDimension(value, label) {
  const dimension = Number(value);
  if (!Number.isSafeInteger(dimension) || dimension < MAP_PREVIEW_LIMITS.minDimension || dimension > MAP_PREVIEW_LIMITS.maxDimension) {
    throw new RangeError(`Map preview ${label} must be ${MAP_PREVIEW_LIMITS.minDimension}–${MAP_PREVIEW_LIMITS.maxDimension} pixels.`);
  }
  return dimension;
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
  summary.textContent = "Authoritative server-materialized minimap preview.";
  const image = documentObj.createElement("img");
  image.className = "map-preview-image";
  image.dataset.mapPreviewImage = "";
  image.alt = `${String(mapName || "Map")} minimap preview`;
  const minimap = documentObj.createElement("button");
  minimap.type = "button";
  minimap.textContent = "Download minimap PNG (2048 px)";
  minimap.addEventListener("click", () => void capture());
  const status = documentObj.createElement("p");
  status.dataset.mapPreviewStatus = "";
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.textContent = "Ready.";
  controls.append(title, summary, image, minimap, status);
  root.appendChild(controls);
  return controls;
}

function slug(value) {
  return String(value || "map").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "map";
}

function boundedErrorMessage(value) {
  return String(value?.message || value || "Map preview failed to start.")
    .replace(/[\u0000-\u001f\u007f]/g, " ")
    .slice(0, 500);
}
