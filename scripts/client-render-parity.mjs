#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import net from "node:net";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { PNG } from "pngjs";
import { buildClientPerfWorkloads } from "./client-perf/workloads.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..");
const DEFAULT_CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const DEFAULT_OUTPUT_ROOT = path.join(REPO_ROOT, "target", "client-perf", "render-parity");
const DEFAULT_VISUAL_TIME_MS = 120_000;
const ASSET_READY_TIMEOUT_MS = 15_000;

export function parseClientRenderParityArgs(argv) {
  const options = {
    baselineWorktree: "",
    candidateWorktree: "",
    workload: "supply-300-hellhole-stream",
    seed: "framewins-phase-1",
    tickFile: "",
    samples: 16,
    viewport: { width: 1440, height: 900 },
    dpr: 1,
    alpha: 1,
    visualTimeMs: DEFAULT_VISUAL_TIME_MS,
    chrome: process.env.CHROME || "",
    outputRoot: DEFAULT_OUTPUT_ROOT,
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    if (arg === "--baseline-worktree") options.baselineWorktree = path.resolve(value());
    else if (arg.startsWith("--baseline-worktree=")) options.baselineWorktree = path.resolve(arg.slice(20));
    else if (arg === "--candidate-worktree") options.candidateWorktree = path.resolve(value());
    else if (arg.startsWith("--candidate-worktree=")) options.candidateWorktree = path.resolve(arg.slice(21));
    else if (arg === "--workload") options.workload = value();
    else if (arg.startsWith("--workload=")) options.workload = arg.slice(11);
    else if (arg === "--seed") options.seed = value();
    else if (arg.startsWith("--seed=")) options.seed = arg.slice(7);
    else if (arg === "--tick-file") options.tickFile = path.resolve(value());
    else if (arg.startsWith("--tick-file=")) options.tickFile = path.resolve(arg.slice(12));
    else if (arg === "--samples") options.samples = positiveInteger(value(), arg);
    else if (arg.startsWith("--samples=")) options.samples = positiveInteger(arg.slice(10), "--samples");
    else if (arg === "--viewport") options.viewport = parseViewport(value());
    else if (arg.startsWith("--viewport=")) options.viewport = parseViewport(arg.slice(11));
    else if (arg === "--dpr") options.dpr = positiveNumber(value(), arg);
    else if (arg.startsWith("--dpr=")) options.dpr = positiveNumber(arg.slice(6), "--dpr");
    else if (arg === "--alpha") options.alpha = unitInterval(value(), arg);
    else if (arg.startsWith("--alpha=")) options.alpha = unitInterval(arg.slice(8), "--alpha");
    else if (arg === "--visual-time-ms") options.visualTimeMs = nonnegativeNumber(value(), arg);
    else if (arg.startsWith("--visual-time-ms=")) options.visualTimeMs = nonnegativeNumber(arg.slice(17), "--visual-time-ms");
    else if (arg === "--chrome") options.chrome = value();
    else if (arg.startsWith("--chrome=")) options.chrome = arg.slice(9);
    else if (arg === "--output-root") options.outputRoot = path.resolve(value());
    else if (arg.startsWith("--output-root=")) options.outputRoot = path.resolve(arg.slice(14));
    else if (arg === "--help" || arg === "-h") options.help = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (!options.help) {
    if (!options.baselineWorktree) throw new Error("--baseline-worktree is required");
    if (!options.candidateWorktree) throw new Error("--candidate-worktree is required");
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(options.workload)) throw new Error("--workload is invalid");
    if (!options.tickFile && !String(options.seed)) throw new Error("--seed must not be empty");
    if (options.alpha !== 1) throw new Error("--alpha must be 1 for the production fixed-capture path");
  }
  return options;
}

export function selectDeterministicTicks({ frameCount, samples, seed }) {
  if (!Number.isInteger(frameCount) || frameCount < 1) throw new Error("frameCount must be positive");
  if (!Number.isInteger(samples) || samples < 1 || samples > frameCount) {
    throw new Error(`samples must be between 1 and ${frameCount}`);
  }
  let state = seedToUint32(String(seed));
  const ticks = new Set();
  while (ticks.size < samples) {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    state >>>= 0;
    ticks.add(state % frameCount);
  }
  return [...ticks].sort((a, b) => a - b);
}

