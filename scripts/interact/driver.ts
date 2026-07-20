// Transport-independent local driver for the Interact browser session.
// This module owns only the selected worktree, private processes, narrow page bridge,
// and bounded local diagnostics.
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { Browser, Page, Viewport } from "puppeteer-core";
import { withAbortSignal as abortable } from "./abort_signal.ts";
import {
  checkMediaCapabilities, createWallClockRecorder, finalizeMp4Artifacts, InteractRecordingError,
  RECORDING_LIMITS, removePartialRecording,
} from "./recording.ts";
import { createFixedCaptureEncoder, FIXED_CAPTURE_LIMITS, fixedFrameTick, fixedRepresentativeIndices, hashFrame } from "./fixed_capture.ts";
import { resolveCaptureRegion, type CaptureClip, type CaptureRegion } from "./capture_region.ts";
import { captureGameTimelapse } from "./game_timelapse.ts";
import { boundedSummary, INTERACT_SUMMARY_LIMITS } from "./manifest_summary.ts";
import { PrivateServer } from "./private_server.ts";
import { findChrome, validateWorkspaceRoot } from "./workspace_inspection.ts";
import { interactLaunchUrl } from "./game_launch_url.ts";
import { createInteractSessionDirectory, interactArtifactRoot } from "./interact_paths.ts";
import { defaultMapForMode } from "./session_defaults.ts";
import { waitForInteractStartup } from "./bridge_startup.ts";
import { evaluateInteractBridgeCall } from "./bridge_call.ts";
import { performMouseDrag, type MouseDragInput } from "./mouse_drag.ts";
export { validateWorkspaceRoot } from "./workspace_inspection.ts";
const DEFAULT_VIEWPORT = Object.freeze({ width: 1440, height: 900, deviceScaleFactor: 1 });
const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_STARTUP_TIMEOUT_MS = 60_000;
const MAX_TIMEOUT_MS = 60_000;
const MAX_STARTUP_TIMEOUT_MS = 120_000;
const LOG_TAIL_LINES = 12;
const LOG_TAIL_LINE_CHARS = 512;
const MAX_PAGE_ERRORS = 80;
const MAX_CAPTURE_BYTES = 16 * 1024 * 1024;
const MAX_CAPTURE_VIEWPORT = 2048;
const ARTIFACT_CAPABILITY_HEADER = "x-interact-lab-capability";
export const DRIVER_STATES = Object.freeze({
  OPENING: "opening",
  OPEN: "open",
  CLOSING: "closing",
  CLOSED: "closed",
});
type JsonObject = Record<string, unknown>;
interface WorkspaceInfo { root: string; branch: string; head: string }
interface BridgeResult extends JsonObject {
  snapshotTick?: number;
  room?: string;
  phase?: string;
  roomTime?: { paused?: boolean; speed?: number };
  frame?: number;
  ready?: boolean;
  visualStartMs?: number;
  rendererFrame?: number;
  frameErrors?: unknown[];
  renderErrors?: unknown[];
  missingTextureSubjectIds?: number[];
  failedAssets?: unknown[];
  pendingAssets?: unknown[];
  subjects?: unknown[];
  assets?: unknown;
  camera?: unknown;
  cameraViewport?: unknown;
  cameraWorldBounds?: unknown;
  visualProfileId?: string;
}
interface Completion<T> { promise: Promise<T>; resolve(value: T): boolean; reject(error: unknown): boolean }
type WallClockRecorder = Awaited<ReturnType<typeof createWallClockRecorder>>;
type MediaTools = Awaited<ReturnType<typeof checkMediaCapabilities>>;
interface RecordingResult extends JsonObject {
  active: false;
  stoppedBy: string;
  videoPath: string;
  framePaths: string[];
  contactSheetPath: string;
  manifestPath: string;
}
interface ActiveRecording {
  name: string;
  recorder: WallClockRecorder;
  tools: MediaTools;
  recordingDir: string;
  mp4Path: string;
  framesDir: string;
  contactSheetPath: string;
  manifestPath: string;
  startedMs: number;
  startedAt: string;
  startStatus: BridgeResult;
  resumeResult: BridgeResult | null;
  resumeSpeed: number | null;
  clip: CaptureClip;
  scale: number;
  presentation: "clean" | "normal";
  region: JsonObject;
  viewport: Viewport | null;
  originalViewport: Viewport | null;
  maxDurationMs: number;
  finalizing: Promise<RecordingResult> | null;
  stoppedBy: string | null;
  operations: unknown[];
  operationCount: number;
  operationsTruncated: boolean;
  aliases: Array<{ alias: string; id: number }>;
  completion: Completion<RecordingResult>;
  watchdog?: NodeJS.Timeout;
  sizeWatchdog?: NodeJS.Timeout;
}
interface ActiveFixedCapture {
  active: true;
  cancelled: boolean;
  name: string;
  fps: number;
  frameCount: number;
  frameIndex: number;
  startStatus: BridgeResult;
  abortController: AbortController;
}
interface DriverOptions {
  workspaceRoot?: string;
  mode?: "lab" | "game" | "scenario";
  map?: string;
  seed?: string;
  scenario?: string;
  devScenario?: { id: string; unit: string; count: number; blocker: string; case: string };
  opponent?: string;
  spectate?: string[] | null; autoSpectator?: boolean;
  renderer?: string;
  viewport?: Viewport;
  timeoutMs?: number;
  startupTimeoutMs?: number;
  chrome?: string;
  baseUrl?: string;
  signal?: AbortSignal | null;
  puppeteerLoader?: typeof loadPuppeteer;
  chromeFinder?: typeof findChrome;
  privateServerFactory?: typeof PrivateServer.open;
}

declare global {
  interface Window {
    __rtsInteract?: {
      status(): { ready?: boolean; launchError?: string };
      call(method: string, input: unknown): Promise<{ ok: boolean; value?: unknown; error?: { code?: string; message?: string; details?: JsonObject } }>;
    };
  }
}

export class InteractDriverError extends Error {
  details: JsonObject;
  code: string;
  constructor(code: string, message: string, details: JsonObject = {}) {
    super(message);
    this.name = "InteractDriverError";
    this.code = code;
    this.details = details;
  }
}

