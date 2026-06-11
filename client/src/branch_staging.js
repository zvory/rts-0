import { S } from "./protocol.js";

export class BranchStaging {
  constructor(rootEl, net) {
    this.root = rootEl;
    this.net = net;
    this._active = false;
    this._last = null;
    this._countdownEl = null;
    this._countdownTimers = [];
    this._onStaging = (m) => this.render(m);
    this._onMatchCountdown = (m) => this.renderCountdown(m);
    this.net.on(S.BRANCH_STAGING, this._onStaging);
    this.net.on(S.MATCH_COUNTDOWN, this._onMatchCountdown);
  }

  show() {
    this._active = true;
    this.root.hidden = false;
    this.root.classList.add("branch-staging-active");
    this.root.replaceChildren();
  }

  hide() {
    this._active = false;
    this._last = null;
    this.clearCountdown();
    this.root.classList.remove("branch-staging-active");
    this.root.hidden = true;
    this.root.replaceChildren();
  }

  destroy() {
    this.net.off(S.BRANCH_STAGING, this._onStaging);
    this.net.off(S.MATCH_COUNTDOWN, this._onMatchCountdown);
    this.hide();
  }

  render(m) {
    if (!this._active || !m) return;
    this._last = m;
    const myId = this.net.playerId;
    const isHost = myId != null && myId === m.hostId;
    const seats = Array.isArray(m.seats) ? m.seats : [];
    const occupants = Array.isArray(m.occupants) ? m.occupants : [];
    const myClaim = seats.find((seat) => seat.claimantId === myId);

    const box = document.createElement("div");
    box.className = "branch-staging-box";

    const title = document.createElement("h1");
    title.className = "logo";
    title.textContent = "Replay Branch";

    const status = document.createElement("p");
    status.className = "branch-staging-status";
    const claimedCount = seats.filter((seat) => seat.claimantId != null).length;
    status.textContent = `Tick ${Number(m.sourceTick) || 0} - ${claimedCount} / ${seats.length} seats claimed`;

    const list = document.createElement("div");
    list.className = "branch-seat-list";
    for (const seat of seats) {
      list.appendChild(this.renderSeat(seat, myId, myClaim));
    }

    const viewerBlock = document.createElement("section");
    viewerBlock.className = "branch-viewers";
    const viewerTitle = document.createElement("h2");
    viewerTitle.textContent = "Viewers";
    const viewerList = document.createElement("ul");
    viewerList.className = "branch-viewer-list";
    const unseated = occupants.filter((occupant) => !seats.some((seat) => seat.claimantId === occupant.id));
    if (!unseated.length) {
      const li = document.createElement("li");
      li.textContent = "All occupants are seated.";
      viewerList.appendChild(li);
    } else {
      for (const viewer of unseated) {
        const li = document.createElement("li");
        li.textContent = viewer.name || `Viewer ${viewer.id}`;
        if (viewer.id === myId) li.classList.add("is-you");
        viewerList.appendChild(li);
      }
    }
    viewerBlock.append(viewerTitle, viewerList);

    const actions = document.createElement("div");
    actions.className = "branch-actions";
    const start = document.createElement("button");
    start.type = "button";
    start.className = "btn primary";
    start.textContent = "Start branch";
    start.hidden = !isHost;
    start.disabled = !m.canStart;
    start.addEventListener("click", () => {
      if (!start.disabled) this.net.startBranch();
    });
    actions.appendChild(start);

    box.append(title, status, list, viewerBlock, actions);
    this.root.replaceChildren(box);
  }

  renderSeat(seat, myId, myClaim) {
    const row = document.createElement("article");
    row.className = "branch-seat";
    if (seat.claimantId === myId) row.classList.add("is-you");

    const swatch = document.createElement("span");
    swatch.className = "player-color";
    swatch.style.background = seat.color || "#888";

    const body = document.createElement("div");
    body.className = "branch-seat-body";
    const name = document.createElement("h2");
    name.textContent = seat.name || `Player ${seat.playerId}`;
    const claim = document.createElement("p");
    claim.textContent = seat.claimantName ? `Claimed by ${seat.claimantName}` : "Open";
    body.append(name, claim);

    const action = document.createElement("button");
    action.type = "button";
    action.className = "btn";
    if (seat.claimantId === myId) {
      action.textContent = "Release";
      action.addEventListener("click", () => this.net.releaseBranchSeat(seat.playerId));
    } else {
      action.textContent = "Claim";
      action.disabled = seat.claimantId != null || !!myClaim;
      action.addEventListener("click", () => this.net.claimBranchSeat(seat.playerId));
    }

    row.append(swatch, body, action);
    return row;
  }

  renderCountdown(m) {
    if (!this._active) return;
    const words = Array.isArray(m?.words) && m.words.length
      ? m.words.map((word) => String(word))
      : ["Drei!", "Zwei!", "Eins!"];
    const durationMs = Math.max(1000, Number(m?.durationMs) || words.length * 1000);
    const wordMs = Math.max(250, durationMs / words.length);

    this.clearCountdown();

    const overlay = document.createElement("div");
    overlay.className = "match-countdown";
    overlay.setAttribute("role", "status");
    overlay.setAttribute("aria-live", "assertive");
    this._countdownEl = overlay;
    this.root.appendChild(overlay);

    const showWord = (word) => {
      if (!this._countdownEl) return;
      this._countdownEl.textContent = word;
      this._countdownEl.classList.remove("pulse");
      void this._countdownEl.offsetWidth;
      this._countdownEl.classList.add("pulse");
    };

    words.forEach((word, index) => {
      const delay = Math.round(index * wordMs);
      if (delay <= 0) {
        showWord(word);
      } else {
        this._countdownTimers.push(globalThis.setTimeout(() => showWord(word), delay));
      }
    });
    this._countdownTimers.push(globalThis.setTimeout(() => this.clearCountdown(), durationMs + 1000));
  }

  clearCountdown() {
    for (const timer of this._countdownTimers) globalThis.clearTimeout(timer);
    this._countdownTimers = [];
    if (this._countdownEl) {
      this._countdownEl.remove();
      this._countdownEl = null;
    }
  }
}
