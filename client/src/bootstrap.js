export function wsUrl() {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${window.location.host}/ws`;
}

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
  const replay = (params.get("replay") || "").trim();
  if (window.location.pathname === "/dev/scenario" || params.has("watchScenario")) {
    const id = (params.get("id") || "").trim();
    const unit = (params.get("unit") || "").trim();
    const count = (params.get("count") || "").trim();
    const blocker = (params.get("blocker") || "").trim();
    if (
      !/^[a-z0-9_]+$/.test(id) ||
      !/^[a-z0-9_]+$/.test(unit) ||
      !/^[1-9][0-9]*$/.test(count) ||
      (blocker && !/^[a-z0-9_]+$/.test(blocker))
    ) {
      return null;
    }
    const blockerRoomPart = blocker ? `:blocker=${blocker}` : "";
    const blockerBannerPart = blocker ? ` blocker=${blocker}` : "";
    return {
      room: `__dev_scenario__:${id}:unit=${unit}:count=${count}${blockerRoomPart}`,
      noFog: true,
      kind: "scenario",
      banner: `local dev scenario no fog ${id} unit=${unit} count=${count}${blockerBannerPart}`,
    };
  }
  if (window.location.pathname !== "/dev/selfplay" && !params.has("watchSelfplay")) return null;
  const room = replay
    ? `__dev_selfplay__replay:${replay}`
    : "__dev_selfplay__live";
  return {
    room,
    noFog: true,
    kind: replay ? "replay" : "selfplay",
    banner: replay ? `local dev  self-play replay  no fog  ${replay}` : "local dev  self-play  no fog",
  };
}



/** Cached DOM handles for the pinned ids in index.html (see its DOM contract). */
export const dom = {
  version: document.getElementById("version"),
  lobbyScreen: document.getElementById("lobby-screen"),
  gameScreen: document.getElementById("game-screen"),
  viewport: document.getElementById("viewport"),
  minimap: document.getElementById("minimap"),
  toast: document.getElementById("toast"),
  gameOver: document.getElementById("game-over"),
  gameOverText: document.getElementById("game-over-text"),
  gameOverScores: document.getElementById("game-over-scores"),
  gameOverButton: document.getElementById("game-over-button"),
  gameMenu: document.getElementById("game-menu"),
  settingsButton: document.getElementById("settings-button"),
  settingsMenu: document.getElementById("settings-menu"),
  pointerLockToggle: document.getElementById("pointer-lock-toggle"),
  debugPathToggle: document.getElementById("debug-path-toggle"),
  giveUpOpen: document.getElementById("give-up-open"),
  giveUpConfirm: document.getElementById("give-up-confirm"),
  giveUpCancel: document.getElementById("give-up-cancel"),
  giveUpConfirmButton: document.getElementById("give-up-confirm-button"),
  commandCard: document.getElementById("command-card"),
  devBanner: document.getElementById("dev-banner"),
  devLinks: document.getElementById("dev-links"),
  replaySpeed: document.getElementById("replay-speed"),
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



export function buildAudioSettings(audio, menuEl) {
  if (menuEl.querySelector(".audio-settings")) return; // idempotent

  const wrap = document.createElement("div");
  wrap.className = "audio-settings";

  const unlockRow = document.createElement("div");
  unlockRow.className = "audio-unlock-row";
  unlockRow.setAttribute("role", "status");

  const unlockText = document.createElement("span");
  unlockText.textContent = "Audio waiting for input";

  const unlockButton = document.createElement("button");
  unlockButton.type = "button";
  unlockButton.textContent = "Start audio";
  unlockButton.addEventListener("click", async (ev) => {
    unlockButton.disabled = true;
    unlockButton.textContent = "Starting...";
    await audio.unlockFromGesture(ev);
    updateUnlockRow();
  });

  function updateUnlockRow() {
    const unlocked = audio.isUnlocked();
    unlockRow.hidden = unlocked;
    unlockButton.disabled = false;
    unlockButton.textContent = unlocked ? "Audio on" : "Start audio";
  }

  unlockRow.append(unlockText, unlockButton);
  wrap.appendChild(unlockRow);
  audio.onUnlockChange(updateUnlockRow);
  updateUnlockRow();

  const rows = [
    {
      label: "Master",
      get: () => audio.getMasterVolume(),
      set: (v) => audio.setMasterVolume(v),
    },
    {
      label: "Alerts",
      get: () => audio.getCategoryVolume("alert"),
      set: (v) => audio.setCategoryVolume("alert", v),
    },
    {
      label: "UI",
      get: () => audio.getCategoryVolume("ui"),
      set: (v) => audio.setCategoryVolume("ui", v),
    },
    {
      label: "Combat",
      get: () => audio.getCategoryVolume("combat_self"),
      set: (v) => {
        audio.setCategoryVolume("combat_self", v);
        audio.setCategoryVolume("combat_other", v);
      },
    },
    {
      label: "Voices",
      get: () => audio.getCategoryVolume("unit_voice"),
      set: (v) => audio.setCategoryVolume("unit_voice", v),
    },
    {
      label: "Ambient",
      get: () => audio.getCategoryVolume("ambient"),
      set: (v) => audio.setCategoryVolume("ambient", v),
    },
  ];

  for (const row of rows) {
    const r = document.createElement("label");
    r.className = "audio-slider";

    const label = document.createElement("span");
    label.className = "audio-slider-label";
    label.textContent = row.label;

    const input = document.createElement("input");
    input.type = "range";
    input.min = "0";
    input.max = "1";
    input.step = "0.01";
    input.value = String(row.get());
    input.addEventListener("input", () => row.set(parseFloat(input.value)));

    r.append(label, input);
    wrap.appendChild(r);
  }

  menuEl.insertBefore(wrap, menuEl.firstChild);
}
