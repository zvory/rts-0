import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { Camera } from "../../client/src/camera.js";
import { mapPreviewLaunchConfig } from "../../client/src/map_preview_launch.js";
import { MapEditorPanel } from "../../client/src/map_editor_panel.js";
import { Fog } from "../../client/src/fog.js";
import { captureMinimapPng } from "../../client/src/minimap_capture.js";
import {
  analyzeRgba,
  installMapPreviewStartupStatus,
  MAP_PREVIEW_CAMERA_MIN_ZOOM,
  MAP_PREVIEW_LIMITS,
  MapPreviewBridge,
  normalizeCaptureRequest,
} from "../../client/src/map_preview_bridge.js";
import {
  createPreviewHandoff,
  parseMapPreviewArgs,
  readJpegDimensions,
  readPngDimensions,
  renderMapPreview,
} from "../../scripts/map-preview.mjs";
import { LOBBY_MAP_PRESENTATION } from "../../client/src/lobby_map_selector.js";

const handoffId = "0123456789abcdef0123456789abcdef";

assert.equal(mapPreviewLaunchConfig({ pathname: "/", search: "" }), null);
assert.deepEqual(
  mapPreviewLaunchConfig({ pathname: "/map-preview", search: `?handoff=${handoffId}` }),
  { handoffId, error: "" },
);
assert.match(mapPreviewLaunchConfig({ pathname: "/map-preview", search: "?handoff=../map" }).error, /valid one-use handoff/);

{
  const startup = installMapPreviewStartupStatus();
  assert.equal(startup.status().state, "starting");
  startup.fail(new Error("handoff expired"));
  assert.deepEqual(
    { state: startup.status().state, error: startup.status().error },
    { state: "failed", error: "handoff expired" },
    "startup failures remain machine-readable for the preview CLI",
  );
  delete globalThis.__rtsMapPreview;
}

assert.deepEqual(normalizeCaptureRequest({ kind: "world", width: 1200, height: 800, padding: 40 }), {
  kind: "world", width: 1200, height: 800, padding: 40,
});
assert.deepEqual(normalizeCaptureRequest({ kind: "minimap", width: 1024, height: 1024 }), {
  kind: "minimap", width: 1024, height: 1024, padding: 0,
});
assert.throws(() => normalizeCaptureRequest({ kind: "minimap", width: 1024, height: 512 }), /square/);
assert.throws(
  () => normalizeCaptureRequest({
    kind: "world",
    width: MAP_PREVIEW_LIMITS.maxDimension,
    height: MAP_PREVIEW_LIMITS.maxDimension,
    padding: 1500,
  }),
  /padding/,
);
assert.throws(() => normalizeCaptureRequest({ kind: "world", width: 63, height: 64 }), /64/);
assert.deepEqual(analyzeRgba(Uint8Array.from([0, 0, 0, 255, 1, 2, 3, 255])), {
  pixelCount: 2,
  uniqueColors: 2,
  dominantColorPixels: 1,
  nonDominantPixels: 1,
});

{
  const outputPixels = 2048;
  const padding = 32;
  const worldPixels = 256 * 32;
  const corners = [
    { x: 0, y: 0 },
    { x: worldPixels, y: 0 },
    { x: worldPixels, y: worldPixels },
    { x: 0, y: worldPixels },
  ];
  const normalCamera = new Camera(outputPixels, outputPixels);
  normalCamera.setMapBounds(worldPixels, worldPixels);
  normalCamera.fitWorldPoints(corners, { paddingCssPx: padding });
  assert.equal(normalCamera.zoom, 0.4, "the normal live camera retains its gameplay zoom floor");
  assert.ok(outputPixels / normalCamera.zoom < worldPixels, "the gameplay floor cannot frame a 256×256 map");

  const previewCamera = new Camera(outputPixels, outputPixels, { minZoom: MAP_PREVIEW_CAMERA_MIN_ZOOM });
  previewCamera.setMapBounds(worldPixels, worldPixels);
  previewCamera.fitWorldPoints(corners, { paddingCssPx: padding });
  assert.equal(previewCamera.zoom, (outputPixels - padding * 2) / worldPixels);
  assert.ok(outputPixels / previewCamera.zoom >= worldPixels,
    "the preview-only camera floor fits every edge of a 256×256 map at the default export size");
  const appSource = fs.readFileSync(new URL("../../client/src/app.js", import.meta.url), "utf8");
  const matchSource = fs.readFileSync(new URL("../../client/src/match.js", import.meta.url), "utf8");
  assert.match(appSource, /cameraMinZoom: this\.mapPreviewLaunch \? MAP_PREVIEW_CAMERA_MIN_ZOOM : undefined/);
  assert.match(matchSource, /minZoom: options\.cameraMinZoom \?\? autoSpectatorCameraMinZoom/);
}

