import assert from "node:assert/strict";
import fs from "node:fs";

import { mapPreviewLaunchConfig } from "../../client/src/map_preview_launch.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import {
  analyzeRgba,
  framedCamera,
  MAP_PREVIEW_LIMITS,
  MapPreviewBridge,
  normalizeCaptureRequest,
} from "../../client/src/map_preview_bridge.js";
import {
  minimapMapTransform,
  paintMinimapMap,
  paintMinimapTerrain,
} from "../../client/src/minimap_map_painter.js";
import {
  parseMapPreviewArgs,
  readPngDimensions,
} from "../../scripts/map-preview.mjs";

const handoffId = "0123456789abcdef0123456789abcdef";

assert.equal(mapPreviewLaunchConfig({ pathname: "/", search: "" }), null);
assert.deepEqual(
  mapPreviewLaunchConfig({ pathname: "/map-preview", search: `?handoff=${handoffId}` }),
  { handoffId, error: "" },
  "map preview is launch-gated by its dedicated route and one-use handoff token",
);
assert.match(
  mapPreviewLaunchConfig({ pathname: "/map-preview", search: "?handoff=../map" }).error,
  /valid one-use handoff/,
);

assert.deepEqual(normalizeCaptureRequest({ kind: "world", width: 1200, height: 800, padding: 40 }), {
  kind: "world", width: 1200, height: 800, padding: 40,
});
assert.deepEqual(normalizeCaptureRequest({ kind: "minimap", width: 1024, height: 1024, padding: 50 }), {
  kind: "minimap", width: 1024, height: 1024, padding: 0,
});
assert.throws(
  () => normalizeCaptureRequest({ kind: "world", width: MAP_PREVIEW_LIMITS.maxDimension, height: MAP_PREVIEW_LIMITS.maxDimension, padding: 1500 }),
  /padding/,
);
assert.throws(() => normalizeCaptureRequest({ kind: "world", width: 63, height: 64 }), /64/);
assert.deepEqual(analyzeRgba(Uint8Array.from([
  0, 0, 0, 255,
  1, 2, 3, 255,
])), {
  pixelCount: 2,
  uniqueColors: 2,
  dominantColorPixels: 1,
  nonDominantPixels: 1,
});

assert.deepEqual(
  framedCamera({ width: 10, height: 5 }, { width: 640, height: 640, padding: 0 }),
  { x: 0, y: -80, zoom: 2 },
  "world capture letterboxes a rectangular map around its exact center",
);

{
  const calls = [];
  const ctx = {
    canvas: { width: 100, height: 100 },
    fillStyle: "",
    fillRect(...args) { calls.push({ color: this.fillStyle, args }); },
  };
  const map = { width: 2, height: 1, tileSize: 1, terrain: [0, 2] };
  assert.deepEqual(minimapMapTransform(map, 100, 100), { scale: 50, offX: 0, offY: 25 });
  paintMinimapMap(ctx, map);
  assert.equal(calls.length, 3, "shared minimap painter emits one background plus one cell per terrain tile");
  assert.deepEqual(calls[1].args, [0, 25, 51, 51]);
  assert.deepEqual(calls[2].args, [50, 25, 51, 51]);
  assert.notEqual(calls[1].color, calls[2].color, "shared minimap painter applies production terrain colors");
  assert.throws(
    () => paintMinimapTerrain(ctx, { ...map, terrain: [0] }, { scale: 1, offX: 0, offY: 0 }),
    /dimensions/,
  );
}

