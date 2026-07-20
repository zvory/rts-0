export const PIXI_WORKER_URL = "https://cdn.jsdelivr.net/npm/pixi.js@8.19.0/dist/pixi.min.mjs";

export function installPixiWorkerEnvironment(canvas, { widthCssPx, heightCssPx } = {}) {
  if (typeof OffscreenCanvas !== "function") throw new Error("Pixi rendering requires OffscreenCanvas.");
  decorateCanvas(canvas, widthCssPx, heightCssPx);
  globalThis.window = globalThis;
  globalThis.document = {
    baseURI: globalThis.location?.href || "",
    fonts: globalThis.fonts || null,
    createElement(tag) {
      if (String(tag).toLowerCase() !== "canvas") {
        throw new Error(`Pixi worker cannot create DOM element ${String(tag)}.`);
      }
      return decorateCanvas(new OffscreenCanvas(1, 1), 1, 1);
    },
  };
}

export function configurePixiForWorker(pixi) {
  if (!pixi?.Application || !pixi?.DOMAdapter) throw new Error("Pinned Pixi worker module is incomplete.");
  globalThis.PIXI = pixi;
  pixi.DOMAdapter.set(createWorkerAdapter());
  for (const extension of [pixi.DOMPipe, pixi.AccessibilitySystem, pixi.EventSystem]) {
    if (extension) pixi.extensions?.remove?.(extension);
  }
}

function createWorkerAdapter() {
  return {
    createCanvas: (width = 1, height = 1) => decorateCanvas(
      new OffscreenCanvas(Math.max(1, width || 1), Math.max(1, height || 1)),
      width,
      height,
    ),
    createImage: () => { throw new Error("DOM Image is unavailable in the Pixi render worker."); },
    getCanvasRenderingContext2D: () => globalThis.OffscreenCanvasRenderingContext2D,
    getWebGLRenderingContext: () => globalThis.WebGLRenderingContext,
    getNavigator: () => globalThis.navigator,
    getBaseUrl: () => globalThis.location?.href || "",
    getFontFaceSet: () => globalThis.fonts || { add() {}, delete() {} },
    fetch: (url, options) => fetch(url, options),
    parseXML: () => { throw new Error("DOMParser is unavailable in the Pixi render worker."); },
  };
}

function decorateCanvas(canvas, widthCssPx = canvas?.width || 1, heightCssPx = canvas?.height || 1) {
  if (!canvas) throw new Error("Pixi worker initialization requires a transferred canvas.");
  const style = canvas.style || {};
  try { canvas.style = style; } catch {}
  canvas.addEventListener ||= (() => {});
  canvas.removeEventListener ||= (() => {});
  canvas.remove ||= (() => {});
  canvas.getBoundingClientRect ||= (() => ({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: widthCssPx,
    height: heightCssPx,
    right: widthCssPx,
    bottom: heightCssPx,
  }));
  return canvas;
}