{
  const fog = new Fog(2, 2);
  fog.setRevealAll(true);
  fog.update([], 32, Uint8Array.of(0, 0, 0, 0), Uint8Array.of(0, 0, 0, 0));
  assert.deepEqual([...fog.visibleGrid], [1, 1, 1, 1],
    "reveal-all keeps worker-facing grids clear despite authoritative player fog");
  assert.deepEqual([...fog.exploredGrid], [1, 1, 1, 1]);
}

{
  const calls = [];
  const minimap = {
    canvas: { width: 220, height: 220, toDataURL: () => "data:image/png;base64,minimap" },
    ctx: { getImageData: () => ({ data: Uint8ClampedArray.from([1, 2, 3, 255]) }) },
    render(_frameViews, { capturePresentation = false } = {}) {
      calls.push({ width: this.canvas.width, capture: capturePresentation });
    },
  };
  const result = captureMinimapPng(minimap, { width: 1, height: 1 });
  assert.equal(result.pngDataUrl, "data:image/png;base64,minimap");
  assert.deepEqual(calls, [
    { width: 1, capture: true },
    { width: 220, capture: false },
  ], "production minimap capture suppresses transients and restores the live canvas");
  assert.equal(minimap.canvas.width, 220);
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
  const requests = [];
  const result = await createPreviewHandoff(
    "http://127.0.0.1:8080/",
    { name: "Authored" },
    { width: 16, height: 16 },
    async (_url, init) => {
      requests.push(init);
      return { ok: true, json: async () => ({ handoffId }) };
    },
  );
  assert.equal(result.handoffId, handoffId);
  assert.equal(JSON.parse(requests[0].body).destination, "lab", "preview uses server-materialized Lab authority");
  assert.ok(requests[0].signal instanceof AbortSignal, "handoff fetch is deadline-bound");
  await assert.rejects(
    createPreviewHandoff("http://127.0.0.1:8080/", {}, {}, (_url, init) => new Promise((_, reject) => {
      init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
    }), 2),
    /timed out/,
  );
}

const savedImageData = globalThis.ImageData;
const savedRaf = globalThis.requestAnimationFrame;
const savedDpr = globalThis.devicePixelRatio;
globalThis.ImageData = class ImageData {
  constructor(data, width, height) { this.data = data; this.width = width; this.height = height; }
};
globalThis.requestAnimationFrame = (callback) => { queueMicrotask(callback); return 1; };
globalThis.devicePixelRatio = 2;
try {
  const fixture = createBridgeFixture();
  const bridge = new MapPreviewBridge({ ...fixture, captureTimeoutMs: 1000 });
  let releaseInitialization;
  globalThis.requestAnimationFrame = (callback) => {
    releaseInitialization = () => queueMicrotask(callback);
    return 1;
  };
  const initialization = bridge.initialize();
  assert.equal(bridge.status().state, "starting", "capture is not advertised before initial fitting settles");
  await assert.rejects(
    bridge.call("capture", { kind: "world", width: 64, height: 64, padding: 0 }),
    /still starting/,
  );
  releaseInitialization();
  await new Promise((resolve) => setTimeout(resolve, 0));
  releaseInitialization();
  await initialization;
  globalThis.requestAnimationFrame = (callback) => { queueMicrotask(callback); return 1; };
  assert.equal(bridge.status().state, "ready");
  assert.ok(fixture.app.cleanCalls.every(Boolean), "initial preview hides app chrome");
  assert.equal(fixture.camera.fitCalls.length, 1, "initial preview fits the complete map before any export");

  const world = await bridge.call("capture", { kind: "world", width: 64, height: 64, padding: 0 });
  assert.equal(world.width, 64);
  assert.equal(world.height, 64);
  assert.deepEqual(fixture.renderer.resizeCalls[0], [64, 64, 1],
    "world output forces DPR 1 even in a DPR 2 browser");
  assert.equal(fixture.camera.fitCalls.length, 3,
    "capture fits its exact surface, then restores the fitted interactive preview");

  const minimap = await bridge.call("capture", { kind: "minimap", width: 64, height: 64 });
  assert.equal(minimap.authoritative, true);
  assert.deepEqual(fixture.minimap.captureCalls, [{ width: 64, height: 64 }],
    "bridge delegates high-resolution output to the live Minimap");
  bridge.destroy();
  assert.equal(fixture.documentObj.body.classList.has("map-preview-mode"), false);
} finally {
  globalThis.ImageData = savedImageData;
  globalThis.requestAnimationFrame = savedRaf;
  globalThis.devicePixelRatio = savedDpr;
}

