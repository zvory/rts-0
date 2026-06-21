import { PLAYABLE_FACTIONS } from "./lobby_view.js";
import { DEFAULT_FACTION_ID, LAB_ROLE, msg } from "./protocol.js";
import { factionCatalog, STATS, UPGRADES } from "./config.js";
import { LabPanelWindowChrome } from "./lab_panel_window.js";

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
    this.targetPlayerId = null;
    this.playerState = {
      steel: null,
      oil: null,
      researchUpgrade: "",
      researchCompleted: true,
    };
    this.spawnPalette = {
      factionId: DEFAULT_FACTION_ID,
      kind: "",
      completed: true,
    };
    this.advancedSpawn = {
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
    this.windowChrome = new LabPanelWindowChrome(this.el);
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
    this.windowChrome.clearRenderListeners();
    this.teamInputs.clear();
    this.fields.clear();
    this.el.replaceChildren();

    this.el.appendChild(this.windowChrome.renderHeader({
      kicker: "Lab",
      title: this.publicRoomName(),
    }));

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
      result.textContent = this.resultText(this.lastResult);
      result.dataset.state = this.lastResult.ok ? "ok" : "error";
    } else {
      result.textContent = "Ready";
      result.dataset.state = "idle";
    }
    this.el.appendChild(result);
    this.el.appendChild(this.windowChrome.renderResizeHandle());
  }

  renderSetupTools() {
    const root = document.createElement("section");
    root.className = "lab-tools";
    root.setAttribute("aria-label", "Lab setup tools");

    const selection = this.selectedEntities();
    const selectedIds = selectedEntityIds(selection);
    const issueOwner = singleOwner(selection);
    const hasSelection = selectedIds.length > 0;
    const selectedActionDisabled = !hasSelection;
    const selectedActionTitle = selectedActionDisabled ? "Select an entity first" : "";

    root.appendChild(this.renderActiveToolStatus());
    root.appendChild(this.renderTargetPlayer());
    root.appendChild(this.renderSpawnPalette());
    root.appendChild(this.renderAdvancedSpawn());

    root.appendChild(this.fieldset("Selected", [
      this.readout(`${selectedIds.length} selected`),
      this.button("Move to point", () => this.armMoveSelectedTool(), {
        disabled: selectedActionDisabled,
        title: selectedActionTitle,
        dataset: { active: this.activeLabTool()?.kind === "moveSelected" ? "true" : "false" },
      }),
      this.playerSelectField("set-owner", "Owner", {
        value: issueOwner ?? undefined,
        disabled: selectedActionDisabled,
      }),
      this.button("Set owner", () => this.setSelectedOwner(), {
        disabled: selectedActionDisabled,
        title: selectedActionTitle,
      }),
      this.button("Delete", () => this.deleteSelected(), {
        disabled: selectedActionDisabled,
        title: selectedActionTitle,
      }),
      this.readout(issueOwner == null ? "Issue-as requires one owner" : `Issue-as P${issueOwner}`),
    ]));

    this.normalizePlayerState();
    root.appendChild(this.fieldset("Player State", [
      this.numberField("resource-steel", "Steel", this.playerState.steel, {
        onChange: (value) => {
          this.playerState.steel = toUint(value);
        },
      }),
      this.numberField("resource-oil", "Oil", this.playerState.oil, {
        onChange: (value) => {
          this.playerState.oil = toUint(value);
        },
      }),
      this.button("Set resources", () => this.setPlayerResources()),
      this.selectField("research-upgrade", "Research", Object.keys(UPGRADES), upgradeLabels(), {
        value: this.playerState.researchUpgrade,
        onChange: (value) => {
          this.playerState.researchUpgrade = value;
        },
      }),
      this.checkboxField("research-completed", "Complete", this.playerState.researchCompleted, {
        onChange: (checked) => {
          this.playerState.researchCompleted = checked;
        },
      }),
      this.button("Set research", () => this.setCompletedResearch()),
    ]));

    root.appendChild(this.fieldset("Scenario", [
      this.inputField("scenario-name", "Name", "text", this.defaultScenarioName()),
      this.textAreaField("scenario-json", "JSON", ""),
      this.button("Export JSON", () => this.exportScenario()),
      this.button("Import JSON", () => this.importScenario()),
    ]));

    return root;
  }

  renderActiveToolStatus() {
    const active = this.activeLabTool();
    const section = document.createElement("section");
    section.className = "lab-active-tool";
    section.dataset.active = active ? "true" : "false";
    section.setAttribute("aria-live", "polite");

    const label = this.readout(active ? `Armed: ${labToolLabel(active)}` : "No setup tool armed");
    label.className = "lab-readout lab-active-tool-label";
    section.appendChild(label);

    if (active) {
      const detailText = active.keepArmedOnWorldClick
        ? "Click the map to apply repeatedly. Drag-select, right-click, or Esc cancels."
        : "Click the map to apply. Drag-select, right-click, or Esc cancels.";
      const detail = this.readout(detailText);
      detail.className = "lab-readout lab-active-tool-detail";
      section.appendChild(detail);
    }

    section.appendChild(this.button("Cancel tool", () => this.cancelActiveTool(), {
      disabled: !active,
      title: active ? `Cancel ${labToolLabel(active)}` : "No active setup tool",
      className: "lab-btn lab-cancel-tool",
    }));
    return section;
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

  resultText(result) {
    const summary = result?.outcome?.summary;
    if (typeof summary === "string" && summary) return summary;
    if (result?.ok) return `${result.op || "request"} accepted`;
    return result?.error || `${result?.op || "request"} rejected`;
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
    if (options.disabled) {
      button.disabled = true;
      button.setAttribute("aria-disabled", "true");
    }
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

  numberField(id, label, value, options = {}) {
    const wrap = this.inputField(id, label, "number", value, options);
    const input = this.fields.get(id);
    input.step = "1";
    return wrap;
  }

  checkboxField(id, label, checked, options = {}) {
    const wrap = this.fieldWrap(label);
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = !!checked;
    if (options.disabled) input.disabled = true;
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
    if (options.disabled) input.disabled = true;
    if (typeof options.onChange === "function") {
      const handleChange = () => options.onChange(input.value);
      this.listen(input, "input", handleChange);
      this.listen(input, "change", handleChange);
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
    if (options.disabled) select.disabled = true;
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

  renderTargetPlayer() {
    return this.fieldset("Target Player", [
      this.playerSelectField("lab-player", "Player", {
        value: this.targetPlayer(),
        onChange: (value) => {
          this.targetPlayerId = this.validOwner(value);
        },
      }),
    ]);
  }

  renderSpawnPalette() {
    this.normalizeSpawnPalette();
    const factionOptions = labSpawnFactionOptions();
    const factionLabels = Object.fromEntries(factionOptions.map((entry) => [entry.id, entry.label]));
    const unitKinds = labSpawnUnitKindsForFaction(this.spawnPalette.factionId);
    const controls = [
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
      this.checkboxField("advanced-spawn-completed", "Complete", this.advancedSpawn.completed, {
        onChange: (checked) => {
          this.advancedSpawn.completed = checked;
        },
      }),
      this.button("Arm spawn", () => this.armAdvancedSpawnTool(), {
        dataset: { active: this.spawnToolActive(this.advancedSpawn.kind) ? "true" : "false" },
      }),
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
          active: this.spawnToolActive(kind) ? "true" : "false",
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
      owner: this.targetPlayer(),
      factionId: this.spawnPalette.factionId,
      kind,
      completed: this.spawnPalette.completed,
    };
    return this.armSpawnTool(payload);
  }

  armAdvancedSpawnTool() {
    this.captureAdvancedSpawnFields();
    if (!this.advancedSpawn.kind) return null;
    return this.armSpawnTool({
      owner: this.targetPlayer(),
      kind: this.advancedSpawn.kind,
      completed: this.advancedSpawn.completed,
    });
  }

  armSpawnTool(payload) {
    if (typeof this.match?.armLabTool !== "function") return null;
    const kind = payload?.kind || "";
    const armed = this.match.armLabTool(
      {
        kind: "spawnEntity",
        payload: { ...payload },
        label: `Spawn ${KIND_LABELS[kind] || kind}`,
        keepArmedOnWorldClick: true,
      },
      { onWorldClick: (event) => this.spawnEntityAt(event) },
    );
    this.render();
    return armed;
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
    this.targetPlayerId = this.validOwner(this.targetPlayerId);
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
    this.targetPlayerId = this.validOwner(this.targetPlayerId);
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
    this.captureTargetPlayerField();
    this.spawnPalette.factionId = this.value("spawn-faction") || this.spawnPalette.factionId;
    this.spawnPalette.completed = this.bool("spawn-completed");
  }

  captureAdvancedSpawnFields() {
    this.captureTargetPlayerField();
    this.advancedSpawn.kind = this.value("advanced-spawn-kind") || this.advancedSpawn.kind;
    this.advancedSpawn.completed = this.bool("advanced-spawn-completed");
  }

  captureTargetPlayerField() {
    this.targetPlayerId = this.validOwner(this.int("lab-player") || this.targetPlayerId);
    return this.targetPlayerId;
  }

  targetPlayer() {
    this.targetPlayerId = this.validOwner(this.targetPlayerId);
    return this.targetPlayerId;
  }

  normalizePlayerState() {
    this.targetPlayer();
    const resources = this.resourcesForTargetPlayer();
    if (this.playerState.steel == null) this.playerState.steel = resources.steel;
    if (this.playerState.oil == null) this.playerState.oil = resources.oil;
    const upgrades = Object.keys(UPGRADES);
    if (!upgrades.includes(this.playerState.researchUpgrade)) {
      this.playerState.researchUpgrade = upgrades[0] || "";
    }
    this.playerState.researchCompleted = !!this.playerState.researchCompleted;
  }

  capturePlayerStateFields() {
    this.captureTargetPlayerField();
    this.playerState.steel = this.uint("resource-steel");
    this.playerState.oil = this.uint("resource-oil");
    this.playerState.researchUpgrade = this.value("research-upgrade") || this.playerState.researchUpgrade;
    this.playerState.researchCompleted = this.bool("research-completed");
  }

  setPlayerResources() {
    this.capturePlayerStateFields();
    return this.labClient.setPlayerResources(
      this.targetPlayer(),
      this.playerState.steel,
      this.playerState.oil,
    );
  }

  setCompletedResearch() {
    this.capturePlayerStateFields();
    return this.labClient.setCompletedResearch(
      this.targetPlayer(),
      this.playerState.researchUpgrade,
      this.playerState.researchCompleted,
    );
  }

  armMoveSelectedTool() {
    if (typeof this.match?.armLabTool !== "function") return null;
    const entityIds = selectedEntityIds(this.selectedEntities());
    if (entityIds.length === 0) {
      return this.publishLocalResult("moveEntity", false, "Select an entity first.");
    }
    const armed = this.match.armLabTool(
      {
        kind: "moveSelected",
        payload: { entityIds },
        label: `Move ${entityIds.length} selected`,
      },
      { onWorldClick: (event) => this.moveSelectedTo(event) },
    );
    this.render();
    return armed;
  }

  cancelActiveTool() {
    return this.match?.cancelLabTool?.("panelCancel") || null;
  }

  applyLabToolChange(change) {
    if (change?.type === "cancelled" && shouldSurfaceToolCancellation(change.reason)) {
      const summary = `${labToolLabel(change.tool)} cancelled.`;
      this.lastResult = {
        requestId: 0,
        ok: true,
        op: "labTool",
        error: "",
        outcome: { summary },
      };
    }
    this.render();
  }

  moveSelectedTo(event) {
    const entityIds = selectedEntityIdsFromPayload(event?.tool?.payload?.entityIds);
    if (!Number.isFinite(event?.x) || !Number.isFinite(event?.y)) {
      return this.publishLocalResult("moveEntity", false, "Pick a valid world point.");
    }
    return this.batchEntityMutation("moveEntity", entityIds, (entityId) => (
      this.labClient.moveEntity(entityId, event.x, event.y)
    ));
  }

  setSelectedOwner() {
    const owner = this.validOwner(this.int("set-owner"));
    return this.batchEntityMutation("setEntityOwner", selectedEntityIds(this.selectedEntities()), (entityId) => (
      this.labClient.setEntityOwner(entityId, owner)
    ));
  }

  deleteSelected() {
    return this.batchEntityMutation("deleteEntity", selectedEntityIds(this.selectedEntities()), (entityId) => (
      this.labClient.deleteEntity(entityId)
    ));
  }

  async batchEntityMutation(op, entityIds, request) {
    const ids = selectedEntityIdsFromPayload(entityIds);
    if (ids.length === 0) {
      return this.publishLocalResult(op, false, "Select an entity first.");
    }
    const results = [];
    for (const entityId of ids) {
      const result = await request(entityId);
      results.push({ entityId, result });
    }
    return this.publishBatchResult(op, results);
  }

  publishBatchResult(op, results) {
    const failures = results
      .filter(({ result }) => !result?.ok)
      .map(({ entityId, result }) => ({
        entityId,
        error: result?.error || `${op} rejected`,
      }));
    const accepted = results.length - failures.length;
    const summary = batchResultSummary(op, accepted, failures);
    return this.publishLocalResult(op, failures.length === 0, summary, {
      requestId: results.at(-1)?.result?.requestId,
      outcome: {
        summary,
        accepted,
        rejected: failures.length,
        failures,
      },
    });
  }

  publishLocalResult(op, ok, message, options = {}) {
    this.lastResult = {
      requestId: Number.isFinite(options.requestId) ? options.requestId : 0,
      ok: !!ok,
      op,
      error: ok ? "" : message,
      outcome: options.outcome || (ok ? { summary: message } : null),
    };
    this.render();
    return Promise.resolve(this.lastResult);
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

  selectedEntities() {
    return typeof this.match?.state?.selectedEntities === "function"
      ? this.match.state.selectedEntities()
      : [];
  }

  activeLabTool() {
    return this.match?.clientIntent?.activeLabTool || null;
  }

  spawnToolActive(kind) {
    const active = this.activeLabTool();
    return active?.kind === "spawnEntity" && active?.payload?.kind === kind;
  }

  resourcesForTargetPlayer() {
    const target = this.targetPlayer();
    const rows = this.match?.state?.playerResources || [];
    const byId = rows.find((row) => Number(row?.id) === target);
    const fallback = rows[target - 1] || rows[0] || null;
    const resources = byId || fallback;
    return { steel: resources?.steel ?? 0, oil: resources?.oil ?? 0 };
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
    this.windowChrome.destroy();
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

function toUint(value) {
  const numeric = toIntOrNull(value);
  return numeric == null ? 0 : Math.max(0, numeric);
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

function selectedEntityIds(selection) {
  return selectedEntityIdsFromPayload((selection || []).map((entity) => entity?.id));
}

function selectedEntityIdsFromPayload(entityIds) {
  if (!Array.isArray(entityIds)) return [];
  const seen = new Set();
  const ids = [];
  for (const value of entityIds) {
    const id = Number(value);
    if (!Number.isInteger(id) || id <= 0 || seen.has(id)) continue;
    seen.add(id);
    ids.push(id);
  }
  return ids;
}

function batchResultSummary(op, accepted, failures) {
  const label = batchOperationLabel(op);
  const rejected = failures.length;
  const acceptedText = accepted > 0 ? `${label.success} ${accepted} ${entityNoun(accepted)}` : "";
  const rejectedText = rejected > 0
    ? `${rejected} rejected${failureDetails(failures)}`
    : "";
  if (acceptedText && rejectedText) return `${acceptedText}; ${rejectedText}`;
  if (acceptedText) return `${acceptedText}.`;
  return `${label.failure} rejected for ${rejected} ${entityNoun(rejected)}${failureDetails(failures)}`;
}

function batchOperationLabel(op) {
  if (op === "moveEntity") return { success: "Moved", failure: "Move" };
  if (op === "setEntityOwner") return { success: "Updated owner for", failure: "Owner change" };
  if (op === "deleteEntity") return { success: "Deleted", failure: "Delete" };
  return { success: `${op} accepted for`, failure: op };
}

function labToolLabel(tool) {
  if (typeof tool?.label === "string" && tool.label) return tool.label;
  if (tool?.kind === "spawnEntity") {
    const kind = tool?.payload?.kind || "";
    return kind ? `Spawn ${KIND_LABELS[kind] || kind}` : "Spawn";
  }
  if (tool?.kind === "moveSelected") return "Move selected";
  return "Setup tool";
}

function shouldSurfaceToolCancellation(reason) {
  return reason === "escape" || reason === "rightClick" || reason === "panelCancel";
}

function entityNoun(count) {
  return count === 1 ? "entity" : "entities";
}

function failureDetails(failures) {
  if (!failures.length) return "";
  const shown = failures.slice(0, 3).map((failure) => `#${failure.entityId}: ${failure.error}`);
  const suffix = failures.length > shown.length ? `; +${failures.length - shown.length} more` : "";
  return `: ${shown.join("; ")}${suffix}.`;
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
