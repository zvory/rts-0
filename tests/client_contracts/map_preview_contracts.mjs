import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { mapPreviewLaunchConfig } from "../../client/src/map_preview_launch.js";
import { MapEditorMinimapPreview, pngDataUrlToBlob } from "../../client/src/map_editor_minimap_preview.js";
import { createMapEditorPreviewButton } from "../../client/src/map_editor_preview_button.js";
import { Fog } from "../../client/src/fog.js";
import { Minimap } from "../../client/src/minimap.js";
import { captureMinimapPng } from "../../client/src/minimap_capture.js";
import {
  analyzeRgba,
  installMapPreviewStartupStatus,
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

assert.deepEqual(normalizeCaptureRequest({ width: 1024, height: 1024 }), {
  kind: "minimap", width: 1024, height: 1024,
});
assert.throws(() => normalizeCaptureRequest({ width: 1024, height: 512 }), /square/);
assert.throws(() => normalizeCaptureRequest({ width: 63, height: 63 }), /64/);
assert.deepEqual(analyzeRgba(Uint8Array.from([0, 0, 0, 255, 1, 2, 3, 255])), {
  pixelCount: 2,
  uniqueColors: 2,
  dominantColorPixels: 1,
  nonDominantPixels: 1,
});

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
  const source = fs.readFileSync(new URL("../../client/src/map_editor_preview_button.js", import.meta.url), "utf8");
  assert.match(source, /pointerenter/);
  assert.match(source, /onCopy\?\.\(payload\(\)\)/);
  assert.match(source, /Copied the 2048 px minimap PNG to the clipboard/);
  assert.doesNotMatch(source, /window\.open|location\.assign/, "preview control never opens or navigates a page");
  const savedDocument = globalThis.document;
  const shown = [];
  const copied = [];
  const statuses = [];
  globalThis.document = { createElement: (kind) => fakeNode(kind) };
  try {
    const control = createMapEditorPreviewButton({
      session: {
        exportMap: () => ({ name: "UI preview" }),
        materialized: () => ({ width: 16, height: 16 }),
      },
      onShow: (anchor, payload) => shown.push({ anchor, payload }),
      onCopy: async (payload) => copied.push(payload),
      onStatus: (message, error = false) => statuses.push({ message, error }),
    });
    control.listeners.pointerenter();
    await control.listeners.click();
    const payload = { authoredMap: { name: "UI preview" }, materializedMap: { width: 16, height: 16 } };
    assert.deepEqual(shown, [{ anchor: control, payload }], "hover requests the authoritative minimap in place");
    assert.deepEqual(copied, [payload], "click copies the same current map preview");
    assert.equal(statuses.at(-1).message, "Copied the 2048 px minimap PNG to the clipboard.");
  } finally {
    if (savedDocument === undefined) delete globalThis.document;
    else globalThis.document = savedDocument;
  }
}

{
  const png = pngDataUrlToBlob("data:image/png;base64,AQIDBA==");
  assert.equal(png.type, "image/png");
  assert.equal(png.size, 4);
  assert.throws(() => pngDataUrlToBlob("data:text/plain;base64,AQID"), /invalid PNG/);

  const writes = [];
  class ClipboardItemFixture {
    constructor(contents) { this.contents = contents; }
  }
  const preview = Object.create(MapEditorMinimapPreview.prototype);
  Object.assign(preview, {
    destroyed: false,
    clipboard: { async write(items) { writes.push(items); } },
    ClipboardItemCtor: ClipboardItemFixture,
    _capture: async () => "data:image/png;base64,AQIDBA==",
  });
  await preview.copy({ authoredMap: { name: "Clipboard" } });
  assert.equal(writes.length, 1);
  const copiedPng = await writes[0][0].contents["image/png"];
  assert.equal(copiedPng.type, "image/png", "clipboard receives a promised PNG Blob");
}

{
  const pending = [];
  const removedFrames = [];
  const preview = Object.create(MapEditorMinimapPreview.prototype);
  Object.assign(preview, {
    destroyed: false,
    mapKey: "",
    frame: null,
    bridgePromise: null,
    generation: 0,
    _createBridge() {
      return new Promise((resolve, reject) => pending.push({ resolve, reject }));
    },
  });
  const payload = { authoredMap: { name: "Undo race" } };
  const stale = preview._bridge(payload);
  preview._discardFrame();
  const replacement = preview._bridge(payload);
  preview.frame = { remove: () => removedFrames.push("replacement") };
  pending[0].reject(new Error("stale preview cancelled"));
  await assert.rejects(stale, /stale preview cancelled/);
  assert.equal(preview.bridgePromise, replacement,
    "a stale same-map rejection does not clear its replacement bridge");
  assert.deepEqual(removedFrames, [], "a stale rejection does not remove the replacement frame");
  pending[1].resolve({ call() {} });
  await replacement;

  preview._discardFrame();
  removedFrames.length = 0;
  const failed = preview._bridge(payload);
  preview.frame = { remove: () => removedFrames.push("failed") };
  pending[2].reject(new Error("preview startup failed"));
  await assert.rejects(failed, /preview startup failed/);
  assert.equal(preview.bridgePromise, null, "the failed current bridge releases its promise");
  assert.deepEqual(removedFrames, ["failed"], "the failed current bridge removes its frame");
}

