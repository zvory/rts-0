// Transport-independent local driver for the Agent Lab browser session.
// MCP transport is intentionally a later concern; this module owns only the selected
// worktree, private processes, narrow page bridge, and bounded local diagnostics.

import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const DEFAULT_VIEWPORT = Object.freeze({ width: 1440, height: 900, deviceScaleFactor: 1 });
const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_STARTUP_TIMEOUT_MS = 60_000;
const MAX_TIMEOUT_MS = 60_000;
const MAX_STARTUP_TIMEOUT_MS = 120_000;
const LOG_TAIL_LINES = 80;
const MAX_PAGE_ERRORS = 80;
const MAX_CAPTURE_BYTES = 16 * 1024 * 1024;
const MAX_CAPTURE_VIEWPORT = 2048;
const AGENT_LAB_ROOT = path.join("target", "agent-lab");

export const DRIVER_STATES = Object.freeze({
  OPENING: "opening",
  OPEN: "open",
  CLOSING: "closing",
  CLOSED: "closed",
});

export class AgentLabDriverError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = "AgentLabDriverError";
    this.code = code;
    this.details = details;
  }
}

export class AgentLabDriver {
  static async open(options = {}) {
    const driver = new AgentLabDriver(options);
    try {
      await driver.open();
      return driver;
    } catch (error) {
      await driver.close().catch(() => {});
      throw driver.decorateError(error);
    }
  }

  constructor({
    workspaceRoot,
    map = "Default",
    seed = "",
    scenario = "blank",
    viewport = DEFAULT_VIEWPORT,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    startupTimeoutMs = DEFAULT_STARTUP_TIMEOUT_MS,
    chrome = process.env.CHROME || "",
    baseUrl = "",
  } = {}) {
    this.options = {
      workspaceRoot,
      map,
      seed,
      scenario,
      viewport,
      timeoutMs: boundedTimeout(timeoutMs, "timeoutMs", MAX_TIMEOUT_MS),
      startupTimeoutMs: boundedTimeout(startupTimeoutMs, "startupTimeoutMs", MAX_STARTUP_TIMEOUT_MS),
      chrome,
      baseUrl,
    };
    this.state = DRIVER_STATES.OPENING;
    this.workspace = null;
    this.sessionDir = "";
    this.server = null;
    this.serverLogPath = "";
    this.browser = null;
    this.browserVersion = "";
    this.page = null;
    this.profileDir = "";
    this.pageConsoleErrors = [];
    this.pageErrors = [];
    this.requestFailures = [];
    this.operationTail = Promise.resolve();
    this.closePromise = null;
    this.signalHandlers = [];
    this.openStarted = false;
  }

  async open() {
    if (this.openStarted || this.state !== DRIVER_STATES.OPENING) {
      throw new AgentLabDriverError("invalidLifecycle", "Agent Lab driver can only be opened once.");
    }
    this.openStarted = true;
    this.workspace = validateWorkspaceRoot(this.options.workspaceRoot);
    this.sessionDir = createSessionDirectory(this.workspace.root, this.options.map);
    this.writeManifest({ status: DRIVER_STATES.OPENING });
    this.installCleanupHandlers();
    this.server = await startOrReusePrivateServer({
      workspace: this.workspace,
      sessionDir: this.sessionDir,
      startupTimeoutMs: this.options.startupTimeoutMs,
      baseUrl: this.options.baseUrl,
      isOpening: () => this.state === DRIVER_STATES.OPENING,
    });
    this.serverLogPath = this.server.logPath || "";

    const puppeteer = await loadPuppeteer(this.workspace.root);
    if (this.state !== DRIVER_STATES.OPENING) {
      throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during browser startup.");
    }
    const chrome = findChrome(this.options.chrome);
    this.profileDir = fs.mkdtempSync(path.join(this.sessionDir, "chrome-profile-"));
    const browser = await puppeteer.launch({
      executablePath: chrome,
      headless: "new",
      defaultViewport: normalizeViewport(this.options.viewport),
      args: [
        "--no-sandbox",
        "--disable-features=PointerLockOptions",
        `--window-size=${this.options.viewport.width},${this.options.viewport.height}`,
        `--user-data-dir=${this.profileDir}`,
      ],
    });
    if (this.state !== DRIVER_STATES.OPENING) {
      await browser.close().catch(() => {});
      throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during browser startup.");
    }
    this.browser = browser;
    this.browserVersion = await browser.version().catch(() => "");
    const page = await browser.newPage();
    if (this.state !== DRIVER_STATES.OPENING) {
      await page.close().catch(() => {});
      throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during page startup.");
    }
    this.page = page;
    this.attachPageDiagnostics();
    await this.page.goto(this.launchUrl(), { waitUntil: "domcontentloaded", timeout: this.options.startupTimeoutMs });
    await this.page.waitForFunction(
      () => window.__rtsAgentLab?.status?.().ready === true,
      { timeout: this.options.startupTimeoutMs },
    );
    if (this.state !== DRIVER_STATES.OPENING) {
      throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during page startup.");
    }
    if (this.pageErrors.length > 0) {
      throw new AgentLabDriverError("pageError", "Agent Lab page reported an error before readiness.");
    }
    this.transition("opened");
    this.writeManifest({
      status: this.state,
      baseUrl: this.server.baseUrl,
      reusedServer: this.server.reused,
      browser: { chrome, viewport: normalizeViewport(this.options.viewport) },
      ready: await this.status(),
    });
  }

