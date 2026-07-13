import { renderHotkeyEditor } from "./hotkey_editor.js";

export function buildSettingsTabs({
  audio,
  hotkeyProfiles = null,
  game = null,
  debug = null,
  replayControls = null,
} = {}) {
  return [
    {
      id: "game",
      label: "Game",
      render: (root) => renderGamePanel(root, game),
    },
    {
      id: "hotkeys",
      label: "Hotkeys",
      render: (root, context) => renderHotkeysPanel(root, hotkeyProfiles, context),
    },
    {
      id: "audio",
      label: "Audio",
      render: (root) => renderAudioPanel(root, audio),
    },
    {
      id: "replay-controls",
      label: "Replay Controls",
      visible: !!replayControls,
      render: (root) => renderReplayControlsPanel(root, replayControls),
    },
    {
      id: "debug",
      label: "Debug",
      visible: !!debug?.available,
      render: (root) => renderDebugPanel(root, debug),
    },
  ];
}

function renderReplayControlsPanel(root, replayControls) {
  const button = document.createElement("button");
  button.id = "auto-spectator-toggle";
  button.type = "button";
  button.className = "settings-toggle";
  button.setAttribute("role", "switch");
  button.addEventListener("click", () => {
    replayControls.onToggle?.();
    sync();
  });
  root.appendChild(button);

  function sync() {
    const state = replayControls.state?.() || {};
    const enabled = !!state.enabled;
    button.disabled = state.available === false;
    button.setAttribute("aria-checked", String(enabled));
    button.textContent = enabled ? "Enable Auto Spectator: on" : "Enable Auto Spectator: off";
    button.title = "Automatically frame active battles and show the whole map when combat is quiet.";
  }
  sync();
}

export function buildGiveUpAction({ visible, onOpen }) {
  return {
    render() {
      if (!visible) return null;
      const button = document.createElement("button");
      button.id = "give-up-open";
      button.type = "button";
      button.className = "settings-danger-action";
      button.textContent = "Give up";
      button.addEventListener("click", () => onOpen?.());
      return button;
    },
  };
}

export function buildBackToLobbyAction({ visible, onBackToLobby }) {
  return {
    render() {
      if (!visible) return null;
      const button = document.createElement("button");
      button.id = "back-to-lobby-open";
      button.type = "button";
      button.className = "settings-match-action";
      button.textContent = "Back to Lobby";
      button.addEventListener("click", () => onBackToLobby?.());
      return button;
    },
  };
}

export function buildPauseAction({ visible, disabled = false, label = "Pause", title = "", onPause }) {
  return {
    render() {
      if (!visible) return null;
      const button = document.createElement("button");
      button.id = "live-pause-open";
      button.type = "button";
      button.className = "settings-match-action";
      button.textContent = label;
      button.disabled = !!disabled;
      if (title) button.title = title;
      button.addEventListener("click", () => onPause?.());
      return button;
    },
  };
}

function renderGamePanel(root, game) {
  root.classList.add("settings-game-panel");
  if (game?.prediction) renderPredictionControl(root, game.prediction);
  if (game?.pointerLock) renderPointerLockControl(root, game.pointerLock);
  if (game?.unitRanges) renderUnitRangeControl(root, game.unitRanges);
  renderContextSummary(root, game);
}

function renderPredictionControl(root, prediction) {
  const button = document.createElement("button");
  button.id = "prediction-toggle";
  button.type = "button";
  button.className = "settings-toggle";
  button.setAttribute("role", "switch");
  button.addEventListener("click", () => {
    prediction.onToggle?.();
    sync();
  });
  root.appendChild(button);

  function sync() {
    const state = prediction.state?.() || {};
    const enabled = !!state.enabled;
    button.hidden = !!state.hidden;
    button.disabled = state.available === false;
    button.setAttribute("aria-checked", String(enabled));
    button.textContent = enabled
      ? (state.pending ? "Movement prediction: on (loading)" : "Movement prediction: on")
      : "Movement prediction: off";
    button.title = enabled
      ? "Predict owned movement locally before authoritative snapshots arrive."
      : "Use authoritative server snapshots only for owned movement.";
  }
  sync();
  prediction.onMount?.(sync);
}