export function readExplicitTicks(filePath, frameCount) {
  const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
  const values = Array.isArray(parsed) ? parsed : parsed?.ticks;
  if (!Array.isArray(values) || values.length === 0) throw new Error("tick file must contain a non-empty array or { ticks: [] }");
  const ticks = [...new Set(values)];
  if (ticks.length !== values.length || ticks.some((tick) => !Number.isInteger(tick) || tick < 0 || tick >= frameCount)) {
    throw new Error(`tick file entries must be unique integers between 0 and ${frameCount - 1}`);
  }
  return ticks.sort((a, b) => a - b);
}

export function assertCaptureHealthy(capture, label = "capture") {
  const failures = [];
  if (!capture || typeof capture !== "object") failures.push("result is missing");
  for (const asset of capture?.readiness?.failedAssets || []) failures.push(`asset ${asset.id} failed: ${asset.message || "unknown error"}`);
  for (const asset of capture?.readiness?.pendingAssets || []) failures.push(`asset ${asset.id} remained pending`);
  for (const error of capture?.readiness?.renderErrors || []) failures.push(`render ${error.label}: ${error.message || "unknown error"}`);
  if ((capture?.readiness?.missingTextureSubjectIds || []).length > 0) {
    failures.push(`missing textures for entities ${capture.readiness.missingTextureSubjectIds.join(",")}`);
  }
  for (const error of capture?.pageErrors || []) failures.push(`page error: ${error}`);
  for (const error of capture?.consoleErrors || []) failures.push(`console error: ${error}`);
  for (const error of capture?.requestFailures || []) failures.push(`request failure: ${error}`);
  if (failures.length > 0) throw new Error(`${label} is invalid: ${failures.join("; ")}`);
}

export function assertCaptureInputsEqual(baseline, candidate, label = "capture") {
  const baselineJson = JSON.stringify(baseline);
  const candidateJson = JSON.stringify(candidate);
  if (baselineJson !== candidateJson) {
    throw new Error(`${label} input mismatch: baseline=${baselineJson} candidate=${candidateJson}`);
  }
}

export function compareDecodedRgba(baseline, candidate) {
  const sameDimensions = baseline.width === candidate.width && baseline.height === candidate.height;
  const width = Math.max(baseline.width, candidate.width);
  const height = Math.max(baseline.height, candidate.height);
  const diff = new PNG({ width, height });
  let changedPixels = 0;
  let bounds = null;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const outIndex = (y * width + x) * 4;
      const baselineIndex = x < baseline.width && y < baseline.height ? (y * baseline.width + x) * 4 : -1;
      const candidateIndex = x < candidate.width && y < candidate.height ? (y * candidate.width + x) * 4 : -1;
      let changed = baselineIndex < 0 || candidateIndex < 0;
      for (let channel = 0; channel < 4 && !changed; channel += 1) {
        changed = baseline.data[baselineIndex + channel] !== candidate.data[candidateIndex + channel];
      }
      if (!changed) continue;
      changedPixels += 1;
      bounds = bounds
        ? { minX: Math.min(bounds.minX, x), minY: Math.min(bounds.minY, y), maxX: Math.max(bounds.maxX, x), maxY: Math.max(bounds.maxY, y) }
        : { minX: x, minY: y, maxX: x, maxY: y };
      diff.data[outIndex] = baselineIndex < 0 || candidateIndex < 0
        ? 255
        : Math.max(32, Math.abs(baseline.data[baselineIndex] - candidate.data[candidateIndex]));
      diff.data[outIndex + 1] = baselineIndex < 0 || candidateIndex < 0
        ? 0
        : Math.abs(baseline.data[baselineIndex + 1] - candidate.data[candidateIndex + 1]);
      diff.data[outIndex + 2] = baselineIndex < 0 || candidateIndex < 0
        ? 255
        : Math.abs(baseline.data[baselineIndex + 2] - candidate.data[candidateIndex + 2]);
      diff.data[outIndex + 3] = 255;
    }
  }
  return { identical: sameDimensions && changedPixels === 0, changedPixels, bounds, diff };
}

