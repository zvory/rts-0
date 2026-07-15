import {
  analyzeSelfProfile,
  renderSelfProfileFlamegraph,
  selfProfileSummary,
} from "./stress_test_profile.js";

const DEVICE_ID_KEY = "rts.stressTest.deviceId";
const PROFILE_INTERVAL_MS = 10;
const PROFILE_BUFFER_SAMPLES = 4_000;

export function stressTestHeadroom(frameWorkP95Ms) {
  const p95 = Number(frameWorkP95Ms);
  if (!(p95 > 0)) return { sustainableFps: 0, text: "No frame samples were recorded." };
  const sustainableFps = p95 <= 4.17 ? 240
    : p95 <= 8.34 ? 120
      : p95 <= 16.67 ? 60
        : p95 <= 33.34 ? 30
          : Math.max(1, Math.floor(1000 / p95));
  const ratio = p95 / (1000 / 60);
  const text = ratio <= 1
    ? `${(1 / ratio).toFixed(1)}× headroom against a 60 FPS frame-work budget.`
    : `Needs about ${ratio.toFixed(1)}× less frame work to hold 60 FPS.`;
  return { sustainableFps, text };
}

export class StressTestRunner {
  constructor({ launch, fetchFn = (...args) => fetch(...args) }) {
    this.launch = launch;
    this.fetchFn = fetchFn;
    this.started = false;
    this.root = null;
    this.publicState = { state: "loading", result: null, error: "" };
  }

  mount() {
    if (this.root || typeof document === "undefined") return;
    this.root = document.createElement("aside");
    this.root.id = "stress-test-status";
    this.root.setAttribute("aria-live", "polite");
    document.getElementById("app")?.appendChild(this.root);
    this.renderStatus("Loading the Hellhole workload…");
    globalThis.__rtsStressTest = this.publicState;
  }

  async run({ match, net }) {
    if (this.started) return;
    this.started = true;
    const visibility = { changes: 0, hiddenDuringRun: document.hidden, focusedAtStart: document.hasFocus() };
    const onVisibilityChange = () => {
      visibility.changes += 1;
      visibility.hiddenDuringRun ||= document.hidden;
    };
    document.addEventListener("visibilitychange", onVisibilityChange);

    try {
      this.setState("warmup");
      this.renderStatus(`Warming up for ${this.launch.warmupSeconds} seconds…`);
      const refreshPromise = estimateRefreshRate();
      const environmentPromise = collectEnvironment(match);
      await delay(this.launch.warmupSeconds * 1000);
      const refreshRateHz = await refreshPromise;
      const environment = await environmentPromise;

      match?.frameProfiler?.reset();
      const observer = new BrowserTimingObserver();
      observer.start();
      let profiler = null;
      let profilerError = "";
      if (typeof globalThis.Profiler === "function") {
        try {
          profiler = new globalThis.Profiler({
            sampleInterval: PROFILE_INTERVAL_MS,
            maxBufferSize: PROFILE_BUFFER_SAMPLES,
          });
        } catch (error) {
          profilerError = String(error?.message || error).slice(0, 240);
        }
      }

      this.setState("measuring");
      this.renderStatus(`Measuring ${this.launch.durationSeconds} seconds… Keep this tab visible.`);
      const measuredAt = new Date().toISOString();
      const measuredStarted = performance.now();
      await delay(this.launch.durationSeconds * 1000);
      const measuredDurationMs = Math.round(performance.now() - measuredStarted);
      const browserTiming = observer.stop();

      let profile = {
        kind: "phase-timings",
        supported: false,
        error: profilerError || (typeof globalThis.Profiler === "function"
          ? "The JS profiler could not start."
          : "The JS Self-Profiling API is unavailable in this browser."),
        trace: null,
        summary: null,
        flamegraphSvg: "",
      };
      if (profiler) {
        try {
          const rawTrace = await profiler.stop();
          const trace = JSON.parse(JSON.stringify(rawTrace));
          const analysis = analyzeSelfProfile(trace);
          profile = {
            kind: "js-self-profile",
            supported: true,
            error: "",
            trace,
            summary: selfProfileSummary(analysis),
            flamegraphSvg: renderSelfProfileFlamegraph(analysis, {
              title: `Hellhole stress test${this.launch.label ? ` — ${this.launch.label}` : ""}`,
            }),
          };
        } catch (error) {
          profile.error = String(error?.message || error).slice(0, 240);
        }
      }

      visibility.hiddenDuringRun ||= document.hidden;
      const frameSummary = match?.frameProfiler?.reportSummary?.() || {};
      const invalidReasons = [];
      if (visibility.hiddenDuringRun || visibility.changes > 0) invalidReasons.push("tab visibility changed");
      if (!visibility.focusedAtStart || !document.hasFocus()) invalidReasons.push("tab was not focused");
      if ((frameSummary.frameCount || 0) < 30) invalidReasons.push("too few rendered frames");
      const payload = {
        schemaVersion: 1,
        workloadId: this.launch.id,
        userLabel: this.launch.label,
        deviceId: stableDeviceId(),
        fingerprint: await environmentFingerprint(environment),
        measuredAt,
        measuredDurationMs,
        status: invalidReasons.length ? "invalid" : "completed",
        invalidReasons,
        environment: { ...environment, refreshRateHz },
        stream: sanitizeStreamState(net?.publicState),
        frameSummary,
        browserTiming,
        profile,
      };

      this.setState("uploading");
      this.renderStatus("Saving labeled diagnostics…");
      const saved = await this.save(payload);
      const result = { ...payload, server: saved };
      this.publicState.result = result;
      this.setState("complete");
      this.renderResult(result);
      console.info("[stress-test] complete", {
        runId: saved.runId,
        artifactLabel: saved.artifactLabel,
        frameWorkP95Ms: frameSummary.frameWorkP95Ms,
        rendererP95Ms: frameSummary.rendererP95Ms,
        profileKind: profile.kind,
        profileSamples: profile.summary?.sampleCount || 0,
        persisted: saved.persisted,
      });
    } catch (error) {
      this.publicState.error = String(error?.message || error);
      this.setState("failed");
      this.renderFailure(this.publicState.error);
      console.error("[stress-test] failed", error);
    } finally {
      document.removeEventListener("visibilitychange", onVisibilityChange);
    }
  }

