#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createMapHandoff } from "../client/src/map_editor_handoff.js";
import { MapEditorSession } from "../client/src/map_editor_session.js";

const DEFAULT_CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const DEFAULT_URL = "http://127.0.0.1:8080/";
const READY_TIMEOUT_MS = 30_000;
const HANDOFF_TIMEOUT_MS = 15_000;
const CAPTURE_TIMEOUT_MS = 50_000;

export function parseMapPreviewArgs(argv) {
  const options = {
    map: "",
    out: "",
    kind: "world",
    width: 2048,
    height: 2048,
    padding: 24,
    url: DEFAULT_URL,
    chrome: process.env.CHROME || "",
    browserDpr: 1,
    jpegQuality: 88,
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    if (arg === "--map") options.map = path.resolve(value());
    else if (arg.startsWith("--map=")) options.map = path.resolve(arg.slice(6));
    else if (arg === "--out") options.out = path.resolve(value());
    else if (arg.startsWith("--out=")) options.out = path.resolve(arg.slice(6));
    else if (arg === "--kind") options.kind = value();
    else if (arg.startsWith("--kind=")) options.kind = arg.slice(7);
    else if (arg === "--width") options.width = integer(value(), arg);
    else if (arg.startsWith("--width=")) options.width = integer(arg.slice(8), "--width");
    else if (arg === "--height") options.height = integer(value(), arg);
    else if (arg.startsWith("--height=")) options.height = integer(arg.slice(9), "--height");
    else if (arg === "--padding") options.padding = integer(value(), arg, { allowZero: true });
    else if (arg.startsWith("--padding=")) options.padding = integer(arg.slice(10), "--padding", { allowZero: true });
    else if (arg === "--url") options.url = value();
    else if (arg.startsWith("--url=")) options.url = arg.slice(6);
    else if (arg === "--chrome") options.chrome = value();
    else if (arg.startsWith("--chrome=")) options.chrome = arg.slice(9);
    else if (arg === "--browser-dpr") options.browserDpr = numberInRange(value(), arg, 1, 4);
    else if (arg.startsWith("--browser-dpr=")) options.browserDpr = numberInRange(arg.slice(14), "--browser-dpr", 1, 4);
    else if (arg === "--jpeg-quality") options.jpegQuality = integerInRange(value(), arg, 1, 100);
    else if (arg.startsWith("--jpeg-quality=")) options.jpegQuality = integerInRange(arg.slice(15), "--jpeg-quality", 1, 100);
    else if (arg === "--help" || arg === "-h") options.help = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (!options.help) {
    if (!options.map) throw new Error("--map is required");
    if (!options.out) throw new Error("--out is required");
    const extension = path.extname(options.out).toLowerCase();
    if (!new Set([".png", ".jpg", ".jpeg"]).has(extension)) {
      throw new Error("--out must use a .png, .jpg, or .jpeg extension");
    }
    options.format = extension === ".png" ? "png" : "jpeg";
    if (!new Set(["world", "minimap"]).has(options.kind)) throw new Error("--kind must be world or minimap");
    options.url = localServerUrl(options.url);
  }
  return options;
}

export async function renderMapPreview(options, dependencies = {}) {
  const authoredMap = JSON.parse(fs.readFileSync(options.map, "utf8"));
  const session = new MapEditorSession();
  session.loadAuthoredMap(authoredMap);
  const handoff = await createPreviewHandoff(
    options.url,
    authoredMap,
    session.materialized(),
    dependencies.fetchImpl || fetch,
    dependencies.handoffTimeoutMs || HANDOFF_TIMEOUT_MS,
  );
  const chrome = findChrome(options.chrome);
  const loadPuppeteer = dependencies.loadPuppeteer || defaultPuppeteerLoader;
  const puppeteer = await loadPuppeteer();
  const browser = await puppeteer.launch({
    executablePath: chrome,
    headless: "new",
    defaultViewport: {
      width: Math.min(options.width, 4096),
      height: Math.min(options.height, 4096),
      deviceScaleFactor: options.browserDpr,
    },
  });
  try {
    const page = await browser.newPage();
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(String(error?.message || error).slice(0, 500)));
    const previewUrl = new URL("/map-preview", options.url);
    previewUrl.searchParams.set("handoff", handoff.handoffId);
    await page.goto(previewUrl.toString(), { waitUntil: "networkidle0", timeout: READY_TIMEOUT_MS });
    await page.waitForFunction(() => {
      const state = globalThis.__rtsMapPreview?.status?.()?.state;
      return state === "ready" || state === "failed";
    }, { timeout: READY_TIMEOUT_MS });
    const status = await page.evaluate(() => globalThis.__rtsMapPreview.status());
    if (status.state !== "ready") throw new Error(status.error || "Map preview page failed to start.");
    const result = await withTimeout(
      page.evaluate(
        (request) => globalThis.__rtsMapPreview.call("capture", request),
        { kind: options.kind, width: options.width, height: options.height, padding: options.padding },
      ),
      dependencies.captureTimeoutMs || CAPTURE_TIMEOUT_MS,
      "Map preview browser capture timed out.",
    );
    if (pageErrors.length) throw new Error(`Map preview page error: ${pageErrors[0]}`);
    const pngPrefix = "data:image/png;base64,";
    if (!String(result?.pngDataUrl).startsWith(pngPrefix)) throw new Error("Map preview page returned no PNG.");
    const pngBytes = Buffer.from(result.pngDataUrl.slice(pngPrefix.length), "base64");
    const pngDimensions = readPngDimensions(pngBytes);
    if (pngDimensions.width !== options.width || pngDimensions.height !== options.height) {
      throw new Error(`Map preview returned ${pngDimensions.width}x${pngDimensions.height}, expected ${options.width}x${options.height}.`);
    }
    let bytes = pngBytes;
    let dimensions = pngDimensions;
    if (options.format === "jpeg") {
      const jpegDataUrl = await encodeJpegDataUrl(page, result.pngDataUrl, options.jpegQuality);
      const jpegPrefix = "data:image/jpeg;base64,";
      if (!String(jpegDataUrl).startsWith(jpegPrefix)) throw new Error("Map preview page returned no JPEG.");
      bytes = Buffer.from(jpegDataUrl.slice(jpegPrefix.length), "base64");
      dimensions = readJpegDimensions(bytes);
    }
    if (dimensions.width !== options.width || dimensions.height !== options.height) {
      throw new Error(`Map preview returned ${dimensions.width}x${dimensions.height}, expected ${options.width}x${options.height}.`);
    }
    fs.mkdirSync(path.dirname(options.out), { recursive: true });
    fs.writeFileSync(options.out, bytes);
    return Object.freeze({
      map: path.resolve(options.map),
      out: options.out,
      kind: options.kind,
      format: options.format,
      width: dimensions.width,
      height: dimensions.height,
      bytes: bytes.length,
      readiness: result.readiness,
      content: result.content || null,
    });
  } finally {
    await browser.close();
  }
}