export function analyzeDecodedRgba(png) {
  const colors = new Map();
  let opaquePixels = 0;
  for (let index = 0; index < png.data.length; index += 4) {
    const color = (
      (png.data[index] * 0x1_000000) +
      (png.data[index + 1] * 0x1_0000) +
      (png.data[index + 2] * 0x100) +
      png.data[index + 3]
    );
    colors.set(color, (colors.get(color) || 0) + 1);
    if (png.data[index + 3] === 255) opaquePixels += 1;
  }
  const pixelCount = png.width * png.height;
  let dominantColorPixels = 0;
  for (const count of colors.values()) dominantColorPixels = Math.max(dominantColorPixels, count);
  return {
    width: png.width,
    height: png.height,
    pixelCount,
    uniqueColors: colors.size,
    dominantColorPixels,
    nonDominantPixels: pixelCount - dominantColorPixels,
    opaquePixels,
  };
}

export function assertCaptureContent(content, label = "capture") {
  if (!content || content.pixelCount < 1) throw new Error(`${label} has no decoded pixels`);
  if (content.uniqueColors < 8 || content.nonDominantPixels < 64) {
    throw new Error(
      `${label} is visually empty: ${content.uniqueColors} colors, ${content.nonDominantPixels} non-dominant pixels`,
    );
  }
}

export function assertCaptureSequenceVaries(captures, label = "capture") {
  if (captures.length < 2) return;
  const hashes = new Set(captures.map((capture) => capture.rgbaSha256));
  if (hashes.size < 2) throw new Error(`${label} produced the same RGBA frame at every selected tick`);
}

export async function withIsolatedServers(worktrees, startServer, work) {
  const servers = [];
  try {
    for (const worktree of worktrees) servers.push(await startServer(worktree));
    return await work(servers);
  } finally {
    await Promise.allSettled(servers.map((server) => server.close()));
  }
}

export async function runClientRenderParity(options, dependencies = {}) {
  const workload = workloadById(options.workload);
  const baselineStream = readStreamIdentity(options.baselineWorktree, workload);
  const candidateStream = readStreamIdentity(options.candidateWorktree, workload);
  if (baselineStream.frameCount !== candidateStream.frameCount || baselineStream.sha256 !== candidateStream.sha256) {
    throw new Error("baseline and candidate snapshot-stream assets differ");
  }
  const ticks = options.tickFile
    ? readExplicitTicks(options.tickFile, baselineStream.frameCount)
    : selectDeterministicTicks({ frameCount: baselineStream.frameCount, samples: options.samples, seed: options.seed });
  const outputRoot = path.resolve(options.outputRoot);
  for (const side of ["baseline", "candidate", "diff"]) fs.mkdirSync(path.join(outputRoot, side), { recursive: true });

  const startServer = dependencies.startServer || startStaticServer;
  const launchBrowser = dependencies.launchBrowser || launchParityBrowser;
  const captureRevision = dependencies.captureRevision || captureWorktree;
  const chrome = findChrome(options.chrome);
  const result = await withIsolatedServers(
      [options.baselineWorktree, options.candidateWorktree],
      startServer,
      async ([baselineServer, candidateServer]) => {
        const captureInFreshBrowser = async (server, side, stream) => {
          const browser = await launchBrowser(chrome, options);
          try {
            return await captureRevision({ browser, server, side, ticks, workload, options, stream });
          } finally {
            await browser.close();
          }
        };
        const baseline = await captureInFreshBrowser(baselineServer, "baseline", baselineStream);
        const candidate = await captureInFreshBrowser(candidateServer, "candidate", candidateStream);
        const comparisons = [];
        for (let index = 0; index < ticks.length; index += 1) {
          const baseCapture = baseline.captures[index];
          const candidateCapture = candidate.captures[index];
          assertCaptureHealthy(baseCapture, `baseline tick ${ticks[index]}`);
          assertCaptureHealthy(candidateCapture, `candidate tick ${ticks[index]}`);
          assertCaptureInputsEqual(baseCapture.inputs, candidateCapture.inputs, `tick ${ticks[index]}`);
          const basePng = PNG.sync.read(fs.readFileSync(baseCapture.pngPath));
          const candidatePng = PNG.sync.read(fs.readFileSync(candidateCapture.pngPath));
          const baselineContent = analyzeDecodedRgba(basePng);
          const candidateContent = analyzeDecodedRgba(candidatePng);
          assertCaptureContent(baselineContent, `baseline tick ${ticks[index]}`);
          assertCaptureContent(candidateContent, `candidate tick ${ticks[index]}`);
          const compared = compareDecodedRgba(basePng, candidatePng);
          const diffPath = path.join(outputRoot, "diff", tickFileName(ticks[index]));
          fs.writeFileSync(diffPath, PNG.sync.write(compared.diff));
          comparisons.push({
            frameIndex: ticks[index],
            tick: baseCapture.inputs.stateTick,
            identical: compared.identical,
            changedPixels: compared.changedPixels,
            bounds: compared.bounds,
            baselineRgbaSha256: hashBytes(basePng.data),
            candidateRgbaSha256: hashBytes(candidatePng.data),
            baselineContent,
            candidateContent,
            baselinePng: relativeArtifact(outputRoot, baseCapture.pngPath),
            candidatePng: relativeArtifact(outputRoot, candidateCapture.pngPath),
            diffPng: relativeArtifact(outputRoot, diffPath),
          });
        }
        assertCaptureSequenceVaries(
          comparisons.map((comparison) => ({ rgbaSha256: comparison.baselineRgbaSha256 })),
          "baseline",
        );
        assertCaptureSequenceVaries(
          comparisons.map((comparison) => ({ rgbaSha256: comparison.candidateRgbaSha256 })),
          "candidate",
        );
        return {
          schemaVersion: 2,
          status: comparisons.every((comparison) => comparison.identical) ? "passed" : "failed",
          workload: workload.id,
          snapshotStream: { id: workload.setup.snapshotStreamId, ...baselineStream },
          selection: { seed: options.tickFile ? null : String(options.seed), tickFile: options.tickFile || null, ticks },
          capture: {
            viewport: options.viewport,
            dpr: options.dpr,
            alpha: options.alpha,
            visualTimeMs: options.visualTimeMs,
            browser: baseline.browser,
          },
          comparisons,
        };
      },
    );
  const summaryPath = path.join(outputRoot, "summary.json");
  fs.writeFileSync(summaryPath, `${JSON.stringify(result, null, 2)}\n`);
  return { ...result, summaryPath };
}