{
  const source = fs.readFileSync(new URL("../../client/src/minimap.js", import.meta.url), "utf8");
  assert.match(source, /paintMinimapTerrain\(ctx, map, \{/, "production Minimap delegates terrain pixels to the shared painter");
  const editorSource = fs.readFileSync(new URL("../../client/src/map_editor_panel.js", import.meta.url), "utf8");
  const previewSource = fs.readFileSync(new URL("../../client/src/map_preview_app.js", import.meta.url), "utf8");
  assert.match(editorSource, /Preview PNGs/, "Map Editor exposes its shared preview handoff flow");
  assert.match(previewSource, /Download world PNG \(2048 px\)/, "capture page exposes a world PNG download");
  assert.match(previewSource, /Download minimap PNG \(2048 px\)/, "capture page exposes a high-resolution minimap download");
}

{
  const handedOff = [];
  const statuses = [];
  const panel = {
    pending: false,
    session: {
      exportMap: () => ({ name: "UI preview" }),
      materialized: () => ({ width: 16, height: 16 }),
    },
    async onOpenPreview(payload) { handedOff.push(payload); },
    setStatus(message, error = false) { statuses.push({ message, error }); },
  };
  await MapEditorPanel.prototype.openPreview.call(panel);
  assert.deepEqual(handedOff, [{ authoredMap: { name: "UI preview" }, materializedMap: { width: 16, height: 16 } }]);
  assert.equal(panel.pending, false);
  assert.equal(statuses.at(-1).message, "Opened the PNG preview page.");
}

{
  const originalImageData = globalThis.ImageData;
  const originalRaf = globalThis.requestAnimationFrame;
  globalThis.ImageData = class ImageData {
    constructor(data, width, height) { this.data = data; this.width = width; this.height = height; }
  };
  globalThis.requestAnimationFrame = (callback) => { callback(); return 1; };
  try {
    const contexts = [];
    const documentObj = {
      createElement(kind) {
        assert.equal(kind, "canvas");
        const ctx = {
          canvas: null,
          fillStyle: "",
          fillRect() {},
          putImageData(image) { this.image = image; },
        };
        const canvas = {
          width: 0,
          height: 0,
          getContext(type) { assert.equal(type, "2d"); ctx.canvas = canvas; contexts.push(ctx); return ctx; },
          toDataURL() { return "data:image/png;base64,fixture"; },
        };
        return canvas;
      },
    };
    const presented = [];
    const presentation = {
      enterFixedCapture() { this.captureLifecycle = ["enter"]; },
      exitFixedCapture() { this.captureLifecycle.push("exit"); },
      resize(width, height) { this.size = { width, height }; },
      async present(record) { presented.push(record); return { status: "presented", frameId: record.frameId }; },
      captureReadiness() { return { ready: true, assets: [], failedAssets: [], pendingAssets: [] }; },
      async readPresentedPixels(frameId) {
        assert.equal(frameId, 1);
        const rgba = new Uint8Array(64 * 64 * 4);
        for (let index = 0; index < rgba.length; index += 4) {
          rgba[index] = (index / 4) % 2;
          rgba[index + 3] = 255;
        }
        return { width: 64, height: 64, rgba };
      },
      destroy() { this.destroyed = true; },
    };
    const materialized = {
      name: "Preview",
      width: 16,
      height: 16,
      terrain: Array(16 * 16).fill(0),
      doodads: [],
      starts: [{ x: 4, y: 4 }, { x: 11, y: 11 }],
      baseSites: [],
      stealthTiles: [],
      noVehicleTiles: [],
    };
    const bridge = new MapPreviewBridge({
      session: { draft: materialized, materialized: () => structuredClone(materialized) },
      presentation,
      documentObj,
      now: () => 0,
    });
    const world = await bridge.call("capture", { kind: "world", width: 64, height: 64, padding: 0 });
    assert.equal(world.pngDataUrl, "data:image/png;base64,fixture");
    assert.deepEqual(presentation.captureLifecycle, ["enter", "exit"]);
    assert.equal(presented[0].terrainUpdate.kind, "replace");
    assert.deepEqual(presented[0].overlay.sites, [], "world capture explicitly omits editor markers and guides");
    const minimap = await bridge.call("capture", { kind: "minimap", width: 64, height: 64 });
    assert.equal(minimap.kind, "minimap");
    bridge.destroy();
    assert.equal(presentation.destroyed, true, "map preview bridge tears down its Pixi worker adapter");
    assert.ok(contexts.some((ctx) => ctx.image?.width === 64), "world RGBA is encoded without taking a DOM screenshot");
  } finally {
    globalThis.ImageData = originalImageData;
    globalThis.requestAnimationFrame = originalRaf;
  }
}

{
  const options = parseMapPreviewArgs([
    "--map", "server/assets/maps/1v1-no-terrain.json",
    "--out", "target/map-preview.png",
    "--kind", "minimap",
    "--width", "1024",
    "--height", "768",
    "--url", "http://localhost:8099/path?ignored=1",
  ]);
  assert.equal(options.kind, "minimap");
  assert.equal(options.width, 1024);
  assert.equal(options.height, 768);
  assert.equal(options.url, "http://localhost:8099/");
  assert.throws(
    () => parseMapPreviewArgs(["--map", "map.json", "--out", "map.png", "--url", "https://example.com"]),
    /loopback/,
    "CLI refuses to send authored maps to a non-local handoff endpoint",
  );
  const pngHeader = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(pngHeader);
  pngHeader.write("IHDR", 12, "ascii");
  pngHeader.writeUInt32BE(640, 16);
  pngHeader.writeUInt32BE(360, 20);
  assert.deepEqual(readPngDimensions(pngHeader), { width: 640, height: 360 });
}
