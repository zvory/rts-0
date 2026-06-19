#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { formatBakeoffMarkdown, runSnapshotCodecBakeoff } from "./snapshot-codec-bakeoff.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..");
const SERVER_DIR = path.join(REPO_ROOT, "server");
const TESTS_DIR = path.join(REPO_ROOT, "tests");
const DEFAULT_OUTPUT_ROOT = path.join(REPO_ROOT, "target", "client-perf");
const DEFAULT_CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const DEFAULT_VIEWPORT = Object.freeze({ width: 1440, height: 900 });
const DEFAULT_DURATION_MS = 6000;
const DEFAULT_CODEC_SAMPLE_LIMIT = 240;
const MATT_ALEX_SOURCE = path.join(
  REPO_ROOT,
  "docs",
  "network-incident-examples",
  "2026-06-19-beta-matt-alex",
  "match-54-replay.json",
);
const MATT_ALEX_ARTIFACT_NAME = "client_perf_matt_alex_match_54";

const WORKLOADS = Object.freeze([
  {
    id: "matt-alex-replay",
    description: "Preserved 2026-06-19 Matt/Alex match 54 replay artifact.",
    kind: "replayArtifact",
    replayName: MATT_ALEX_ARTIFACT_NAME,
    source: MATT_ALEX_SOURCE,
    url: `/dev/replay-artifact?replay=${MATT_ALEX_ARTIFACT_NAME}`,
  },
  {
    id: "vehicle-wall-stress",
    description: "No-fog dev scenario with 15 tanks moving through a wall chokepoint.",
    kind: "devScenario",
    url: "/dev/scenarios?id=scout_car_wall_chokepoint&unit=tank&count=15",
  },
]);

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.list) {
    for (const workload of WORKLOADS) {
      console.log(`${workload.id}\t${workload.description}`);
    }
    return;
  }

  const selected = selectedWorkloads(args);
  const outputRoot = path.resolve(args.outputRoot || DEFAULT_OUTPUT_ROOT);
  fs.mkdirSync(outputRoot, { recursive: true });
  const puppeteer = await loadPuppeteer();
  const chrome = findChrome(args.chrome);
  const server = await startOrReuseServer(args);
  const browser = await launchBrowser(puppeteer, chrome, args);

  const results = [];
  let failed = 0;
  try {
    for (const workload of selected) {
      const result = await runWorkload({ workload, server, browser, outputRoot, args, chrome });
      results.push(result);
      const status = result.status === "passed" ? "PASS" : "FAIL";
      console.log(`${status} ${workload.id} ${result.artifactDir}`);
      if (result.status !== "passed") {
        failed += 1;
        for (const error of result.errors) console.error(`  ${error}`);
      }
    }
  } finally {
    await browser.close().catch(() => {});
    await server.close();
  }

  if (failed > 0) {
    process.exitCode = 1;
  } else if (results.length > 0) {
    console.log(`client perf artifacts: ${outputRoot}`);
  }
}

