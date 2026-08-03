#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
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
    else if (arg === "--help" || arg === "-h") options.help = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (!options.help) {
    if (!options.map) throw new Error("--map is required");
    if (!options.out) throw new Error("--out is required");
    if (!options.out.toLowerCase().endsWith(".png")) throw new Error("--out must use a .png extension");
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
    const prefix = "data:image/png;base64,";
    if (!String(result?.pngDataUrl).startsWith(prefix)) throw new Error("Map preview page returned no PNG.");
    const bytes = Buffer.from(result.pngDataUrl.slice(prefix.length), "base64");
    const dimensions = readPngDimensions(bytes);
    if (dimensions.width !== options.width || dimensions.height !== options.height) {
      throw new Error(`Map preview returned ${dimensions.width}x${dimensions.height}, expected ${options.width}x${options.height}.`);
    }
    fs.mkdirSync(path.dirname(options.out), { recursive: true });
    fs.writeFileSync(options.out, bytes);
    return Object.freeze({
      map: path.resolve(options.map),
      out: options.out,
      kind: options.kind,
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
  const timeout = numberInRange(timeoutMs, "handoff timeout", 1, 60_000);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error("Map preview handoff timed out.")), timeout);
  let response;
  let payload;
  try {
    response = await fetchImpl(new URL("/api/map-handoffs", baseUrl), {
    method: "POST",
    headers: { "content-type": "application/json" },
      body: JSON.stringify({ destination: "lab", authoredMap, materializedMap }),
      signal: controller.signal,
    });
    payload = await response.json().catch(() => ({}));
  } catch (error) {
    if (controller.signal.aborted) throw new Error("Map preview handoff timed out.");
    throw error;
  } finally {
    clearTimeout(timer);
  }
  if (!response.ok) throw new Error(payload?.error || `Map preview handoff failed (HTTP ${response.status}).`);
  if (!/^[a-f0-9]{32}$/.test(payload?.handoffId || "")) throw new Error("Map preview handoff returned an invalid id.");
  return payload;
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

function usage() {
  return `Usage:
  node scripts/map-preview.mjs --map MAP.json --out PREVIEW.png [options]

Options:
  --kind world|minimap   Live Match world renderer or live Match minimap (default: world)
  --width N              Output width in pixels (default: 2048; capture page allows 64–4096)
  --height N             Output height in pixels (default: 2048; capture page allows 64–4096)
  --padding N            World-preview edge padding in pixels (default: 24)
  --url URL              Running local RTS server (default: ${DEFAULT_URL})
  --chrome PATH          Chrome/Chromium executable (or set CHROME)
  --browser-dpr N        Browser device scale for DPR regression checks (default: 1)
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
