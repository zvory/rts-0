const COUNTDOWN_SOUND_IDS = ["countdown_drei", "countdown_zwei", "countdown_eins"];

export class LivePauseOverlay {
  constructor({ root, settingsRoot = null, onUnpause, onOpenSettings, playerNameForId, audio = null }) {
    this.root = root;
    this.settingsRoot = settingsRoot;
    this.onUnpause = onUnpause;
    this.onOpenSettings = onOpenSettings;
    this.playerNameForId = playerNameForId;
    this.audio = audio;
    this.countdownTimers = [];
    this.state = {
      paused: false,
      canUnpause: false,
    };
    this.el = document.createElement("div");
    this.el.className = "live-pause-overlay";
    this.el.hidden = true;
    this.el.setAttribute("role", "dialog");
    this.el.setAttribute("aria-labelledby", "live-pause-title");

    this.panel = document.createElement("div");
    this.panel.className = "live-pause-panel";

    this.title = document.createElement("h2");
    this.title.id = "live-pause-title";
    this.title.textContent = "Game Paused";

    this.meta = document.createElement("p");
    this.meta.className = "live-pause-meta";

    this.button = document.createElement("button");
    this.button.id = "live-pause-unpause";
    this.button.type = "button";
    this.button.className = "btn primary";
    this.button.textContent = "Unpause";
    this.onButtonClick = () => this.onUnpause?.();
    this.button.addEventListener("click", this.onButtonClick);

    this.settingsButton = document.createElement("button");
    this.settingsButton.id = "live-pause-settings";
    this.settingsButton.type = "button";
    this.settingsButton.className = "btn";
    this.settingsButton.textContent = "Settings";
    this.onSettingsClick = () => this.onOpenSettings?.("game");
    this.settingsButton.addEventListener("click", this.onSettingsClick);

    this.hotkeysButton = document.createElement("button");
    this.hotkeysButton.id = "live-pause-hotkeys";
    this.hotkeysButton.type = "button";
    this.hotkeysButton.className = "btn";
    this.hotkeysButton.textContent = "Edit Hotkeys";
    this.onHotkeysClick = () => this.onOpenSettings?.("hotkeys");
    this.hotkeysButton.addEventListener("click", this.onHotkeysClick);

    this.actions = document.createElement("div");
    this.actions.className = "live-pause-actions";
    this.actions.append(this.settingsButton, this.hotkeysButton, this.button);

    this.countdown = document.createElement("div");
    this.countdown.className = "live-resume-countdown match-countdown";
    this.countdown.hidden = true;
    this.countdown.setAttribute("role", "status");
    this.countdown.setAttribute("aria-live", "assertive");

    this.panel.append(this.title, this.meta, this.actions);
    this.el.append(this.panel, this.countdown);
    this.root?.appendChild(this.el);
  }

  applyLivePauseState(state = {}) {
    this.state = {
      paused: state.paused === true,
      pausedBy: Number.isInteger(state.pausedBy) ? state.pausedBy : null,
      pausesRemaining: Number.isInteger(state.pausesRemaining) ? state.pausesRemaining : null,
      pauseLimit: Number.isInteger(state.pauseLimit) ? state.pauseLimit : null,
      canUnpause: state.canUnpause === true,
      resumeCountdown: state.resumeCountdown || null,
    };
    this.render();
  }

  render() {
    this.el.hidden = !this.state.paused;
    this.settingsRoot?.classList.toggle("live-pause-active", this.state.paused);
    this.clearCountdown();
    if (!this.state.paused) {
      this.panel.hidden = false;
      return;
    }
    if (this.state.resumeCountdown) {
      this.panel.hidden = true;
      this.startCountdown(this.state.resumeCountdown);
      return;
    }
    this.panel.hidden = false;
    const resolvedName = this.state.pausedBy == null
      ? ""
      : String(this.playerNameForId?.(this.state.pausedBy) || "").trim();
    const pausedBy = resolvedName || (this.state.pausedBy == null ? "" : `Player ${this.state.pausedBy}`);
    this.meta.textContent = pausedBy ? `Paused by ${pausedBy}` : "";
    this.button.hidden = !this.state.canUnpause;
    this.button.disabled = !this.state.canUnpause;
  }

  startCountdown({ durationMs, remainingMs, words }) {
    const wordMs = Math.max(250, durationMs / words.length);
    const elapsedMs = Math.max(0, durationMs - remainingMs);
    const startIndex = Math.min(words.length - 1, Math.floor(elapsedMs / wordMs));
    const showWord = (index) => {
      if (this.countdown.hidden) return;
      this.countdown.textContent = words[index];
      this.countdown.classList.remove("pulse");
      void this.countdown.offsetWidth;
      this.countdown.classList.add("pulse");
      const soundId = COUNTDOWN_SOUND_IDS.length === words.length
        ? COUNTDOWN_SOUND_IDS[index]
        : null;
      if (soundId) {
        this.audio?.playUI?.(soundId, {
          priority: 8,
          pitchVariance: 0,
          dedupKey: `live-resume-countdown:${index}`,
        });
      }
    };
    this.countdown.hidden = false;
    showWord(startIndex);
    for (let index = startIndex + 1; index < words.length; index += 1) {
      const delay = Math.max(0, index * wordMs - elapsedMs);
      this.countdownTimers.push(globalThis.setTimeout(() => showWord(index), delay));
    }
  }

  clearCountdown() {
    for (const timer of this.countdownTimers) globalThis.clearTimeout(timer);
    this.countdownTimers = [];
    this.countdown.hidden = true;
    this.countdown.textContent = "";
  }

  destroy() {
    this.clearCountdown();
    this.button.removeEventListener("click", this.onButtonClick);
    this.settingsButton.removeEventListener("click", this.onSettingsClick);
    this.hotkeysButton.removeEventListener("click", this.onHotkeysClick);
    this.settingsRoot?.classList.remove("live-pause-active");
    this.el.remove();
  }
}