  async status() {
    const status = await this.call("status", {});
    return this.pageErrors.length === 0
      ? status
      : { ...status, ready: false, reason: "pageError" };
  }

  async catalog(query = {}) {
    return this.call("catalog", query);
  }

  async spawn(spec) {
    return this.call("spawn", spec);
  }

  async update(operation) {
    return this.call("update", operation);
  }

  async remove(entityIds) {
    return this.call("remove", { entityIds });
  }

  async order({ playerId, command, ignoreCommandLimits = false }) {
    return this.call("order", { playerId, command, ignoreCommandLimits });
  }

  async time(control) {
    return this.call("time", control);
  }

  async inspect(query = {}) {
    return this.call("inspect", query);
  }

  async camera(command) {
    return this.call("camera", command);
  }

  async reset() {
    return this.call("reset", {});
  }

  async screenshot({
    sessionId,
    name = "scene",
    presentation = "clean",
    viewport = null,
    subjectIds = [],
    subjectSummaries = [],
    request = {},
  } = {}) {
    return this.enqueue(() => this.captureScreenshot({
      sessionId,
      name,
      presentation,
      viewport,
      subjectIds,
      subjectSummaries,
      request,
    }));
  }

  diagnostics() {
    return {
      sessionDir: this.sessionDir,
      serverLog: this.server?.logPath || this.serverLogPath || null,
      pageConsoleErrors: [...this.pageConsoleErrors],
      pageErrors: [...this.pageErrors],
      requestFailures: [...this.requestFailures],
    };
  }

  async call(method, input) {
    return this.enqueue(() => this.callBridge(method, input));
  }

  async callBridge(method, input) {
    if (this.state !== DRIVER_STATES.OPEN || !this.page) {
      throw new AgentLabDriverError("sessionClosed", "Agent Lab driver session is not open.");
    }
    const result = await withTimeout(
      this.page.evaluate(
        ({ method: bridgeMethod, input: bridgeInput }) => window.__rtsAgentLab.call(bridgeMethod, bridgeInput),
        { method, input },
      ),
      this.options.timeoutMs,
      `Agent Lab ${method}`,
    );
    if (!result?.ok) {
      throw new AgentLabDriverError(
        result?.error?.code || "bridgeError",
        result?.error?.message || `Agent Lab ${method} failed.`,
        { method },
      );
    }
    return result.value;
  }

