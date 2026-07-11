// Transport-independent local driver for the Lab Interact browser session.
// This module owns only the selected worktree, private processes, narrow page bridge,
// and bounded local diagnostics.

import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

import {
  checkMediaCapabilities, finalizeMedia, LabInteractRecordingError, RECORDING_LIMITS,
  removePartialRecording, stopRecorderWithin, waitForMediaFile,
} from "./recording.mjs";
import { encodeFixedCapture, fixedFrameTick, hashFrame } from "./fixed_capture.mjs";

const DEFAULT_VIEWPORT = Object.freeze({ width: 1440, height: 900, deviceScaleFactor: 1 });
const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_STARTUP_TIMEOUT_MS = 60_000;
const MAX_TIMEOUT_MS = 60_000;
const MAX_STARTUP_TIMEOUT_MS = 120_000;
const LOG_TAIL_LINES = 80;
const MAX_PAGE_ERRORS = 80;
const MAX_CAPTURE_BYTES = 16 * 1024 * 1024;
const MAX_CAPTURE_VIEWPORT = 2048;
const LAB_INTERACT_ROOT = path.join("target", "lab-interact");
const ARTIFACT_CAPABILITY_HEADER = "x-lab-interact-capability";

export const DRIVER_STATES = Object.freeze({
  OPENING: "opening",
  OPEN: "open",
  CLOSING: "closing",
  CLOSED: "closed",
});

export class LabInteractDriverError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = "LabInteractDriverError";
    this.code = code;
    this.details = details;
  }
}

export class LabInteractDriver {
  static async open(options = {}) {
    const driver = new LabInteractDriver(options);
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
    this.recording = null;
    this.lastRecording = null;
    this.fixedCapture = null;
    this.lastFixedCapture = null;
    this.signalHandlers = [];
    this.openStarted = false;
    const configuredArtifactCapability = process.env.RTS_LAB_INTERACT_ARTIFACT_CAPABILITY || "";
    this.artifactCapability = /^[a-f0-9]{64}$/.test(configuredArtifactCapability)
      ? configuredArtifactCapability
      : crypto.randomBytes(32).toString("hex");
  }

  async open() {
    if (this.openStarted || this.state !== DRIVER_STATES.OPENING) {
      throw new LabInteractDriverError("invalidLifecycle", "Lab Interact driver can only be opened once.");
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
      artifactCapability: this.artifactCapability,
    });
    this.serverLogPath = this.server.logPath || "";

    const puppeteer = await loadPuppeteer(this.workspace.root);
    if (this.state !== DRIVER_STATES.OPENING) {
      throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during browser startup.");
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
      throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during browser startup.");
    }
    this.browser = browser;
    this.browserVersion = await browser.version().catch(() => "");
    const page = await browser.newPage();
    if (this.state !== DRIVER_STATES.OPENING) {
      await page.close().catch(() => {});
      throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during page startup.");
    }
    this.page = page;
    this.attachPageDiagnostics();
    await this.page.goto(this.launchUrl(), { waitUntil: "domcontentloaded", timeout: this.options.startupTimeoutMs });
    await this.page.waitForFunction(
      () => window.__rtsLabInteract?.status?.().ready === true,
      { timeout: this.options.startupTimeoutMs },
    );
    if (this.state !== DRIVER_STATES.OPENING) {
      throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during page startup.");
    }
    if (this.pageErrors.length > 0) {
      throw new LabInteractDriverError("pageError", "Lab Interact page reported an error before readiness.");
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

  async exportSetup(name = "") {
    return this.call("exportSetup", { name });
  }

  async importSetup(scenario) {
    return this.call("importSetup", { scenario });
  }

  async exportReplay(name = "") {
    const room = (await this.status()).room;
    const transfer = await this.artifactRequest("export", { room, name }, "json");
    const response = await fetch(new URL(`dev/lab-interact/artifacts/${transfer.artifactId}`, this.server.baseUrl), {
      headers: {
        [ARTIFACT_CAPABILITY_HEADER]: this.artifactCapability,
        "x-lab-interact-room": room,
      },
      signal: AbortSignal.timeout(this.options.timeoutMs),
    });
    if (!response.ok) throw await artifactHttpError(response, "replay download failed");
    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.length > 8 * 1024 * 1024) throw new LabInteractDriverError("artifactTooLarge", "Replay artifact exceeds 8 MiB.");
    return { bytes, transfer };
  }