async function runWorkload({ workload, server, browser, outputRoot, args, chrome }) {
  const timestamp = timestampForPath(new Date());
  const artifactDir = path.join(outputRoot, workload.id, timestamp);
  fs.mkdirSync(artifactDir, { recursive: true });
  const consoleErrors = [];
  const pageErrors = [];
  const requestFailures = [];
  const errors = [];
  let tracePath = null;
  let summary = null;
  let snapshotCodecBakeoff = null;

  try {
    await prepareWorkload(workload);
    const page = await browser.newPage();
    if (args.snapshotCodecBakeoff) {
      await installSnapshotCodecCapture(page, args.snapshotCodecMaxSamples);
    }
    page.on("console", (message) => {
      const text = message.text();
      if (message.type() === "error") consoleErrors.push(text);
    });
    page.on("pageerror", (error) => pageErrors.push(error.message));
    page.on("requestfailed", (request) => {
      if (!request.url().includes("favicon")) {
        requestFailures.push(`${request.failure()?.errorText || "request failed"} ${request.url()}`);
      }
    });

    if (args.trace) {
      tracePath = path.join(artifactDir, "trace.json");
      await page.tracing.start({
        path: tracePath,
        screenshots: false,
        categories: ["devtools.timeline", "disabled-by-default-devtools.timeline"],
      });
    }

    await page.setViewport(args.viewport);
    const targetUrl = new URL(workload.url, server.baseUrl).href;
    const startedAt = new Date().toISOString();
    await page.goto(targetUrl, { waitUntil: "networkidle2", timeout: args.navTimeoutMs });
    await page.waitForFunction(
      () => !!window.__rts?.match && !!window.__rtsPerf?.summary,
      { timeout: args.startTimeoutMs },
    );
    await page.waitForSelector("#viewport canvas", { timeout: 5000 });
    await page.waitForFunction(
      () => (window.__rtsPerf?.summary?.()?.frameCount || 0) >= 30,
      { timeout: Math.max(args.durationMs, 1000) + 10000 },
    );
    await sleep(args.durationMs);

    summary = await collectPageSummary(page);
    if (args.snapshotCodecBakeoff) {
      const frames = await collectSnapshotCodecFrames(page);
      const framesPath = path.join(artifactDir, "snapshot-frames.jsonl");
      fs.writeFileSync(framesPath, frames.map((frame) => JSON.stringify(frame)).join("\n") + "\n");
      if (frames.length > 0) {
        const bakeoff = runSnapshotCodecBakeoff({
          frames,
          label: workload.id,
        });
        const summaryPath = path.join(artifactDir, "snapshot-codec-bakeoff.json");
        const markdownPath = path.join(artifactDir, "snapshot-codec-bakeoff.md");
        fs.writeFileSync(summaryPath, `${JSON.stringify(bakeoff, null, 2)}\n`);
        fs.writeFileSync(markdownPath, formatBakeoffMarkdown(bakeoff));
        snapshotCodecBakeoff = {
          samples: frames.length,
          framesJsonl: framesPath,
          summaryJson: summaryPath,
          markdown: markdownPath,
          recommendation: bakeoff.recommendation,
          candidates: bakeoff.candidates.map((candidate) => ({
            id: candidate.id,
            p95Bytes: candidate.bytes.p95,
            maxBytes: candidate.bytes.max,
            overBudgetPctX100: candidate.bytes.overBudgetPctX100,
            encodeP95Ms: candidate.encodeMs.p95,
            decodeP95Ms: candidate.decodeMs.p95,
          })),
        };
      } else {
        snapshotCodecBakeoff = {
          samples: 0,
          framesJsonl: framesPath,
          recommendation: {
            summary: "No snapshot frames were captured for this workload.",
            reason: "The page did not receive compact snapshot frames before collection ended.",
          },
          candidates: [],
        };
      }
    }
    const version = await fetchText(new URL("/version", server.baseUrl).href).catch((err) => ({
      error: err.message,
    }));
    const endedAt = new Date().toISOString();
    const artifact = {
      schemaVersion: 1,
      status: "passed",
      workload: {
        id: workload.id,
        kind: workload.kind,
        description: workload.description,
        url: workload.url,
        replayName: workload.replayName || null,
      },
      run: {
        startedAt,
        endedAt,
        durationMs: args.durationMs,
        targetUrl,
        baseUrl: server.baseUrl,
        reusedServer: server.reused,
      },
      browser: {
        chrome,
        viewport: args.viewport,
        userAgent: summary.userAgent,
        devicePixelRatio: summary.devicePixelRatio,
      },
      build: version,
      websocket: summary.websocket,
      health: summary.health,
      perf: summary.perf,
      clientNetReport: summary.clientNetReport,
      snapshotPacketBudget: snapshotPacketBudgetSummary(summary.clientNetReport),
      snapshotCodecBakeoff,
      page: {
        title: summary.title,
        location: summary.location,
        canvas: summary.canvas,
        consoleErrors,
        pageErrors,
        requestFailures,
      },
      artifacts: {
        summaryJson: path.join(artifactDir, "summary.json"),
        traceJson: tracePath,
      },
      notes: [
        "This harness fails on runtime errors and missing summaries, not absolute FPS thresholds.",
        "Numbers are machine-local evidence for optimization work.",
      ],
    };

    if (!summary.perf?.summary || summary.perf.summary.frameCount <= 0) {
      errors.push("window.__rtsPerf.summary() was missing or empty");
    }
    if (!summary.clientNetReport) {
      errors.push("ClientNetReport snapshot could not be generated");
    }
    errors.push(...consoleErrors.map((error) => `console error: ${error}`));
    errors.push(...pageErrors.map((error) => `page error: ${error}`));
    errors.push(...requestFailures.map((error) => `request failure: ${error}`));

    if (args.trace) await page.tracing.stop();
    await page.close().catch(() => {});

    if (errors.length > 0) artifact.status = "failed";
    fs.writeFileSync(path.join(artifactDir, "summary.json"), `${JSON.stringify(artifact, null, 2)}\n`);
    return {
      status: artifact.status,
      artifactDir,
      errors,
      frameCount: summary.perf?.summary?.frameCount || 0,
      frameWorkP95Ms: summary.perf?.reportSummary?.frameWorkP95Ms || 0,
      rendererP95Ms: summary.perf?.reportSummary?.rendererP95Ms || 0,
    };
  } catch (err) {
    errors.push(err.stack || err.message);
    try {
      if (args.trace) await browser.pages().then((pages) => pages.at(-1)?.tracing?.stop?.()).catch(() => {});
    } catch {
      // Best effort cleanup only.
    }
    const artifact = {
      schemaVersion: 1,
      status: "failed",
      workload: { id: workload.id, kind: workload.kind, description: workload.description },
      errors,
      partialSummary: summary,
      page: { consoleErrors, pageErrors, requestFailures },
      artifacts: { summaryJson: path.join(artifactDir, "summary.json"), traceJson: tracePath },
    };
    fs.writeFileSync(path.join(artifactDir, "summary.json"), `${JSON.stringify(artifact, null, 2)}\n`);
    return { status: "failed", artifactDir, errors };
  }
}