  async save(payload) {
    const response = await this.fetchFn("/api/stress-tests", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!response.ok) throw new Error(`Saving diagnostics failed (${response.status})`);
    return response.json();
  }

  setState(state) {
    this.publicState.state = state;
    if (this.root) this.root.dataset.state = state;
  }

  fail(message) {
    if (this.publicState.state === "complete") return;
    this.publicState.error = String(message || "The stress test failed.");
    this.setState("failed");
    this.renderFailure(this.publicState.error);
  }

  renderStatus(text) {
    if (!this.root) return;
    this.root.replaceChildren();
    const strong = document.createElement("strong");
    strong.textContent = "Stress Test";
    const status = document.createElement("span");
    status.textContent = text;
    this.root.append(strong, status);
  }

  renderResult(result) {
    if (!this.root) return;
    this.root.replaceChildren();
    const frame = result.frameSummary || {};
    const profile = result.profile || {};
    const saved = result.server || {};
    const headroom = stressTestHeadroom(frame.frameWorkP95Ms);
    const averageFps = result.measuredDurationMs > 0
      ? (Number(frame.frameCount || 0) * 1000 / result.measuredDurationMs)
      : 0;
    const title = document.createElement("strong");
    title.textContent = result.status === "completed" ? "Stress Test Complete" : "Stress Test Invalid";
    const label = document.createElement("code");
    label.textContent = saved.artifactLabel || "unsaved-result";
    const metrics = document.createElement("dl");
    addMetric(metrics, "Rendered average", `${averageFps.toFixed(1)} FPS`);
    addMetric(metrics, "Frame work p95", `${frame.frameWorkP95Ms || 0} ms`);
    addMetric(metrics, "Renderer p95", `${frame.rendererP95Ms || 0} ms`);
    addMetric(metrics, "Frame-work tier", `${headroom.sustainableFps || "<1"} FPS`);
    addMetric(metrics, "JS profile", profile.supported
      ? `${profile.summary?.sampleCount || 0} samples`
      : "unavailable; phase timings saved");
    const verdict = document.createElement("p");
    verdict.textContent = headroom.text;
    if (result.invalidReasons?.length) verdict.textContent += ` Invalid: ${result.invalidReasons.join(", ")}.`;
    const storage = document.createElement("p");
    storage.className = "stress-test-storage";
    storage.textContent = saved.persisted
      ? "Saved durably to Postgres and server logs."
      : "Saved in this server process and server logs; Postgres persistence is off.";
    const links = document.createElement("div");
    links.className = "stress-test-links";
    if (saved.resultUrl) links.append(downloadLink(saved.resultUrl, "Result JSON"));
    if (saved.flamegraphUrl && profile.supported) {
      links.append(downloadLink(saved.flamegraphUrl, "Flame graph SVG"));
    }
    const retry = document.createElement("button");
    retry.type = "button";
    retry.textContent = "Run again";
    retry.addEventListener("click", () => location.reload());
    links.append(retry);
    this.root.append(title, label, metrics, verdict, storage, links);
  }

