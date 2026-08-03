import { createMapEditorPresentation } from "./map_editor_presentation.js";
import { paintMinimapMap } from "./minimap_map_painter.js";
import { PRESENTATION_OUTCOME } from "./presentation/submission.js";

export const MAP_PREVIEW_LIMITS = Object.freeze({
  minDimension: 64,
  maxDimension: 4096,
  maxPixels: 16_777_216,
  assetTimeoutMs: 15_000,
});

const TILE_SIZE = 32;
const WORLD_VISUAL_TIME_MS = 120_000;

export class MapPreviewBridge {
  constructor({ session, presentation, documentObj = document, now = () => performance.now() }) {
    if (!session?.draft || !presentation) throw new TypeError("Map preview requires a loaded map and presentation adapter.");
    this.session = session;
    this.presentation = presentation;
    this.documentObj = documentObj;
    this.now = now;
    this.frameId = 0;
    this.captureActive = false;
    this.destroyed = false;
  }

  status() {
    return Object.freeze({
      version: 1,
      state: this.destroyed ? "destroyed" : "ready",
      map: this.destroyed ? null : Object.freeze({
        name: String(this.session.draft?.name || "Map"),
        width: this.session.draft?.width || 0,
        height: this.session.draft?.height || 0,
      }),
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
      return request.kind === "minimap"
        ? this._captureMinimap(request)
        : await this._captureWorld(request);
    } finally {
      this.captureActive = false;
    }
  }

  _captureMinimap(request) {
    const materialized = this.session.materialized();
    const canvas = this.documentObj.createElement("canvas");
    canvas.width = request.width;
    canvas.height = request.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("Map preview could not create a minimap canvas.");
    paintMinimapMap(ctx, { ...materialized, tileSize: TILE_SIZE });
    return captureResult(request, canvas.toDataURL("image/png"), {
      ready: true,
      assets: [],
      failedAssets: [],
      pendingAssets: [],
    });
  }

  async _captureWorld(request) {
    const materialized = this.session.materialized();
    this.presentation.resize(request.width, request.height);
    const camera = framedCamera(materialized, request);
    const deadline = this.now() + MAP_PREVIEW_LIMITS.assetTimeoutMs;
    let firstFrame = true;
    let readiness = null;
    this.presentation.enterFixedCapture();
    try {
      while (true) {
        this.frameId += 1;
        const outcome = await this.presentation.present(createMapEditorPresentation({
          frameId: this.frameId,
          camera,
          terrainUpdate: firstFrame ? {
            kind: "replace",
            revision: 1,
            width: materialized.width,
            height: materialized.height,
            tileSize: TILE_SIZE,
            terrain: materialized.terrain,
          } : null,
          doodadUpdate: firstFrame ? {
            kind: "replace",
            revision: 1,
            doodads: materialized.doodads || [],
          } : null,
          overlay: firstFrame ? emptyEditorOverlay() : null,
          visualTimeMs: WORLD_VISUAL_TIME_MS,
        }));
        firstFrame = false;
        if (outcome?.status !== PRESENTATION_OUTCOME.PRESENTED) {
          throw new Error(outcome?.error?.message || `Map preview frame was not presented (${String(outcome?.status || "unknown")}).`);
        }
        readiness = this.presentation.captureReadiness();
        if (readiness?.failedAssets?.length) {
          const asset = readiness.failedAssets[0];
          throw new Error(`Map preview asset ${asset.id || "unknown"} failed: ${asset.message || "unknown error"}`);
        }
        if (readiness?.ready) break;
        if (this.now() >= deadline) {
          const pending = (readiness?.pendingAssets || []).map((asset) => asset.id).join(", ");
          throw new Error(`Map preview assets did not become ready${pending ? `: ${pending}` : "."}`);
        }
        await nextFrame();
      }
      const pixels = await this.presentation.readPresentedPixels(this.frameId);
      const content = analyzeRgba(pixels.rgba);
      if (content.uniqueColors < 2 || content.nonDominantPixels < 64) {
        throw new Error(`Map preview renderer returned a visually empty frame (${content.uniqueColors} color).`);
      }
      const canvas = rgbaCanvas(this.documentObj, pixels);
      return captureResult(request, canvas.toDataURL("image/png"), readiness, content);
    } finally {
      this.presentation.exitFixedCapture();
    }
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.presentation.destroy();
  }
}

export function normalizeCaptureRequest(input) {
  const kind = input?.kind === "minimap" ? "minimap" : input?.kind === "world" ? "world" : "";
  if (!kind) throw new RangeError("Map preview kind must be world or minimap.");
  const width = boundedDimension(input?.width, "width");
  const height = boundedDimension(input?.height, "height");
  if (width * height > MAP_PREVIEW_LIMITS.maxPixels) throw new RangeError("Map preview pixel count exceeds the capture limit.");
  const padding = kind === "world" ? boundedPadding(input?.padding ?? 24, width, height) : 0;
  return Object.freeze({ kind, width, height, padding });
}

export function framedCamera(map, { width, height, padding }) {
  const worldWidth = map.width * TILE_SIZE;
  const worldHeight = map.height * TILE_SIZE;
  const zoom = Math.min((width - padding * 2) / worldWidth, (height - padding * 2) / worldHeight);
  if (!Number.isFinite(zoom) || zoom <= 0) throw new RangeError("Map preview framing has no drawable area.");
  return Object.freeze({
    x: (worldWidth - width / zoom) / 2,
    y: (worldHeight - height / zoom) / 2,
    zoom,
  });
}

function emptyEditorOverlay() {
  return {
    revision: 1,
    stealthTiles: [],
    noVehicleTiles: [],
    gridPaths: [],
    guides: [],
    guideCentre: null,
    sites: [],
    doodadSelections: [],
    doodadSelectionBox: null,
    doodadBrushPreview: null,
    paintPreview: null,
  };
}

function rgbaCanvas(documentObj, pixels) {
  const width = Number(pixels?.width);
  const height = Number(pixels?.height);
  const rgba = pixels?.rgba;
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || rgba?.length !== width * height * 4) {
    throw new Error("Map preview renderer returned malformed pixels.");
  }
  const canvas = documentObj.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Map preview could not encode renderer pixels.");
  ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), width, height), 0, 0);
  return canvas;
}

export function analyzeRgba(rgba) {
  if (!(rgba instanceof Uint8Array) || rgba.length < 4 || rgba.length % 4 !== 0) {
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
  return Object.freeze({
    pixelCount,
    uniqueColors: colors.size,
    dominantColorPixels,
    nonDominantPixels: pixelCount - dominantColorPixels,
  });
}

function captureResult(request, pngDataUrl, readiness, content = null) {
  if (!String(pngDataUrl).startsWith("data:image/png;base64,")) throw new Error("Map preview PNG encoding failed.");
  return Object.freeze({
    version: 1,
    kind: request.kind,
    width: request.width,
    height: request.height,
    pngDataUrl,
    readiness: structuredClone(readiness),
    content,
  });
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
  if (!Number.isSafeInteger(padding) || padding < 0 || padding > maximum) {
    throw new RangeError(`Map preview padding must be a whole number from 0 to ${maximum}.`);
  }
  return padding;
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}