export async function createPreviewHandoff(baseUrl, authoredMap, materializedMap, fetchImpl = fetch, timeoutMs = HANDOFF_TIMEOUT_MS) {
  return createMapHandoff({
    destination: "lab",
    authoredMap,
    materializedMap,
    fetchImpl,
    timeoutMs: numberInRange(timeoutMs, "handoff timeout", 1, 60_000),
    collectionUrl: new URL("/api/map-handoffs", baseUrl),
  });
}

function localServerUrl(value) {
  const url = new URL(value);
  if (!new Set(["http:", "https:"]).has(url.protocol)) throw new Error("--url must use HTTP or HTTPS");
  if (!new Set(["127.0.0.1", "localhost", "::1", "[::1]"]).has(url.hostname)) {
    throw new Error("--url must target a loopback server; map handoffs are local-only");
  }
  url.pathname = "/";
  url.search = "";
  url.hash = "";
  return url.toString();
}

function findChrome(explicit) {
  const candidates = [explicit, DEFAULT_CHROME, "/Applications/Chromium.app/Contents/MacOS/Chromium"].filter(Boolean);
  const found = candidates.find((candidate) => fs.existsSync(candidate));
  if (!found) throw new Error("Chrome/Chromium not found; set CHROME or pass --chrome");
  return found;
}