async function captureWorktree({ browser, server, side, ticks, workload, options, stream }) {
  const page = await browser.newPage();
  const consoleErrors = [];
  const pageErrors = [];
  const requestFailures = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("requestfailed", (request) => {
    if (!request.url().includes("favicon")) requestFailures.push(`${request.failure()?.errorText || "request failed"} ${request.url()}`);
  });
  await page.evaluateOnNewDocument((initialSeed) => {
    let randomState = initialSeed >>> 0;
    Math.random = () => {
      randomState ^= randomState << 13;
      randomState ^= randomState >>> 17;
      randomState ^= randomState << 5;
      randomState >>>= 0;
      return randomState / 0x1_0000_0000;
    };
  }, seedToUint32(String(options.seed || "client-render-parity")));
  await page.setViewport({ ...options.viewport, deviceScaleFactor: options.dpr });
  try {
    const targetUrl = new URL(workload.url, server.baseUrl).href;
    await page.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: 20_000 });
    await page.waitForFunction(
      () => !!window.__rts?.match && window.__rts?.net?.frames?.length > 0,
      { timeout: 20_000 },
    );
    await page.evaluate(() => {
      const app = window.__rts;
      const net = app.net;
      if (net.timer !== null) net.clearTimeoutFn(net.timer);
      net.timer = null;
      window.__rtsParityLastSnapshot = null;
      net.on("snapshot", (snapshot) => { window.__rtsParityLastSnapshot = snapshot; });
      window.__rtsParityPreviousMatch = app.match;
      net.restartFromBeginning();
      if (net.timer !== null) net.clearTimeoutFn(net.timer);
      net.timer = null;
    });
    await page.waitForFunction(
      () => !!window.__rts?.match && window.__rts.match !== window.__rtsParityPreviousMatch,
      { timeout: 20_000 },
    );
    const initialized = await page.evaluate(() => {
      const app = window.__rts;
      const net = app.net;
      const match = app.match;
      match.enterFixedCapture();
      match.captureClock.valueMs = 0;
      match.lastFrame = 0;
      window.__rtsParityNextFrame = 0;
      return { frameCount: net.frames.length, streamId: net.id };
    });
    if (initialized.frameCount !== stream.frameCount || initialized.streamId !== workload.setup.snapshotStreamId) {
      throw new Error(`${side} loaded the wrong snapshot stream`);
    }

    let warmup = await renderCaptureFrame(page, ticks[0], options);
    const warmupDeadline = Date.now() + ASSET_READY_TIMEOUT_MS;
    while (!warmup.readiness.ready && Date.now() < warmupDeadline) {
      if (warmup.readiness.failedAssets.length > 0) break;
      await delay(50);
      warmup = await renderCaptureFrame(page, ticks[0], options, { deliver: false });
    }
    assertCaptureHealthy({
      readiness: warmup.readiness,
      consoleErrors,
      pageErrors,
      requestFailures,
    }, `${side} asset warmup`);
    if (!warmup.readiness.ready) throw new Error(`${side} asset warmup did not become ready`);
    await resetCaptureFromBeginning(page);

    const captures = [];
    for (const frameIndex of ticks) {
      let capture = await renderCaptureFrame(page, frameIndex, options);
      const deadline = Date.now() + ASSET_READY_TIMEOUT_MS;
      while (!capture.readiness.ready && Date.now() < deadline) {
        if (capture.readiness.failedAssets.length > 0) break;
        await delay(50);
        capture = await renderCaptureFrame(page, frameIndex, options, { deliver: false });
      }
      if (capture.readiness.ready) {
        capture = await renderCaptureFrame(page, frameIndex, options, { deliver: false, capturePng: true });
      } else {
        capture = await renderCaptureFrame(page, frameIndex, options, { deliver: false, capturePng: true });
      }
      if (!capture.pngDataUrl?.startsWith("data:image/png;base64,")) {
        throw new Error(`${side} tick ${frameIndex} has no immediately readable Pixi canvas`);
      }
      const pngPath = path.join(options.outputRoot, side, tickFileName(frameIndex));
      fs.writeFileSync(pngPath, Buffer.from(capture.pngDataUrl.slice(capture.pngDataUrl.indexOf(",") + 1), "base64"));
      const stateHash = hashBytes(Buffer.from(capture.stateJson));
      captures.push({
        pngPath,
        inputs: {
          workload: workload.id,
          streamSha256: stream.sha256,
          frameIndex,
          stateTick: capture.stateTick,
          stateSha256: stateHash,
          camera: capture.camera,
          viewport: options.viewport,
          dpr: options.dpr,
          alpha: options.alpha,
          visualTimeMs: options.visualTimeMs,
          randomSeed: String(options.seed || "client-render-parity"),
          canvas: capture.canvas,
        },
        readiness: capture.readiness,
        consoleErrors: [...consoleErrors],
        pageErrors: [...pageErrors],
        requestFailures: [...requestFailures],
      });
    }
    const version = await browser.version();
    return { browser: { product: version, executablePath: browser.process()?.spawnfile || "" }, captures };
  } finally {
    await page.evaluate(() => {
      window.__rts?.match?.exitFixedCapture?.();
      window.__rts?.net?.close?.();
    }).catch(() => {});
    await page.close();
  }
}