{
  const savedImageData2 = globalThis.ImageData;
  const savedRaf2 = globalThis.requestAnimationFrame;
  globalThis.ImageData = class ImageData {
    constructor(data, width, height) { this.data = data; this.width = width; this.height = height; }
  };
  globalThis.requestAnimationFrame = (callback) => { queueMicrotask(callback); return 1; };
  try {
    let resolveLateFrame;
    const lateFrame = new Promise((resolve) => { resolveLateFrame = resolve; });
    const fixture = createBridgeFixture({ renderPromise: lateFrame });
    const bridge = new MapPreviewBridge({ ...fixture, captureTimeoutMs: 2 });
    await bridge.initialize();
    await Promise.race([
      assert.rejects(
        bridge.call("capture", { kind: "world", width: 64, height: 64, padding: 0 }),
        /timed out/,
      ),
      new Promise((_, reject) => setTimeout(() => reject(new Error("capture deadline did not settle")), 100)),
    ]);
    assert.equal(fixture.renderer.readCalls, 0,
      "deadline rejection does not continue into framebuffer readback");
    assert.equal(bridge.captureActive, false, "deadline releases the capture lock");
    assert.equal(fixture.match.exitCount, 1, "hung fixed capture exits before rejection");
    assert.equal(fixture.match.resizeCount, 3,
      "hung capture restores renderer, then fitted preview, after the initial preview resize");
    const stable = {
      exits: fixture.match.exitCount,
      resizes: fixture.match.resizeCount,
      fits: fixture.camera.fitCalls.length,
      reads: fixture.renderer.readCalls,
    };
    resolveLateFrame({ rendererFrame: 9 });
    await new Promise((resolve) => setTimeout(resolve, 0));
    assert.deepEqual(
      {
        exits: fixture.match.exitCount,
        resizes: fixture.match.resizeCount,
        fits: fixture.camera.fitCalls.length,
        reads: fixture.renderer.readCalls,
      },
      stable,
      "late fixed-frame completion cannot mutate restored preview state",
    );
  } finally {
    globalThis.ImageData = savedImageData2;
    globalThis.requestAnimationFrame = savedRaf2;
  }
}

{
  const options = parseMapPreviewArgs([
    "--map", "server/assets/maps/1v1-no-terrain.json",
    "--out", "target/map-preview.png",
    "--kind", "minimap",
    "--width", "1024",
    "--height", "1024",
    "--browser-dpr", "2",
    "--url", "http://localhost:8099/path?ignored=1",
  ]);
  assert.equal(options.browserDpr, 2);
  assert.equal(options.url, "http://localhost:8099/");
  assert.throws(
    () => parseMapPreviewArgs(["--map", "map.json", "--out", "map.png", "--url", "https://example.com"]),
    /loopback/,
  );
  const pngHeader = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(pngHeader);
  pngHeader.write("IHDR", 12, "ascii");
  pngHeader.writeUInt32BE(640, 16);
  pngHeader.writeUInt32BE(360, 20);
  assert.deepEqual(readPngDimensions(pngHeader), { width: 640, height: 360 });
  const jpegHeader = Buffer.alloc(21);
  Buffer.from([0xff, 0xd8, 0xff, 0xc0]).copy(jpegHeader);
  jpegHeader.writeUInt16BE(17, 4);
  jpegHeader[6] = 8;
  jpegHeader.writeUInt16BE(512, 7);
  jpegHeader.writeUInt16BE(512, 9);
  assert.deepEqual(readJpegDimensions(jpegHeader), { width: 512, height: 512 });
  const jpegOptions = parseMapPreviewArgs([
    "--map", "server/assets/maps/schone-tage.json",
    "--out", "target/map-preview.jpg",
    "--kind", "minimap",
    "--width", "512",
    "--height", "512",
    "--browser-dpr", "2",
    "--jpeg-quality", "91",
  ]);
  assert.equal(jpegOptions.format, "jpeg");
  assert.equal(jpegOptions.jpegQuality, 91);
  assert.throws(
    () => parseMapPreviewArgs(["--map", "map.json", "--out", "map.webp"]),
    /\.png, \.jpg, or \.jpeg/,
  );
  assert.throws(
    () => parseMapPreviewArgs(["--map", "map.json", "--out", "map.jpg", "--jpeg-quality", "101"]),
    /integer from 1 to 100/,
  );
  const cliSource = fs.readFileSync(new URL("../../scripts/map-preview.mjs", import.meta.url), "utf8");
  const mainSource = fs.readFileSync(new URL("../../client/src/main.js", import.meta.url), "utf8");
  const appSource = fs.readFileSync(new URL("../../client/src/app.js", import.meta.url), "utf8");
  assert.doesNotMatch(cliSource, /--no-sandbox/, "CLI keeps Chrome's sandbox enabled");
  assert.match(mainSource, /const stressTestLaunch = mapPreviewLaunch \? null : stressTestLaunchConfig\(\)/,
    "map preview takes precedence over the stress-test transport");
  assert.match(mainSource, /const snapshotStreamLaunch = mapPreviewLaunch\s+\? null/,
    "map preview takes precedence over offline snapshot streams");
  assert.match(appSource, /this\.replayLaunch = mapPreviewLaunch \? null : replayLaunchConfig\(\)/);
  assert.match(appSource, /this\.matchLaunch = mapPreviewLaunch \? null : matchLaunchConfig\(\)/,
    "map preview's one-use handoff takes precedence over unrelated replay and match launches");
}