  async importReplay(bytes) {
    if (!Buffer.isBuffer(bytes) || bytes.length > 8 * 1024 * 1024) {
      throw new LabInteractDriverError("artifactTooLarge", "Replay artifact must be a buffer no larger than 8 MiB.");
    }
    const room = (await this.status()).room;
    const uploadUrl = new URL("dev/lab-interact/artifacts/upload", this.server.baseUrl);
    const uploadedResponse = await fetch(uploadUrl, {
      method: "POST",
      headers: {
        [ARTIFACT_CAPABILITY_HEADER]: this.artifactCapability,
        "x-lab-interact-room": room,
        "content-type": "application/json",
      },
      body: bytes,
      signal: AbortSignal.timeout(this.options.timeoutMs),
    });
    if (!uploadedResponse.ok) throw await artifactHttpError(uploadedResponse, "replay upload failed");
    const uploaded = await uploadedResponse.json();
    const imported = await this.artifactRequest("import", { room, artifactId: uploaded.artifactId }, "json");
    await this.callBridge("status", {});
    return { uploaded, imported };
  }

  async artifactRequest(action, body) {
    const response = await fetch(new URL(`dev/lab-interact/artifacts/${action}`, this.server.baseUrl), {
      method: "POST",
      headers: { [ARTIFACT_CAPABILITY_HEADER]: this.artifactCapability, "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(this.options.timeoutMs),
    });
    if (!response.ok) throw await artifactHttpError(response, `replay ${action} failed`);
    return response.json();
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

  recordingStatus() {
    const recording = this.recording;
    if (!recording) return { active: false, last: this.lastRecording };
    return {
      active: true,
      name: recording.name,
      startedAt: recording.startedAt,
      elapsedMs: Date.now() - recording.startedMs,
      maxDurationMs: recording.maxDurationMs,
      webmPath: recording.webmPath,
      finalizing: recording.finalizing != null,
    };
  }

  async recordStart({ sessionId, name = "recording", maxDurationMs = RECORDING_LIMITS.defaultDurationMs, viewport = null, crop = null, scale = 1 } = {}) {
    return this.enqueue(async () => {
      if (this.recording) throw new LabInteractDriverError("recordingActive", "A recording is already active for this session. Stop it before starting another.");
      const tools = checkMediaCapabilities();
      const normalizedSessionId = safeCaptureSessionId(sessionId);
      const normalizedViewport = viewport ? normalizeCaptureViewport(viewport) : null;
      const originalViewport = this.page.viewport?.() || null;
      let recordingDir = "";
      let recorder = null;
      try {
        if (normalizedViewport) await this.page.setViewport(normalizedViewport);
        await this.callBridge("presentation", { mode: "clean" });
        await this.page.evaluate(() => document.fonts?.ready || Promise.resolve());
        await this.waitForCaptureReadiness([]);
        const viewportClip = await this.page.evaluate(() => {
          const rect = document.getElementById("viewport")?.getBoundingClientRect?.();
          return rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null;
        });
        if (!validClip(viewportClip)) throw new LabInteractDriverError("viewportUnavailable", "The Pixi viewport is not available for recording.");
        const clip = crop ? normalizeRecordingCrop(crop, viewportClip) : viewportClip;
        const artifactName = safeArtifactName(name, "recording");
        const suffix = new Date().toISOString().replace(/[:.]/g, "-");
        recordingDir = path.join(this.workspace.root, LAB_INTERACT_ROOT, normalizedSessionId, "recordings", `${artifactName}-${suffix}`);
        fs.mkdirSync(recordingDir, { recursive: true });
        const webmPath = path.join(recordingDir, `${artifactName}.webm`);
        recorder = await this.page.screencast({ path: webmPath, crop: clip, scale, ffmpegPath: tools.ffmpeg });
        const startedMs = Date.now();
        const startStatus = await this.callBridge("status", {});
        const recording = {
          name: artifactName, recorder, tools, recordingDir, webmPath,
          framesDir: path.join(recordingDir, "frames"), contactSheetPath: path.join(recordingDir, `${artifactName}-contact-sheet.png`),
          manifestPath: path.join(recordingDir, `${artifactName}.json`), startedMs, startedAt: new Date(startedMs).toISOString(),
          startStatus, clip, scale, viewport: normalizedViewport, originalViewport,
          maxDurationMs, finalizing: null, stoppedBy: null, operations: [], operationCount: 0,
          operationsTruncated: false, aliases: [],
        };
        this.recording = recording;
        recording.watchdog = setTimeout(() => { void this.finishRecording("watchdog").catch(() => {}); }, maxDurationMs);
        recording.watchdog.unref?.();
        recording.sizeWatchdog = setInterval(() => {
          let size = 0;
          try { size = fs.statSync(recording.webmPath).size; } catch {}
          if (size > RECORDING_LIMITS.maxBytes) void this.finishRecording("sizeLimit").catch(() => {});
        }, 250);
        recording.sizeWatchdog.unref?.();
        return { ...this.recordingStatus(), clip, scale, authoritativeStartTick: startStatus.snapshotTick ?? null };
      } catch (error) {
        if (recorder) await stopRecorderWithin(recorder).catch(() => {});
        removePartialRecording([recordingDir]);
        if (originalViewport) await this.page.setViewport(originalViewport).catch(() => {});
        await this.callBridge("presentation", { mode: "default" }).catch(() => {});
        throw error;
      }
    });
  }

  async captureFixed({ sessionId, name = "fixed", fps = 30, frameCount = 30, viewport = null, sceneIdentity = null, sceneRevision = 0, aliases = [] } = {}) {
    return this.enqueue(async () => {
      if (this.recording) throw new LabInteractDriverError("recordingActive", "Fixed capture is unavailable while real-time recording is active.");
      const normalizedSessionId = safeCaptureSessionId(sessionId);
      const artifactName = safeArtifactName(name, "fixed");
      const originalViewport = this.page.viewport?.() || null;
      const normalizedViewport = viewport ? normalizeCaptureViewport(viewport) : null;
      let captureDir = "";
      let captureEntered = false;
      try {
        const status = await this.callBridge("status", {});
        if (!status.roomTime?.paused) throw new LabInteractDriverError("roomTimeNotPaused", "capture-fixed requires paused authoritative room time.");
        if (normalizedViewport) await this.page.setViewport(normalizedViewport);
        await this.callBridge("presentation", { mode: "clean" });
        await this.page.evaluate(() => document.fonts?.ready || Promise.resolve());
        await this.waitForCaptureReadiness([]);
        const clip = await this.page.evaluate(() => {
          const rect = document.getElementById("viewport")?.getBoundingClientRect?.();
          return rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null;
        });
        if (!validClip(clip)) throw new LabInteractDriverError("viewportUnavailable", "The Pixi viewport is not available for fixed capture.");
        this.fixedCapture = { active: true, cancelled: false, name: artifactName, fps, frameCount, frameIndex: 0, startStatus: status };
        const suffix = new Date().toISOString().replace(/[:.]/g, "-");
        captureDir = path.join(this.workspace.root, LAB_INTERACT_ROOT, normalizedSessionId, "fixed", `${artifactName}-${suffix}`);
        const framesDir = path.join(captureDir, "frames");
        fs.mkdirSync(framesDir, { recursive: true });
        const entered = await this.callBridge("captureFixedEnter", {});
        captureEntered = true;
        const startTick = status.snapshotTick;
        let currentTick = startTick;
        let sequenceBytes = 0;
        const frames = [];
        for (let index = 0; index < frameCount; index += 1) {
          if (this.fixedCapture?.cancelled) throw new LabInteractDriverError("captureCancelled", "Fixed capture was cancelled and its partial artifacts were removed.");
          this.fixedCapture.frameIndex = index;
          const tick = fixedFrameTick(startTick, index, fps);
          const ticks = tick - currentTick;
          if (ticks > 0) {
            await this.callBridge("time", { action: "step", ticks });
            currentTick = tick;
          }
          const visualTimeMs = entered.visualStartMs + index * (1000 / fps);
          const rendered = await this.callBridge("captureFixedFrame", { visualTimeMs });
          const framePath = path.join(framesDir, `frame-${String(index).padStart(4, "0")}.png`);
          const screenshot = Buffer.from(await this.page.screenshot({ type: "png", clip, path: framePath }) || []);
          if (screenshot.length === 0) throw new LabInteractDriverError("captureEmpty", "Chrome returned an empty fixed-capture frame.");
          sequenceBytes += screenshot.length;
          if (screenshot.length > FIXED_CAPTURE_LIMITS.maxFrameBytes || sequenceBytes > FIXED_CAPTURE_LIMITS.maxSequenceBytes) {
            throw new LabInteractDriverError("captureTooLarge", "Fixed-capture PNG sequence exceeded its bounded disk budget.");
          }
          frames.push({ index, tick, visualTimeMs, rendererFrame: rendered.rendererFrame, path: framePath, sha256: hashFrame(framePath) });
        }
        const videoPath = path.join(captureDir, `${artifactName}.webm`);
        const contactSheetPath = path.join(captureDir, `${artifactName}-contact-sheet.png`);
        const media = encodeFixedCapture({ framesDir, outputPath: videoPath, contactSheetPath, fps, frameCount });
        const endStatus = await this.callBridge("status", {});
        const diagnostics = this.diagnostics();
        const manifestPath = path.join(captureDir, `${artifactName}.json`);
        const manifest = {
          schemaVersion: 1, kind: "labInteractFixedCapture", deterministicEnvironmentOnly: true,
          workspace: this.workspace, serverBuild: this.server?.build || null,
          scene: { identity: sceneIdentity || { source: "launch", scenario: this.options.scenario, seed: this.options.seed || null, map: this.options.map }, revision: sceneRevision, aliases: aliases.slice(0, 100) },
          mapping: { simulationHz: 30, outputFps: fps, rule: "frame i uses startTick + floor(i * 30 / outputFps); repeated ticks do not interpolate world state" },
          authoritative: { startTick, endTick: endStatus.snapshotTick },
          capture: { frameCount, clip, viewport: normalizedViewport, visualStartMs: entered.visualStartMs, sequenceBytes },
          frames, media: { videoPath, contactSheetPath, bytes: media.bytes, tools: media.tools, probe: media.probe, contactSheet: media.contactSheet },
          runtime: { node: process.version, platform: process.platform, architecture: process.arch, browser: this.browserVersion || null },
          errors: { pageConsole: diagnostics.pageConsoleErrors, page: diagnostics.pageErrors, requestFailures: diagnostics.requestFailures },
        };
        fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
        const result = { videoPath, contactSheetPath, manifestPath, framePaths: frames.map((frame) => frame.path), frameHashes: frames.map((frame) => frame.sha256), authoritative: manifest.authoritative, mapping: manifest.mapping, probe: media.probe };
        this.lastFixedCapture = result;
        return result;
      } catch (error) {
        removePartialRecording([captureDir]);
        if (error instanceof LabInteractRecordingError) throw new LabInteractDriverError(error.code, error.message);
        throw error;
      } finally {
        if (captureEntered) await this.callBridge("captureFixedExit", {}).catch(() => {});
        await this.callBridge("presentation", { mode: "default" }).catch(() => {});
        if (originalViewport) await this.page?.setViewport(originalViewport).catch(() => {});
        this.fixedCapture = null;
      }
    });
  }

  fixedCaptureStatus() {
    if (!this.fixedCapture) return { active: false, last: this.lastFixedCapture };
    const { cancelled, name, fps, frameCount, frameIndex } = this.fixedCapture;
    return { active: true, cancelled, name, fps, frameCount, frameIndex };
  }

  cancelFixedCapture() {
    if (!this.fixedCapture) throw new LabInteractDriverError("captureInactive", "No fixed capture is active.");
    this.fixedCapture.cancelled = true;
    return { cancelling: true };
  }

  recordAcceptedOperation(operation, aliases = []) {
    if (!this.recording || this.recording.finalizing) return false;
    this.recording.operationCount += 1;
    if (this.recording.operations.length < RECORDING_LIMITS.maxOperations) this.recording.operations.push(operation);
    else this.recording.operationsTruncated = true;
    this.recording.aliases = Array.isArray(aliases) ? aliases.slice(0, RECORDING_LIMITS.maxAliases) : [];
    return true;
  }

  async recordStop(metadata = {}) {
    return this.enqueue(() => this.finishRecording("explicit", metadata));
  }

  async finishRecording(reason, metadata = {}) {
    const recording = this.recording;
    if (!recording) throw new LabInteractDriverError("recordingInactive", "No recording is active for this session. Start one before stopping.");
    if (recording.finalizing) return recording.finalizing;
    recording.stoppedBy = reason;
    recording.finalizing = (async () => {
      clearTimeout(recording.watchdog);
      clearInterval(recording.sizeWatchdog);
      try {
        await stopRecorderWithin(recording.recorder);
        await waitForMediaFile(recording.webmPath);
        const endedMs = Date.now();
        const endStatus = await this.callBridge("status", {}).catch(() => null);
        const media = finalizeMedia({
          webmPath: recording.webmPath, framesDir: recording.framesDir,
          contactSheetPath: recording.contactSheetPath, tools: recording.tools,
        });
        const diagnostics = this.diagnostics();
        const manifest = {
          schemaVersion: 1,
          kind: "labInteractRealTimeRecording",
          createdAt: recording.startedAt,
          finalizedAt: new Date(endedMs).toISOString(),
          stoppedBy: reason,
          nondeterministic: true,
          workspace: this.workspace,
          serverBuild: this.server?.build || null,
          runtime: { node: process.version, platform: process.platform, architecture: process.arch },
          browser: { chrome: this.browserVersion || null, userAgent: await this.page.evaluate(() => navigator.userAgent).catch(() => null) },
          mediaTools: recording.tools,
          authoritative: {
            startTick: recording.startStatus?.snapshotTick ?? null,
            endTick: endStatus?.snapshotTick ?? null,
            startRoomTime: recording.startStatus?.roomTime ?? null,
            endRoomTime: endStatus?.roomTime ?? null,
          },
          capture: { fps: 30, audio: false, clip: recording.clip, scale: recording.scale, viewport: recording.viewport, wallDurationMs: endedMs - recording.startedMs, maxDurationMs: recording.maxDurationMs },
          aliases: Array.isArray(metadata.aliases) ? metadata.aliases.slice(0, RECORDING_LIMITS.maxAliases) : recording.aliases,
          operations: recording.operations,
          operationDiagnostics: {
            accepted: recording.operationCount,
            captured: recording.operations.length,
            truncated: recording.operationsTruncated,
          },
          media,
          errors: { pageConsole: diagnostics.pageConsoleErrors, page: diagnostics.pageErrors, requestFailures: diagnostics.requestFailures },
        };
        fs.writeFileSync(recording.manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
        const result = {
          active: false, stoppedBy: reason, webmPath: recording.webmPath,
          framePaths: media.framePaths, contactSheetPath: recording.contactSheetPath,
          manifestPath: recording.manifestPath, probe: media.probe,
          frameDiagnostics: media.frameDiagnostics,
          authoritative: manifest.authoritative,
        };
        this.lastRecording = result;
        return result;
      } catch (error) {
        removePartialRecording([recording.recordingDir]);
        if (error instanceof LabInteractRecordingError) throw new LabInteractDriverError(error.code, error.message);
        throw error;
      } finally {
        if (recording.originalViewport) await this.page?.setViewport(recording.originalViewport).catch(() => {});
        await this.callBridge("presentation", { mode: "default" }).catch(() => {});
        if (this.recording === recording) this.recording = null;
      }
    })();
    return recording.finalizing;
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
      throw new LabInteractDriverError("sessionClosed", "Lab Interact driver session is not open.");
    }
    const result = await withTimeout(
      this.page.evaluate(
        ({ method: bridgeMethod, input: bridgeInput }) => window.__rtsLabInteract.call(bridgeMethod, bridgeInput),
        { method, input },
      ),
      this.options.timeoutMs,
      `Lab Interact ${method}`,
    );
    if (!result?.ok) {
      throw new LabInteractDriverError(
        result?.error?.code || "bridgeError",
        result?.error?.message || `Lab Interact ${method} failed.`,
        { method },
      );
    }
    return result.value;
  }

  async captureScreenshot({ sessionId, name, presentation, viewport, subjectIds, subjectSummaries, request }) {
    if (this.recording) {
      throw new LabInteractDriverError(
        "recordingActive",
        "Screenshot capture is unavailable while recording because it can change the active viewport or presentation. Stop the recording first.",
      );
    }
    if (presentation !== "clean" && presentation !== "normal") {
      throw new LabInteractDriverError("invalidPresentation", "presentation must be clean or normal.");
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
      if (!validClip(clip)) throw new LabInteractDriverError("viewportUnavailable", "The Pixi viewport is not available for capture.");

      const captureDir = path.join(this.workspace.root, LAB_INTERACT_ROOT, normalizedSessionId, "captures");
      fs.mkdirSync(captureDir, { recursive: true });
      const suffix = new Date().toISOString().replace(/[:.]/g, "-");
      const baseName = `${artifactName}-${suffix}`;
      const pngPath = path.join(captureDir, `${baseName}.png`);
      const manifestPath = path.join(captureDir, `${baseName}.json`);
      const screenshot = await this.page.screenshot({ type: "png", clip, path: pngPath });
      const png = Buffer.from(screenshot || []);
      if (png.length === 0) {
        throw new LabInteractDriverError("captureEmpty", "Chrome returned an empty Pixi screenshot.");
      }
      if (png.length > MAX_CAPTURE_BYTES) {
        fs.rmSync(pngPath, { force: true });
        throw new LabInteractDriverError("captureTooLarge", `Screenshot exceeds the ${MAX_CAPTURE_BYTES} byte response bound.`);
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
      if (errors) throw new LabInteractDriverError("captureRenderError", captureReadinessMessage(readiness, this.diagnostics()));
      if (readiness.failedAssets?.length) {
        throw new LabInteractDriverError("assetLoadFailed", captureReadinessMessage(readiness, this.diagnostics()));
      }
      if (readiness.ready && Number(readiness.frame) >= initialFrame + 2) return readiness;
      await sleep(25);
    }
    throw new LabInteractDriverError("captureTimeout", captureReadinessMessage(last, this.diagnostics()));
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
    url.searchParams.set("labInteract", "1");
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
    this.page.on("close", () => {
      if (this.recording) void this.finishRecording("pageClosed").catch(() => {});
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
      this.removeCleanupHandlers();
      if (this.recording) {
        await this.finishRecording("sessionClose").catch(() => {
          removePartialRecording([this.recording?.recordingDir]);
          this.recording = null;
        });
      }
      if (this.page && this.server) {
        const room = await this.callBridge("status", {}).then((status) => status.room).catch(() => "");
        if (room) await this.artifactRequest("cleanup", { room }).catch(() => {});
      }
      if (this.state !== DRIVER_STATES.CLOSING) this.transition("closing");
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
    if (error instanceof LabInteractDriverError && error.details?.diagnostics) return error;
    const diagnostics = this.diagnostics();
    const serverTail = diagnostics.serverLog ? readTail(diagnostics.serverLog, LOG_TAIL_LINES) : [];
    const message = [error?.message || "Lab Interact driver failed."]
      .concat(serverTail.length ? [`Server log tail:\n${serverTail.join("\n")}`] : [])
      .join("\n");
    return new LabInteractDriverError(error?.code || "driverError", message, {
      ...error?.details,
      diagnostics: { ...diagnostics, serverTail },
    });
  }
}

export function validateWorkspaceRoot(workspaceRoot) {
  if (!workspaceRoot) throw new LabInteractDriverError("workspaceRequired", "workspaceRoot is required.");
  let root;
  try {
    root = fs.realpathSync(workspaceRoot);
  } catch {
    throw new LabInteractDriverError("invalidWorkspace", `Workspace does not exist: ${workspaceRoot}`);
  }
  if (!fs.existsSync(path.join(root, "server", "Cargo.toml")) || !fs.existsSync(path.join(root, "client", "src", "main.js"))) {
    throw new LabInteractDriverError("invalidWorkspace", "workspaceRoot is not a Bewegungskrieg checkout.");
  }
  const topLevel = git(root, ["rev-parse", "--show-toplevel"]);
  if (!topLevel || fs.realpathSync(topLevel) !== root) {
    throw new LabInteractDriverError("invalidWorkspace", "workspaceRoot must be the Git checkout top level.");
  }
  const head = git(root, ["rev-parse", "HEAD"]);
  if (!/^[0-9a-f]{40}$/i.test(head || "")) {
    throw new LabInteractDriverError("invalidWorkspace", "workspaceRoot has no valid Git HEAD.");
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
    throw new LabInteractDriverError("invalidSession", "sessionId must be a valid Lab Interact session id.");
  }
  return sessionId;
}

function normalizeCaptureViewport(viewport) {
  const normalized = normalizeViewport(viewport);
  if (normalized.width > MAX_CAPTURE_VIEWPORT || normalized.height > MAX_CAPTURE_VIEWPORT) {
    throw new LabInteractDriverError("invalidViewport", `capture viewport width and height must be at most ${MAX_CAPTURE_VIEWPORT}.`);
  }
  return normalized;
}

function normalizeRecordingCrop(crop, viewportClip) {
  const normalized = { x: Number(crop.x), y: Number(crop.y), width: Number(crop.width), height: Number(crop.height) };
  if (!Object.values(normalized).every(Number.isFinite) || normalized.x < 0 || normalized.y < 0 || normalized.width < 2 || normalized.height < 2) {
    throw new LabInteractDriverError("invalidCrop", "recording crop must contain finite non-negative x/y and width/height of at least 2.");
  }
  const absolute = { x: viewportClip.x + normalized.x, y: viewportClip.y + normalized.y, width: normalized.width, height: normalized.height };
  if (absolute.x + absolute.width > viewportClip.x + viewportClip.width || absolute.y + absolute.height > viewportClip.y + viewportClip.height) {
    throw new LabInteractDriverError("invalidCrop", "recording crop must stay inside the game viewport.");
  }
  return absolute;
}

function boundedEntityIds(values) {
  if (!Array.isArray(values) || values.length > 20) {
    throw new LabInteractDriverError("invalidSubjects", "subjectIds must contain at most 20 positive entity ids.");
  }
  const ids = [...new Set(values.map(Number))];
  if (!ids.every((id) => Number.isInteger(id) && id > 0)) {
    throw new LabInteractDriverError("invalidSubjects", "subjectIds must contain positive integer entity ids.");
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
    throw new LabInteractDriverError("invalidCapture", "Chrome did not return a PNG image.");
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

export function generatedRoomId(head = "") {
  const suffix = crypto.randomBytes(6).toString("hex");
  return safeToken(`labinteract-${safeToken(head.slice(0, 8), "head", 8)}-${process.pid}-${suffix}`, "labinteract", 40);
}

export function transitionDriverState(state, event) {
  const next = {
    [DRIVER_STATES.OPENING]: { opened: DRIVER_STATES.OPEN, closing: DRIVER_STATES.CLOSING },
    [DRIVER_STATES.OPEN]: { closing: DRIVER_STATES.CLOSING },
    [DRIVER_STATES.CLOSING]: { closed: DRIVER_STATES.CLOSED },
    [DRIVER_STATES.CLOSED]: {},
  }[state]?.[event];
  if (!next) throw new LabInteractDriverError("invalidLifecycle", `Cannot ${event} Lab Interact driver from ${state}.`);
  return next;
}

export async function withTimeout(promise, timeoutMs, detail = "operation") {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new LabInteractDriverError("timeout", `${detail} timed out after ${timeoutMs}ms.`)), timeoutMs);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function createSessionDirectory(workspaceRoot, map) {
  const root = path.join(workspaceRoot, LAB_INTERACT_ROOT, "sessions");
  fs.mkdirSync(root, { recursive: true });
  const name = `${safeToken(map, "default", 32)}-${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
  const directory = path.join(root, name);
  fs.mkdirSync(directory, { recursive: true });
  return directory;
}

async function startOrReusePrivateServer({ workspace, sessionDir, startupTimeoutMs, baseUrl, isOpening, artifactCapability }) {
  if (!isOpening()) throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during server startup.");
  if (baseUrl) {
    const normalized = privateLoopbackUrl(baseUrl);
    if (await isHealthy(normalized)) {
      if (!isOpening()) throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during server startup.");
      return {
        baseUrl: normalized,
        reused: true,
        logPath: "",
        build: { reused: true, binary: null, head: workspace.head },
        close: async () => {},
      };
    }
    throw new LabInteractDriverError("unhealthyServer", `Requested private server is not healthy: ${normalized}`);
  }
  const port = await allocatePort();
  if (!isOpening()) throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during server startup.");
  const targetDir = path.join(workspace.root, LAB_INTERACT_ROOT, "cargo");
  const binary = path.join(targetDir, "debug", "rts-server");
  // The target directory is only a build cache. Always let Cargo check the selected worktree so
  // a prior Lab Interact session cannot silently serve an old server binary.
  runOrThrow("cargo", ["build", "--manifest-path", path.join(workspace.root, "server", "Cargo.toml")], {
    cwd: workspace.root,
    env: { ...process.env, CARGO_TARGET_DIR: targetDir },
    stdio: "inherit",
  });
  if (!fs.existsSync(binary)) throw new LabInteractDriverError("serverBuild", "Lab Interact server binary was not produced.");

  const logPath = path.join(sessionDir, "server.log");
  const log = fs.openSync(logPath, "w");
  const child = spawn(binary, [], {
    cwd: path.join(workspace.root, "server"),
    env: {
      ...process.env,
      RTS_ADDR: `127.0.0.1:${port}`,
      RTS_MATCH_SEED: process.env.RTS_MATCH_SEED || "1",
      RTS_LAB_INTERACT_ARTIFACT_CAPABILITY: artifactCapability,
    },
    stdio: ["ignore", log, log],
  });
  child.once("exit", () => fs.closeSync(log));
  const url = `http://127.0.0.1:${port}/`;
  const deadline = Date.now() + startupTimeoutMs;
  while (Date.now() < deadline) {
    if (!isOpening()) {
      await stopChild(child);
      throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during server startup.");
    }
    if (child.exitCode != null) {
      throw new LabInteractDriverError("serverExited", `Private server exited during startup; see ${logPath}`);
    }
    if (await isHealthy(url)) {
      if (!isOpening()) {
        await stopChild(child);
        throw new LabInteractDriverError("sessionClosed", "Lab Interact driver was closed during server startup.");
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
  throw new LabInteractDriverError("serverTimeout", `Private server did not become healthy; see ${logPath}`);
}

async function artifactHttpError(response, fallback) {
  let message = fallback;
  try { message = (await response.json())?.error || message; } catch {}
  return new LabInteractDriverError("artifactTransferFailed", message);
}

function normalizeViewport(viewport) {
  const width = Number(viewport?.width);
  const height = Number(viewport?.height);
  const deviceScaleFactor = Number(viewport?.deviceScaleFactor ?? viewport?.dpr ?? 1);
  if (!Number.isInteger(width) || width < 320 || width > 4096 || !Number.isInteger(height) || height < 240 || height > 4096 || !Number.isFinite(deviceScaleFactor) || deviceScaleFactor <= 0 || deviceScaleFactor > 4) {
    throw new LabInteractDriverError("invalidViewport", "viewport must have bounded width, height, and DPR.");
  }
  return { width, height, deviceScaleFactor };
}

function boundedTimeout(value, label, maximum) {
  const timeoutMs = Number(value);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > maximum) {
    throw new LabInteractDriverError("invalidTimeout", `${label} must be an integer from 1 to ${maximum}ms.`);
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
  if (!chrome) throw new LabInteractDriverError("chromeUnavailable", "Chrome/Chromium not found; set CHROME=/path/to/chrome.");
  return chrome;
}

function privateLoopbackUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new LabInteractDriverError("invalidServerUrl", "baseUrl must be a valid loopback URL.");
  }
  if (!new Set(["127.0.0.1", "::1", "localhost"]).has(url.hostname) || !["http:", "https:"].includes(url.protocol)) {
    throw new LabInteractDriverError("invalidServerUrl", "Lab Interact may reuse only a private loopback server.");
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
  if (result.status !== 0) throw new LabInteractDriverError("processFailed", `${command} ${args.join(" ")} failed with exit ${result.status}.`);
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