function renderPointerLockControl(root, pointerLock) {
  const button = document.createElement("button");
  button.id = "pointer-lock-toggle";
  button.type = "button";
  button.className = "settings-toggle";
  button.setAttribute("role", "switch");
  button.addEventListener("click", () => pointerLock.onToggle?.());
  root.appendChild(button);

  const sync = () => {
    const state = pointerLock.state?.() || {};
    const supported = state.supported !== false;
    const enabled = !!state.enabled;
    const locked = !!state.locked;
    button.hidden = !!state.hidden;
    button.disabled = !supported;
    button.setAttribute("aria-checked", String(enabled));
    button.textContent = locked ? "Cursor locked" : "Lock cursor pan";
    button.title = supported
      ? "Trap the cursor in the game view for multi-monitor edge panning."
      : "Cursor lock is not supported by this browser.";
  };
  sync();
  pointerLock.onMount?.(sync);
}

function renderUnitRangeControl(root, unitRanges) {
  const button = document.createElement("button");
  button.id = "unit-range-toggle";
  button.type = "button";
  button.className = "settings-toggle";
  button.setAttribute("role", "switch");
  button.addEventListener("click", () => {
    unitRanges.onToggle?.();
    sync();
  });
  root.appendChild(button);

  function sync() {
    const state = unitRanges.state?.() || {};
    const enabled = !!state.enabled;
    button.hidden = !!state.hidden;
    button.disabled = state.available === false;
    button.setAttribute("aria-checked", String(enabled));
    button.textContent = enabled ? "Show Unit Ranges: on" : "Show Unit Ranges: off";
    button.title = "Draw selected units' firing ranges.";
  }
  sync();
  unitRanges.onMount?.(sync);
}

function renderContextSummary(root, game) {
  const row = document.createElement("div");
  row.className = "settings-context-row";
  const label = document.createElement("span");
  label.textContent = contextLabel(game?.kind);
  row.appendChild(label);
  if (game?.spectator) {
    const badge = document.createElement("span");
    badge.className = "settings-badge";
    badge.textContent = "Spectator";
    row.appendChild(badge);
  }
  root.appendChild(row);
}

function renderHotkeysPanel(root, hotkeyProfiles, context) {
  return renderHotkeyEditor(root, hotkeyProfiles, context);
}

function renderAudioPanel(root, audio) {
  root.classList.add("audio-settings");
  if (!audio) return;
  const cleanup = [];

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
  root.appendChild(unlockRow);
  cleanup.push(audio.onUnlockChange(updateUnlockRow));
  updateUnlockRow();

  const rows = [
    ["Master", () => audio.getMasterVolume(), (v) => audio.setMasterVolume(v)],
    ["Alerts", () => audio.getCategoryVolume("alert"), (v) => audio.setCategoryVolume("alert", v)],
    ["UI", () => audio.getCategoryVolume("ui"), (v) => audio.setCategoryVolume("ui", v)],
    ["Combat", () => audio.getCategoryVolume("combat_self"), (v) => {
      audio.setCategoryVolume("combat_self", v);
      audio.setCategoryVolume("combat_other", v);
    }],
    ["Voices", () => audio.getCategoryVolume("unit_voice"), (v) => audio.setCategoryVolume("unit_voice", v)],
    ["Ambient", () => audio.getCategoryVolume("ambient"), (v) => audio.setCategoryVolume("ambient", v)],
  ];

  for (const [labelText, get, set] of rows) {
    const row = document.createElement("label");
    row.className = "audio-slider";
    const label = document.createElement("span");
    label.className = "audio-slider-label";
    label.textContent = labelText;
    const input = document.createElement("input");
    input.type = "range";
    input.min = "0";
    input.max = "1";
    input.step = "0.01";
    input.value = String(get());
    input.addEventListener("input", () => set(parseFloat(input.value)));
    row.append(label, input);
    root.appendChild(row);
  }

  return () => {
    for (const fn of cleanup) fn();
  };
}

function renderDebugPanel(root, debug) {
  if (!debug?.available) return;
  const button = document.createElement("button");
  button.id = "debug-path-toggle";
  button.type = "button";
  button.className = "settings-toggle";
  button.setAttribute("role", "switch");
  button.addEventListener("click", () => {
    debug.onToggle?.();
    sync();
  });
  root.appendChild(button);

  function sync() {
    const state = debug.state?.() || {};
    const enabled = !!state.enabled;
    button.disabled = state.available === false;
    button.hidden = state.available === false;
    button.setAttribute("aria-checked", String(enabled));
    button.textContent = enabled ? "Movement waypoints: on" : "Movement waypoints: off";
    button.title = "Show the current and queued movement path waypoints.";
  }
  sync();
}

function renderMutedText(root, text) {
  const el = document.createElement("div");
  el.className = "settings-muted";
  el.textContent = text;
  root.appendChild(el);
}

function contextLabel(kind) {
  if (kind === "match") return "Live match";
  if (kind === "lab") return "Lab";
  if (kind === "replay") return "Replay";
  if (kind === "spectator") return "Spectator match";
  return "Lobby";
}
