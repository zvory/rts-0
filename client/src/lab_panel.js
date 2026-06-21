import { PLAYABLE_FACTIONS } from "./lobby_view.js";
import { DEFAULT_FACTION_ID, LAB_ROLE, msg } from "./protocol.js";
import { factionCatalog, STATS, UPGRADES } from "./config.js";

const labVision = Object.freeze({
  fullWorld: () => msg.labVisionFullWorld(),
  team: (teamId) => msg.labVisionTeam(teamId),
  teams: (teamIds) => msg.labVisionTeams(teamIds),
});

export class LabPanel {
  constructor({ root, labClient, launch = null, startPayload = null, match = null }) {
    this.root = root;
    this.labClient = labClient;
    this.launch = launch;
    this.startPayload = startPayload;
    this.match = match;
    this.state = labClient?.state || startPayload?.lab || null;
    this.lastResult = labClient?.lastResult || null;
    this.spawnPalette = {
      owner: null,
      factionId: DEFAULT_FACTION_ID,
      kind: "",
      completed: true,
    };
    this.advancedSpawn = {
      owner: null,
      kind: "",
      completed: true,
    };
    this.teamInputs = new Map();
    this.fields = new Map();
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
    this.fields.clear();
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

    if (this.canOperate()) {
      this.el.appendChild(this.renderSetupTools());
    }

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

  renderSetupTools() {
    const root = document.createElement("section");
    root.className = "lab-tools";
    root.setAttribute("aria-label", "Lab setup tools");

    const selection = this.selectedEntities();
    const selectedIds = selection.map((entity) => entity.id);
    const issueOwner = singleOwner(selection);
    const point = this.defaultWorldPoint();

    root.appendChild(this.renderSpawnPalette());
    root.appendChild(this.renderAdvancedSpawn());

    root.appendChild(this.fieldset("Selected", [
      this.readout(`${selectedIds.length} selected`),
      this.numberField("move-x", "X", point.x),
      this.numberField("move-y", "Y", point.y),
      this.button("Pick point", () => this.armPointFieldTool("move-x", "move-y")),
      this.button("Move", () => this.batchSelected((entity) => this.labClient.moveEntity(entity.id, this.num("move-x"), this.num("move-y")))),
      this.playerSelectField("set-owner", "Owner"),
      this.button("Set owner", () => this.batchSelected((entity) => this.labClient.setEntityOwner(entity.id, this.int("set-owner")))),
      this.button("Delete", () => this.batchSelected((entity) => this.labClient.deleteEntity(entity.id))),
      this.readout(issueOwner == null ? "Issue-as requires one owner" : `Issue-as P${issueOwner}`),
    ]));

    root.appendChild(this.fieldset("Player State", [
      this.playerSelectField("resource-player", "Player"),
      this.numberField("resource-steel", "Steel", this.resourcesForFirstPlayer().steel),
      this.numberField("resource-oil", "Oil", this.resourcesForFirstPlayer().oil),
      this.button("Set resources", () => this.labClient.setPlayerResources(
        this.int("resource-player"),
        this.uint("resource-steel"),
        this.uint("resource-oil"),
      )),
      this.playerSelectField("research-player", "Player"),
      this.selectField("research-upgrade", "Research", Object.keys(UPGRADES), upgradeLabels()),
      this.checkboxField("research-completed", "Complete", true),
      this.button("Set research", () => this.labClient.setCompletedResearch(
        this.int("research-player"),
        this.value("research-upgrade"),
        this.bool("research-completed"),
      )),
    ]));

    root.appendChild(this.fieldset("Scenario", [
      this.inputField("scenario-name", "Name", "text", this.defaultScenarioName()),
      this.textAreaField("scenario-json", "JSON", ""),
      this.button("Export JSON", () => this.exportScenario()),
      this.button("Import JSON", () => this.importScenario()),
    ]));

    return root;
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

  listen(target, type, handler) {
    target.addEventListener(type, handler);
    this.listeners.push([target, type, handler]);
  }

  button(label, onClick, options = {}) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = options.className || "lab-btn";
    button.textContent = label;
    if (options.title) button.title = options.title;
    if (options.dataset) {
      for (const [key, value] of Object.entries(options.dataset)) {
        button.dataset[key] = String(value);
      }
    }
    this.listen(button, "click", onClick);
    return button;
  }

  fieldset(title, children) {
    const section = document.createElement("section");
    section.className = "lab-tool-group";
    const h = document.createElement("h3");
    h.textContent = title;
    section.appendChild(h);
    for (const child of children) section.appendChild(child);
    return section;
  }

  readout(text) {
    const node = document.createElement("p");
    node.className = "lab-readout";
    node.textContent = text;
    return node;
  }

  numberField(id, label, value) {
    const wrap = this.inputField(id, label, "number", value);
    const input = this.fields.get(id);
    input.step = "1";
    return wrap;
  }

  checkboxField(id, label, checked, options = {}) {
    const wrap = this.fieldWrap(label);
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = !!checked;
    if (typeof options.onChange === "function") {
      this.listen(input, "change", () => options.onChange(!!input.checked));
    }
    this.fields.set(id, input);
    wrap.appendChild(input);
    return wrap;
  }

  inputField(id, label, type, value, options = {}) {
    const wrap = this.fieldWrap(label);
    const input = document.createElement("input");
    input.type = type;
    input.value = String(value ?? "");
    if (typeof options.onChange === "function") {
      this.listen(input, "change", () => options.onChange(input.value));
    }
    this.fields.set(id, input);
    wrap.appendChild(input);
    return wrap;
  }

  textAreaField(id, label, value) {
    const wrap = this.fieldWrap(label);
    const input = document.createElement("textarea");
    input.value = String(value ?? "");
    input.rows = 5;
    this.fields.set(id, input);
    wrap.appendChild(input);
    return wrap;
  }

  selectField(id, label, values, labels = {}, options = {}) {
    const wrap = this.fieldWrap(label);
    const select = document.createElement("select");
    for (const value of values) {
      const option = document.createElement("option");
      option.value = String(value);
      option.textContent = labels[value] || String(value);
      select.appendChild(option);
    }
    if (values.map(String).includes(String(options.value))) {
      select.value = String(options.value);
    }
    if (typeof options.onChange === "function") {
      this.listen(select, "change", () => options.onChange(select.value));
    }
    this.fields.set(id, select);
    wrap.appendChild(select);
    return wrap;
  }

  playerSelectField(id, label, options = {}) {
    const labels = {};
    const values = this.players().map((player) => {
      labels[player.id] = player.name ? `P${player.id} ${player.name}` : `P${player.id}`;
      return player.id;
    });
    return this.selectField(id, label, values, labels, options);
  }

  fieldWrap(labelText) {
    const label = document.createElement("label");
    label.className = "lab-field";
    const span = document.createElement("span");
    span.textContent = labelText;
    label.appendChild(span);
    return label;
  }

  renderSpawnPalette() {
    this.normalizeSpawnPalette();
    const factionOptions = labSpawnFactionOptions();
    const factionLabels = Object.fromEntries(factionOptions.map((entry) => [entry.id, entry.label]));
    const unitKinds = labSpawnUnitKindsForFaction(this.spawnPalette.factionId);
    const controls = [
      this.playerSelectField("spawn-owner", "Owner", {
        value: this.spawnPalette.owner,
        onChange: (value) => {
          this.spawnPalette.owner = toIntOrNull(value);
        },
      }),
      this.selectField(
        "spawn-faction",
        "Faction",
        factionOptions.map((entry) => entry.id),
        factionLabels,
        {
          value: this.spawnPalette.factionId,
          onChange: (value) => {
            this.spawnPalette.factionId = value;
            this.spawnPalette.kind = "";
            this.render();
          },
        },
      ),
      this.checkboxField("spawn-completed", "Complete", this.spawnPalette.completed, {
        onChange: (checked) => {
          this.spawnPalette.completed = checked;
        },
      }),
      this.spawnPaletteReadout(unitKinds),
      this.spawnPaletteGrid(unitKinds),
    ];
    return this.fieldset("Unit Spawn", controls);
  }

  renderAdvancedSpawn() {
    this.normalizeAdvancedSpawn();
    return this.fieldset("Advanced Spawn", [
      this.selectField("advanced-spawn-kind", "Kind", spawnKinds(), KIND_LABELS, {
        value: this.advancedSpawn.kind,
        onChange: (value) => {
          this.advancedSpawn.kind = value;
        },
      }),
      this.playerSelectField("advanced-spawn-owner", "Owner", {
        value: this.advancedSpawn.owner,
        onChange: (value) => {
          this.advancedSpawn.owner = toIntOrNull(value);
        },
      }),
      this.checkboxField("advanced-spawn-completed", "Complete", this.advancedSpawn.completed, {
        onChange: (checked) => {
          this.advancedSpawn.completed = checked;
        },
      }),
      this.button("Arm spawn", () => this.armAdvancedSpawnTool()),
    ]);
  }

  spawnPaletteGrid(unitKinds) {
    const grid = document.createElement("div");
    grid.className = "lab-spawn-palette";
    for (const kind of unitKinds) {
      const stats = STATS[kind] || {};
      const button = this.button(stats.label || kind, () => this.armSpawnPaletteTool(kind), {
        className: "lab-btn lab-spawn-option",
        title: `Spawn ${stats.label || kind}`,
        dataset: {
          kind,
          selected: kind === this.spawnPalette.kind ? "true" : "false",
        },
      });
      grid.appendChild(button);
    }
    return grid;
  }

  spawnPaletteReadout(unitKinds) {
    if (unitKinds.length > 0) {
      return this.readout(`${factionLabel(this.spawnPalette.factionId)} units`);
    }
    return this.readout("No unit catalog entries");
  }

  armSpawnPaletteTool(kind = this.spawnPalette.kind) {
    this.captureSpawnPaletteFields();
    if (!kind) return null;
    this.spawnPalette.kind = kind;
    const payload = {
      owner: this.spawnPalette.owner,
      factionId: this.spawnPalette.factionId,
      kind,
      completed: this.spawnPalette.completed,
    };
    const armed = this.armSpawnTool(payload);
    this.render();
    return armed;
  }

  armAdvancedSpawnTool() {
    this.captureAdvancedSpawnFields();
    if (!this.advancedSpawn.kind) return null;
    return this.armSpawnTool({
      owner: this.advancedSpawn.owner,
      kind: this.advancedSpawn.kind,
      completed: this.advancedSpawn.completed,
    });
  }

  armSpawnTool(payload) {
    if (typeof this.match?.armLabTool !== "function") return null;
    const kind = payload?.kind || "";
    return this.match.armLabTool(
      {
        kind: "spawnEntity",
        payload: { ...payload },
        label: `Spawn ${KIND_LABELS[kind] || kind}`,
      },
      { onWorldClick: (event) => this.spawnEntityAt(event) },
    );
  }

  spawnEntityAt(event) {
    const payload = event?.tool?.payload || {};
    if (!Number.isFinite(event?.x) || !Number.isFinite(event?.y)) return Promise.resolve(null);
    return this.labClient.spawnEntity({
      kind: payload.kind,
      owner: Number(payload.owner),
      x: event.x,
      y: event.y,
      completed: !!payload.completed,
    });
  }

  normalizeSpawnPalette() {
    this.spawnPalette.owner = this.validOwner(this.spawnPalette.owner);
    const factions = labSpawnFactionOptions();
    if (!factions.some((entry) => entry.id === this.spawnPalette.factionId)) {
      this.spawnPalette.factionId = factions[0]?.id || DEFAULT_FACTION_ID;
    }
    const unitKinds = labSpawnUnitKindsForFaction(this.spawnPalette.factionId);
    if (!unitKinds.includes(this.spawnPalette.kind)) {
      this.spawnPalette.kind = unitKinds[0] || "";
    }
    this.spawnPalette.completed = !!this.spawnPalette.completed;
  }

  normalizeAdvancedSpawn() {
    this.advancedSpawn.owner = this.validOwner(this.advancedSpawn.owner);
    const kinds = spawnKinds();
    if (!kinds.includes(this.advancedSpawn.kind)) {
      this.advancedSpawn.kind = kinds[0] || "";
    }
    this.advancedSpawn.completed = !!this.advancedSpawn.completed;
  }

  validOwner(owner) {
    const numericOwner = Number(owner);
    const owners = this.players().map((player) => Number(player.id)).filter((id) => Number.isFinite(id));
    return owners.includes(numericOwner) ? numericOwner : (owners[0] ?? 1);
  }

  captureSpawnPaletteFields() {
    this.spawnPalette.owner = this.int("spawn-owner") || this.spawnPalette.owner;
    this.spawnPalette.factionId = this.value("spawn-faction") || this.spawnPalette.factionId;
    this.spawnPalette.completed = this.bool("spawn-completed");
  }

  captureAdvancedSpawnFields() {
    this.advancedSpawn.owner = this.int("advanced-spawn-owner") || this.advancedSpawn.owner;
    this.advancedSpawn.kind = this.value("advanced-spawn-kind") || this.advancedSpawn.kind;
    this.advancedSpawn.completed = this.bool("advanced-spawn-completed");
  }

  armPointFieldTool(xField, yField) {
    if (typeof this.match?.armLabTool !== "function") return null;
    return this.match.armLabTool(
      { kind: "fieldPoint", payload: { xField, yField }, label: "Pick point" },
      { onWorldClick: (event) => this.applyPointFieldTool(event) },
    );
  }

  applyPointFieldTool(event) {
    const payload = event?.tool?.payload || {};
    if (!Number.isFinite(event?.x) || !Number.isFinite(event?.y)) return;
    const xInput = this.fields.get(payload.xField);
    const yInput = this.fields.get(payload.yField);
    if (!xInput || !yInput) return;
    xInput.value = String(Math.round(event.x));
    yInput.value = String(Math.round(event.y));
  }

  async exportScenario() {
    const result = await this.labClient.exportScenario(this.value("scenario-name"));
    const scenario = result?.outcome?.scenario;
    if (!result?.ok || !scenario) return result;
    const text = `${JSON.stringify(scenario, null, 2)}\n`;
    const field = this.fields.get("scenario-json");
    if (field) field.value = text;
    this.downloadScenarioJson(scenario, text);
    return result;
  }

  importScenario() {
    const text = this.value("scenario-json").trim();
    if (!text) return Promise.resolve(null);
    let scenario;
    try {
      scenario = JSON.parse(text);
    } catch (err) {
      this.lastResult = {
        ok: false,
        op: "importScenario",
        error: `Invalid JSON: ${err.message || err}`,
      };
      this.render();
      return Promise.resolve(this.lastResult);
    }
    return this.labClient.importScenario(scenario);
  }

  batchSelected(request) {
    const selected = this.selectedEntities();
    if (selected.length === 0) return Promise.resolve(null);
    return selected.reduce(
      (chain, entity) => chain.then(() => request(entity)),
      Promise.resolve(null),
    );
  }

  selectedEntities() {
    return typeof this.match?.state?.selectedEntities === "function"
      ? this.match.state.selectedEntities()
      : [];
  }

  defaultWorldPoint() {
    const camera = this.match?.camera;
    const map = this.match?.state?.map;
    if (camera && Number.isFinite(camera.x) && Number.isFinite(camera.y)) {
      return { x: Math.round(camera.x), y: Math.round(camera.y) };
    }
    return {
      x: Math.round((map?.width || 1024) / 2),
      y: Math.round((map?.height || 1024) / 2),
    };
  }

  resourcesForFirstPlayer() {
    const first = this.match?.state?.playerResources?.[0] || null;
    return { steel: first?.steel ?? 0, oil: first?.oil ?? 0 };
  }

  value(id) {
    return this.fields.get(id)?.value ?? "";
  }

  num(id) {
    const value = Number(this.value(id));
    return Number.isFinite(value) ? value : 0;
  }

  int(id) {
    return Math.trunc(this.num(id));
  }

  uint(id) {
    return Math.max(0, this.int(id));
  }

  bool(id) {
    return !!this.fields.get(id)?.checked;
  }

  canOperate() {
    return this.state?.role === LAB_ROLE.OPERATOR;
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

  defaultScenarioName() {
    const room = this.publicRoomName();
    const map = this.mapName();
    return `${room}-${map}`;
  }

  teamIds() {
    const ids = new Set();
    for (const player of this.startPayload?.players || []) {
      const teamId = Number(player?.teamId);
      if (Number.isFinite(teamId) && teamId > 0) ids.add(teamId);
    }
    return Array.from(ids).sort((a, b) => a - b);
  }

  players() {
    return (this.startPayload?.players || []).filter((player) => Number.isFinite(Number(player?.id)));
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

  downloadScenarioJson(scenario, text) {
    if (typeof Blob !== "function" || !globalThis.URL?.createObjectURL) return;
    const anchor = document.createElement("a");
    if (typeof anchor.click !== "function") return;
    const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
    anchor.href = url;
    anchor.download = `${slugifyScenarioName(scenario?.name || "lab-scenario")}.json`;
    anchor.click();
    URL.revokeObjectURL?.(url);
  }

  destroy() {
    this.match?.cancelLabTool?.("panelDestroy");
    this.unsubscribeState?.();
    this.unsubscribeResult?.();
    this.removeListeners();
    this.el.remove();
  }
}

const KIND_LABELS = Object.fromEntries(
  Object.entries(STATS).map(([kind, st]) => [kind, st.label || kind]),
);

function spawnKinds() {
  return Object.keys(STATS).filter((kind) => STATS[kind]?.cost || STATS[kind]?.trains);
}

export function labSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnUnitKindsForFaction(entry.id).length > 0);
}

export function labSpawnUnitKindsForFaction(factionId) {
  return factionCatalog(factionId).units.filter((kind) => STATS[kind]);
}

function factionLabel(factionId) {
  return labSpawnFactionOptions().find((entry) => entry.id === factionId)?.label || String(factionId || "");
}

function toIntOrNull(value) {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? Math.trunc(numeric) : null;
}

function upgradeLabels() {
  return Object.fromEntries(
    Object.entries(UPGRADES).map(([upgrade, def]) => [upgrade, def.label || upgrade]),
  );
}

function singleOwner(selection) {
  const owners = new Set((selection || []).map((entity) => Number(entity.owner)).filter((owner) => owner > 0));
  return owners.size === 1 ? Array.from(owners)[0] : null;
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

function slugifyScenarioName(name) {
  const slug = String(name || "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "lab-scenario";
}