  async captureScreenshot({ sessionId, name, presentation, viewport, subjectIds, subjectSummaries, request }) {
    if (presentation !== "clean" && presentation !== "normal") {
      throw new AgentLabDriverError("invalidPresentation", "presentation must be clean or normal.");
    }
    const normalizedSessionId = safeCaptureSessionId(sessionId);
    const artifactName = safeArtifactName(name);
    const normalizedViewport = viewport ? normalizeCaptureViewport(viewport) : null;
    const originalViewport = this.page.viewport?.() || null;
    const requestedSubjectIds = boundedEntityIds(subjectIds);
    try {
      if (normalizedViewport) await this.page.setViewport(normalizedViewport);
      await this.callBridge("presentation", { mode: presentation === "clean" ? "clean" : "default" });
      await this.page.evaluate(() => document.fonts?.ready || Promise.resolve());
      const readiness = await this.waitForCaptureReadiness(requestedSubjectIds);
      const clip = await this.page.evaluate(() => {
        const viewportEl = document.getElementById("viewport");
        const rect = viewportEl?.getBoundingClientRect?.();
        return rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null;
      });
      if (!validClip(clip)) throw new AgentLabDriverError("viewportUnavailable", "The Pixi viewport is not available for capture.");

      const captureDir = path.join(this.workspace.root, AGENT_LAB_ROOT, normalizedSessionId, "captures");
      fs.mkdirSync(captureDir, { recursive: true });
      const suffix = new Date().toISOString().replace(/[:.]/g, "-");
      const baseName = `${artifactName}-${suffix}`;
      const pngPath = path.join(captureDir, `${baseName}.png`);
      const manifestPath = path.join(captureDir, `${baseName}.json`);
      const screenshot = await this.page.screenshot({ type: "png", clip, path: pngPath });
      const png = Buffer.from(screenshot || []);
      if (png.length === 0) {
        throw new AgentLabDriverError("captureEmpty", "Chrome returned an empty Pixi screenshot.");
      }
      if (png.length > MAX_CAPTURE_BYTES) {
        fs.rmSync(pngPath, { force: true });
        throw new AgentLabDriverError("captureTooLarge", `Screenshot exceeds the ${MAX_CAPTURE_BYTES} byte response bound.`);
      }
      const dimensions = readPngDimensions(png);
      const diagnostics = this.diagnostics();
      const manifest = {
        schemaVersion: 1,
        createdAt: new Date().toISOString(),
        workspace: this.workspace,
        serverBuild: this.server?.build || { reused: true, baseUrl: this.server?.baseUrl || null },
        url: this.launchUrl(),
        map: this.options.map,
        scenario: this.options.scenario,
        seed: this.options.seed || null,
        authoritative: {
          tick: readiness.snapshotTick,
          roomTime: readiness.roomTime,
        },
        viewport: {
          requested: normalizedViewport,
          clip,
          output: dimensions,
        },
        camera: readiness.camera,
        subjects: Array.isArray(subjectSummaries) ? subjectSummaries.slice(0, 20) : [],
        visualProfileId: readiness.visualProfileId || null,
        assetReadiness: readiness.assets,
        errors: {
          pageConsole: diagnostics.pageConsoleErrors,
          page: diagnostics.pageErrors,
          requestFailures: diagnostics.requestFailures,
          frame: readiness.frameErrors,
          render: readiness.renderErrors,
          missingTextureSubjectIds: readiness.missingTextureSubjectIds,
        },
        presentation,
        request: boundedRequestMetadata(request),
        browser: {
          chrome: this.browserVersion || null,
          puppeteer: await this.page.evaluate(() => navigator.userAgent),
        },
      };
      fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
      return {
        pngPath,
        manifestPath,
        image: {
          mimeType: "image/png",
          data: png.toString("base64"),
          bytes: png.length,
          width: dimensions.width,
          height: dimensions.height,
        },
        presentation,
        readiness,
      };
    } finally {
      if (originalViewport) await this.page.setViewport(originalViewport).catch(() => {});
      await this.callBridge("presentation", { mode: "default" }).catch(() => {});
    }
  }

  async waitForCaptureReadiness(subjectIds) {
    const deadline = Date.now() + this.options.timeoutMs;
    let initialFrame = null;
    let last = null;
    while (Date.now() < deadline) {
      const readiness = await this.callBridge("captureReadiness", { subjectIds });
      if (initialFrame == null) initialFrame = Number(readiness.frame) || 0;
      last = readiness;
      const errors = readiness.frameErrors?.length || readiness.renderErrors?.length ||
        readiness.missingTextureSubjectIds?.length || this.pageErrors.length || this.pageConsoleErrors.length;
      if (errors) throw new AgentLabDriverError("captureRenderError", captureReadinessMessage(readiness, this.diagnostics()));
      if (readiness.failedAssets?.length) {
        throw new AgentLabDriverError("assetLoadFailed", captureReadinessMessage(readiness, this.diagnostics()));
      }
      if (readiness.ready && Number(readiness.frame) >= initialFrame + 2) return readiness;
      await sleep(25);
    }
    throw new AgentLabDriverError("captureTimeout", captureReadinessMessage(last, this.diagnostics()));
  }

  enqueue(operation) {
    const run = this.operationTail.then(operation, operation);
    this.operationTail = run.catch(() => {});
    return run.catch((error) => { throw this.decorateError(error); });
  }