  renderFailure(message) {
    if (!this.root) return;
    this.root.replaceChildren();
    const title = document.createElement("strong");
    title.textContent = "Stress Test Failed";
    const detail = document.createElement("span");
    detail.textContent = message;
    const retry = document.createElement("button");
    retry.type = "button";
    retry.textContent = "Try again";
    retry.addEventListener("click", () => location.reload());
    this.root.append(title, detail, retry);
  }
}

class BrowserTimingObserver {
  constructor() {
    this.observers = [];
    this.longTasks = [];
    this.animationFrames = [];
  }

  start() {
    if (typeof PerformanceObserver !== "function") return;
    if (PerformanceObserver.supportedEntryTypes?.includes("longtask")) {
      this.observe("longtask", (entry) => this.longTasks.push({ durationMs: round1(entry.duration) }));
    }
    if (PerformanceObserver.supportedEntryTypes?.includes("long-animation-frame")) {
      this.observe("long-animation-frame", (entry) => {
        const scripts = Array.from(entry.scripts || [])
          .map((script) => ({
            durationMs: round1(script.duration),
            function: String(script.sourceFunctionName || script.invoker || "").slice(0, 120),
            url: shortUrl(script.sourceURL),
          }))
          .sort((a, b) => b.durationMs - a.durationMs)
          .slice(0, 8);
        this.animationFrames.push({
          durationMs: round1(entry.duration),
          blockingDurationMs: round1(entry.blockingDuration),
          scripts,
        });
      });
    }
  }

  observe(type, collect) {
    try {
      const observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) collect(entry);
      });
      observer.observe({ type, buffered: false });
      this.observers.push(observer);
    } catch {
      // Browser advertised an entry type it would not observe; omit it from the report.
    }
  }

  stop() {
    for (const observer of this.observers) observer.disconnect();
    const longTasks = this.longTasks.sort((a, b) => b.durationMs - a.durationMs);
    const animationFrames = this.animationFrames.sort((a, b) => b.durationMs - a.durationMs);
    return {
      longTaskCount: longTasks.length,
      longTaskTotalMs: round1(longTasks.reduce((sum, entry) => sum + entry.durationMs, 0)),
      longestTasks: longTasks.slice(0, 20),
      longAnimationFrameCount: animationFrames.length,
      longestAnimationFrames: animationFrames.slice(0, 20),
    };
  }
}

async function collectEnvironment(match) {
  const uaData = navigator.userAgentData;
  let highEntropy = {};
  if (uaData?.getHighEntropyValues) {
    try {
      highEntropy = await uaData.getHighEntropyValues([
        "architecture", "bitness", "fullVersionList", "model", "platformVersion", "wow64",
      ]);
    } catch {
      // Low-entropy fields and the legacy UA are still useful.
    }
  }
  const connection = navigator.connection || navigator.mozConnection || navigator.webkitConnection;
  return {
    userAgent: String(navigator.userAgent || "").slice(0, 500),
    browserBrands: Array.from(uaData?.brands || []).slice(0, 8),
    platform: String(uaData?.platform || navigator.platform || "").slice(0, 120),
    architecture: String(highEntropy.architecture || "").slice(0, 60),
    bitness: String(highEntropy.bitness || "").slice(0, 20),
    platformVersion: String(highEntropy.platformVersion || "").slice(0, 80),
    fullVersionList: Array.from(highEntropy.fullVersionList || []).slice(0, 8),
    model: String(highEntropy.model || "").slice(0, 120),
    hardwareConcurrency: finite(navigator.hardwareConcurrency),
    deviceMemoryGiB: finite(navigator.deviceMemory),
    maxTouchPoints: finite(navigator.maxTouchPoints),
    language: String(navigator.language || "").slice(0, 40),
    timezone: String(Intl.DateTimeFormat().resolvedOptions().timeZone || "").slice(0, 80),
    screen: {
      width: finite(screen.width), height: finite(screen.height),
      availWidth: finite(screen.availWidth), availHeight: finite(screen.availHeight),
      colorDepth: finite(screen.colorDepth),
    },
    viewport: { width: innerWidth, height: innerHeight, devicePixelRatio: finite(devicePixelRatio) },
    network: connection ? {
      effectiveType: String(connection.effectiveType || "").slice(0, 20),
      downlinkMbps: finite(connection.downlink), rttMs: finite(connection.rtt),
      saveData: !!connection.saveData,
    } : null,
    gpu: collectGpuInfo(match),
    performanceEntryTypes: Array.from(globalThis.PerformanceObserver?.supportedEntryTypes || []).slice(0, 40),
  };
}

