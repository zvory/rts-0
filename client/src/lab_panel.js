import { LAB_ROLE, msg } from "./protocol.js";

const labVision = Object.freeze({
  fullWorld: () => msg.labVisionFullWorld(),
  team: (teamId) => msg.labVisionTeam(teamId),
  teams: (teamIds) => msg.labVisionTeams(teamIds),
});

export class LabPanel {
  constructor({ root, labClient, launch = null, startPayload = null }) {
    this.root = root;
    this.labClient = labClient;
    this.launch = launch;
    this.startPayload = startPayload;
    this.state = labClient?.state || startPayload?.lab || null;
    this.lastResult = labClient?.lastResult || null;
    this.teamInputs = new Map();
    this.listeners = [];
    this.unsubscribeState = null;
    this.unsubscribeResult = null;
    this.el = document.createElement("aside");
    this.el.id = "lab-panel";
    this.el.className = "lab-panel";
    this.el.setAttribute("aria-label", "Lab controls");
    this.root.appendChild(this.el);
    this.render();
    this.unsubscribeState = this.labClient.subscribeState((state) => {
      this.state = state;
      this.render();
    });
    this.unsubscribeResult = this.labClient.subscribeResult((result) => {
      this.lastResult = result;
      this.render();
    });
  }

  render() {
    this.removeListeners();
    this.teamInputs.clear();
    this.el.replaceChildren();

    const header = document.createElement("header");
    const kicker = document.createElement("span");
    kicker.textContent = "Lab";
    const title = document.createElement("h2");
    title.textContent = this.publicRoomName();
    header.append(kicker, title);
    this.el.appendChild(header);

    const status = document.createElement("dl");
    status.className = "lab-status-grid";
    this.addStatus(status, "Role", roleLabel(this.state?.role));
    this.addStatus(status, "Map", this.mapName());
    this.addStatus(status, "Vision", labVisionLabel(this.state?.vision));
    this.addStatus(status, "Dirty", this.state?.dirty ? "Yes" : "No");
    this.addStatus(status, "Ops", String(this.state?.operationCount ?? 0));
    this.el.appendChild(status);

    const controls = document.createElement("section");
    controls.className = "lab-vision-controls";
    controls.setAttribute("aria-label", "Lab vision");

    const fullButton = this.button("Full", () => this.requestVision(labVision.fullWorld()));
    controls.appendChild(fullButton);

    for (const teamId of this.teamIds()) {
      const button = this.button(`Team ${teamId}`, () => this.requestVision(labVision.team(teamId)));
      controls.appendChild(button);
    }

    const union = document.createElement("div");
    union.className = "lab-team-union";
    for (const teamId of this.teamIds()) {
      const label = document.createElement("label");
      const input = document.createElement("input");
      input.type = "checkbox";
      input.value = String(teamId);
      input.checked = this.visionIncludesTeam(teamId);
      const text = document.createElement("span");
      text.textContent = `T${teamId}`;
      this.teamInputs.set(teamId, input);
      label.append(input, text);
      union.appendChild(label);
    }
    if (this.teamInputs.size > 0) {
      const apply = this.button("Apply teams", () => this.requestTeamUnion());
      union.appendChild(apply);
      controls.appendChild(union);
    }
    this.el.appendChild(controls);

    const result = document.createElement("p");
    result.className = "lab-result";
    if (this.lastResult) {
      result.textContent = this.lastResult.ok
        ? `${this.lastResult.op || "request"} accepted`
        : this.lastResult.error || `${this.lastResult.op || "request"} rejected`;
      result.dataset.state = this.lastResult.ok ? "ok" : "error";
    } else {
      result.textContent = "Ready";
      result.dataset.state = "idle";
    }
    this.el.appendChild(result);
  }

  addStatus(root, label, value) {
    const row = document.createElement("div");
    const dt = document.createElement("dt");
    dt.textContent = label;
    const dd = document.createElement("dd");
    dd.textContent = value;
    row.append(dt, dd);
    root.appendChild(row);
  }

  button(label, onClick) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "lab-btn";
    button.textContent = label;
    button.addEventListener("click", onClick);
    this.listeners.push([button, "click", onClick]);
    return button;
  }

  requestVision(vision) {
    void this.labClient.setVision(vision);
  }

  requestTeamUnion() {
    const teamIds = Array.from(this.teamInputs.entries())
      .filter(([, input]) => input.checked)
      .map(([teamId]) => teamId);
    if (teamIds.length === 1) this.requestVision(labVision.team(teamIds[0]));
    else if (teamIds.length > 1) this.requestVision(labVision.teams(teamIds));
  }

  publicRoomName() {
    return this.launch?.publicRoom || this.state?.room || this.startPayload?.lab?.room || "default";
  }

  mapName() {
    return this.launch?.map || this.startPayload?.map?.name || "Default";
  }

  teamIds() {
    const ids = new Set();
    for (const player of this.startPayload?.players || []) {
      const teamId = Number(player?.teamId);
      if (Number.isFinite(teamId) && teamId > 0) ids.add(teamId);
    }
    return Array.from(ids).sort((a, b) => a - b);
  }

  visionIncludesTeam(teamId) {
    const vision = this.state?.vision;
    if (vision?.mode === "team") return Number(vision.teamId) === teamId;
    if (vision?.mode === "teams") return (vision.teamIds || []).map(Number).includes(teamId);
    return false;
  }

  removeListeners() {
    for (const [target, type, handler] of this.listeners) {
      target.removeEventListener?.(type, handler);
    }
    this.listeners = [];
  }

  destroy() {
    this.unsubscribeState?.();
    this.unsubscribeResult?.();
    this.removeListeners();
    this.el.remove();
  }
}

function roleLabel(role) {
  if (role === LAB_ROLE.OPERATOR) return "Operator";
  if (role === LAB_ROLE.READ_ONLY) return "Read-only";
  return "-";
}

function labVisionLabel(vision) {
  if (!vision || typeof vision !== "object") return "-";
  if (vision.mode === "fullWorld") return "Full world";
  if (vision.mode === "team") return `Team ${vision.teamId}`;
  if (vision.mode === "teams") return `Teams ${(vision.teamIds || []).join(", ")}`;
  return String(vision.mode || "-");
}