  launchUrl() {
    const url = new URL("/lab", this.server.baseUrl);
    url.searchParams.set("room", generatedRoomId(this.workspace.head));
    url.searchParams.set("map", safeToken(this.options.map, "Default", 48));
    if (this.options.seed !== "" && this.options.seed != null) url.searchParams.set("seed", String(this.options.seed));
    if (this.options.scenario) url.searchParams.set("scenario", safeToken(this.options.scenario, "blank", 48));
    url.searchParams.set("agentLab", "1");
    url.searchParams.set("rtsNoAutoPointerLock", "1");
    return url.href;
  }

  attachPageDiagnostics() {
    this.page.on("console", (message) => {
      if (message.type() === "error") appendBounded(this.pageConsoleErrors, message.text());
    });
    this.page.on("pageerror", (error) => appendBounded(this.pageErrors, error.message));
    this.page.on("requestfailed", (request) => {
      if (!request.url().includes("favicon")) {
        appendBounded(this.requestFailures, `${request.failure()?.errorText || "request failed"} ${request.url()}`);
      }
    });
  }

  installCleanupHandlers() {
    const closeOnSignal = () => { void this.close(); };
    for (const signal of ["SIGINT", "SIGTERM"]) {
      process.once(signal, closeOnSignal);
      this.signalHandlers.push([signal, closeOnSignal]);
    }
    const closeOnException = (error) => {
      // `uncaughtExceptionMonitor` cannot wait for async teardown. Hold the fatal exception
      // until the browser and private server have stopped, then restore normal process failure.
      void this.close().finally(() => {
        process.nextTick(() => { throw error; });
      });
    };
    process.once("uncaughtException", closeOnException);
    this.signalHandlers.push(["uncaughtException", closeOnException]);
  }

  removeCleanupHandlers() {
    for (const [event, handler] of this.signalHandlers) process.removeListener(event, handler);
    this.signalHandlers = [];
  }

  async close() {
    if (this.closePromise) return this.closePromise;
    this.closePromise = (async () => {
      if (this.state === DRIVER_STATES.CLOSED) return;
      if (this.state !== DRIVER_STATES.CLOSING) this.transition("closing");
      this.removeCleanupHandlers();
      await this.page?.close().catch(() => {});
      await this.browser?.close().catch(() => {});
      await this.server?.close?.().catch(() => {});
      if (this.profileDir) fs.rmSync(this.profileDir, { recursive: true, force: true });
      this.page = null;
      this.browser = null;
      this.server = null;
      this.transition("closed");
      this.writeManifest({ status: this.state, diagnostics: this.diagnostics() });
    })();
    return this.closePromise;
  }

  transition(event) {
    this.state = transitionDriverState(this.state, event);
  }