export class InteractDriver {
  gameRoom: string;
  artifactCapability: string;
  openStarted: boolean;
  lastFixedCapture: JsonObject | null;
  fixedCapture: ActiveFixedCapture | null;
  lastRecordingCompletion: Completion<RecordingResult> | null;
  lastRecording: RecordingResult | null;
  recording: ActiveRecording | null;
  closePromise: Promise<void> | null;
  requestFailures: string[];
  pageErrors: string[];
  pageConsoleErrors: string[];
  profileDir: string;
  page: Page | null;
  browserVersion: string;
  browser: Browser | null;
  serverLogPath: string;
  server: PrivateServer | null;
  sessionDir: string;
  workspace: WorkspaceInfo | null;
  state: string;
  puppeteerLoader: typeof loadPuppeteer;
  chromeFinder: typeof findChrome;
  privateServerFactory: typeof PrivateServer.open;
  options: Required<Omit<DriverOptions, "workspaceRoot" | "signal" | "puppeteerLoader" | "chromeFinder" | "privateServerFactory">> & Pick<DriverOptions, "workspaceRoot" | "signal">;
  static async open(options: DriverOptions = {}) {
    const driver = new InteractDriver(options);
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
    mode = "lab",
    map,
    seed = "",
    scenario = "blank",
    devScenario = { id: "", unit: "", count: 1, blocker: "", case: "" },
    opponent = "ai_2_1",
    spectate = null, autoSpectator = false,
    renderer = "pixi",
    viewport = DEFAULT_VIEWPORT,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    startupTimeoutMs = DEFAULT_STARTUP_TIMEOUT_MS,
    chrome = process.env.CHROME || "",
    baseUrl = "",
    signal = null,
    puppeteerLoader = loadPuppeteer,
    chromeFinder = findChrome,
    privateServerFactory = PrivateServer.open,
  }: DriverOptions = {}) {
    this.options = {
      workspaceRoot,
      mode,
      map: map || defaultMapForMode(mode),
      seed,
      scenario,
      devScenario,
      opponent,
      spectate, autoSpectator,
      renderer,
      viewport,
      timeoutMs: boundedTimeout(timeoutMs, "timeoutMs", MAX_TIMEOUT_MS),
      startupTimeoutMs: boundedTimeout(startupTimeoutMs, "startupTimeoutMs", MAX_STARTUP_TIMEOUT_MS),
      chrome,
      baseUrl,
      signal,
    };
    this.state = DRIVER_STATES.OPENING;
    this.gameRoom = `interact-game-${crypto.randomBytes(8).toString("hex")}`;
    this.puppeteerLoader = puppeteerLoader;
    this.chromeFinder = chromeFinder;
    this.privateServerFactory = privateServerFactory;
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
    this.closePromise = null;
    this.recording = null;
    this.lastRecording = null;
    this.lastRecordingCompletion = null;
    this.fixedCapture = null;
    this.lastFixedCapture = null;
    this.openStarted = false;
    const configuredArtifactCapability = process.env.RTS_INTERACT_LAB_ARTIFACT_CAPABILITY || "";
    this.artifactCapability = /^[a-f0-9]{64}$/.test(configuredArtifactCapability)
      ? configuredArtifactCapability
      : crypto.randomBytes(32).toString("hex");
  }

  async open() {
    if (this.openStarted || this.state !== DRIVER_STATES.OPENING) {
      throw new InteractDriverError("invalidLifecycle", "Interact driver can only be opened once.");
    }
    this.openStarted = true;
    this.workspace = validateWorkspaceRoot(this.options.workspaceRoot || process.cwd());
    this.sessionDir = createInteractSessionDirectory(this.workspace!.root, this.options.map, this.options.mode);
    this.writeManifest({ status: DRIVER_STATES.OPENING });
    // Local browser prerequisites are deterministic and cheap. Check them before
    // a clean worktree spends minutes compiling the private Rust server.
    const puppeteer = await this.openStep(this.puppeteerLoader(), "Puppeteer loading");
    const chrome = this.chromeFinder(this.options.chrome);
    this.server = await this.privateServerFactory({
      workspace: this.workspace,
      sessionDir: this.sessionDir,
      startupTimeoutMs: this.options.startupTimeoutMs,
      baseUrl: this.options.baseUrl,
      artifactCapability: this.artifactCapability,
      signal: this.options.signal || undefined,
    });
    this.serverLogPath = this.server!.logPath || "";
    if (this.state !== DRIVER_STATES.OPENING) {
      throw new InteractDriverError("sessionClosed", "Interact driver was closed during browser startup.");
    }
    this.profileDir = fs.mkdtempSync(path.join(this.sessionDir, "chrome-profile-"));
    const browser = await this.openStep(
      puppeteer.launch({
        executablePath: chrome,
        headless: true,
        defaultViewport: normalizeViewport(this.options.viewport),
        args: [
          "--no-sandbox",
          "--disable-features=PointerLockOptions",
          `--window-size=${this.options.viewport.width},${this.options.viewport.height}`,
          `--user-data-dir=${this.profileDir}`,
        ],
      }),
      "browser startup",
      (lateBrowser: Browser) => lateBrowser.close(),
    );
    if (this.state !== DRIVER_STATES.OPENING) {
      await browser.close().catch(() => {});
      throw new InteractDriverError("sessionClosed", "Interact driver was closed during browser startup.");
    }
    this.browser = browser;
    this.browserVersion = await this.openStep(browser.version().catch(() => ""), "browser version inspection");
    const page = await this.openStep(browser.newPage(), "page startup");
    if (this.state !== DRIVER_STATES.OPENING) {
      await page.close().catch(() => {});
      throw new InteractDriverError("sessionClosed", "Interact driver was closed during page startup.");
    }
    this.page = page; this.attachPageDiagnostics();
    if (this.options.autoSpectator) await this.openStep(this.page!.evaluateOnNewDocument(() => localStorage.setItem("rts.autoSpectator.enabled", "1")), "automatic spectator preference");
    await this.openStep(
      this.page!.goto(this.launchUrl(), { waitUntil: "domcontentloaded", timeout: this.options.startupTimeoutMs }),
      "page navigation",
    );
    const startupStatus = await this.openStep(
      waitForInteractStartup(this.page!, this.options.startupTimeoutMs),
      "page readiness",
    );
    if (startupStatus?.launchError) {
      throw new InteractDriverError("launchFailed", startupStatus.launchError, { status: startupStatus });
    }
    if (this.state !== DRIVER_STATES.OPENING) {
      throw new InteractDriverError("sessionClosed", "Interact driver was closed during page startup.");
    }
    if (this.pageErrors.length > 0) {
      throw new InteractDriverError("pageError", "Interact page reported an error before readiness.");
    }
    this.transition("opened");
    const ready = await this.openStep(this.status(), "session verification");
    this.writeManifest({
      status: this.state,
      baseUrl: this.server!.baseUrl,
      reusedServer: this.server!.reused,
      browser: { chrome, viewport: normalizeViewport(this.options.viewport) },
      ready,
    });
  }

  async status() {
    const status = await this.call("status", {});
    return this.pageErrors.length === 0
      ? status
      : { ...status, ready: false, reason: "pageError" };
  }

  openStep<T>(promise: PromiseLike<T>, detail: string, disposeLateValue: ((value: T) => void | Promise<void>) | null = null): Promise<T> {
    return abortable(
      promise,
      this.options.signal,
      () => new InteractDriverError("sessionClosed", `Interact driver was closed during ${detail}.`),
      disposeLateValue,
    );
  }

  async catalog(query: JsonObject = {}) {
    return this.call("catalog", query);
  }

  async spawn(spawns: JsonObject[]) {
    return this.call("spawn", { spawns });
  }

  async update(updates: JsonObject[]) {
    return this.call("update", { updates });
  }

  async remove(entityIds: number[]) {
    return this.call("remove", { entityIds });
  }

  async order({ playerId, command, ignoreCommandLimits = false }: { playerId: number; command: JsonObject; ignoreCommandLimits?: boolean }) {
    return this.call("order", { playerId, command, ignoreCommandLimits });
  }

  async move(input: { units: number[]; x?: number; y?: number; queued?: boolean }) {
    return this.call("move", input);
  }

  async giveUp() { return this.call("giveUp", {}); }

  async time(control: JsonObject) { return this.call("time", control); }

  async inspect(query: JsonObject = {}) { return this.call("inspect", query); }
  async select(entityIds: number[]) { return this.call("select", { entityIds }); }
  async camera(command: JsonObject) { return this.call("camera", command); }

  async drag(input: MouseDragInput) {
    if (this.state !== DRIVER_STATES.OPEN || !this.page) throw new InteractDriverError("sessionClosed", "Interact driver session is not open.");
    return performMouseDrag(this.page, input, (code, message, details) => new InteractDriverError(code, message, details));
  }

  async reset() { return this.call("reset", {}); }

  async exportSetup(name = "") {
    return this.call("exportSetup", { name });
  }

  async importSetup(scenario: JsonObject) {
    return this.call("importSetup", { scenario });
  }

  async exportReplay(name = "") {
    const room = (await this.status()).room;
    if (typeof room !== "string") throw new InteractDriverError("artifactTransferFailed", "Lab room identity is unavailable.");
    const transfer = await this.artifactRequest("export", { room, name });
    const response = await fetch(new URL(`dev/interact/lab/artifacts/${transfer.artifactId}`, this.server!.baseUrl), {
      headers: {
        [ARTIFACT_CAPABILITY_HEADER]: this.artifactCapability,
        "x-interact-lab-room": room,
      },
      signal: AbortSignal.timeout(this.options.timeoutMs),
    });
    if (!response.ok) throw await artifactHttpError(response, "replay download failed");
    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.length > 8 * 1024 * 1024) throw new InteractDriverError("artifactTooLarge", "Replay artifact exceeds 8 MiB.");
    return { bytes, transfer };
  }

  async importReplay(bytes: Buffer) {
    if (!Buffer.isBuffer(bytes) || bytes.length > 8 * 1024 * 1024) {
      throw new InteractDriverError("artifactTooLarge", "Replay artifact must be a buffer no larger than 8 MiB.");
    }
    const room = (await this.status()).room;
    if (typeof room !== "string") throw new InteractDriverError("artifactTransferFailed", "Lab room identity is unavailable.");
    const uploadUrl = new URL("dev/interact/lab/artifacts/upload", this.server!.baseUrl);
    const uploadedResponse = await fetch(uploadUrl, {
      method: "POST",
      headers: {
        [ARTIFACT_CAPABILITY_HEADER]: this.artifactCapability,
        "x-interact-lab-room": room,
        "content-type": "application/json",
      },
      body: new Uint8Array(bytes),
      signal: AbortSignal.timeout(this.options.timeoutMs),
    });
    if (!uploadedResponse.ok) throw await artifactHttpError(uploadedResponse, "replay upload failed");
    const uploaded = jsonObject(await uploadedResponse.json(), "replay upload response");
    if (typeof uploaded.artifactId !== "string") throw new InteractDriverError("artifactTransferFailed", "Replay upload response has no artifact id.");
    const imported = await this.artifactRequest("import", { room, artifactId: uploaded.artifactId });
    await this.callBridge("status", {});
    return { uploaded, imported };
  }

  async artifactRequest(action: string, body: { room: string; name?: string; artifactId?: string }) {
    const response = await fetch(new URL(`dev/interact/lab/artifacts/${action}`, this.server!.baseUrl), {
      method: "POST",
      headers: { [ARTIFACT_CAPABILITY_HEADER]: this.artifactCapability, "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(this.options.timeoutMs),
    });
    if (!response.ok) throw await artifactHttpError(response, `replay ${action} failed`);
    return jsonObject(await response.json(), `replay ${action} response`);
  }

  async screenshot({
    sessionId,
    name = "scene",
    presentation = "clean",
    viewport = null,
    subjectIds = [],
    subjectSummaries = [],
    request = {}, region = "viewport",
  }: {
    sessionId?: string;
    name?: string;
    presentation?: string;
    viewport?: Viewport | null;
    subjectIds?: number[];
    subjectSummaries?: unknown[];
    request?: JsonObject; region?: CaptureRegion;
  } = {}) {
    try {
      return await this.captureScreenshot({
        sessionId,
        name,
        presentation,
        viewport,
        subjectIds,
        subjectSummaries,
        request, region,
      });
    } catch (error) {
      throw this.decorateError(error);
    }
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
      videoPath: recording.mp4Path,
      finalizing: recording.finalizing != null,
    };
  }

  async recordStart({
    sessionId,
    name = "recording",
    maxDurationMs = RECORDING_LIMITS.defaultDurationMs,
    viewport = null,
    crop = null, region = null,
    scale = 1,
    resumeSpeed = null,
    presentation = this.options.mode === "game" ? "normal" : "clean",
  }: {
    sessionId?: string;
    name?: string;
    maxDurationMs?: number;
    viewport?: Viewport | null;
    crop?: CaptureClip | null; region?: CaptureRegion | null;
    scale?: number;
    resumeSpeed?: number | null;
    presentation?: "clean" | "normal";
  } = {}) {
    try {
      if (this.recording) throw new InteractDriverError("recordingActive", "A recording is already active for this session. Stop it before starting another.");
      if (crop && region) throw new InteractDriverError("invalidRegion", "recording accepts crop or region, not both.");
      if (presentation !== "clean" && presentation !== "normal") throw new InteractDriverError("invalidPresentation", "recording presentation must be clean or normal.");
      if (region === "minimap" && presentation === "clean") throw new InteractDriverError("invalidPresentation", "The minimap is hidden in clean presentation; use normal presentation.");
      const tools = await checkMediaCapabilities();
      const normalizedSessionId = safeCaptureSessionId(sessionId);
      const normalizedViewport = viewport ? normalizeCaptureViewport(viewport) : null;
      const originalViewport = this.page!.viewport?.() || null;
      let recordingDir = "";
      let recorder = null;
      try {
        if (normalizedViewport) await this.page!.setViewport(normalizedViewport);
        await this.callBridge("presentation", { mode: presentation === "clean" ? "clean" : "default" });
        await this.page!.evaluate(() => document.fonts?.ready || Promise.resolve());
        await this.waitForCaptureReadiness([]);
        const resolvedRegion = await resolveCaptureRegion(this.page!, region || "viewport");
        const clip = crop ? normalizeRecordingCrop(crop, resolvedRegion.viewport) : resolvedRegion.clip;
        const artifactName = safeArtifactName(name, "recording");
        const suffix = new Date().toISOString().replace(/[:.]/g, "-");
        recordingDir = path.join(this.workspace!.root, interactArtifactRoot(this.options.mode), normalizedSessionId, "recordings", `${artifactName}-${suffix}`);
        fs.mkdirSync(recordingDir, { recursive: true });
        const mp4Path = path.join(recordingDir, `${artifactName}.mp4`);
        const startStatus = await this.callBridge("status", {});
        recorder = await createWallClockRecorder({
          page: this.page!, outputPath: mp4Path, clip, scale, tools, maxDurationMs,
          timeoutMs: this.options.timeoutMs,
        });
        let resumeResult = null;
        if (resumeSpeed != null) {
          resumeResult = await this.callBridge("time", { action: "resume", speed: resumeSpeed });
        }
        recorder.start();
        const startedMs = Date.now();
        const completion = recordingCompletion<RecordingResult>();
        const recording: ActiveRecording = {
          name: artifactName, recorder, tools, recordingDir, mp4Path,
          framesDir: path.join(recordingDir, "frames"), contactSheetPath: path.join(recordingDir, `${artifactName}-contact-sheet.png`),
          manifestPath: path.join(recordingDir, `${artifactName}.json`), startedMs, startedAt: new Date(startedMs).toISOString(),
          startStatus, resumeResult, resumeSpeed, clip, scale, presentation, region: resolvedRegion, viewport: normalizedViewport, originalViewport,
          maxDurationMs, finalizing: null, stoppedBy: null, operations: [], operationCount: 0,
          operationsTruncated: false, aliases: [], completion,
        };
        this.lastRecording = null;
        this.lastRecordingCompletion = completion;
        this.recording = recording;
        recording.watchdog = setTimeout(() => { void this.finishRecording("watchdog").catch(() => {}); }, maxDurationMs);
        recording.watchdog.unref?.();
        recording.sizeWatchdog = setInterval(() => {
          let size = 0;
          try { size = fs.statSync(recording.mp4Path).size; } catch {}
          if (size > RECORDING_LIMITS.maxBytes) void this.finishRecording("sizeLimit").catch(() => {});
        }, 250);
        recording.sizeWatchdog.unref?.();
        return {
          ...this.recordingStatus(), clip, scale,
          authoritativeStartTick: startStatus.snapshotTick ?? null,
          authoritativeResumed: resumeSpeed != null,
          resumeSpeed, presentation, region: resolvedRegion,
        };
      } catch (error) {
        if (recorder) await recorder.abort().catch(() => {});
        removePartialRecording([recordingDir]);
        if (originalViewport) await this.page!.setViewport(originalViewport).catch(() => {});
        await this.callBridge("presentation", { mode: "default" }).catch(() => {});
        throw error;
      }
    } catch (error) {
      throw this.decorateError(error);
    }
  }

  async captureFixed({ sessionId, name = "fixed", fps = 30, frameCount = 30, viewport = null, sceneIdentity = null, sceneRevision = 0, aliases = [] }: {
    sessionId?: string;
    name?: string;
    fps?: number;
    frameCount?: number;
    viewport?: Viewport | null;
    sceneIdentity?: JsonObject | null;
    sceneRevision?: number;
    aliases?: Array<{ alias: string; id: number }>;
  } = {}) {
    try {
      if (this.recording) throw new InteractDriverError("recordingActive", "Fixed capture is unavailable while real-time recording is active.");
      const normalizedSessionId = safeCaptureSessionId(sessionId);
      const artifactName = safeArtifactName(name, "fixed");
      const originalViewport = this.page!.viewport?.() || null;
      const normalizedViewport = viewport ? normalizeCaptureViewport(viewport) : null;
      let captureDir = "";
      let captureEntered = false;
      let encoder = null;
      try {
        const status = await this.callBridge("status", {});
        if (!status.roomTime?.paused) throw new InteractDriverError("roomTimeNotPaused", "capture-fixed requires paused authoritative room time.");
        if (normalizedViewport) await this.page!.setViewport(normalizedViewport);
        await this.callBridge("presentation", { mode: "clean" });
        await this.page!.evaluate(() => document.fonts?.ready || Promise.resolve());
        await this.waitForCaptureReadiness([]);
        const clip = await this.page!.evaluate(() => {
          const rect = document.getElementById("viewport")?.getBoundingClientRect?.();
          return rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null;
        });
        if (!validClip(clip)) throw new InteractDriverError("viewportUnavailable", "The Pixi viewport is not available for fixed capture.");
        this.fixedCapture = {
          active: true, cancelled: false, name: artifactName, fps, frameCount, frameIndex: 0,
          startStatus: status, abortController: new AbortController(),
        };
        const suffix = new Date().toISOString().replace(/[:.]/g, "-");
        captureDir = path.join(this.workspace!.root, interactArtifactRoot(this.options.mode), normalizedSessionId, "fixed", `${artifactName}-${suffix}`);
        const framesDir = path.join(captureDir, "frames");
        fs.mkdirSync(framesDir, { recursive: true });
        const videoPath = path.join(captureDir, `${artifactName}.mp4`);
        const contactSheetPath = path.join(captureDir, `${artifactName}-contact-sheet.png`);
        encoder = await createFixedCaptureEncoder({
          outputPath: videoPath,
          contactSheetPath,
          fps,
          frameCount,
          signal: this.fixedCapture.abortController.signal,
        });
        const representativeIndices = fixedRepresentativeIndices(frameCount);
        const entered = await this.callBridge("captureFixedEnter", {});
        captureEntered = true;
        const startTick = status.snapshotTick;
        if (typeof startTick !== "number" || !Number.isInteger(startTick)) throw new InteractDriverError("captureStateInvalid", "Fixed capture requires an authoritative start tick.");
        let currentTick = startTick;
        let processedPngBytes = 0;
        const frames = [];
        for (let index = 0; index < frameCount; index += 1) {
          if (this.fixedCapture?.cancelled) throw new InteractDriverError("captureCancelled", "Fixed capture was cancelled and its partial artifacts were removed.");
          this.fixedCapture.frameIndex = index;
          const tick = fixedFrameTick(startTick, index, fps);
          const ticks = tick - currentTick;
          if (ticks > 0) {
            await this.callBridge("time", { action: "step", ticks });
            currentTick = tick;
          }
          if (typeof entered.visualStartMs !== "number") throw new InteractDriverError("captureStateInvalid", "Fixed capture did not receive a visual start time.");
          const visualTimeMs = entered.visualStartMs + index * (1000 / fps);
          const rendered = await this.callBridge("captureFixedFrame", { visualTimeMs });
          const screenshot = Buffer.from(await this.page!.screenshot({ type: "png", clip }) || []);
          if (screenshot.length === 0) throw new InteractDriverError("captureEmpty", "Chrome returned an empty fixed-capture frame.");
          processedPngBytes += screenshot.length;
          if (screenshot.length > FIXED_CAPTURE_LIMITS.maxFrameBytes) throw new InteractDriverError("captureTooLarge", "One fixed-capture PNG exceeded its bounded frame budget.");
          await encoder.write(screenshot);
          let representativePath = null;
          if (representativeIndices.has(index)) {
            representativePath = path.join(framesDir, `frame-${String(index).padStart(4, "0")}.png`);
            fs.writeFileSync(representativePath, new Uint8Array(screenshot), { mode: 0o600 });
          }
          frames.push({ index, tick, visualTimeMs, rendererFrame: rendered.rendererFrame, sha256: hashFrame(new Uint8Array(screenshot)), representativePath });
        }
        const media = await encoder.finish();
        encoder = null;
        const endStatus = await this.callBridge("status", {});
        const diagnostics = this.diagnostics();
        const manifestPath = path.join(captureDir, `${artifactName}.json`);
        const manifest = {
          schemaVersion: 2, kind: "interactFixedCapture", deterministicEnvironmentOnly: true,
          workspace: this.workspace, serverBuild: this.server?.build || null,
          scene: {
            identity: sceneIdentity || { source: "launch", scenario: this.options.scenario, seed: this.options.seed || null, map: this.options.map },
            revision: sceneRevision,
            aliases: boundedSummary(aliases, INTERACT_SUMMARY_LIMITS.detailedAliases),
            selection: boundedEntityIds(Array.isArray(status.selection) ? status.selection : []),
          },
          mapping: { simulationHz: 30, outputFps: fps, rule: "frame i uses startTick + floor(i * 30 / outputFps); repeated ticks do not interpolate world state" },
          authoritative: { startTick, endTick: endStatus.snapshotTick },
          camera: {
            start: status.camera || null,
            end: endStatus.camera || null,
            viewport: endStatus.cameraViewport || null,
            worldBounds: endStatus.cameraWorldBounds || null,
          },
          capture: {
            frameCount, clip, viewport: normalizedViewport, visualStartMs: entered.visualStartMs,
            streaming: true, retainedPngFrames: representativeIndices.size, processedPngBytes,
          },
          frames, media: { videoPath, contactSheetPath, bytes: media.bytes, tools: media.tools, probe: media.probe, contactSheet: media.contactSheet },
          runtime: { node: process.version, platform: process.platform, architecture: process.arch, browser: this.browserVersion || null },
          errors: { pageConsole: diagnostics.pageConsoleErrors, page: diagnostics.pageErrors, requestFailures: diagnostics.requestFailures },
        };
        fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
        const result = {
          videoPath, contactSheetPath, manifestPath,
          frameSummary: {
            count: frames.length,
            uniqueHashes: new Set(frames.map((frame) => frame.sha256)).size,
            representativeFramePaths: frames.map((frame) => frame.representativePath).filter(Boolean),
            detailsInManifest: true,
          },
          authoritative: manifest.authoritative, mapping: manifest.mapping, probe: media.probe,
        };
        this.lastFixedCapture = result;
        return result;
      } catch (error) {
        if (encoder) await encoder.abort().catch(() => {});
        removePartialRecording([captureDir]);
        if (this.fixedCapture?.cancelled) {
          throw new InteractDriverError("captureCancelled", "Fixed capture was cancelled and its partial artifacts were removed.");
        }
        if (error instanceof InteractRecordingError) throw new InteractDriverError(error.code, error.message);
        throw error;
      } finally {
        if (captureEntered) await this.callBridge("captureFixedExit", {}).catch(() => {});
        await this.callBridge("presentation", { mode: "default" }).catch(() => {});
        if (originalViewport) await this.page?.setViewport(originalViewport).catch(() => {});
        this.fixedCapture = null;
      }
    } catch (error) {
      throw this.decorateError(error);
    }
  }

  captureTimelapse(input = {}) { return captureGameTimelapse(this, input); }
  fixedCaptureStatus() {
    if (!this.fixedCapture) return { active: false, last: this.lastFixedCapture };
    const { cancelled, name, fps, frameCount, frameIndex } = this.fixedCapture;
    return { active: true, cancelled, name, fps, frameCount, frameIndex };
  }

  cancelFixedCapture() {
    if (!this.fixedCapture) throw new InteractDriverError("captureInactive", "No fixed or time-lapse capture is active.");
    this.fixedCapture.cancelled = true;
    this.fixedCapture.abortController?.abort();
    return { cancelling: true };
  }

  recordAcceptedOperation(operation: unknown, aliases: Array<{ alias: string; id: number }> = []) {
    if (!this.recording || this.recording.finalizing) return false;
    this.recording.operationCount += 1;
    if (this.recording.operations.length < RECORDING_LIMITS.maxOperations) this.recording.operations.push(operation);
    else this.recording.operationsTruncated = true;
    this.recording.aliases = Array.isArray(aliases) ? aliases.slice(0, RECORDING_LIMITS.maxAliases) : [];
    return true;
  }

  async recordStop(metadata: JsonObject = {}) {
    try {
      const recording = this.recording;
      if (!recording) {
        throw new InteractDriverError(
          "recordingInactive",
          "No recording is active for this session. Start one before stopping.",
        );
      }
      return await this.finishRecording("explicit", metadata);
    } catch (error) {
      throw this.decorateError(error);
    }
  }

  recordWait() {
    const completion = this.recording?.completion || this.lastRecordingCompletion;
    if (!completion) {
      return Promise.reject(new InteractDriverError(
        "recordingInactive",
        "No recording has been started for this session. Start one before waiting.",
      ));
    }
    return completion.promise;
  }

  settleRecording(reason: string, metadata: JsonObject = {}) {
    if (!this.recording) return null;
    return this.finishRecording(reason, metadata);
  }

  async finishRecording(reason: string, metadata: JsonObject = {}) {
    const recording = this.recording;
    if (!recording) throw new InteractDriverError("recordingInactive", "No recording is active for this session. Start one before stopping.");
    if (recording.finalizing) return recording.finalizing;
    recording.stoppedBy = reason;
    const finalizing = (async (): Promise<RecordingResult> => {
      clearTimeout(recording.watchdog);
      clearInterval(recording.sizeWatchdog);
      try {
        const endStatus = await this.callBridge("status", {}).catch(() => null);
        // The recorder owns the monotonic clock used to assign frame slots.
        // Reuse that exact duration for probing and the manifest so wall-clock
        // rounding cannot disagree with the encoded frame count.
        const { wallDurationMs, diagnostics: frameDiagnostics } = await recording.recorder.stop();
        const media = await finalizeMp4Artifacts({
          mp4Path: recording.mp4Path, framesDir: recording.framesDir,
          contactSheetPath: recording.contactSheetPath, targetDurationMs: wallDurationMs,
          tools: recording.tools, frameDiagnostics,
          signal: undefined,
        });
        const diagnostics = this.diagnostics();
        const manifest = {
          schemaVersion: 2,
          kind: "interactRealTimeRecording",
          createdAt: recording.startedAt,
          finalizedAt: new Date().toISOString(),
          stoppedBy: reason,
          nondeterministic: true,
          workspace: this.workspace,
          serverBuild: this.server?.build || null,
          runtime: { node: process.version, platform: process.platform, architecture: process.arch },
          browser: { chrome: this.browserVersion || null, userAgent: await this.page!.evaluate(() => navigator.userAgent).catch(() => null) },
          mediaTools: recording.tools,
          authoritative: {
            startTick: recording.startStatus?.snapshotTick ?? null,
            endTick: endStatus?.snapshotTick ?? null,
            startRoomTime: recording.startStatus?.roomTime ?? null,
            endRoomTime: endStatus?.roomTime ?? null,
          },
          camera: {
            start: recording.startStatus?.camera ?? null,
            end: endStatus?.camera ?? null,
            viewport: endStatus?.cameraViewport ?? null,
            worldBounds: endStatus?.cameraWorldBounds ?? null,
          },
          capture: {
            fps: RECORDING_LIMITS.fps, audio: false, timingAuthority: "monotonicWallClockFrameSlots",
            clip: recording.clip, scale: recording.scale, viewport: recording.viewport,
            region: recording.region, presentation: recording.presentation,
            wallDurationMs, maxDurationMs: recording.maxDurationMs,
            atomicResume: recording.resumeSpeed == null ? null : { speed: recording.resumeSpeed, result: recording.resumeResult },
          },
          aliases: boundedSummary(
            Array.isArray(metadata.aliases) ? metadata.aliases.slice(0, RECORDING_LIMITS.maxAliases) : recording.aliases,
            RECORDING_LIMITS.maxDetailedAliases,
          ),
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
        const result: RecordingResult = {
          active: false, stoppedBy: reason, videoPath: recording.mp4Path,
          framePaths: media.framePaths, contactSheetPath: recording.contactSheetPath,
          manifestPath: recording.manifestPath, probe: media.probe,
          frameDiagnostics: media.frameDiagnostics,
          authoritative: manifest.authoritative,
        };
        this.lastRecording = result;
        return result;
      } catch (error) {
        removePartialRecording([recording.recordingDir]);
        const failure = error instanceof InteractRecordingError
          ? new InteractDriverError(error.code, error.message, error.details)
          : error;
        // finishRecording is also called by watchdog and lifecycle settlement.
        // Decorate here so every observer of the shared
        // completion receives the same normalized failure object.
        throw this.decorateError(failure);
      } finally {
        if (recording.originalViewport) await this.page?.setViewport(recording.originalViewport).catch(() => {});
        await this.callBridge("presentation", { mode: "default" }).catch(() => {});
        if (this.recording === recording) this.recording = null;
      }
    })();
    recording.finalizing = finalizing;
    void finalizing.then(
      (result) => recording.completion.resolve(result),
      (error) => recording.completion.reject(error),
    );
    return finalizing;
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

  async call(method: string, input: JsonObject): Promise<BridgeResult> {
    try {
      return await this.callBridge(method, input);
    } catch (error) {
      throw this.decorateError(error);
    }
  }

  async callBridge(method: string, input: JsonObject): Promise<BridgeResult> {
    if (this.state !== DRIVER_STATES.OPEN || !this.page) {
      throw new InteractDriverError("sessionClosed", "Interact driver session is not open.");
    }
    const result = await evaluateInteractBridgeCall({
      page: this.page, method, input, timeoutMs: this.options.timeoutMs,
      startupTimeoutMs: this.options.startupTimeoutMs, withTimeout,
    });
    if (!result?.ok) {
      throw new InteractDriverError(
        result?.error?.code || "bridgeError",
        result?.error?.message || `Interact ${method} failed.`,
        { method, ...(result?.error?.details || {}) },
      );
    }
    if (!result.value || typeof result.value !== "object" || Array.isArray(result.value)) {
      throw new InteractDriverError("bridgeError", `Interact ${method} returned a non-object result.`);
    }
    return result.value as BridgeResult;
  }

  async captureScreenshot({ sessionId, name, presentation, viewport, region, subjectIds, subjectSummaries, request }: {
    sessionId?: string;
    name: string;
    presentation: string;
    viewport: Viewport | null; region: CaptureRegion;
    subjectIds: number[];
    subjectSummaries: unknown[];
    request: JsonObject;
  }) {
    if (this.recording) {
      throw new InteractDriverError(
        "recordingActive",
        "Screenshot capture is unavailable while recording because it can change the active viewport or presentation. Stop the recording first.",
      );
    }
    if (presentation !== "clean" && presentation !== "normal") {
      throw new InteractDriverError("invalidPresentation", "presentation must be clean or normal.");
    }
    const normalizedSessionId = safeCaptureSessionId(sessionId);
    const artifactName = safeArtifactName(name);
    const normalizedViewport = viewport ? normalizeCaptureViewport(viewport) : null;
    const originalViewport = this.page!.viewport?.() || null;
    const requestedSubjectIds = boundedEntityIds(subjectIds);
    try {
      if (normalizedViewport) await this.page!.setViewport(normalizedViewport);
      await this.callBridge("presentation", { mode: presentation === "clean" ? "clean" : "default" });
      if (region === "minimap" && presentation === "clean") throw new InteractDriverError("invalidPresentation", "The minimap is hidden in clean presentation; use normal presentation.");
      await this.page!.evaluate(() => document.fonts?.ready || Promise.resolve());
      const readiness = await this.waitForCaptureReadiness(requestedSubjectIds);
      const resolvedRegion = await resolveCaptureRegion(this.page!, region);
      const clip = resolvedRegion.clip;

      const captureDir = path.join(this.workspace!.root, interactArtifactRoot(this.options.mode), normalizedSessionId, "captures");
      fs.mkdirSync(captureDir, { recursive: true });
      const suffix = new Date().toISOString().replace(/[:.]/g, "-");
      const baseName = `${artifactName}-${suffix}`;
      const pngPath = path.join(captureDir, `${baseName}.png`);
      const manifestPath = path.join(captureDir, `${baseName}.json`);
      const screenshot = await this.page!.screenshot({ type: "png", clip, path: pngPath });
      const png = Buffer.from(screenshot || []);
      if (png.length === 0) {
        throw new InteractDriverError("captureEmpty", "Chrome returned an empty Pixi screenshot.");
      }
      if (png.length > MAX_CAPTURE_BYTES) {
        fs.rmSync(pngPath, { force: true });
        throw new InteractDriverError("captureTooLarge", `Screenshot exceeds the ${MAX_CAPTURE_BYTES} byte response bound.`);
      }
      const dimensions = readPngDimensions(png);
      const diagnostics = this.diagnostics();
      const subjectSummary = boundedSummary(subjectSummaries, INTERACT_SUMMARY_LIMITS.detailedSubjects);
      const readinessSummary = summarizeCaptureReadiness(readiness);
      const manifest = {
        schemaVersion: 2,
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
          clip, region: resolvedRegion,
          output: dimensions,
        },
        camera: readiness.camera,
        cameraViewport: readiness.cameraViewport,
        cameraWorldBounds: readiness.cameraWorldBounds,
        selection: boundedEntityIds(Array.isArray(readiness.selection) ? readiness.selection : []),
        subjects: subjectSummary,
        visualProfileId: readiness.visualProfileId || null,
        assetReadiness: readiness.assets,
        errors: {
          pageConsole: diagnostics.pageConsoleErrors,
          page: diagnostics.pageErrors,
          requestFailures: diagnostics.requestFailures,
          frame: readiness.frameErrors,
          render: readiness.renderErrors,
          missingTextureSubjectIds: readinessSummary.missingTextureSubjectIds,
          missingTextureSubjectCount: readinessSummary.missingTextureSubjectCount,
          missingTextureSubjectsTruncated: readinessSummary.missingTextureSubjectsTruncated,
        },
        presentation,
        request: boundedRequestMetadata(request),
        browser: {
          chrome: this.browserVersion || null,
          puppeteer: await this.page!.evaluate(() => navigator.userAgent),
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
        presentation, region: resolvedRegion,
        readiness: readinessSummary,
      };
    } finally {
      if (originalViewport) await this.page!.setViewport(originalViewport).catch(() => {});
      await this.callBridge("presentation", { mode: "default" }).catch(() => {});
    }
  }

  async waitForCaptureReadiness(subjectIds: number[]) {
    const deadline = Date.now() + this.options.timeoutMs;
    let initialFrame = null;
    let last = null;
    while (Date.now() < deadline) {
      const readiness = await this.callBridge("captureReadiness", { subjectIds });
      if (initialFrame == null) initialFrame = Number(readiness.frame) || 0;
      last = readiness;
      const errors = readiness.frameErrors?.length || readiness.renderErrors?.length ||
        readiness.missingTextureSubjectIds?.length || this.pageErrors.length || this.pageConsoleErrors.length;
      if (errors) throw new InteractDriverError("captureRenderError", captureReadinessMessage(readiness, this.diagnostics()));
      if (readiness.failedAssets?.length) {
        throw new InteractDriverError("assetLoadFailed", captureReadinessMessage(readiness, this.diagnostics()));
      }
      if (readiness.ready && (readiness.phase === "concluded" || Number(readiness.frame) >= initialFrame + 2)) return readiness;
      await sleep(25);
    }
    throw new InteractDriverError("captureTimeout", captureReadinessMessage(last, this.diagnostics()));
  }

  launchUrl() {
    return interactLaunchUrl({
      mode: this.options.mode,
      baseUrl: this.server!.baseUrl,
      room: this.options.mode === "game" ? this.gameRoom : generatedRoomId(this.workspace!.head),
      map: this.options.map,
      opponent: this.options.opponent,
      spectate: this.options.spectate,
      renderer: this.options.renderer,
      seed: this.options.seed,
      scenario: this.options.scenario,
      devScenario: this.options.devScenario,
    });
  }

  attachPageDiagnostics() {
    this.page!.on("console", (message) => {
      if (message.type() === "error") appendBounded(this.pageConsoleErrors, message.text());
    });
    this.page!.on("pageerror", (error) => appendBounded(this.pageErrors, error.message));
    this.page!.on("requestfailed", (request) => {
      if (!request.url().includes("favicon")) {
        appendBounded(this.requestFailures, `${request.failure()?.errorText || "request failed"} ${request.url()}`);
      }
    });
    this.page!.on("close", () => {
      if (this.recording) void this.finishRecording("pageClosed").catch(() => {});
    });
  }

  async close() {
    if (this.closePromise) return this.closePromise;
    this.closePromise = (async () => {
      if (this.state === DRIVER_STATES.CLOSED) return;
      if (this.recording) {
        await this.settleRecording("sessionClose")?.catch(() => {
          removePartialRecording(this.recording?.recordingDir ? [this.recording.recordingDir] : []);
          this.recording = null;
        });
      }
      if (this.options.mode === "lab" && this.page && this.server) {
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

  transition(event: string) {
    this.state = transitionDriverState(this.state, event);
  }

  writeManifest(extra: JsonObject) {
    if (!this.sessionDir) return;
    const manifest = {
      schemaVersion: 1,
      workspace: this.workspace ? {
        root: this.workspace!.root,
        branch: this.workspace!.branch,
        head: this.workspace!.head,
      } : null,
      session: {
        state: this.state,
        createdAt: new Date().toISOString(),
      },
      ...extra,
    };
    fs.writeFileSync(path.join(this.sessionDir, "session.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  }

  decorateError(error: unknown) {
    if (error instanceof InteractDriverError && error.details?.diagnostics) return error;
    const diagnostics = this.diagnostics();
    const serverTail = diagnostics.serverLog ? readTail(diagnostics.serverLog, LOG_TAIL_LINES) : [];
    const source = error instanceof Error ? error : null;
    const code = source && "code" in source && typeof source.code === "string" ? source.code : "driverError";
    const details = source && "details" in source && source.details && typeof source.details === "object" && !Array.isArray(source.details)
      ? source.details as JsonObject
      : {};
    const message = [source?.message || "Interact driver failed."]
      .concat(serverTail.length ? [`Server log tail:\n${serverTail.join("\n")}`] : [])
      .join("\n");
    return new InteractDriverError(code, message, {
      ...details,
      diagnostics: { ...diagnostics, serverTail },
    });
  }
}

function recordingCompletion<T>(): Completion<T> {
  let resolvePromise!: (value: T) => void;
  let rejectPromise!: (reason?: unknown) => void;
  let settled = false;
  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  // A watchdog or lifecycle close may settle before a caller starts waiting.
  // Register a rejection observer without changing the promise shared by waiters.
  void promise.catch(() => {});
  return {
    promise,
    resolve(value: T) {
      if (settled) return false;
      settled = true;
      resolvePromise(value);
      return true;
    },
    reject(error: unknown) {
      if (settled) return false;
      settled = true;
      rejectPromise(error);
      return true;
    },
  };
}

export function safeToken(value: string, fallback = "session", maxLength = 64) {
  const token = String(value || "").trim();
  return /^[A-Za-z0-9_-]+$/.test(token) && token.length <= maxLength ? token : fallback;
}

export function safeArtifactName(value: string, fallback = "scene") {
  return safeToken(value, fallback, 48);
}

function safeCaptureSessionId(value: unknown) {
  const sessionId = String(value || "").trim();
  if (!/^(?:lab|game|scenario)_[a-f0-9]{32}$/.test(sessionId)) {
    throw new InteractDriverError("invalidSession", "sessionId must be a valid Interact session id.");
  }
  return sessionId;
}

function normalizeCaptureViewport(viewport: Viewport) {
  const normalized = normalizeViewport(viewport);
  if (normalized.width > MAX_CAPTURE_VIEWPORT || normalized.height > MAX_CAPTURE_VIEWPORT) {
    throw new InteractDriverError("invalidViewport", `capture viewport width and height must be at most ${MAX_CAPTURE_VIEWPORT}.`);
  }
  return normalized;
}

function normalizeRecordingCrop(crop: CaptureClip, viewportClip: CaptureClip) {
  const normalized = { x: Number(crop.x), y: Number(crop.y), width: Number(crop.width), height: Number(crop.height) };
  if (!Object.values(normalized).every(Number.isFinite) || normalized.x < 0 || normalized.y < 0 || normalized.width < 2 || normalized.height < 2) {
    throw new InteractDriverError("invalidCrop", "recording crop must contain finite non-negative x/y and width/height of at least 2.");
  }
  const absolute = { x: viewportClip.x + normalized.x, y: viewportClip.y + normalized.y, width: normalized.width, height: normalized.height };
  if (absolute.x + absolute.width > viewportClip.x + viewportClip.width || absolute.y + absolute.height > viewportClip.y + viewportClip.height) {
    throw new InteractDriverError("invalidCrop", "recording crop must stay inside the game viewport.");
  }
  return absolute;
}

function boundedEntityIds(values: unknown) {
  if (!Array.isArray(values) || values.length > 400) {
    throw new InteractDriverError("invalidSubjects", "subjectIds must contain at most 400 positive entity ids.");
  }
  const ids = [...new Set(values.map(Number))];
  if (!ids.every((id) => Number.isInteger(id) && id > 0)) {
    throw new InteractDriverError("invalidSubjects", "subjectIds must contain positive integer entity ids.");
  }
  return ids;
}

function summarizeCaptureReadiness(readiness: BridgeResult) {
  const subjects = boundedSummary(readiness.subjects || [], INTERACT_SUMMARY_LIMITS.detailedSubjects);
  const missingTextures = boundedSummary(
    readiness.missingTextureSubjectIds || [],
    INTERACT_SUMMARY_LIMITS.detailedSubjects,
  );
  return {
    ...(readiness || {}),
    subjects,
    missingTextureSubjectIds: missingTextures.details,
    missingTextureSubjectCount: missingTextures.count,
    missingTextureSubjectsTruncated: missingTextures.truncated,
  };
}

function validClip(clip: unknown): clip is CaptureClip {
  if (!clip || typeof clip !== "object" || Array.isArray(clip)) return false;
  const value = clip as JsonObject;
  return typeof value.x === "number" && typeof value.y === "number" && typeof value.width === "number" && typeof value.height === "number" &&
    Number.isFinite(value.x) && Number.isFinite(value.y) && Number.isFinite(value.width) && Number.isFinite(value.height) &&
    value.width >= 1 && value.height >= 1 && value.width <= MAX_CAPTURE_VIEWPORT && value.height <= MAX_CAPTURE_VIEWPORT;
}

function boundedRequestMetadata(request: unknown): JsonObject {
  const text = JSON.stringify(request && typeof request === "object" ? request : {});
  if (text.length > 4000) return { truncated: true };
  return jsonObject(JSON.parse(text) as unknown, "capture request metadata");
}

function captureReadinessMessage(readiness: BridgeResult | null, diagnostics: ReturnType<InteractDriver["diagnostics"]>) {
  const failures = [
    ...(readiness?.failedAssets || []).map((asset) => `${recordField(asset, "id")}: ${recordField(asset, "message") || "failed"}`),
    ...(readiness?.pendingAssets || []).map((asset) => `${recordField(asset, "id")}: pending`),
    ...(readiness?.frameErrors || []).map((error) => `frame: ${recordField(error, "message") || "failed"}`),
    ...(readiness?.renderErrors || []).map((error) => `render ${recordField(error, "label")}: ${recordField(error, "message") || "failed"}`),
    ...(readiness?.missingTextureSubjectIds || []).map((id) => `subject ${id}: missing texture fallback`),
    ...(diagnostics?.pageErrors || []).map((error) => `page: ${error}`),
    ...(diagnostics?.pageConsoleErrors || []).map((error) => `console: ${error}`),
  ];
  return failures.length ? `Screenshot readiness failed: ${failures.slice(0, 12).join("; ")}` : "Screenshot did not become ready before the timeout.";
}

function readPngDimensions(buffer: Buffer) {
  if (buffer.length < 24 || buffer.toString("ascii", 1, 4) !== "PNG") {
    throw new InteractDriverError("invalidCapture", "Chrome did not return a PNG image.");
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

export function generatedRoomId(head = "") {
  const suffix = crypto.randomBytes(6).toString("hex");
  return safeToken(`interact-lab-${safeToken(head.slice(0, 8), "head", 8)}-${process.pid}-${suffix}`, "interact-lab", 40);
}

export function transitionDriverState(state: string, event: string) {
  const transitions: Record<string, Record<string, string>> = {
    [DRIVER_STATES.OPENING]: { opened: DRIVER_STATES.OPEN, closing: DRIVER_STATES.CLOSING },
    [DRIVER_STATES.OPEN]: { closing: DRIVER_STATES.CLOSING },
    [DRIVER_STATES.CLOSING]: { closed: DRIVER_STATES.CLOSED },
    [DRIVER_STATES.CLOSED]: {},
  };
  const next = transitions[state]?.[event];
  if (!next) throw new InteractDriverError("invalidLifecycle", `Cannot ${event} Interact driver from ${state}.`);
  return next;
}

export async function withTimeout<T>(promise: PromiseLike<T>, timeoutMs: number | undefined, detail = "operation"): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new InteractDriverError("timeout", `${detail} timed out after ${timeoutMs}ms.`)), timeoutMs);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

async function artifactHttpError(response: Response, fallback: string) {
  let message = fallback;
  try { message = (await response.json())?.error || message; } catch {}
  return new InteractDriverError("artifactTransferFailed", message);
}

function normalizeViewport(viewport: { width: number; height: number; deviceScaleFactor?: number; dpr?: number }): Viewport {
  const width = Number(viewport?.width);
  const height = Number(viewport?.height);
  const deviceScaleFactor = Number(viewport?.deviceScaleFactor ?? viewport?.dpr ?? 1);
  if (!Number.isInteger(width) || width < 320 || width > 4096 || !Number.isInteger(height) || height < 240 || height > 4096 || !Number.isFinite(deviceScaleFactor) || deviceScaleFactor <= 0 || deviceScaleFactor > 4) {
    throw new InteractDriverError("invalidViewport", "viewport must have bounded width, height, and DPR.");
  }
  return { width, height, deviceScaleFactor };
}

function boundedTimeout(value: number, label: string, maximum: number) {
  const timeoutMs = Number(value);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > maximum) {
    throw new InteractDriverError("invalidTimeout", `${label} must be an integer from 1 to ${maximum}ms.`);
  }
  return timeoutMs;
}

async function loadPuppeteer() {
  let imported;
  try {
    imported = await import("puppeteer-core");
  } catch (error) {
    throw new InteractDriverError(
      "puppeteerUnavailable",
      `puppeteer-core is not installed from the repository package lock; run npm ci at the repository root (${error instanceof Error && "code" in error ? String(error.code) : "import failed"}).`,
    );
  }
  return imported.default || imported;
}

function recordField(value: unknown, field: string): unknown {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as JsonObject)[field] : undefined;
}

function jsonObject(value: unknown, label: string): JsonObject {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new InteractDriverError("artifactTransferFailed", `${label} was not an object.`);
  }
  return value as JsonObject;
}

function readTail(file: fs.PathOrFileDescriptor, maxLines: number) {
  try {
    return fs.readFileSync(file, "utf8")
      .trimEnd()
      .split("\n")
      .slice(-maxLines)
      .map((line) => boundLogLine(line));
  } catch {
    return [];
  }
}

export function boundLogLine(value: string, maxChars = LOG_TAIL_LINE_CHARS) {
  const line = String(value ?? "");
  const marker = " …<truncated>… ";
  const limit = Number.isInteger(maxChars) ? Math.max(0, maxChars) : LOG_TAIL_LINE_CHARS;
  if (line.length <= limit) return line;
  if (limit <= marker.length) return marker.slice(0, limit);
  const available = limit - marker.length;
  const leading = Math.ceil(available / 2);
  const trailing = Math.floor(available / 2);
  return `${line.slice(0, leading)}${marker}${trailing > 0 ? line.slice(-trailing) : ""}`;
}

function appendBounded(values: string[], value: string) {
  values.push(boundLogLine(value));
  if (values.length > MAX_PAGE_ERRORS) values.splice(0, values.length - MAX_PAGE_ERRORS);
}

function sleep(ms: number|undefined) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
