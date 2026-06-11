import { S } from "./protocol.js";

export class BranchStaging {
  constructor(rootEl, net) {
    this.root = rootEl;
    this.net = net;
    this._active = false;
    this._last = null;
    this._onStaging = (m) => this.render(m);
    this.net.on(S.REPLAY_BRANCH_STAGING, this._onStaging);
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
    this.root.classList.remove("branch-staging-active");
    this.root.hidden = true;
    this.root.replaceChildren();
  }

  destroy() {
    this.net.off(S.REPLAY_BRANCH_STAGING, this._onStaging);
    this.hide();
  }

  render(m) {
    if (!this._active || !m) return;
    this._last = m;
    const myId = this.net.playerId;
    const isHost = myId != null && myId === m.hostId;
    const seats = Array.isArray(m.seats) ? m.seats : [];
    const viewers = Array.isArray(m.viewers) ? m.viewers : [];
    const myClaim = seats.find((seat) => seat.claimedBy === myId);

    const box = document.createElement("div");
    box.className = "branch-staging-box";

    const title = document.createElement("h1");
    title.className = "logo";
    title.textContent = "Replay Branch";

    const status = document.createElement("p");
    status.className = "branch-staging-status";
    const claimedCount = seats.filter((seat) => seat.claimedBy != null || !seat.claimable).length;
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
    if (!viewers.length) {
      const li = document.createElement("li");
      li.textContent = "All viewers are seated.";
      viewerList.appendChild(li);
    } else {
      for (const viewer of viewers) {
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
      if (!start.disabled) this.net.start();
    });
    actions.appendChild(start);

    box.append(title, status, list, viewerBlock, actions);
    this.root.replaceChildren(box);
  }

  renderSeat(seat, myId, myClaim) {
    const row = document.createElement("article");
    row.className = "branch-seat";
    if (seat.claimedBy === myId) row.classList.add("is-you");
    if (!seat.claimable) row.classList.add("is-locked");

    const swatch = document.createElement("span");
    swatch.className = "player-color";
    swatch.style.background = seat.color || "#888";

    const body = document.createElement("div");
    body.className = "branch-seat-body";
    const name = document.createElement("h2");
    name.textContent = seat.name || `Player ${seat.playerId}`;
    const claim = document.createElement("p");
    claim.textContent = seat.claimedByName
      ? `Claimed by ${seat.claimedByName}`
      : seat.claimable ? "Open" : "Unavailable";
    body.append(name, claim);

    const action = document.createElement("button");
    action.type = "button";
    action.className = "btn";
    if (seat.claimedBy === myId) {
      action.textContent = "Release";
      action.addEventListener("click", () => this.net.releaseReplayBranchSeat());
    } else {
      action.textContent = "Claim";
      action.disabled = !seat.claimable || seat.claimedBy != null || !!myClaim;
      action.addEventListener("click", () => this.net.claimReplayBranchSeat(seat.playerId));
    }

    row.append(swatch, body, action);
    return row;
  }
}
