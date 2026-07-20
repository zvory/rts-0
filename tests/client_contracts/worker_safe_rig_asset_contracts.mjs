import { assert } from "./assertions.mjs";
import { loadFrameStripTexture } from "../../client/src/renderer/rigs/frame_strip_routing.js";
import { loadPngRigAtlasTexture } from "../../client/src/renderer/rigs/png_routing.js";

await assertWorkerSafeFallback({
  label: "adjusted strip",
  dimensions: [48, 8],
  load: (pixi) => loadFrameStripTexture(pixi, {
    image: "/assets/rigs/test-strip.png?v=contract",
    frameWidth: 12,
    frameHeight: 8,
    frameCount: 4,
    bakedColorAdjustment: { brightness: 100, saturation: 100, hue: 100 },
  }),
});

await assertWorkerSafeFallback({
  label: "adjusted atlas",
  dimensions: [32, 24],
  load: (pixi) => loadPngRigAtlasTexture(pixi, {
    image: "/assets/rigs/test-atlas.png?v=contract",
    grid: { width: 32, height: 24 },
    runtimeColorAdjustment: { brightness: 105, saturation: 100, hue: 100 },
  }),
});

async function assertWorkerSafeFallback({ label, dimensions, load }) {
  const saved = saveGlobals("OffscreenCanvas", "fetch", "createImageBitmap");
  const canvases = [];
  const textureSources = [];
  const assetLoads = [];
  globalThis.OffscreenCanvas = fakeOffscreenCanvas(canvases);
  globalThis.fetch = async () => ({ ok: true, blob: async () => ({}) });
  globalThis.createImageBitmap = async () => ({ width: 0, height: 0, close() {} });
  try {
    const texture = await load({
      Assets: { load: async (src) => assetLoads.push(src) },
      Texture: {
        from(source) {
          textureSources.push(source);
          if (textureSources.length === 1) throw new Error("canvas texture unavailable");
          return { source: {} };
        },
      },
    });
    assert(texture?.source?.scaleMode === "nearest", `${label} retries with a worker-safe bitmap`);
    assert(textureSources.length === 2, `${label} retries texture creation exactly once`);
    assert(assetLoads.length === 0, `${label} never falls back to main-thread Pixi Assets`);
    assert(canvases[0]?.width === dimensions[0], `${label} preserves fallback width`);
    assert(canvases[0]?.height === dimensions[1], `${label} preserves fallback height`);
  } finally {
    restoreGlobals(saved);
  }
}

function fakeOffscreenCanvas(canvases) {
  return class FakeOffscreenCanvas {
    constructor(width, height) {
      this.width = width;
      this.height = height;
      canvases.push(this);
    }

    getContext(type) {
      assert(type === "2d", "worker-safe color loader requests a 2D canvas context");
      return {
        clearRect() {},
        drawImage() {},
        getImageData: () => ({ data: new Uint8ClampedArray(this.width * this.height * 4) }),
        putImageData() {},
      };
    }
  };
}

function saveGlobals(...names) {
  return new Map(names.map((name) => [name, globalThis[name]]));
}

function restoreGlobals(saved) {
  for (const [name, value] of saved) {
    if (value === undefined) delete globalThis[name];
    else globalThis[name] = value;
  }
}