async function resetCaptureFromBeginning(page) {
  await page.evaluate(() => {
    const app = window.__rts;
    const net = app.net;
    app.match?.exitFixedCapture?.();
    window.__rtsParityPreviousMatch = app.match;
    net.restartFromBeginning();
    if (net.timer !== null) net.clearTimeoutFn(net.timer);
    net.timer = null;
    window.__rtsParityLastSnapshot = null;
    window.__rtsParityNextFrame = 0;
  });
  await page.waitForFunction(
    () => !!window.__rts?.match && window.__rts.match !== window.__rtsParityPreviousMatch,
    { timeout: 20_000 },
  );
  return page.evaluate(() => {
    const app = window.__rts;
    const net = app.net;
    const match = app.match;
    match.enterFixedCapture();
    match.captureClock.valueMs = 0;
    match.lastFrame = 0;
    return { frameCount: net.frames.length, streamId: net.id };
  });
}

async function renderCaptureFrame(page, targetFrame, options, { deliver = true, capturePng = false } = {}) {
  return page.evaluate(async ({ target, visualTimeMs, shouldDeliver, shouldCapturePng }) => {
    const app = window.__rts;
    const net = app.net;
    const match = app.match;
    if (shouldDeliver) {
      while (window.__rtsParityNextFrame <= target) {
        const index = window.__rtsParityNextFrame;
        net._onMessage({ data: net.frames[index] });
        window.__rtsParityNextFrame = index + 1;
      }
    }
    await match.renderFixedCaptureFrame(visualTimeMs);
    const entities = match.state.entitiesInterpolated(1, { includePrediction: false });
    const readiness = match.renderer.captureReadiness({
      subjectIds: entities.map((entity) => entity.id),
      subjectKinds: [...new Set(entities.map((entity) => entity.kind))],
    });
    const canvas = document.querySelector("#viewport canvas");
    const stable = (value) => {
      if (Array.isArray(value)) return value.map(stable);
      if (ArrayBuffer.isView(value)) return Array.from(value, stable);
      if (!value || typeof value !== "object") return value;
      const out = {};
      for (const key of Object.keys(value).sort()) out[key] = stable(value[key]);
      return out;
    };
    const snapshot = window.__rtsParityLastSnapshot;
    return {
      stateTick: match.state.tick,
      stateJson: JSON.stringify(stable(snapshot)),
      camera: match.camera.snapshot(),
      canvas: canvas ? {
        width: canvas.width,
        height: canvas.height,
        clientWidth: canvas.clientWidth,
        clientHeight: canvas.clientHeight,
      } : null,
      readiness,
      pngDataUrl: shouldCapturePng ? canvas?.toDataURL("image/png") || "" : null,
    };
  }, {
    target: targetFrame,
    visualTimeMs: options.visualTimeMs,
    shouldDeliver: deliver,
    shouldCapturePng: capturePng,
  });
}