function collectGpuInfo(match) {
  const gl = match?.renderer?.app?.renderer?.gl;
  if (!gl?.getParameter) return null;
  try {
    const debug = gl.getExtension("WEBGL_debug_renderer_info");
    return {
      vendor: String(gl.getParameter(debug?.UNMASKED_VENDOR_WEBGL || gl.VENDOR) || "").slice(0, 200),
      renderer: String(gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL || gl.RENDERER) || "").slice(0, 240),
      version: String(gl.getParameter(gl.VERSION) || "").slice(0, 160),
    };
  } catch {
    return null;
  }
}

async function estimateRefreshRate() {
  if (typeof requestAnimationFrame !== "function") return 0;
  const timestamps = [];
  await new Promise((resolve) => {
    let finished = false;
    const finish = () => {
      if (finished) return;
      finished = true;
      resolve();
    };
    setTimeout(finish, 1500);
    const step = (at) => {
      if (finished) return;
      timestamps.push(at);
      if (timestamps.length >= 46) finish();
      else requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  });
  const deltas = timestamps.slice(1).map((at, index) => at - timestamps[index])
    .filter((delta) => delta > 0 && delta < 100);
  deltas.sort((a, b) => a - b);
  const median = deltas[Math.floor(deltas.length / 2)];
  return median ? Math.round(1000 / median) : 0;
}

function stableDeviceId() {
  try {
    const existing = localStorage.getItem(DEVICE_ID_KEY);
    if (/^[a-f0-9-]{16,64}$/i.test(existing || "")) return existing;
    const created = crypto.randomUUID?.() || randomHex(16);
    localStorage.setItem(DEVICE_ID_KEY, created);
    return created;
  } catch {
    return randomHex(16);
  }
}

async function environmentFingerprint(environment) {
  const stable = {
    userAgent: environment.userAgent,
    platform: environment.platform,
    architecture: environment.architecture,
    bitness: environment.bitness,
    hardwareConcurrency: environment.hardwareConcurrency,
    deviceMemoryGiB: environment.deviceMemoryGiB,
    screen: environment.screen,
    gpu: environment.gpu,
    timezone: environment.timezone,
  };
  try {
    const bytes = new TextEncoder().encode(JSON.stringify(stable));
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(digest)).slice(0, 12)
      .map((byte) => byte.toString(16).padStart(2, "0")).join("");
  } catch {
    return "unavailable";
  }
}

function sanitizeStreamState(state) {
  return {
    id: String(state?.id || "").slice(0, 64),
    source: String(state?.source || "").slice(0, 80),
    offline: state?.offline === true,
    websocket: state?.websocket === true,
    serverSimulation: state?.serverSimulation === true,
    frameCount: finite(state?.frameCount),
    tickRateHz: finite(state?.tickRateHz),
    loopCount: finite(state?.loopCount),
  };
}

function addMetric(dl, term, value) {
  const dt = document.createElement("dt");
  dt.textContent = term;
  const dd = document.createElement("dd");
  dd.textContent = value;
  dl.append(dt, dd);
}

function downloadLink(href, text) {
  const link = document.createElement("a");
  link.href = href;
  link.textContent = text;
  link.download = "";
  return link;
}

function shortUrl(value) {
  const raw = String(value || "");
  try {
    const url = new URL(raw, location.href);
    return `${url.origin}${url.pathname}`.slice(0, 300);
  } catch {
    return raw.slice(0, 300);
  }
}

function randomHex(bytes) {
  const values = new Uint8Array(bytes);
  crypto.getRandomValues(values);
  return Array.from(values).map((value) => value.toString(16).padStart(2, "0")).join("");
}

function finite(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function round1(value) {
  return Math.round(Number(value || 0) * 10) / 10;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