{
  const rectangles = [];
  const ctx = {
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 0,
    fillRect(x, y, width, height) { rectangles.push({ x, y, width, height }); },
    strokeRect() {},
  };
  const minimap = Object.create(Minimap.prototype);
  Object.assign(minimap, {
    size: 2048,
    _basePresentationSize: 242,
    _worldToCanvas: () => ({ x: 100, y: 100 }),
  });
  minimap._paintResources(ctx, { resources: [{ kind: "steel", x: 1, y: 1, remaining: 10 }] });
  assert.ok(rectangles[0].width > 35 && rectangles[0].width < 40,
    "2048 px captures scale resource marks with the minimap instead of leaving them at 4.4 px");
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
  assert.equal(bridge.status().state, "starting", "capture is not advertised before initial minimap rendering settles");
  await assert.rejects(
    bridge.call("capture", { width: 64, height: 64 }),
    /still starting/,
  );
  releaseInitialization();
  await new Promise((resolve) => setTimeout(resolve, 0));
  releaseInitialization();
  await initialization;
  globalThis.requestAnimationFrame = (callback) => { queueMicrotask(callback); return 1; };
  assert.equal(bridge.status().state, "ready");
  assert.ok(fixture.app.cleanCalls.every(Boolean), "initial preview hides app chrome");
  const previewChildren = fixture.root.children[0].children;
  assert.ok(previewChildren.some((child) => child.className === "map-preview-image"
      && child.src === "data:image/png;base64,minimap"),
    "the preview page presents the captured minimap instead of the world viewport");
  assert.ok(previewChildren.some((child) => child.textContent === "Download minimap PNG (2048 px)"));
  assert.ok(previewChildren.every((child) => !/world/i.test(child.textContent || "")),
    "the preview controls expose no world-render capture");

  const minimap = await bridge.call("capture", { width: 64, height: 64 });
  assert.equal(minimap.authoritative, true);
  assert.equal(minimap.kind, "minimap");
  assert.deepEqual(fixture.minimap.captureCalls, [
    { width: 512, height: 512 },
    { width: 64, height: 64 },
  ], "the visible preview and high-resolution export both use the live Minimap");
  bridge.destroy();
  assert.equal(fixture.documentObj.body.classList.has("map-preview-mode"), false);
} finally {
  globalThis.ImageData = savedImageData;
  globalThis.requestAnimationFrame = savedRaf;
  globalThis.devicePixelRatio = savedDpr;
}

{
  const options = parseMapPreviewArgs([
    "--map", "server/assets/maps/1v1-no-terrain.json",
    "--out", "target/map-preview.png",
    "--width", "1024",
    "--height", "1024",
    "--browser-dpr", "2",
    "--url", "http://localhost:8099/path?ignored=1",
  ]);
  assert.equal(options.browserDpr, 2);
  assert.equal(options.url, "http://localhost:8099/");
  assert.equal(options.width, options.height, "the CLI accepts only square minimap output");
  assert.throws(
    () => parseMapPreviewArgs(["--map", "map.json", "--out", "map.png", "--width", "512", "--height", "256"]),
    /must match/,
  );
  assert.throws(
    () => parseMapPreviewArgs(["--map", "map.json", "--out", "map.png", "--kind", "world"]),
    /unknown argument/,
  );
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
    .filter((map) => map.version === 10);
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

function createBridgeFixture() {
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
    minimap,
    fog: { revealAll: false, setRevealAll(value) { this.revealAll = value; } },
    resizeCount: 0,
    handleResize() { this.resizeCount += 1; },
  };
  const app = { cleanCalls: [], setCleanPresentation(value) { this.cleanCalls.push(value); } };
  return { app, match, minimap, documentObj, root };
}

function fakeNode(kind) {
  return {
    kind,
    children: [],
    listeners: {},
    dataset: {},
    disabled: false,
    textContent: "",
    append(...children) { this.children.push(...children); },
    appendChild(child) { this.children.push(child); },
    remove() { this.removed = true; },
    addEventListener(type, listener) { this.listeners[type] = listener; },
    setAttribute() {},
    querySelector(selector) {
      const dataKey = selector === "[data-map-preview-image]"
        ? "mapPreviewImage"
        : selector === "[data-map-preview-status]" ? "mapPreviewStatus" : "";
      if (!dataKey) return null;
      return this.children.find((child) => Object.hasOwn(child.dataset || {}, dataKey)) || null;
    },
    querySelectorAll(selector) { return selector === "button" ? this.children.filter((child) => child.kind === "button") : []; },
    click() {},
  };
}