async function startStaticServer(worktree) {
  const clientRoot = path.join(worktree, "client");
  const port = await allocatePort();
  const server = http.createServer((request, response) => {
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(request.url || "/", `http://${request.headers.host || "localhost"}`).pathname);
    } catch {
      response.writeHead(400).end("bad request");
      return;
    }
    if (pathname === "/version") {
      response.writeHead(200, { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" }).end("client-render-parity");
      return;
    }
    if (pathname === "/api/matches") {
      response.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }).end("[]");
      return;
    }
    const relative = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
    const filePath = path.resolve(clientRoot, relative);
    if (filePath !== clientRoot && !filePath.startsWith(`${clientRoot}${path.sep}`)) {
      response.writeHead(403).end("forbidden");
      return;
    }
    let stat;
    try {
      stat = fs.statSync(filePath);
    } catch {
      response.writeHead(404).end("not found");
      return;
    }
    const resolved = stat.isDirectory() ? path.join(filePath, "index.html") : filePath;
    response.writeHead(200, { "content-type": contentType(resolved), "cache-control": "no-store" });
    fs.createReadStream(resolved).pipe(response);
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });
  return {
    baseUrl: `http://127.0.0.1:${port}/`,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

async function launchParityBrowser(chrome, options) {
  const imported = await import("puppeteer-core");
  const puppeteer = imported.default || imported;
  return puppeteer.launch({
    executablePath: chrome,
    headless: "new",
    defaultViewport: { ...options.viewport, deviceScaleFactor: options.dpr },
    args: ["--no-sandbox", `--window-size=${options.viewport.width},${options.viewport.height}`],
  });
}

function readStreamIdentity(worktree, workload) {
  const id = workload.setup.snapshotStreamId;
  const assetPath = path.join(worktree, "client", "assets", "snapshot-streams", `${id}.rtsstream`);
  const bytes = fs.readFileSync(assetPath);
  if (bytes.subarray(0, 8).toString("utf8") !== "RTSSTRM1") throw new Error(`${assetPath} has an invalid stream header`);
  const headerLength = bytes.readUInt32LE(8);
  const header = JSON.parse(bytes.subarray(12, 12 + headerLength).toString("utf8"));
  if (header.id !== id || !Number.isInteger(header.frameCount) || header.frameCount < 1) {
    throw new Error(`${assetPath} has an invalid stream identity`);
  }
  return { frameCount: header.frameCount, tickRateHz: header.tickRateHz, sha256: hashBytes(bytes) };
}

function workloadById(id) {
  const workload = buildClientPerfWorkloads().find((entry) => entry.id === id);
  if (!workload) throw new Error(`unknown workload ${id}`);
  if (workload.kind !== "snapshotStream" || !workload.setup?.snapshotStreamId) {
    throw new Error(`workload ${id} is not a snapshot-stream workload`);
  }
  return workload;
}

function findChrome(explicit) {
  for (const candidate of [explicit, DEFAULT_CHROME, "/Applications/Chromium.app/Contents/MacOS/Chromium"].filter(Boolean)) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error("Chrome/Chromium not found; set CHROME or pass --chrome");
}

function contentType(filePath) {
  const types = {
    ".css": "text/css; charset=utf-8",
    ".html": "text/html; charset=utf-8",
    ".ico": "image/x-icon",
    ".js": "text/javascript; charset=utf-8",
    ".json": "application/json",
    ".mjs": "text/javascript; charset=utf-8",
    ".png": "image/png",
    ".rtsstream": "application/octet-stream",
    ".svg": "image/svg+xml",
    ".webmanifest": "application/manifest+json",
    ".webp": "image/webp",
  };
  return types[path.extname(filePath).toLowerCase()] || "application/octet-stream";
}

function allocatePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

function parseViewport(raw) {
  const match = /^(\d+)x(\d+)$/.exec(String(raw));
  if (!match) throw new Error("--viewport must look like 1440x900");
  const width = Number(match[1]);
  const height = Number(match[2]);
  if (width < 320 || height < 240 || width > 7680 || height > 4320) throw new Error("--viewport is out of bounds");
  return { width, height };
}

function positiveInteger(raw, label) {
  const value = Number(raw);
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

function positiveNumber(raw, label) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) throw new Error(`${label} must be positive`);
  return value;
}