  writeManifest(extra) {
    if (!this.sessionDir) return;
    const manifest = {
      schemaVersion: 1,
      workspace: this.workspace ? {
        root: this.workspace.root,
        branch: this.workspace.branch,
        head: this.workspace.head,
      } : null,
      session: {
        state: this.state,
        createdAt: new Date().toISOString(),
      },
      ...extra,
    };
    fs.writeFileSync(path.join(this.sessionDir, "session.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  }

  decorateError(error) {
    if (error instanceof AgentLabDriverError && error.details?.diagnostics) return error;
    const diagnostics = this.diagnostics();
    const serverTail = diagnostics.serverLog ? readTail(diagnostics.serverLog, LOG_TAIL_LINES) : [];
    const message = [error?.message || "Agent Lab driver failed."]
      .concat(serverTail.length ? [`Server log tail:\n${serverTail.join("\n")}`] : [])
      .join("\n");
    return new AgentLabDriverError(error?.code || "driverError", message, {
      ...error?.details,
      diagnostics: { ...diagnostics, serverTail },
    });
  }
}

export function validateWorkspaceRoot(workspaceRoot) {
  if (!workspaceRoot) throw new AgentLabDriverError("workspaceRequired", "workspaceRoot is required.");
  let root;
  try {
    root = fs.realpathSync(workspaceRoot);
  } catch {
    throw new AgentLabDriverError("invalidWorkspace", `Workspace does not exist: ${workspaceRoot}`);
  }
  if (!fs.existsSync(path.join(root, "server", "Cargo.toml")) || !fs.existsSync(path.join(root, "client", "src", "main.js"))) {
    throw new AgentLabDriverError("invalidWorkspace", "workspaceRoot is not a Bewegungskrieg checkout.");
  }
  const topLevel = git(root, ["rev-parse", "--show-toplevel"]);
  if (!topLevel || fs.realpathSync(topLevel) !== root) {
    throw new AgentLabDriverError("invalidWorkspace", "workspaceRoot must be the Git checkout top level.");
  }
  const head = git(root, ["rev-parse", "HEAD"]);
  if (!/^[0-9a-f]{40}$/i.test(head || "")) {
    throw new AgentLabDriverError("invalidWorkspace", "workspaceRoot has no valid Git HEAD.");
  }
  return {
    root,
    branch: git(root, ["branch", "--show-current"]) || "HEAD",
    head,
  };
}

export function safeToken(value, fallback = "session", maxLength = 64) {
  const token = String(value || "").trim();
  return /^[A-Za-z0-9_-]+$/.test(token) && token.length <= maxLength ? token : fallback;
}

export function safeArtifactName(value, fallback = "scene") {
  return safeToken(value, fallback, 48);
}

function safeCaptureSessionId(value) {
  const sessionId = String(value || "").trim();
  if (!/^lab_[a-f0-9]{32}$/.test(sessionId)) {
    throw new AgentLabDriverError("invalidSession", "sessionId must be a valid Agent Lab session id.");
  }
  return sessionId;
}

function normalizeCaptureViewport(viewport) {
  const normalized = normalizeViewport(viewport);
  if (normalized.width > MAX_CAPTURE_VIEWPORT || normalized.height > MAX_CAPTURE_VIEWPORT) {
    throw new AgentLabDriverError("invalidViewport", `capture viewport width and height must be at most ${MAX_CAPTURE_VIEWPORT}.`);
  }
  return normalized;
}

function boundedEntityIds(values) {
  if (!Array.isArray(values) || values.length > 20) {
    throw new AgentLabDriverError("invalidSubjects", "subjectIds must contain at most 20 positive entity ids.");
  }
  const ids = [...new Set(values.map(Number))];
  if (!ids.every((id) => Number.isInteger(id) && id > 0)) {
    throw new AgentLabDriverError("invalidSubjects", "subjectIds must contain positive integer entity ids.");
  }
  return ids;
}

function validClip(clip) {
  return Number.isFinite(clip?.x) && Number.isFinite(clip?.y) &&
    Number.isFinite(clip?.width) && Number.isFinite(clip?.height) &&
    clip.width >= 1 && clip.height >= 1 &&
    clip.width <= MAX_CAPTURE_VIEWPORT && clip.height <= MAX_CAPTURE_VIEWPORT;
}

function boundedRequestMetadata(request) {
  const text = JSON.stringify(request && typeof request === "object" ? request : {});
  if (text.length > 4000) return { truncated: true };
  return JSON.parse(text);
}

function captureReadinessMessage(readiness, diagnostics) {
  const failures = [
    ...(readiness?.failedAssets || []).map((asset) => `${asset.id}: ${asset.message || "failed"}`),
    ...(readiness?.pendingAssets || []).map((asset) => `${asset.id}: pending`),
    ...(readiness?.frameErrors || []).map((error) => `frame: ${error.message || "failed"}`),
    ...(readiness?.renderErrors || []).map((error) => `render ${error.label}: ${error.message || "failed"}`),
    ...(readiness?.missingTextureSubjectIds || []).map((id) => `subject ${id}: missing texture fallback`),
    ...(diagnostics?.pageErrors || []).map((error) => `page: ${error}`),
    ...(diagnostics?.pageConsoleErrors || []).map((error) => `console: ${error}`),
  ];
  return failures.length ? `Screenshot readiness failed: ${failures.slice(0, 12).join("; ")}` : "Screenshot did not become ready before the timeout.";
}

function readPngDimensions(buffer) {
  if (buffer.length < 24 || buffer.toString("ascii", 1, 4) !== "PNG") {
    throw new AgentLabDriverError("invalidCapture", "Chrome did not return a PNG image.");
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

export function generatedRoomId(head = "") {
  const suffix = crypto.randomBytes(6).toString("hex");
  return safeToken(`agentlab-${safeToken(head.slice(0, 8), "head", 8)}-${process.pid}-${suffix}`, "agentlab", 40);
}

export function transitionDriverState(state, event) {
  const next = {
    [DRIVER_STATES.OPENING]: { opened: DRIVER_STATES.OPEN, closing: DRIVER_STATES.CLOSING },
    [DRIVER_STATES.OPEN]: { closing: DRIVER_STATES.CLOSING },
    [DRIVER_STATES.CLOSING]: { closed: DRIVER_STATES.CLOSED },
    [DRIVER_STATES.CLOSED]: {},
  }[state]?.[event];
  if (!next) throw new AgentLabDriverError("invalidLifecycle", `Cannot ${event} Agent Lab driver from ${state}.`);
  return next;
}

export async function withTimeout(promise, timeoutMs, detail = "operation") {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new AgentLabDriverError("timeout", `${detail} timed out after ${timeoutMs}ms.`)), timeoutMs);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function createSessionDirectory(workspaceRoot, map) {
  const root = path.join(workspaceRoot, AGENT_LAB_ROOT, "sessions");
  fs.mkdirSync(root, { recursive: true });
  const name = `${safeToken(map, "default", 32)}-${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
  const directory = path.join(root, name);
  fs.mkdirSync(directory, { recursive: true });
  return directory;
}

async function startOrReusePrivateServer({ workspace, sessionDir, startupTimeoutMs, baseUrl, isOpening }) {
  if (!isOpening()) throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during server startup.");
  if (baseUrl) {
    const normalized = privateLoopbackUrl(baseUrl);
    if (await isHealthy(normalized)) {
      if (!isOpening()) throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during server startup.");
      return {
        baseUrl: normalized,
        reused: true,
        logPath: "",
        build: { reused: true, binary: null, head: workspace.head },
        close: async () => {},
      };
    }
    throw new AgentLabDriverError("unhealthyServer", `Requested private server is not healthy: ${normalized}`);
  }
  const port = await allocatePort();
  if (!isOpening()) throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during server startup.");
  const targetDir = path.join(workspace.root, AGENT_LAB_ROOT, "cargo");
  const binary = path.join(targetDir, "debug", "rts-server");
  // The target directory is only a build cache. Always let Cargo check the selected worktree so
  // a prior Agent Lab session cannot silently serve an old server binary.
  runOrThrow("cargo", ["build", "--manifest-path", path.join(workspace.root, "server", "Cargo.toml")], {
    cwd: workspace.root,
    env: { ...process.env, CARGO_TARGET_DIR: targetDir },
    stdio: "inherit",
  });
  if (!fs.existsSync(binary)) throw new AgentLabDriverError("serverBuild", "Agent Lab server binary was not produced.");

  const logPath = path.join(sessionDir, "server.log");
  const log = fs.openSync(logPath, "w");
  const child = spawn(binary, [], {
    cwd: path.join(workspace.root, "server"),
    env: {
      ...process.env,
      RTS_ADDR: `127.0.0.1:${port}`,
      RTS_TEST_TICK_MS: process.env.RTS_TEST_TICK_MS || "5",
      RTS_MATCH_SEED: process.env.RTS_MATCH_SEED || "1",
    },
    stdio: ["ignore", log, log],
  });
  child.once("exit", () => fs.closeSync(log));
  const url = `http://127.0.0.1:${port}/`;
  const deadline = Date.now() + startupTimeoutMs;
  while (Date.now() < deadline) {
    if (!isOpening()) {
      await stopChild(child);
      throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during server startup.");
    }
    if (child.exitCode != null) {
      throw new AgentLabDriverError("serverExited", `Private server exited during startup; see ${logPath}`);
    }
    if (await isHealthy(url)) {
      if (!isOpening()) {
        await stopChild(child);
        throw new AgentLabDriverError("sessionClosed", "Agent Lab driver was closed during server startup.");
      }
      return {
        baseUrl: url,
        reused: false,
        logPath,
        build: {
          reused: false,
          binary,
          head: workspace.head,
          modifiedAt: fs.statSync(binary).mtime.toISOString(),
        },
        close: async () => stopChild(child),
      };
    }
    await sleep(150);
  }
  await stopChild(child);
  throw new AgentLabDriverError("serverTimeout", `Private server did not become healthy; see ${logPath}`);
}

function normalizeViewport(viewport) {
  const width = Number(viewport?.width);
  const height = Number(viewport?.height);
  const deviceScaleFactor = Number(viewport?.deviceScaleFactor ?? viewport?.dpr ?? 1);
  if (!Number.isInteger(width) || width < 320 || width > 4096 || !Number.isInteger(height) || height < 240 || height > 4096 || !Number.isFinite(deviceScaleFactor) || deviceScaleFactor <= 0 || deviceScaleFactor > 4) {
    throw new AgentLabDriverError("invalidViewport", "viewport must have bounded width, height, and DPR.");
  }
  return { width, height, deviceScaleFactor };
}

function boundedTimeout(value, label, maximum) {
  const timeoutMs = Number(value);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > maximum) {
    throw new AgentLabDriverError("invalidTimeout", `${label} must be an integer from 1 to ${maximum}ms.`);
  }
  return timeoutMs;
}

async function loadPuppeteer(workspaceRoot) {
  const testsDir = path.join(workspaceRoot, "tests");
  ensureTestNodeModules(testsDir);
  const requireFromTests = createRequire(path.join(testsDir, "package.json"));
  const resolved = requireFromTests.resolve("puppeteer-core");
  const imported = await import(pathToFileURL(resolved).href);
  return imported.default || imported;
}

export function ensureTestNodeModules(testsDir, requiredPackage = "puppeteer-core") {
  const packageLock = path.join(testsDir, "package-lock.json");
  const localNodeModules = path.join(testsDir, "node_modules");
  const packagePath = path.join(...String(requiredPackage).split("/"));
  if (fs.existsSync(path.join(localNodeModules, packagePath))) return;
  const cacheRoot = process.env.RTS_NODE_DEPS_CACHE_DIR || "/tmp/rts-node-deps";
  const hash = crypto.createHash("sha256").update(fs.readFileSync(packageLock)).digest("hex");
  const cacheNodeModules = path.join(cacheRoot, hash, "node_modules");
  if (fs.existsSync(path.join(cacheNodeModules, packagePath))) {
    if (fs.existsSync(localNodeModules)) fs.rmSync(localNodeModules, { recursive: true, force: true });
    fs.symlinkSync(cacheNodeModules, localNodeModules, "dir");
    return;
  }
  runOrThrow("npm", ["ci", "--ignore-scripts", "--no-audit", "--fund=false"], { cwd: testsDir, stdio: "inherit" });
}

function findChrome(explicit) {
  const candidates = [
    explicit,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    which("google-chrome-stable"),
    which("google-chrome"),
    which("chromium-browser"),
    which("chromium"),
  ].filter(Boolean);
  const chrome = candidates.find((candidate) => fs.existsSync(candidate));
  if (!chrome) throw new AgentLabDriverError("chromeUnavailable", "Chrome/Chromium not found; set CHROME=/path/to/chrome.");
  return chrome;
}

function privateLoopbackUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new AgentLabDriverError("invalidServerUrl", "baseUrl must be a valid loopback URL.");
  }
  if (!new Set(["127.0.0.1", "::1", "localhost"]).has(url.hostname) || !["http:", "https:"].includes(url.protocol)) {
    throw new AgentLabDriverError("invalidServerUrl", "Agent Lab may reuse only a private loopback server.");
  }
  url.pathname = url.pathname.endsWith("/") ? url.pathname : `${url.pathname}/`;
  return url.href;
}

function git(cwd, args) {
  const result = spawnSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
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
    const response = await fetch(baseUrl, { signal: AbortSignal.timeout(1500) });
    return response.ok;
  } catch {
    return false;
  }
}

function runOrThrow(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options });
  if (result.status !== 0) throw new AgentLabDriverError("processFailed", `${command} ${args.join(" ")} failed with exit ${result.status}.`);
  return result;
}

function which(command) {
  const result = spawnSync("which", [command], { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
}

async function stopChild(child) {
  if (!child || child.exitCode != null) return;
  child.kill("SIGTERM");
  const exited = await Promise.race([
    new Promise((resolve) => child.once("exit", () => resolve(true))),
    sleep(3_000).then(() => false),
  ]);
  if (!exited && child.exitCode == null) child.kill("SIGKILL");
}

function readTail(file, maxLines) {
  try {
    return fs.readFileSync(file, "utf8").trimEnd().split("\n").slice(-maxLines);
  } catch {
    return [];
  }
}

function appendBounded(values, value) {
  values.push(String(value));
  if (values.length > MAX_PAGE_ERRORS) values.splice(0, values.length - MAX_PAGE_ERRORS);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