function snapshotPacketBudgetSummary(report) {
  if (!report) return null;
  return {
    snapshotBytesP95: numberOrNull(report.snapshotBytesP95),
    snapshotSegmentBudgetBytes: numberOrNull(report.snapshotSegmentBudgetBytes),
    snapshotOverSegmentBudgetCount: numberOrNull(report.snapshotOverSegmentBudgetCount),
    snapshotOverSegmentBudgetPctX100: numberOrNull(report.snapshotOverSegmentBudgetPctX100),
    snapshotByteSource: stringOrNull(report.snapshotByteSource),
    websocketCompression: stringOrNull(report.websocketCompression),
    websocketExtensions: stringOrNull(report.websocketExtensions),
  };
}

function numberOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function stringOrNull(value) {
  return typeof value === "string" ? value : null;
}

async function collectPageSummary(page) {
  return page.evaluate(() => {
    const app = window.__rts || null;
    const match = app?.match || null;
    const health = match?.health || null;
    const healthSnapshot = {
      metrics: health?.metrics?.() || null,
      reportStats: health?.reportStats ? JSON.parse(JSON.stringify(health.reportStats)) : null,
      reportStartedAt: health?.reportStartedAt || null,
    };
    const perfSnapshot = {
      summary: window.__rtsPerf?.summary?.() || null,
      reportSummary: window.__rtsPerf?.reportSummary?.() || null,
    };
    let clientNetReport = null;
    if (match && typeof match.sendNetReport === "function" && match.net) {
      const original = match.net.netReport;
      try {
        match.net.netReport = (report) => {
          clientNetReport = JSON.parse(JSON.stringify(report));
        };
        match.sendNetReport();
      } finally {
        match.net.netReport = original;
      }
    }
    const websocketExtensions = typeof match?.net?.ws?.extensions === "string" ? match.net.ws.extensions : "";
    const websocketCompression = websocketExtensions
      .toLowerCase()
      .split(",")
      .map((part) => part.trim().split(";")[0]?.trim())
      .includes("permessage-deflate")
      ? "permessage-deflate"
      : "none";
    const canvas = document.querySelector("#viewport canvas");
    return {
      title: document.title,
      location: window.location.href,
      userAgent: navigator.userAgent,
      devicePixelRatio: window.devicePixelRatio,
      websocket: {
        extensions: websocketExtensions,
        compression: websocketCompression,
        compressionNegotiated: websocketCompression === "permessage-deflate",
        bufferedAmount: match?.net?.bufferedAmount || 0,
      },
      canvas: canvas ? { width: canvas.width, height: canvas.height } : null,
      health: healthSnapshot,
      perf: perfSnapshot,
      clientNetReport,
    };
  });
}