async function defaultPuppeteerLoader() {
  try {
    const imported = await import("puppeteer-core");
    return imported.default || imported;
  } catch (error) {
    throw new Error(`puppeteer-core is not installed; run npm ci at the repository root (${String(error?.code || "import failed")})`);
  }
}

function integer(value, label, { allowZero = false } = {}) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < (allowZero ? 0 : 1)) throw new Error(`${label} requires ${allowZero ? "a non-negative" : "a positive"} integer`);
  return parsed;
}

function numberInRange(value, label, minimum, maximum) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < minimum || parsed > maximum) {
    throw new Error(`${label} requires a number from ${minimum} to ${maximum}`);
  }
  return parsed;
}

function integerInRange(value, label, minimum, maximum) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    throw new Error(`${label} requires an integer from ${minimum} to ${maximum}`);
  }
  return parsed;
}

async function encodeJpegDataUrl(page, pngDataUrl, jpegQuality) {
  return page.evaluate(async ({ source, quality }) => {
    const image = await new Promise((resolve, reject) => {
      const candidate = new Image();
      candidate.onload = () => resolve(candidate);
      candidate.onerror = () => reject(new Error("Could not decode captured minimap PNG."));
      candidate.src = source;
    });
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) throw new Error("Could not create JPEG conversion canvas.");
    context.fillStyle = "#11110f";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(image, 0, 0);
    return canvas.toDataURL("image/jpeg", quality / 100);
  }, { source: pngDataUrl, quality: jpegQuality });
}

async function withTimeout(promise, timeoutMs, message) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(message)), timeoutMs); }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

export function readPngDimensions(bytes) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!Buffer.isBuffer(bytes) || bytes.length < 24 || !bytes.subarray(0, 8).equals(signature) || bytes.toString("ascii", 12, 16) !== "IHDR") {
    throw new Error("Map preview returned malformed PNG bytes.");
  }
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}

export function readJpegDimensions(bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length < 11 || bytes[0] !== 0xff || bytes[1] !== 0xd8) {
    throw new Error("Map preview returned malformed JPEG bytes.");
  }
  const startOfFrameMarkers = new Set([
    0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7,
    0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
  ]);
  let offset = 2;
  while (offset < bytes.length) {
    while (bytes[offset] === 0xff) offset += 1;
    const marker = bytes[offset];
    offset += 1;
    if (marker === 0xd9 || marker === 0xda || offset + 2 > bytes.length) break;
    const segmentLength = bytes.readUInt16BE(offset);
    if (segmentLength < 2 || offset + segmentLength > bytes.length) break;
    if (startOfFrameMarkers.has(marker) && segmentLength >= 7) {
      return {
        width: bytes.readUInt16BE(offset + 5),
        height: bytes.readUInt16BE(offset + 3),
      };
    }
    offset += segmentLength;
  }
  throw new Error("Map preview JPEG has no dimensions.");
}

function usage() {
  return `Usage:
  node scripts/map-preview.mjs --map MAP.json --out PREVIEW.png|PREVIEW.jpg [options]

Options:
  --kind world|minimap   Live Match world renderer or live Match minimap (default: world)
  --width N              Output width in pixels (default: 2048; capture page allows 64–4096)
  --height N             Output height in pixels (default: 2048; capture page allows 64–4096)
  --padding N            World-preview edge padding in pixels (default: 24)
  --url URL              Running local RTS server (default: ${DEFAULT_URL})
  --chrome PATH          Chrome/Chromium executable (or set CHROME)
  --browser-dpr N        Browser device scale for DPR regression checks (default: 1)
  --jpeg-quality N       JPEG quality from 1 to 100 (default: 88)
`;
}

async function main() {
  try {
    const options = parseMapPreviewArgs(process.argv.slice(2));
    if (options.help) {
      process.stdout.write(usage());
      return;
    }
    const result = await renderMapPreview(options);
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } catch (error) {
    process.stderr.write(`map-preview: ${error?.message || String(error)}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
