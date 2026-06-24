import { PLAYABLE_FACTIONS } from "./lobby_view.js";
import { DEFAULT_FACTION_ID, LAB_ROLE, msg } from "./protocol.js";
import { factionCatalog, PLAYER_PALETTE, STATS, UPGRADES } from "./config.js";
import { LabPanelWindowChrome } from "./lab_panel_window.js";

const labVision = Object.freeze({
  fullWorld: () => msg.labVisionFullWorld(),
  team: (teamId) => msg.labVisionTeam(teamId),
  teams: (teamIds) => msg.labVisionTeams(teamIds),
});
const GIVE_ALL_RESOURCE_AMOUNT = 99999;
const OPTIONS_PANEL_STORAGE_KEY = "rts.labPanel.options.window.v1";
const TOOLS_PANEL_STORAGE_KEY = "rts.labPanel.tools.window.v1";

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
    };
    this.spawnPalette = {
      factionId: DEFAULT_FACTION_ID,
      kind: "",
    };
    this.buildingSpawnPalette = {
      factionId: DEFAULT_FACTION_ID,
      kind: "",
    };
    this.teamInputs = new Map();
    this.playerButtons = new Map();
    this.spawnPanels = new Map();
    this.fields = new Map();
    this.listeners = [];
    this.unsubscribeState = null;
    this.unsubscribeResult = null;
    this.optionsEl = this.createPanelElement("lab-options-panel", "lab-options-window", "Lab options and room information");
    this.toolsEl = this.createPanelElement("lab-tools-panel", "lab-tools-window", "Lab setup tools");
    this.el = this.optionsEl;
    this.root.append(this.optionsEl, this.toolsEl);
    this.optionsWindowChrome = new LabPanelWindowChrome(this.optionsEl, {
      storageKey: OPTIONS_PANEL_STORAGE_KEY,
    });
    this.toolsWindowChrome = new LabPanelWindowChrome(this.toolsEl, {
      storageKey: TOOLS_PANEL_STORAGE_KEY,
    });
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

  createPanelElement(id, className, ariaLabel) {
    const el = document.createElement("aside");
    el.id = id;
    el.className = `lab-panel ${className}`;
    el.setAttribute("aria-label", ariaLabel);
    return el;
  }

  render() {
    this.removeListeners();
    this.optionsWindowChrome.clearRenderListeners();
    this.toolsWindowChrome.clearRenderListeners();
    this.teamInputs.clear();
    this.playerButtons.clear();
    this.spawnPanels.clear();
    this.fields.clear();

    this.renderOptionsWindow();
    this.renderToolsWindow();
  }

  renderOptionsWindow() {
    this.optionsEl.hidden = false;
    this.optionsEl.replaceChildren();
    this.optionsEl.appendChild(this.optionsWindowChrome.renderHeader({
      kicker: "Options",
      collapseLabel: "options panel",
    }));

    const body = this.panelBody();
    const status = document.createElement("dl");
    status.className = "lab-status-grid";
    this.addStatus(status, "Role", roleLabel(this.state?.role));
    this.addStatus(status, "Map", this.mapName());
    this.addStatus(status, "Vision", labVisionLabel(this.state?.vision));
    this.addStatus(status, "Dirty", this.state?.dirty ? "Yes" : "No");
    this.addStatus(status, "Ops", String(this.state?.operationCount ?? 0));
    body.appendChild(status);
    body.appendChild(this.renderOptionsPanel());
    body.appendChild(this.renderResultStatus());

    this.optionsEl.appendChild(body);
    this.optionsEl.appendChild(this.optionsWindowChrome.renderResizeHandle());
  }

  renderToolsWindow() {
    if (!this.canOperate()) {
      this.toolsEl.hidden = true;
      this.toolsEl.replaceChildren();
      return;
    }

    this.toolsEl.hidden = false;
    this.toolsEl.replaceChildren();
    this.toolsEl.appendChild(this.toolsWindowChrome.renderHeader({
      kicker: "Tools",
      collapseLabel: "tools panel",
    }));

    const body = this.panelBody();
    body.appendChild(this.renderToolsPanel());
    this.toolsEl.appendChild(body);
    this.toolsEl.appendChild(this.toolsWindowChrome.renderResizeHandle());
  }

  panelBody(...children) {
    const body = document.createElement("div");
    body.className = "lab-panel-body";
    for (const child of children) body.appendChild(child);
    return body;
  }

  renderResultStatus() {
    const result = document.createElement("p");
    result.className = "lab-result";
    if (this.lastResult) {
      result.textContent = this.resultText(this.lastResult);
      result.dataset.state = this.lastResult.ok ? "ok" : "error";
    } else {
      result.textContent = "Ready";
      result.dataset.state = "idle";
    }
    return result;
  }

  panelSection(title, className) {
    const section = document.createElement("section");
    section.className = `lab-panel-section ${className}`;
    section.setAttribute("aria-label", title);
    const heading = document.createElement("h3");
    heading.className = "lab-panel-section-title";
    heading.textContent = title;
    section.appendChild(heading);
    return section;
  }

  renderOptionsPanel() {
    const root = this.panelSection("Options", "lab-options");
    root.appendChild(this.renderVisionOptions());

    if (!this.canOperate()) return root;

    root.appendChild(this.renderCommandOptions());

    root.appendChild(this.fieldset("Scenario", [
      this.inputField("scenario-name", "Name", "text", this.defaultScenarioName()),
      this.textAreaField("scenario-json", "JSON", ""),
      this.button("Export JSON", () => this.exportScenario()),
      this.button("Import JSON", () => this.importScenario()),
      this.button("Reset scenario", () => this.resetScenario()),
    ]));

    return root;
  }

  renderVisionOptions() {
    const controls = [];
    controls.push(this.button("Full", () => this.requestVision(labVision.fullWorld())));

    for (const teamId of this.teamIds()) {
      controls.push(this.button(`Team ${teamId}`, () => this.requestVision(labVision.team(teamId))));
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
      controls.push(union);
    }

    return this.fieldset("Vision", controls, { className: "lab-tool-group lab-vision-group" });
  }

  renderToolsPanel() {
    const root = this.panelSection("Tools", "lab-tools");

    root.appendChild(this.renderActiveToolStatus());
    root.appendChild(this.renderTargetPlayer());
    root.appendChild(this.renderPlayerStatePanel());
    root.appendChild(this.renderRemoveTool());
    root.appendChild(this.renderSpawnPalette());
    root.appendChild(this.renderBuildingSpawnPalette());

    return root;
  }

  renderCommandOptions() {
    return this.fieldset("Commands", [
      this.checkboxField("ignore-command-limits", "Unlimited commands", this.ignoreCommandLimitsEnabled(), {
        onChange: (enabled) => this.setIgnoreCommandLimits(enabled),
      }),
    ]);
  }

  renderPlayerStatePanel() {
    this.normalizePlayerState();
    return this.fieldset("Player State", [
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
      this.button("Give All", () => this.giveAllPlayerResources(), {
        title: "Give every player 99999 steel and 99999 oil",
      }),
      this.checkboxField("player-god-mode", "God mode", this.playerGodModeEnabled(), {
        onChange: (enabled) => this.setPlayerGodMode(enabled),
      }),
      this.selectField("research-upgrade", "Research", Object.keys(UPGRADES), upgradeLabels(), {
        value: this.playerState.researchUpgrade,
        onChange: (value) => {
          this.playerState.researchUpgrade = value;
        },
      }),
      this.button("Set research", () => this.setCompletedResearch()),
    ]);
  }

  renderRemoveTool() {
    const wrap = document.createElement("div");
    wrap.className = "lab-remove-tool-row";
    wrap.appendChild(
      this.button("Remove tool", () => this.armRemoveTool(), {
        title: "Click or drag over selectable units to delete them",
        dataset: { active: this.activeLabTool()?.kind === "removeSelectableUnits" ? "true" : "false" },
      }),
    );
    return wrap;
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
      const detailText = labToolDetailText(active);
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

  fieldset(title, children, options = {}) {
    const section = document.createElement("section");
    section.className = options.className || "lab-tool-group";
    if (options.dataset) {
      for (const [key, value] of Object.entries(options.dataset)) {
        section.dataset[key] = String(value);
      }
    }
    if (options.styles) {
      for (const [key, value] of Object.entries(options.styles)) {
        section.style.setProperty(key, String(value));
      }
    }
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
      this.playerButtonField("lab-player", "Player"),
    ]);
  }

  playerButtonField(id, labelText) {
    const wrap = document.createElement("div");
    wrap.className = "lab-player-field";
    const label = document.createElement("span");
    label.className = "lab-player-label";
    label.textContent = labelText;
    const group = document.createElement("div");
    group.className = "lab-player-buttons";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", labelText);
    const selected = this.targetPlayer();
    this.fields.set(id, { value: String(selected) });
    this.players().forEach((player, index) => {
      const playerId = Number(player.id);
      if (!Number.isFinite(playerId)) return;
      const color = playerColor(player, index);
      const button = this.button(playerButtonLabel(player), () => this.selectTargetPlayer(playerId), {
        className: "lab-btn lab-player-btn",
        title: playerButtonTitle(player),
        dataset: {
          playerId,
          selected: playerId === selected ? "true" : "false",
          color,
        },
      });
      button.setAttribute("aria-pressed", playerId === selected ? "true" : "false");
      button.style.setProperty("--lab-player-color", color);
      button.style.setProperty("--lab-player-bg", hexToRgba(color, 0.18));
      button.style.setProperty("--lab-player-bg-active", hexToRgba(color, 0.42));
      button.style.setProperty("--lab-player-ring", hexToRgba(color, 0.72));
      this.playerButtons.set(playerId, button);
      group.appendChild(button);
    });
    wrap.append(label, group);
    return wrap;
  }

  selectTargetPlayer(owner) {
    this.captureVisibleSetupFields();
    this.targetPlayerId = this.validOwner(owner);
    this.syncTargetPlayerButtons();
    this.syncSpawnPanelTargetColors();
  }

  syncTargetPlayerButtons() {
    const selected = this.targetPlayer();
    const field = this.fields.get("lab-player");
    if (field) field.value = String(selected);
    for (const [playerId, button] of this.playerButtons.entries()) {
      const isSelected = playerId === selected;
      button.dataset.selected = isSelected ? "true" : "false";
      button.setAttribute("aria-pressed", isSelected ? "true" : "false");
    }
  }

  syncSpawnPanelTargetColors() {
    for (const [kind, section] of this.spawnPanels.entries()) {
      this.applySpawnTargetFieldsetOptions(section, kind);
    }
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
      this.spawnPaletteReadout(unitKinds),
      this.spawnPaletteGrid(unitKinds),
    ];
    const section = this.fieldset("Unit Spawn", controls, this.spawnTargetFieldsetOptions("units"));
    this.spawnPanels.set("units", section);
    return section;
  }

  renderBuildingSpawnPalette() {
    this.normalizeBuildingSpawnPalette();
    const factionOptions = labBuildingSpawnFactionOptions();
    const factionLabels = Object.fromEntries(factionOptions.map((entry) => [entry.id, entry.label]));
    const buildingKinds = labSpawnBuildingKindsForFaction(this.buildingSpawnPalette.factionId);
    const controls = [
      this.selectField(
        "building-spawn-faction",
        "Faction",
        factionOptions.map((entry) => entry.id),
        factionLabels,
        {
          value: this.buildingSpawnPalette.factionId,
          onChange: (value) => {
            this.buildingSpawnPalette.factionId = value;
            this.buildingSpawnPalette.kind = "";
            this.render();
          },
        },
      ),
      this.buildingSpawnPaletteReadout(buildingKinds),
      this.buildingSpawnPaletteGrid(buildingKinds),
    ];
    const section = this.fieldset("Building Spawn", controls, this.spawnTargetFieldsetOptions("buildings"));
    this.spawnPanels.set("buildings", section);
    return section;
  }

  spawnTargetFieldsetOptions(kind) {
    const target = this.targetPlayerInfo();
    return {
      dataset: {
        spawnPanel: kind,
        targetPlayerId: target.id,
        targetColor: target.color,
      },
      styles: {
        "--lab-spawn-player-color": target.color,
        "--lab-spawn-player-bg": hexToRgba(target.color, 0.16),
        "--lab-spawn-player-bg-strong": hexToRgba(target.color, 0.3),
        "--lab-spawn-player-ring": hexToRgba(target.color, 0.72),
      },
    };
  }

  applySpawnTargetFieldsetOptions(section, kind) {
    const options = this.spawnTargetFieldsetOptions(kind);
    for (const [key, value] of Object.entries(options.dataset)) {
      section.dataset[key] = String(value);
    }
    for (const [key, value] of Object.entries(options.styles)) {
      section.style.setProperty(key, String(value));
    }
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

  buildingSpawnPaletteGrid(buildingKinds) {
    const grid = document.createElement("div");
    grid.className = "lab-spawn-palette";
    for (const kind of buildingKinds) {
      const stats = STATS[kind] || {};
      const button = this.button(stats.label || kind, () => this.armBuildingSpawnPaletteTool(kind), {
        className: "lab-btn lab-spawn-option",
        title: `Spawn ${stats.label || kind}`,
        dataset: {
          kind,
          selected: kind === this.buildingSpawnPalette.kind ? "true" : "false",
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

  buildingSpawnPaletteReadout(buildingKinds) {
    if (buildingKinds.length > 0) {
      return this.readout(`${factionLabel(this.buildingSpawnPalette.factionId, labBuildingSpawnFactionOptions())} buildings`);
    }
    return this.readout("No building catalog entries");
  }

  armSpawnPaletteTool(kind = this.spawnPalette.kind) {
    this.captureSpawnPaletteFields();
    if (!kind) return null;
    this.spawnPalette.kind = kind;
    const payload = {
      owner: this.targetPlayer(),
      factionId: this.spawnPalette.factionId,
      kind,
      completed: true,
    };
    return this.armSpawnTool(payload);
  }

  armBuildingSpawnPaletteTool(kind = this.buildingSpawnPalette.kind) {
    this.captureBuildingSpawnPaletteFields();
    if (!kind) return null;
    this.buildingSpawnPalette.kind = kind;
    const payload = {
      owner: this.targetPlayer(),
      factionId: this.buildingSpawnPalette.factionId,
      kind,
      completed: true,
    };
    return this.armSpawnTool(payload);
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
      completed: true,
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
  }

  normalizeBuildingSpawnPalette() {
    this.targetPlayerId = this.validOwner(this.targetPlayerId);
    const factions = labBuildingSpawnFactionOptions();
    if (!factions.some((entry) => entry.id === this.buildingSpawnPalette.factionId)) {
      this.buildingSpawnPalette.factionId = factions[0]?.id || DEFAULT_FACTION_ID;
    }
    const buildingKinds = labSpawnBuildingKindsForFaction(this.buildingSpawnPalette.factionId);
    if (!buildingKinds.includes(this.buildingSpawnPalette.kind)) {
      this.buildingSpawnPalette.kind = buildingKinds[0] || "";
    }
  }

  validOwner(owner) {
    const numericOwner = Number(owner);
    const owners = this.players().map((player) => Number(player.id)).filter((id) => Number.isFinite(id));
    return owners.includes(numericOwner) ? numericOwner : (owners[0] ?? 1);
  }

  captureSpawnPaletteFields() {
    this.captureTargetPlayerField();
    this.spawnPalette.factionId = this.value("spawn-faction") || this.spawnPalette.factionId;
  }

  captureBuildingSpawnPaletteFields() {
    this.captureTargetPlayerField();
    this.buildingSpawnPalette.factionId = this.value("building-spawn-faction") || this.buildingSpawnPalette.factionId;
  }

  captureTargetPlayerField() {
    this.targetPlayerId = this.validOwner(this.int("lab-player") || this.targetPlayerId);
    return this.targetPlayerId;
  }

  captureVisibleSetupFields() {
    if (this.fields.has("resource-steel")) this.playerState.steel = this.uint("resource-steel");
    if (this.fields.has("resource-oil")) this.playerState.oil = this.uint("resource-oil");
    if (this.fields.has("research-upgrade")) {
      this.playerState.researchUpgrade = this.value("research-upgrade") || this.playerState.researchUpgrade;
    }
    if (this.fields.has("spawn-faction")) this.spawnPalette.factionId = this.value("spawn-faction") || this.spawnPalette.factionId;
    if (this.fields.has("building-spawn-faction")) {
      this.buildingSpawnPalette.factionId = this.value("building-spawn-faction") || this.buildingSpawnPalette.factionId;
    }
  }

  targetPlayer() {
    this.targetPlayerId = this.validOwner(this.targetPlayerId);
    return this.targetPlayerId;
  }

  targetPlayerInfo() {
    const selected = this.targetPlayer();
    const players = this.players();
    const index = players.findIndex((player) => Number(player.id) === selected);
    const player = index >= 0 ? players[index] : null;
    return {
      id: selected,
      color: playerColor(player, Math.max(index, 0)),
    };
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
  }

  capturePlayerStateFields() {
    this.captureTargetPlayerField();
    this.playerState.steel = this.uint("resource-steel");
    this.playerState.oil = this.uint("resource-oil");
    this.playerState.researchUpgrade = this.value("research-upgrade") || this.playerState.researchUpgrade;
  }

  setPlayerResources() {
    this.capturePlayerStateFields();
    return this.labClient.setPlayerResources(
      this.targetPlayer(),
      this.playerState.steel,
      this.playerState.oil,
    );
  }

  async giveAllPlayerResources() {
    const players = this.players()
      .map((player) => Number(player.id))
      .filter((id) => Number.isInteger(id) && id > 0);
    if (players.length === 0) {
      return this.publishLocalResult("setPlayerResources", false, "No players available.");
    }
    const results = [];
    for (const playerId of players) {
      const result = await this.labClient.setPlayerResources(
        playerId,
        GIVE_ALL_RESOURCE_AMOUNT,
        GIVE_ALL_RESOURCE_AMOUNT,
      );
      results.push({ playerId, result });
    }
    return this.publishPlayerResourceBatchResult(results);
  }

  setCompletedResearch() {
    this.capturePlayerStateFields();
    return this.labClient.setCompletedResearch(
      this.targetPlayer(),
      this.playerState.researchUpgrade,
      true,
    );
  }

  setPlayerGodMode(enabled) {
    this.captureTargetPlayerField();
    return this.labClient.setPlayerGodMode(this.targetPlayer(), enabled);
  }

  playerGodModeEnabled() {
    const target = this.targetPlayer();
    return (this.state?.godModePlayers || []).map(Number).includes(target);
  }

  setIgnoreCommandLimits(enabled) {
    const policy = this.labControlPolicy();
    policy?.setIgnoreCommandLimits?.(enabled);
    const summary = enabled ? "Unlimited commands enabled." : "Command limit restored.";
    return this.publishLocalResult("ignoreCommandLimits", true, summary);
  }

  ignoreCommandLimitsEnabled() {
    return this.labControlPolicy()?.ignoreCommandLimitsEnabled?.() ?? true;
  }

  labControlPolicy() {
    return this.match?.state?.controlPolicy || null;
  }

  armRemoveTool() {
    if (typeof this.match?.armLabTool !== "function") return null;
    const armed = this.match.armLabTool(
      {
        kind: "removeSelectableUnits",
        label: "Remove entities",
        keepArmedOnWorldClick: true,
        consumeBoxSelection: true,
        keepArmedOnBoxSelection: true,
      },
      {
        onWorldClick: (event) => this.deleteRemoveToolTargets(event),
        onBoxSelection: (event) => this.deleteRemoveToolTargets(event),
      },
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

  deleteRemoveToolTargets(event) {
    return this.deleteEntities(
      selectedEntityIdsFromPayload(event?.entityIds),
      "No selectable entities in the remove tool target.",
    );
  }

  deleteEntities(entityIds, emptyMessage = "Select an entity first.") {
    return this.batchEntityMutation("deleteEntity", entityIds, (entityId) => (
      this.labClient.deleteEntity(entityId)
    ), { emptyMessage });
  }

  async batchEntityMutation(op, entityIds, request, options = {}) {
    const ids = selectedEntityIdsFromPayload(entityIds);
    if (ids.length === 0) {
      return this.publishLocalResult(op, false, options.emptyMessage || "Select an entity first.");
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

  publishPlayerResourceBatchResult(results) {
    const failures = results
      .filter(({ result }) => !result?.ok)
      .map(({ playerId, result }) => ({
        playerId,
        error: result?.error || "setPlayerResources rejected",
      }));
    const accepted = results.length - failures.length;
    const summary = playerResourceBatchSummary(accepted, failures);
    return this.publishLocalResult("setPlayerResources", failures.length === 0, summary, {
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

  resetScenario() {
    const sent = this.labClient.resetScenario();
    const summary = sent ? "Scenario reset requested." : "Scenario reset could not be sent.";
    return this.publishLocalResult("resetScenario", sent, summary);
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
    this.optionsWindowChrome.destroy();
    this.toolsWindowChrome.destroy();
    this.optionsEl.remove();
    this.toolsEl.remove();
  }
}

const KIND_LABELS = Object.fromEntries(
  Object.entries(STATS).map(([kind, st]) => [kind, st.label || kind]),
);

export function labSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnUnitKindsForFaction(entry.id).length > 0);
}

export function labSpawnUnitKindsForFaction(factionId) {
  return factionCatalog(factionId).units.filter((kind) => STATS[kind]);
}

export function labBuildingSpawnFactionOptions() {
  return PLAYABLE_FACTIONS.filter((entry) => labSpawnBuildingKindsForFaction(entry.id).length > 0);
}

export function labSpawnBuildingKindsForFaction(factionId) {
  return factionCatalog(factionId).buildings.filter((kind) => STATS[kind]);
}

function factionLabel(factionId, options = labSpawnFactionOptions()) {
  return options.find((entry) => entry.id === factionId)?.label || String(factionId || "");
}

function playerButtonLabel(player) {
  const id = Number(player?.id);
  return Number.isFinite(id) ? `P${id}` : "P?";
}

function playerButtonTitle(player) {
  const id = Number(player?.id);
  const name = String(player?.name || "").trim();
  return name ? `Player ${id}: ${name}` : `Player ${id}`;
}

function playerColor(player, index) {
  const color = String(player?.color || "").trim();
  if (/^#[0-9a-f]{6}$/i.test(color)) return color;
  return PLAYER_PALETTE[index % PLAYER_PALETTE.length] || "#9aa0a8";
}

function hexToRgba(hex, alpha) {
  const match = /^#([0-9a-f]{6})$/i.exec(String(hex || ""));
  if (!match) return `rgba(154, 160, 168, ${alpha})`;
  const value = Number.parseInt(match[1], 16);
  const r = (value >> 16) & 0xff;
  const g = (value >> 8) & 0xff;
  const b = value & 0xff;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
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

function playerResourceBatchSummary(accepted, failures) {
  const rejected = failures.length;
  const acceptedText = accepted > 0
    ? `Gave ${accepted} ${playerNoun(accepted)} ${GIVE_ALL_RESOURCE_AMOUNT} steel and ${GIVE_ALL_RESOURCE_AMOUNT} oil`
    : "";
  const rejectedText = rejected > 0
    ? `${rejected} rejected${playerFailureDetails(failures)}`
    : "";
  if (acceptedText && rejectedText) return `${acceptedText}; ${rejectedText}`;
  if (acceptedText) return `${acceptedText}.`;
  return `Give All rejected for ${rejected} ${playerNoun(rejected)}${playerFailureDetails(failures)}`;
}

function labToolLabel(tool) {
  if (typeof tool?.label === "string" && tool.label) return tool.label;
  if (tool?.kind === "spawnEntity") {
    const kind = tool?.payload?.kind || "";
    return kind ? `Spawn ${KIND_LABELS[kind] || kind}` : "Spawn";
  }
  if (tool?.kind === "moveSelected") return "Move selected";
  if (tool?.kind === "removeSelectableUnits") return "Remove entities";
  return "Setup tool";
}

function labToolDetailText(tool) {
  const clickRepeatedly = !!tool?.keepArmedOnWorldClick;
  const boxApplies = !!tool?.consumeBoxSelection;
  const boxRepeatedly = !!tool?.keepArmedOnBoxSelection;
  if (boxApplies) {
    const cadence = clickRepeatedly || boxRepeatedly ? " repeatedly" : "";
    return `Click or drag-select to apply${cadence}. Right-click or Esc cancels.`;
  }
  return clickRepeatedly
    ? "Click the map to apply repeatedly. Drag-select, right-click, or Esc cancels."
    : "Click the map to apply. Drag-select, right-click, or Esc cancels.";
}

function shouldSurfaceToolCancellation(reason) {
  return reason === "escape" || reason === "rightClick" || reason === "panelCancel";
}

function entityNoun(count) {
  return count === 1 ? "entity" : "entities";
}

function playerNoun(count) {
  return count === 1 ? "player" : "players";
}

function failureDetails(failures) {
  if (!failures.length) return "";
  const shown = failures.slice(0, 3).map((failure) => `#${failure.entityId}: ${failure.error}`);
  const suffix = failures.length > shown.length ? `; +${failures.length - shown.length} more` : "";
  return `: ${shown.join("; ")}${suffix}.`;
}

function playerFailureDetails(failures) {
  if (!failures.length) return "";
  const shown = failures.slice(0, 3).map((failure) => `P${failure.playerId}: ${failure.error}`);
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