async function prepareWorkload(workload) {
  if (workload.kind !== "replayArtifact") return;
  const targetDir = path.join(SERVER_DIR, "target", "selfplay-artifacts", workload.replayName);
  fs.mkdirSync(targetDir, { recursive: true });
  fs.copyFileSync(workload.source, path.join(targetDir, "replay.json"));
}

async function startOrReuseServer(args) {
  const fromEnv = args.baseUrl || process.env.RTS_URL;
  if (fromEnv && await isHealthy(fromEnv)) {
    return {
      baseUrl: normalizeBaseUrl(fromEnv),
      reused: true,
      close: async () => {},
    };
  }

  const port = args.port || await allocatePort();
  const baseUrl = `http://127.0.0.1:${port}/`;
  const targetDir = cargoTargetDir();
  const serverBin = process.env.RTS_SERVER_BIN || path.join(targetDir, "debug", "rts-server");
  if (!fs.existsSync(serverBin)) {
    runOrThrow("cargo", ["build", "--manifest-path", path.join(SERVER_DIR, "Cargo.toml")], {
      cwd: REPO_ROOT,
      env: { ...process.env, CARGO_TARGET_DIR: targetDir },
      stdio: "inherit",
    });
  }
  if (!fs.existsSync(serverBin)) {
    throw new Error(`server binary not found at ${serverBin}`);
  }

  const logPath = path.join(os.tmpdir(), `rts-client-perf-server-${process.pid}.log`);
  const log = fs.openSync(logPath, "w");
  const child = spawn(serverBin, [], {
    cwd: SERVER_DIR,
    env: {
      ...process.env,
      RTS_ADDR: `127.0.0.1:${port}`,
      RTS_TEST_TICK_MS: process.env.RTS_TEST_TICK_MS || "5",
      RTS_MATCH_SEED: process.env.RTS_MATCH_SEED || "1",
    },
    stdio: ["ignore", log, log],
  });
  child.on("exit", () => fs.closeSync(log));

  const deadline = Date.now() + args.serverTimeoutMs;
  while (Date.now() < deadline) {
    if (child.exitCode != null) {
      throw new Error(`server exited during startup; see ${logPath}`);
    }
    if (await isHealthy(baseUrl)) {
      return {
        baseUrl,
        reused: false,
        logPath,
        close: async () => stopChild(child),
      };
    }
    await sleep(250);
  }
  await stopChild(child);
  throw new Error(`server did not become healthy within ${args.serverTimeoutMs}ms; see ${logPath}`);
}

async function launchBrowser(puppeteer, chrome, args) {
  const profileDir = fs.mkdtempSync(path.join(os.tmpdir(), "rts-client-perf-chrome-"));
  return puppeteer.launch({
    executablePath: chrome,
    headless: "new",
    defaultViewport: args.viewport,
    args: [
      "--no-sandbox",
      `--window-size=${args.viewport.width},${args.viewport.height}`,
      `--user-data-dir=${profileDir}`,
    ],
  });
}

async function installSnapshotCodecCapture(page, maxSamples) {
  await page.evaluateOnNewDocument((limit) => {
    const NativeWebSocket = window.WebSocket;
    const frames = [];
    window.__rtsSnapshotCodecCapture = { frames, limit };

    class SnapshotCaptureWebSocket extends NativeWebSocket {
      constructor(...args) {
        super(...args);
        this.addEventListener("message", (event) => {
          if (frames.length >= limit || typeof event.data !== "string") return;
          if (!event.data.startsWith("{\"t\":\"snapshot\"")) return;
          frames.push(event.data);
        });
      }
    }

    for (const key of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      Object.defineProperty(SnapshotCaptureWebSocket, key, { value: NativeWebSocket[key] });
    }
    window.WebSocket = SnapshotCaptureWebSocket;
  }, maxSamples);
}