{
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "rts-map-preview-contract-"));
  const out = path.join(temporary, "never-written.png");
  let browserClosed = false;
  const page = {
    on() {},
    async goto() {},
    async waitForFunction() {},
    async evaluate(fn) {
      if (String(fn).includes(".status()")) return { state: "ready" };
      return new Promise(() => {});
    },
  };
  const browser = {
    async newPage() { return page; },
    async close() { browserClosed = true; },
  };
  const options = parseMapPreviewArgs([
    "--map", "server/assets/maps/1v1-no-terrain.json",
    "--out", out,
    "--chrome", "/bin/sh",
  ]);
  await assert.rejects(renderMapPreview(options, {
    fetchImpl: async () => ({ ok: true, json: async () => ({ handoffId }) }),
    loadPuppeteer: async () => ({ launch: async (launch) => {
      assert.equal(launch.args, undefined, "CLI passes no Chrome sandbox-disabling argument");
      return browser;
    } }),
    captureTimeoutMs: 2,
  }), /browser capture timed out/);
  assert.equal(browserClosed, true, "browser closes after a hung page evaluation");
}

{
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "rts-map-preview-jpeg-contract-"));
  const out = path.join(temporary, "preview.jpg");
  const pngHeader = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(pngHeader);
  pngHeader.write("IHDR", 12, "ascii");
  pngHeader.writeUInt32BE(512, 16);
  pngHeader.writeUInt32BE(512, 20);
  const jpegHeader = Buffer.alloc(21);
  Buffer.from([0xff, 0xd8, 0xff, 0xc0]).copy(jpegHeader);
  jpegHeader.writeUInt16BE(17, 4);
  jpegHeader[6] = 8;
  jpegHeader.writeUInt16BE(512, 7);
  jpegHeader.writeUInt16BE(512, 9);
  const evaluations = [];
  const page = {
    on() {},
    async goto() {},
    async waitForFunction() {},
    async evaluate(fn) {
      const source = String(fn);
      evaluations.push(source);
      if (source.includes(".status()")) return { state: "ready" };
      if (source.includes('__rtsMapPreview.call("capture"')) {
        return { pngDataUrl: `data:image/png;base64,${pngHeader.toString("base64")}` };
      }
      return `data:image/jpeg;base64,${jpegHeader.toString("base64")}`;
    },
  };
  const result = await renderMapPreview(parseMapPreviewArgs([
    "--map", "server/assets/maps/schone-tage.json",
    "--out", out,
    "--kind", "minimap",
    "--width", "512",
    "--height", "512",
    "--chrome", "/bin/sh",
  ]), {
    fetchImpl: async () => ({ ok: true, json: async () => ({ handoffId }) }),
    loadPuppeteer: async () => ({ launch: async () => ({
      async newPage() { return page; },
      async close() {},
    }) }),
  });
  assert.equal(result.format, "jpeg");
  assert.deepEqual(readJpegDimensions(fs.readFileSync(out)), { width: 512, height: 512 });
  assert.equal(evaluations.length, 3, "JPEG export transcodes the authoritative PNG in the preview browser");
}

