/** Center-screen progress notice for authoritative replay reconstruction. */
export class ReplaySeekOverlay {
  constructor({ root, onStopSeeking = null }) {
    this.root = root;
    this.onStopSeeking = onStopSeeking;
    this.dismissed = false;

    this.el = document.createElement("div");
    this.el.className = "replay-seek-overlay";
    this.el.hidden = true;
    this.el.setAttribute("role", "status");
    this.el.setAttribute("aria-live", "polite");
    this.el.setAttribute("aria-labelledby", "replay-seek-title");

    this.panel = document.createElement("div");
    this.panel.className = "replay-seek-panel";

    this.closeButton = document.createElement("button");
    this.closeButton.type = "button";
    this.closeButton.className = "replay-seek-close";
    this.closeButton.setAttribute("aria-label", "Dismiss seeking notice");
    this.closeButton.title = "Dismiss seeking notice";
    this.closeButton.textContent = "×";
    this.onClose = () => this.dismiss();
    this.closeButton.addEventListener("click", this.onClose);

    this.title = document.createElement("div");
    this.title.id = "replay-seek-title";
    this.title.className = "replay-seek-title";
    this.title.textContent = "Seeking";

    this.detail = document.createElement("div");
    this.detail.className = "replay-seek-detail";

    this.stopButton = document.createElement("button");
    this.stopButton.type = "button";
    this.stopButton.className = "replay-seek-stop";
    this.stopButton.textContent = "Stop seeking";
    this.stopButton.title = "Stop seeking at the current replay position and pause playback.";
    this.onStop = () => this.onStopSeeking?.();
    this.stopButton.addEventListener("click", this.onStop);

    this.panel.append(this.closeButton, this.title, this.detail, this.stopButton);
    this.el.appendChild(this.panel);
    this.root?.appendChild(this.el);
  }

  show(detail = "") {
    this.dismissed = false;
    this.detail.textContent = String(detail || "");
    this.detail.hidden = !this.detail.textContent;
    this.el.hidden = false;
  }

  hide() {
    this.el.hidden = true;
  }

  dismiss() {
    this.dismissed = true;
    this.hide();
  }

  destroy() {
    this.closeButton.removeEventListener("click", this.onClose);
    this.stopButton.removeEventListener("click", this.onStop);
    this.el.remove();
  }
}