async function collectSnapshotCodecFrames(page) {
  return page.evaluate(() => window.__rtsSnapshotCodecCapture?.frames || []);
}

async function loadPuppeteer() {
  ensureTestNodeModules();
  const requireFromTests = createRequire(path.join(TESTS_DIR, "package.json"));
  const resolved = requireFromTests.resolve("puppeteer-core");
  const imported = await import(pathToFileURL(resolved).href);
  return imported.default || imported;
}

function ensureTestNodeModules() {
  const packageLock = path.join(TESTS_DIR, "package-lock.json");
  const packageJson = path.join(TESTS_DIR, "package.json");
  const localNodeModules = path.join(TESTS_DIR, "node_modules");
  const localPuppeteer = path.join(localNodeModules, "puppeteer-core");
  if (fs.existsSync(localPuppeteer)) return;
  const cacheRoot = process.env.RTS_NODE_DEPS_CACHE_DIR || "/tmp/rts-node-deps";
  const hash = crypto.createHash("sha256").update(fs.readFileSync(packageLock)).digest("hex");
  const cacheNodeModules = path.join(cacheRoot, hash, "node_modules");
  if (fs.existsSync(path.join(cacheNodeModules, "puppeteer-core"))) {
    if (fs.existsSync(localNodeModules)) fs.rmSync(localNodeModules, { recursive: true, force: true });
    fs.symlinkSync(cacheNodeModules, localNodeModules, "dir");
    return;
  }
  runOrThrow("npm", ["ci", "--ignore-scripts", "--no-audit", "--fund=false"], {
    cwd: TESTS_DIR,
    stdio: "inherit",
  });
  if (!fs.existsSync(localPuppeteer)) {
    throw new Error(`puppeteer-core was not installed from ${packageJson}`);
  }
}

function selectedWorkloads(args) {
  if (args.workloads.length === 0) return WORKLOADS;
  const byId = new Map(WORKLOADS.map((workload) => [workload.id, workload]));
  return args.workloads.map((id) => {
    const workload = byId.get(id);
    if (!workload) throw new Error(`unknown workload ${id}; run --list`);
    return workload;
  });
}