{
  const authoredMaps = fs.readdirSync(new URL("../../server/assets/maps/", import.meta.url))
    .filter((file) => file.endsWith(".json"))
    .map((file) => JSON.parse(fs.readFileSync(new URL(`../../server/assets/maps/${file}`, import.meta.url), "utf8")))
    .filter((map) => map.version === 8);
  assert.deepEqual(
    authoredMaps.map((map) => map.name).sort(),
    Object.keys(LOBBY_MAP_PRESENTATION).sort(),
    "every selectable authored map has one lobby presentation entry",
  );
  for (const [name, presentation] of Object.entries(LOBBY_MAP_PRESENTATION)) {
    const asset = new URL(`../../client${presentation.preview}`, import.meta.url);
    assert.equal(fs.existsSync(asset), true, `${name} has a checked-in lobby preview asset`);
    const bytes = fs.readFileSync(asset);
    assert.deepEqual(readJpegDimensions(bytes), { width: 512, height: 512 },
      `${name} lobby preview is exactly 512x512`);
    assert.ok(bytes.length > 10_000, `${name} lobby preview contains nontrivial minimap detail`);
  }
}

function createBridgeFixture({ renderPromise = null } = {}) {
  const rgba = new Uint8Array(64 * 64 * 4);
  for (let index = 0; index < rgba.length; index += 4) {
    rgba[index] = (index / 4) % 2;
    rgba[index + 3] = 255;
  }
  const classList = new Set();
  classList.add = Set.prototype.add;
  classList.remove = Set.prototype.delete;
  const root = fakeNode("root");
  const documentObj = {
    title: "",
    body: Object.assign(fakeNode("body"), { classList }),
    getElementById: () => root,
    createElement(kind) {
      const node = fakeNode(kind);
      if (kind === "canvas") {
        node.getContext = () => ({ putImageData() {} });
        node.toDataURL = () => "data:image/png;base64,fixture";
      }
      return node;
    },
  };
  const renderer = {
    resizeCalls: [],
    readCalls: 0,
    resize(...args) { this.resizeCalls.push(args); },
    captureReadiness() {
      return { ready: true, failedAssets: [], pendingAssets: [], renderErrors: [], missingTextureSubjectIds: [] };
    },
    async readPresentedPixels(frameId) {
      this.readCalls += 1;
      assert.equal(frameId, 9);
      return { width: 64, height: 64, rgba };
    },
  };
  const camera = {
    fitCalls: [],
    snapshot: () => ({ version: 1, focus: { x: 1, y: 1 }, zoom: 1 }),
    resize() {},
    restore() {},
    fitWorldPoints(points, options) { this.fitCalls.push({ points, options }); },
  };
  const minimap = {
    captureCalls: [],
    canvas: { width: 220, height: 220, toDataURL: () => "data:image/png;base64,minimap" },
    ctx: { getImageData: () => ({ data: rgba }) },
    render(_frameViews, { capturePresentation = false } = {}) {
      if (capturePresentation) this.captureCalls.push({ width: this.canvas.width, height: this.canvas.height });
    },
  };
  const match = {
    state: {
      map: { name: "Authority", width: 16, height: 16, tileSize: 32, resources: [{ id: 7 }] },
      entitiesInterpolated: () => [{ id: 1, kind: "resourceDepot" }, { id: 2, kind: "steelNode" }],
    },
    renderer,
    camera,
    minimap,
    fog: { revealAll: false, setRevealAll(value) { this.revealAll = value; } },
    resizeCount: 0,
    exitCount: 0,
    handleResize() { this.resizeCount += 1; },
    enterFixedCapture() { return { visualStartMs: 100 }; },
    async renderFixedCaptureFrame() {
      if (renderPromise) return renderPromise;
      return { rendererFrame: 9 };
    },
    exitFixedCapture() { this.exitCount += 1; },
  };
  const app = { cleanCalls: [], setCleanPresentation(value) { this.cleanCalls.push(value); } };
  return { app, match, renderer, camera, minimap, documentObj, root };
}

function fakeNode(kind) {
  return {
    kind,
    children: [],
    dataset: {},
    disabled: false,
    append(...children) { this.children.push(...children); },
    appendChild(child) { this.children.push(child); },
    remove() { this.removed = true; },
    addEventListener() {},
    setAttribute() {},
    querySelector() { return null; },
    querySelectorAll() { return []; },
    click() {},
  };
}
