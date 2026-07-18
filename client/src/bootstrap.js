export function wsUrl() {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${window.location.host}/ws`;
}

export function snapshotStreamLaunchConfig() {
  const params = new URLSearchParams(window.location.search);
  if (!params.has("snapshotStream")) return null;
  const id = (params.get("snapshotStream") || "").trim();
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(id)) return null;
  return {
    id,
    banner: `offline snapshot stream · ${id} · no WebSocket or live simulation`,
  };
}
export { stressTestLaunchConfig } from "./stress_test_launch.js";

function diagnosticsEnabled() {
  try {
    const params = new URLSearchParams(window.location.search);
    if (params.has("rtsDebug")) return params.get("rtsDebug") !== "0";
    return window.localStorage.getItem("rts.debug") === "1";
  } catch {
    return false;
  }
}

function summarizeDetail(detail) {
  if (!detail || typeof detail !== "object") return detail;
  const out = {};
  for (const [key, value] of Object.entries(detail)) {
    if (Array.isArray(value)) out[key] = `array(${value.length})`;
    else if (value && typeof value === "object") out[key] = "{...}";
    else out[key] = value;
  }
  return out;
}

export const diagnostics = {
  enabled: diagnosticsEnabled(),
  marks: [],
  counts: {},
  last: {},

  mark(label, detail, options = {}) {
    if (!this.enabled) return;
    const shouldStore = options.store !== false;
    const shouldLog = options.log !== false;
    const at = performance.now();
    const entry = { at, label, detail: summarizeDetail(detail) };
    if (shouldStore) {
      this.marks.push(entry);
      if (this.marks.length > 500) this.marks.splice(0, this.marks.length - 500);
      try {
        performance.mark(`rts:${label}`);
      } catch {
        // Some WebViews reject dynamic mark names; console output still has the data.
      }
    }
    this.last[label] = entry;
    if (shouldLog) console.debug(`[rts-debug] ${at.toFixed(1)} ${label}`, entry.detail || "");
    return entry;
  },

  count(label, detail) {
    if (!this.enabled) return;
    this.counts[label] = (this.counts[label] || 0) + 1;
    return this.mark(label, detail, { store: false, log: false });
  },

  time(label, detail, fn) {
    if (!this.enabled) return fn();
    const start = performance.now();
    this.mark(`${label}:start`, detail);
    try {
      return fn();
    } finally {
      const durationMs = performance.now() - start;
      this.mark(`${label}:end`, { ...detail, durationMs: Number(durationMs.toFixed(1)) });
    }
  },

  events(filter) {
    if (!filter) return this.marks.slice();
    if (typeof filter === "function") return this.marks.filter(filter);
    const pattern = filter instanceof RegExp ? filter : new RegExp(String(filter));
    return this.marks.filter((m) => pattern.test(m.label));
  },

  rows(filter) {
    return this.events(filter).map((m) => ({
      at: Number(m.at.toFixed(1)),
      label: m.label,
      detail: JSON.stringify(m.detail || {}),
    }));
  },

  table(filter) {
    const rows = this.rows(filter);
    console.table(rows);
    return rows;
  },

  text(filter) {
    return this.rows(filter)
      .map((row) => `${row.at}\t${row.label}\t${row.detail}`)
      .join("\n");
  },

  copy(filter) {
    const text = this.text(filter);
    if (navigator.clipboard?.writeText) {
      void navigator.clipboard.writeText(text);
      return text;
    }
    console.log(text);
    return text;
  },

  summary() {
    const rows = this.rows((m) => !m.label.startsWith("server.recv.snapshot"));
    console.table(rows);
    console.table(Object.entries(this.counts).map(([label, count]) => ({ label, count })));
    return { marks: rows, counts: { ...this.counts }, last: { ...this.last } };
  },
};

if (typeof window !== "undefined") window.__rtsDebug = diagnostics;



export function devWatchConfig() {
  const params = new URLSearchParams(window.location.search);
  if (window.location.pathname === "/dev/scenario" || params.has("watchScenario")) {
    const id = (params.get("id") || "").trim();
    const unit = (params.get("unit") || "").trim();
    const count = (params.get("count") || "").trim();
    const blocker = (params.get("blocker") || "").trim();
    const scenarioCase = (params.get("case") || "").trim();
    if (
      !/^[a-z0-9_]+$/.test(id) ||
      !/^[a-z0-9_]+$/.test(unit) ||
      !/^[1-9][0-9]*$/.test(count) ||
      (blocker && !/^[a-z0-9_]+$/.test(blocker)) ||
      (scenarioCase && !/^[a-z0-9_]+$/.test(scenarioCase))
    ) {
      return null;
    }
    const blockerRoomPart = blocker ? `:blocker=${blocker}` : "";
    const blockerBannerPart = blocker ? ` blocker=${blocker}` : "";
    const caseRoomPart = scenarioCase ? `:case=${scenarioCase}` : "";
    const caseBannerPart = scenarioCase ? ` case=${scenarioCase}` : "";
    return {
      id,
      room: `__dev_scenario__:${id}:unit=${unit}:count=${count}${blockerRoomPart}${caseRoomPart}`,
      noFog: true,
      kind: "scenario",
      banner: `local dev scenario no fog ${id} unit=${unit} count=${count}${blockerBannerPart}${caseBannerPart}`,
    };
  }
  return null;
}

function safeLabToken(value, fallback, maxLen) {
  const raw = (value || "").trim() || fallback;
  if (!/^[A-Za-z0-9_-]+$/.test(raw) || raw.length > maxLen) return fallback;
  return raw;
}

function safeLabSeed(value) {
  const raw = String(value || "").trim();
  return /^[0-9]+$/.test(raw) && Number(raw) <= 0xffffffff ? raw : "";
}

function safeLabVisualProfile(value) {
  const raw = String(value || "").trim();
  if (!raw) return { id: "", error: null };
  if (!/^[A-Za-z0-9_-]{1,48}$/.test(raw)) {
    return { id: "", error: { code: "invalid" } };
  }
  return { id: raw, error: null };
}

function isLabPath(pathname) {
  return pathname === "/lab" || pathname === "/lab/";
}

export function buildLabLaunchConfig({ room, map, seed = "", scenario = "", visualProfile = "" } = {}) {
  const publicRoom = safeLabToken(room, "default", 40);
  const mapName = safeLabToken(map, "1v1", 48);
  const seedPart = safeLabSeed(seed);
  const scenarioId = safeLabToken(scenario, "", 48);
  const visualProfileResult = safeLabVisualProfile(visualProfile);
  return {
    room: `__lab__:${publicRoom}:map=${mapName}${seedPart ? `:seed=${seedPart}` : ""}${scenarioId ? `:scenario=${scenarioId}` : ""}`,
    publicRoom,
    map: mapName,
    scenario: scenarioId,
    visualProfileId: visualProfileResult.id,
    visualProfileError: visualProfileResult.error,
    banner: `lab ${publicRoom} map=${mapName}${scenarioId ? ` scenario=${scenarioId}` : ""}`,
  };
}

export function labCatalogRouteConfig() {
  if (!isLabPath(window.location.pathname)) return null;
  const params = new URLSearchParams(window.location.search);
  if (params.has("scenario") || params.has("map") || params.has("seed") || params.has("handoff")) return null;
  const visualProfileResult = safeLabVisualProfile(params.get("visualProfile"));
  return {
    room: safeLabToken(params.get("room"), "default", 40),
    visualProfileId: visualProfileResult.id,
    visualProfileError: visualProfileResult.error,
  };
}

export function labLaunchConfig() {
  if (!isLabPath(window.location.pathname)) return null;
  if (new URLSearchParams(window.location.search).has("handoff")) return null;
  if (labCatalogRouteConfig()) return null;
  const params = new URLSearchParams(window.location.search);
  return buildLabLaunchConfig({
    room: params.get("room"),
    map: params.get("map"),
    seed: params.get("seed"),
    scenario: params.get("scenario"),
    visualProfile: params.get("visualProfile"),
  });
}

export function replaceLabCatalogRoute(launch, {
  locationLike = window.location,
  historyLike = window.history,
} = {}) {
  if (!isLabPath(locationLike?.pathname) || !launch?.scenario) return "";
  const params = new URLSearchParams(locationLike.search || "");
  params.delete("handoff");
  params.delete("workspace");
  params.set("scenario", safeLabToken(launch.scenario, "blank", 48));

  const mapName = safeLabToken(launch.map, "1v1", 48);
  if (mapName === "1v1") params.delete("map");
  else params.set("map", mapName);

  const room = safeLabToken(launch.publicRoom, "default", 40);
  if (room === "default") params.delete("room");
  else params.set("room", room);

  const visualProfile = safeLabVisualProfile(launch.visualProfileId);
  if (visualProfile.id) params.set("visualProfile", visualProfile.id);
  else params.delete("visualProfile");

  const search = params.toString();
  const url = `${locationLike.pathname}${search ? `?${search}` : ""}${locationLike.hash || ""}`;
  historyLike?.replaceState?.(historyLike.state ?? null, "", url);
  return url;
}

export function labHandoffLaunchConfig() {
  if (!isLabPath(window.location.pathname)) return null;
  const params = new URLSearchParams(window.location.search);
  const raw = String(params.get("handoff") || "").trim().toLowerCase();
  if (!raw) return null;
  const workspace = String(params.get("workspace") || "default").trim();
  return {
    handoffId: /^[a-f0-9]{32}$/.test(raw) ? raw : "",
    workspaceId: /^[A-Za-z0-9_-]{1,48}$/.test(workspace) ? workspace : "default",
    error: /^[a-f0-9]{32}$/.test(raw) ? "" : "Invalid Map Editor handoff id.",
  };
}

export function replayLaunchConfig() {
  const params = new URLSearchParams(window.location.search);
  const artifact = (params.get("replayArtifact") || "").trim();
  if (artifact) {
    if (!/^[A-Za-z0-9_-]+$/.test(artifact)) return null;
    return { room: `__replay_artifact__:${artifact}` };
  }
  const room = (params.get("replayRoom") || "").trim();
  if (!room) return null;
  if (!/^__match_replay__:[A-Za-z0-9_-]+$/.test(room)) return null;
  return { room, staging: true };
}



/** Cached DOM handles for the pinned ids in index.html (see its DOM contract). */
export const dom = {
  app: document.getElementById("app"),
  version: document.getElementById("version"),
  lobbyScreen: document.getElementById("lobby-screen"),
  labEntryScreen: document.getElementById("lab-entry-screen"),
  branchScreen: document.getElementById("branch-screen"),
  gameScreen: document.getElementById("game-screen"),
  viewport: document.getElementById("viewport"),
  minimap: document.getElementById("minimap"),
  toast: document.getElementById("toast"),
  connectionLost: document.getElementById("connection-lost"),
  connectionLostDetail: document.getElementById("connection-lost-detail"),
  gameOver: document.getElementById("game-over"),
  gameOverText: document.getElementById("game-over-text"),
  gameOverObservation: document.getElementById("game-over-observation"),
  gameOverScores: document.getElementById("game-over-scores"),
  gameOverButton: document.getElementById("game-over-button"),
  gameOverClose: document.getElementById("game-over-close"),
  gameMenu: document.getElementById("game-menu"),
  settingsButton: document.getElementById("settings-button"),
  settingsMenu: document.getElementById("settings-menu"),
  pointerLockToggle: document.getElementById("pointer-lock-toggle"),
  debugPathToggle: document.getElementById("debug-path-toggle"),
  giveUpOpen: document.getElementById("give-up-open"),
  giveUpConfirm: document.getElementById("give-up-confirm"),
  giveUpCancel: document.getElementById("give-up-cancel"),
  giveUpConfirmButton: document.getElementById("give-up-confirm-button"),
  selectionArea: document.getElementById("selection-area"),
  commandCard: document.getElementById("command-card"),
  devBanner: document.getElementById("dev-banner"),
  devLinks: document.getElementById("dev-links"),
  roomTimeControls: document.getElementById("room-time-controls"),
};

export function formatScore(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return "0";
  return Math.trunc(n).toLocaleString();
}



export function isTextEntry(target) {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}