function parseArgs(argv) {
  const args = {
    list: false,
    workloads: [],
    durationMs: DEFAULT_DURATION_MS,
    outputRoot: DEFAULT_OUTPUT_ROOT,
    viewport: { ...DEFAULT_VIEWPORT },
    chrome: process.env.CHROME || "",
    baseUrl: "",
    port: 0,
    trace: false,
    snapshotCodecBakeoff: false,
    snapshotCodecMaxSamples: DEFAULT_CODEC_SAMPLE_LIMIT,
    navTimeoutMs: 15000,
    startTimeoutMs: 20000,
    serverTimeoutMs: 30000,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const value = () => {
      i += 1;
      if (i >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[i];
    };
    if (arg === "--list") args.list = true;
    else if (arg === "--trace") args.trace = true;
    else if (arg === "--snapshot-codec-bakeoff") args.snapshotCodecBakeoff = true;
    else if (arg === "--snapshot-codec-max-samples") args.snapshotCodecMaxSamples = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--snapshot-codec-max-samples=")) args.snapshotCodecMaxSamples = parsePositiveInt(arg.slice("--snapshot-codec-max-samples=".length), "--snapshot-codec-max-samples");
    else if (arg === "--workload") args.workloads.push(value());
    else if (arg.startsWith("--workload=")) args.workloads.push(arg.slice("--workload=".length));
    else if (arg === "--duration-ms") args.durationMs = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--duration-ms=")) args.durationMs = parsePositiveInt(arg.slice("--duration-ms=".length), "--duration-ms");
    else if (arg === "--seconds") args.durationMs = parsePositiveInt(value(), arg) * 1000;
    else if (arg.startsWith("--seconds=")) args.durationMs = parsePositiveInt(arg.slice("--seconds=".length), "--seconds") * 1000;
    else if (arg === "--output-root") args.outputRoot = value();
    else if (arg.startsWith("--output-root=")) args.outputRoot = arg.slice("--output-root=".length);
    else if (arg === "--chrome") args.chrome = value();
    else if (arg.startsWith("--chrome=")) args.chrome = arg.slice("--chrome=".length);
    else if (arg === "--base-url") args.baseUrl = value();
    else if (arg.startsWith("--base-url=")) args.baseUrl = arg.slice("--base-url=".length);
    else if (arg === "--port") args.port = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--port=")) args.port = parsePositiveInt(arg.slice("--port=".length), "--port");
    else if (arg === "--viewport") args.viewport = parseViewport(value());
    else if (arg.startsWith("--viewport=")) args.viewport = parseViewport(arg.slice("--viewport=".length));
    else if (arg === "-h" || arg === "--help") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown arg: ${arg}`);
    }
  }
  return args;
}

function printHelp() {
  console.log(`Usage: node scripts/client-perf-harness.mjs [options]

Options:
  --list                         List available workloads.
  --workload <id>                Run one workload; repeatable. Defaults to all workloads.
  --seconds <n>                  Browser collection time per workload. Default: ${DEFAULT_DURATION_MS / 1000}.
  --duration-ms <n>              Browser collection time per workload in milliseconds.
  --output-root <path>           Artifact root. Default: target/client-perf.
  --trace                        Also write a Chrome trace.json per workload.
  --snapshot-codec-bakeoff       Capture local raw snapshot frames and write codec bake-off artifacts.
  --snapshot-codec-max-samples <n> Maximum snapshot frames captured per workload. Default: ${DEFAULT_CODEC_SAMPLE_LIMIT}.
  --base-url <url>               Reuse an already-running server when healthy.
  --port <n>                     Port for a harness-started server.
  --chrome <path>                Chrome/Chromium executable. Defaults to CHROME or common paths.
  --viewport <width>x<height>    Browser viewport. Default: ${DEFAULT_VIEWPORT.width}x${DEFAULT_VIEWPORT.height}.
`);
}

function parsePositiveInt(raw, label) {
  const value = Number(raw);
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

function parseViewport(raw) {
  const match = /^([1-9][0-9]*)x([1-9][0-9]*)$/.exec(raw);
  if (!match) throw new Error("--viewport must look like 1440x900");
  return { width: Number(match[1]), height: Number(match[2]) };
}

function findChrome(explicit) {
  const candidates = [
    explicit,
    DEFAULT_CHROME,
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    which("google-chrome-stable"),
    which("google-chrome"),
    which("chromium-browser"),
    which("chromium"),
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error("Chrome/Chromium not found; set CHROME=/path/to/chrome or pass --chrome");
}

function which(command) {
  const result = spawnSync("which", [command], { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
}

function cargoTargetDir() {
  if (process.env.CARGO_TARGET_DIR) return process.env.CARGO_TARGET_DIR;
  const script = path.join(REPO_ROOT, "scripts", "cargo-shared-target.sh");
  const result = spawnSync(script, ["--print-target-dir"], { encoding: "utf8" });
  if (result.status === 0 && result.stdout.trim()) return result.stdout.trim();
  return path.join(SERVER_DIR, "target");
}

async function allocatePort() {
  const net = await import("node:net");
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
  });
}

async function isHealthy(baseUrl) {
  try {
    const response = await fetch(normalizeBaseUrl(baseUrl), { signal: AbortSignal.timeout(1500) });
    return response.ok;
  } catch {
    return false;
  }
}

async function fetchText(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(2500) });
  if (!response.ok) throw new Error(`${url} returned HTTP ${response.status}`);
  return response.text();
}

function normalizeBaseUrl(raw) {
  const url = new URL(raw);
  url.pathname = url.pathname.endsWith("/") ? url.pathname : `${url.pathname}/`;
  return url.href;
}

function runOrThrow(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit ${result.status}`);
  }
  return result;
}

async function stopChild(child) {
  if (child.exitCode != null) return;
  child.kill("SIGTERM");
  const exited = await Promise.race([
    new Promise((resolve) => child.once("exit", () => resolve(true))),
    sleep(3000).then(() => false),
  ]);
  if (!exited) child.kill("SIGKILL");
}

function timestampForPath(date) {
  return date.toISOString().replace(/[:.]/g, "-");
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((err) => {
  console.error(err.stack || err.message);
  process.exit(1);
});
