import { TANK_PNG_RIG_ATLAS } from "../../client/src/renderer/rigs/tank_png_atlas.js";
import { SCOUT_CAR_PNG_RIG_ATLAS } from "../../client/src/renderer/rigs/scout_car_png_atlas.js";
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

// Composite atlases reuse existing pixels and release temporary textures after packing.
{
  const saved = saveGlobals("OffscreenCanvas", "fetch", "createImageBitmap");
  const draws = [];
  const destroyed = [];
  globalThis.OffscreenCanvas = class {
    constructor(width, height) { this.width = width; this.height = height; }
    getContext() { return { drawImage: (...args) => draws.push(args) }; }
  };
  globalThis.fetch = async (url) => ({ ok: true, blob: async () => ({ url }) });
  globalThis.createImageBitmap = async (blob) => ({ ...blob, width: 32, height: 24, close() {} });
  const pixi = { Texture: { from: (resource) => ({
    source: { resource }, destroy: () => destroyed.push(resource),
  }) } };
  try {
    const result = await loadPngRigAtlasTexture(pixi, {
      image: "/tank.png", grid: { width: 32, height: 30 },
      attachments: [{ atlas: { image: "/scout.png" },
        source: { x: 2, y: 3, w: 10, h: 6 }, x: 0, y: 24 }],
    });
    assert(draws[0][0].url === "/tank.png", "composed atlas preserves original tank pixels");
    assert(draws[1][0].url === "/scout.png", "composed atlas reuses the original Scout Car image");
    assert(draws[1].slice(1).join(",") === "2,3,10,6,0,24,10,6", "gun packing preserves source pixels and crop");
    assert(destroyed.length === 2, "composition releases both temporary textures");
    assert(result.rtsRendererOwnedTexture && result.source.scaleMode === "nearest", "composed texture follows renderer ownership and sampling");
  } finally { restoreGlobals(saved); }
}

{
  const scout = SCOUT_CAR_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.rearMachineGun");
  const tank = TANK_PNG_RIG_ATLAS.sprites.find((sprite) => sprite.id === "sprite.commanderGun");
  assert(TANK_PNG_RIG_ATLAS.attachments[0].source === scout.frame, "tank reuses the exact Scout Car gun crop");
  assert(Math.abs(scout.frame.pixelsPerUnitX / tank.frame.pixelsPerUnitX - 0.7) < 1e-12, "roof gun width is reduced by 30%");
  assert(Math.abs(scout.frame.pixelsPerUnitY / tank.frame.pixelsPerUnitY - 0.7) < 1e-12, "roof gun height is reduced by 30%");
}