function nonnegativeNumber(raw, label) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0) throw new Error(`${label} must be non-negative`);
  return value;
}

function unitInterval(raw, label) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0 || value > 1) throw new Error(`${label} must be between 0 and 1`);
  return value;
}

function seedToUint32(seed) {
  let value = 2166136261;
  for (const char of seed) {
    value ^= char.codePointAt(0);
    value = Math.imul(value, 16777619);
  }
  return value >>> 0 || 0x9e3779b9;
}

function hashBytes(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function tickFileName(frameIndex) {
  return `frame-${String(frameIndex).padStart(4, "0")}.png`;
}

function relativeArtifact(root, filePath) {
  return path.relative(root, filePath).split(path.sep).join("/");
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function printHelp() {
  console.log(`Usage: node scripts/client-render-parity.mjs [options]

Required:
  --baseline-worktree <path>   Clean baseline worktree.
  --candidate-worktree <path>  Candidate worktree.

Options:
  --workload <id>              Snapshot-stream workload (default supply-300-hellhole-stream).
  --seed <text>                Deterministic tick-selection seed.
  --tick-file <json>           Explicit tick array, replacing seeded selection.
  --samples <n>                Unique seeded ticks (default 16).
  --viewport <WxH>             CSS viewport (default 1440x900).
  --dpr <n>                    Device pixel ratio (default 1).
  --alpha <0..1>               Fixed interpolation alpha input (default 1).
  --visual-time-ms <n>         Fixed visual timestamp (default ${DEFAULT_VISUAL_TIME_MS}).
  --chrome <path>              Chrome/Chromium executable.
  --output-root <path>         Baseline/candidate/diff PNGs and summary.json.

PNG files are read back in the render task, rejected when visually empty, decoded with pngjs, and
compared byte-for-byte. Baseline and candidate each run in a fresh browser process.
`);
}

async function main() {
  let options;
  try {
    options = parseClientRenderParityArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }
    const result = await runClientRenderParity(options);
    console.log(`${result.status.toUpperCase()} client render parity: ${result.comparisons.length} ticks`);
    console.log(`ticks: ${result.selection.ticks.join(",")}`);
    console.log(`summary: ${result.summaryPath}`);
    if (result.status !== "passed") process.exitCode = 1;
  } catch (error) {
    const outputRoot = options?.outputRoot ? path.resolve(options.outputRoot) : null;
    if (outputRoot) {
      fs.mkdirSync(outputRoot, { recursive: true });
      fs.writeFileSync(path.join(outputRoot, "summary.json"), `${JSON.stringify({
        schemaVersion: 1,
        status: "failed",
        error: error?.stack || error?.message || String(error),
      }, null, 2)}\n`);
    }
    console.error(error?.stack || error?.message || String(error));
    process.exitCode = 1;
  }
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  await main();
}
