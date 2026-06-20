export class LivePauseOverlay {
  constructor({ root, onUnpause }) {
    this.root = root;
    this.onUnpause = onUnpause;
    this.state = {
      paused: false,
      canUnpause: false,
    };
    this.el = document.createElement("div");
    this.el.className = "live-pause-overlay";
    this.el.hidden = true;
    this.el.setAttribute("role", "status");
    this.el.setAttribute("aria-live", "polite");

    this.panel = document.createElement("div");
    this.panel.className = "live-pause-panel";

    this.title = document.createElement("h2");
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

    this.panel.append(this.title, this.meta, this.button);
    this.el.appendChild(this.panel);
    this.root?.appendChild(this.el);
  }

  applyLivePauseState(state = {}) {
    this.state = {
      paused: state.paused === true,
      pausedBy: Number.isInteger(state.pausedBy) ? state.pausedBy : null,
      pausesRemaining: Number.isInteger(state.pausesRemaining) ? state.pausesRemaining : null,
      pauseLimit: Number.isInteger(state.pauseLimit) ? state.pauseLimit : null,
      canUnpause: state.canUnpause === true,
    };
    this.render();
  }

  render() {
    this.el.hidden = !this.state.paused;
    if (!this.state.paused) return;
    const pausedBy = this.state.pausedBy == null ? "" : `Player ${this.state.pausedBy}`;
    this.meta.textContent = pausedBy ? `Paused by ${pausedBy}` : "";
    this.button.hidden = !this.state.canUnpause;
    this.button.disabled = !this.state.canUnpause;
  }

  destroy() {
    this.button.removeEventListener("click", this.onButtonClick);
    this.el.remove();
  }
}
