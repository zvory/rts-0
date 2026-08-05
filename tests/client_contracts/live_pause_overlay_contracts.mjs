import { assert } from "./assertions.mjs";
import { withFakeOverlayDocument } from "./fakes.mjs";
import { LivePauseOverlay } from "../../client/src/live_pause_overlay.js";

withFakeOverlayDocument(({ FakeElement }) => {
  const root = new FakeElement("section");
  const settingsRoot = new FakeElement("div");
  let unpaused = false;
  const openedTabs = [];
  const overlay = new LivePauseOverlay({
    root,
    settingsRoot,
    onUnpause: () => { unpaused = true; },
    onOpenSettings: (tabId) => openedTabs.push(tabId),
    playerNameForId: (playerId) => playerId === 2 ? "Alex" : "",
  });
  overlay.applyLivePauseState({ paused: true, pausedBy: 2, pauseLimit: 3, canUnpause: true });
  assert(root.children.length === 1, "live pause overlay mounts generated DOM");
  assert(!root.children[0].hidden, "live pause overlay shows when paused");
  assert(root.children[0].attributes.get("role") === "dialog", "live pause actions use dialog semantics");
  assert(root.querySelector(".live-pause-meta")?.textContent === "Paused by Alex", "live pause overlay resolves the pausing player's roster name");
  assert(settingsRoot.classList.contains("live-pause-active"), "live pause overlay raises settings above its screen blocker");
  root.querySelector("#live-pause-settings").listeners.click();
  root.querySelector("#live-pause-hotkeys").listeners.click();
  assert(openedTabs.join(",") === "game,hotkeys", "live pause overlay opens game settings and hotkey editing");
  const button = root.querySelector("#live-pause-unpause");
  assert(button && !button.hidden && !button.disabled, "live pause overlay enables unpause for pause-authorized viewers");
  button.listeners.click();
  assert(unpaused, "live pause overlay calls injected unpause action");

  const playedCountdown = [];
  overlay.audio = { playUI(id) { playedCountdown.push(id); } };
  overlay.applyLivePauseState({
    paused: true,
    canUnpause: false,
    resumeCountdown: {
      durationMs: 3000,
      remainingMs: 1900,
      words: ["Drei!", "Zwei!", "Eins!"],
    },
  });
  const countdown = root.querySelector(".live-resume-countdown");
  assert(!countdown.hidden && countdown.textContent === "Zwei!", "resume countdown joins at the server-reported phase");
  assert(playedCountdown[0] === "countdown_zwei", "resume countdown plays the matching spoken cue");
  assert(overlay.panel.hidden, "resume countdown replaces pause actions until play restarts");
  overlay.applyLivePauseState({ paused: true, canUnpause: false });
  assert(button.hidden && button.disabled && countdown.hidden, "live pause overlay hides unpause without authority");
  overlay.applyLivePauseState({ paused: false });
  assert(root.children[0].hidden, "live pause overlay hides when running");
  assert(!settingsRoot.classList.contains("live-pause-active"), "live pause overlay restores normal settings stacking after unpause");
  overlay.destroy();
  assert(root.children.length === 0, "live pause overlay tears down DOM");
});
