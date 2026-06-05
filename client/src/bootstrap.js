export function wsUrl() {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${window.location.host}/ws`;
}



export function devWatchConfig() {
  const params = new URLSearchParams(window.location.search);
  const replay = (params.get("replay") || "").trim();
  if (window.location.pathname === "/dev/scenario" || params.has("watchScenario")) {
    const id = (params.get("id") || "").trim();
    const cars = (params.get("cars") || "").trim();
    if (id !== "scout_car_snaking_corridor" || (cars !== "1" && cars !== "4")) {
      return null;
    }
    return {
      room: `__dev_scenario__:scout_car_snaking_corridor:cars=${cars}`,
      noFog: true,
      kind: "scenario",
      banner: `local dev scenario no fog scout_car_snaking_corridor cars=${cars}`,
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
  settingsButton: document.getElementById("settings-button"),
  settingsMenu: document.getElementById("settings-menu"),
  pointerLockToggle: document.getElementById("pointer-lock-toggle"),
  giveUpOpen: document.getElementById("give-up-open"),
  giveUpConfirm: document.getElementById("give-up-confirm"),
  giveUpCancel: document.getElementById("give-up-cancel"),
  giveUpConfirmButton: document.getElementById("give-up-confirm-button"),
  commandCard: document.getElementById("command-card"),
  devBanner: document.getElementById("dev-banner"),
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
