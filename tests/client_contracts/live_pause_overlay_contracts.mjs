import { assert } from "./assertions.mjs";
import { withFakeOverlayDocument } from "./fakes.mjs";
import { createMatchCommandIssuer } from "../../client/src/match_command_issuer.js";
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
  assert(
    !settingsRoot.classList.contains("live-pause-active"),
    "resume countdown lowers settings beneath its screen blocker",
  );
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const scheduled = [];
  globalThis.setTimeout = (callback, delay) => { scheduled.push({ callback, delay }); return scheduled.length; };
  globalThis.clearTimeout = () => {};
  try {
    playedCountdown.length = 0;
    overlay.applyLivePauseState({
      paused: true,
      resumeCountdown: {
        durationMs: 13000, remainingMs: 13000, setupDurationMs: 10000,
        words: ["Drei!", "Zwei!", "Eins!"],
      },
    });
    assert(countdown.textContent === "Set up your hotkey groups 10", "setup starts at ten seconds");
    assert(overlay.el.classList.contains("hotkey-setup-active"), "setup passes pointer input through for selection");
    assert(playedCountdown.length === 0, "setup does not play the spoken match countdown early");
    for (let second = 1; second < 10; second += 1) {
      scheduled.find(({ delay }) => delay === second * 1000).callback();
      assert(countdown.textContent === `Set up your hotkey groups ${10 - second}`, "setup counts down each second");
    }
    for (let index = 0; index < 3; index += 1) {
      scheduled.find(({ delay }) => delay === 10000 + index * 1000).callback();
      assert(countdown.textContent === ["Drei!", "Zwei!", "Eins!"][index], "setup transitions to the existing spoken countdown");
    }
    assert(!overlay.el.classList.contains("hotkey-setup-active"), "final countdown restores the pointer blocker");
    assert(playedCountdown.join(",") === "countdown_drei,countdown_zwei,countdown_eins", "spoken countdown plays once in order");
    overlay.applyLivePauseState({
      paused: true,
      resumeCountdown: {
        durationMs: 13000, remainingMs: 8500, setupDurationMs: 10000,
        words: ["Drei!", "Zwei!", "Eins!"],
      },
    });
    assert(countdown.textContent === "Set up your hotkey groups 6", "late attach joins the current setup second");
    overlay.applyLivePauseState({ paused: false });
    assert(overlay.countdownTimers.length === 0, "unpause clears setup timers");
    assert(!overlay.el.classList.contains("hotkey-setup-active"), "unpause clears setup pointer policy");
  } finally {
    globalThis.setTimeout = originalSetTimeout;
    globalThis.clearTimeout = originalClearTimeout;
  }
  overlay.applyLivePauseState({ paused: true, canUnpause: false });
  assert(button.hidden && button.disabled && countdown.hidden, "live pause overlay hides unpause without authority");
  overlay.applyLivePauseState({ paused: false });
  assert(root.children[0].hidden, "live pause overlay hides when running");
  assert(!settingsRoot.classList.contains("live-pause-active"), "live pause overlay restores normal settings stacking after unpause");
  overlay.destroy();
  assert(root.children.length === 0, "live pause overlay tears down DOM");
});

{
  let issued = 0;
  const match = {
    livePauseState: { paused: true },
    state: {},
    predictionVisualsPaused: () => false,
    prediction: {
      issueCommand: () => { issued += 1; return { sent: true }; },
      predictionDisplayOverlay: () => null,
    },
    apmTracker: { recordAction() {} },
    health: { noteCommandIssued() {} },
    applyPredictionDisplayOverlay() {},
  };
  const issuer = createMatchCommandIssuer(match);
  for (const command of [{ type: "move", units: [1], x: 50, y: 50 }, { type: "rally", building: 2, x: 60, y: 60 }]) {
    assert(issuer.issueCommand(command).blocked === "livePause", "paused command issuer blocks gameplay including rallies");
  }
  assert(issued === 0, "paused commands never reach prediction or transport");
  match.livePauseState.paused = false;
  assert(issuer.issueCommand({ type: "stop", units: [1] }).sent && issued === 1, "commands resume after authoritative unpause");
}
