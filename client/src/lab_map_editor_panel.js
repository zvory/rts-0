import { TERRAIN } from "./protocol.js";
import { LabPanelWindowChrome } from "./lab_panel_window.js";
import {
  paintDraftRect,
  placeDraftSite,
  protectDraftBaseTerrain,
  removeDraftSite,
} from "./lab_map_editor_session.js";

const MAP_PANEL_STORAGE_KEY = "rts.labPanel.mapEditor.window.v1";

export class LabMapEditorPanel {
  constructor({ root, session, labClient, match, startPayload, mapName = "Lab map", applyLabMapReset = null }) {
    this.root = root;
    this.session = session;
    this.labClient = labClient;
    this.match = match;
    this.startPayload = startPayload;
    this.mapName = mapName;
    this.applyLabMapReset = applyLabMapReset;
    this.destroyed = false;
    this.selectedTerrain = TERRAIN.ROCK;
    this.selectedSiteKind = "main";
    this.selectedSiteId = "";
    this.lastStatus = session.lastAction
      ? `${session.lastAction} applied to the live map.`
      : "Edit live terrain or bases; changing the base layout resets the battle.";
    this.lastStatusError = false;
    this.applyPending = false;
    this.applyQueued = false;

    this.el = document.createElement("aside");
    this.el.className = "lab-panel lab-map-window";
    this.el.setAttribute("aria-label", "Lab map editor");
    this.root.appendChild(this.el);
    this.chrome = new LabPanelWindowChrome(this.el, {
      storageKey: MAP_PANEL_STORAGE_KEY,
      windowObj: globalThis.window,
    });

    this.onKeyDown = (event) => this.handleKeyDown(event);
    globalThis.window?.addEventListener?.("keydown", this.onKeyDown);
    this.unsubscribe = this.session.subscribe(() => this.render());
    this.session.initializeFromStart(startPayload, { name: mapName });
    this.hydrateInitialMap();
    this.restoreDesiredTool();
  }

  async hydrateInitialMap() {
    if (this.session.undoStack.length > 0 || this.session.lastAction) return;
    const result = await this.labClient.exportScenario("Lab map editor bootstrap");
    if (this.destroyed || !result?.ok || !result?.outcome?.scenario) return;
    if (this.session.undoStack.length > 0 || this.session.lastAction) return;
    if (this.session.initializeFromScenario(result.outcome.scenario, { force: true })) {
      this.lastStatus = "Loaded terrain and expansion sites from the live map.";
      this.render();
    }
  }

  render() {
    if (this.destroyed || !this.el) return;
    this.el.replaceChildren();
    this.el.appendChild(this.chrome.renderHeader({ kicker: "Map editor", collapseLabel: "map editor" }));
    const body = document.createElement("div");
    body.className = "lab-panel-body lab-map-editor";
    if (!this.session.draft) {
      body.appendChild(readout("Loading current map…"));
    } else {
      body.append(
        this.renderHistory(),
        this.renderMetadata(),
        this.renderTerrainTools(),
        this.renderBaseTools(),
        this.renderSlots(),
        this.renderSaveActions(),
        this.renderStatus(),
      );
    }
    this.el.append(body, this.chrome.renderResizeHandle());
  }

  renderHistory() {
    const row = document.createElement("section");
    row.className = "lab-map-history";
    row.append(
      button("Undo", () => this.undo(), { disabled: !this.session.undoStack.length, title: "Ctrl/Cmd-Z" }),
      button("Redo", () => this.redo(), { disabled: !this.session.redoStack.length, title: "Ctrl/Cmd-Shift-Z or Ctrl-Y" }),
      readout(`${this.session.undoStack.length}/25 states`),
    );
    return row;
  }

  renderMetadata() {
    const draft = this.session.draft;
    const fieldset = group("Map details");
    fieldset.append(
      textField("Name", draft.name, (value) => {
        this.session.mutate("Renamed map", (next) => { next.name = value; });
      }),
      textField("Description", draft.description, (value) => {
        this.session.mutate("Changed map description", (next) => { next.description = value; });
      }),
    );
    return fieldset;
  }

  renderTerrainTools() {
    const fieldset = group("Terrain paint");
    const palette = document.createElement("div");
    palette.className = "lab-map-palette";
    for (const [code, label] of [
      [TERRAIN.GRASS, "Grass"],
      [TERRAIN.ROCK, "Stone"],
      [TERRAIN.WATER, "Water"],
    ]) {
      palette.appendChild(terrainPaletteButton(code, label, () => {
        this.selectedTerrain = code;
        this.armTerrainTool();
      }, { active: this.selectedTerrain === code && this.desiredToolKind() === "terrain" }));
    }
    fieldset.append(
      palette,
      readout("Click or drag to paint one tile at a time. Protected base circles remain grass."),
    );
    return fieldset;
  }

  renderBaseTools() {
    const fieldset = group("Base sites");
    const palette = document.createElement("div");
    palette.className = "lab-map-palette";
    for (const kind of ["main", "natural"]) {
      palette.appendChild(button(kind === "main" ? "Main" : "Natural", () => {
        this.selectedSiteKind = kind;
        this.armBaseTool();
      }, { active: this.selectedSiteKind === kind && this.desiredToolKind() === "base" }));
    }
    const sites = document.createElement("div");
    sites.className = "lab-map-site-list";
    for (const site of this.session.draft.sites) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "lab-map-site";
      row.dataset.active = site.id === this.selectedSiteId ? "true" : "false";
      row.textContent = `${site.id} · ${site.kind} · ${site.x},${site.y}`;
      row.addEventListener("click", () => {
        this.selectedSiteId = site.id;
        this.selectedSiteKind = site.kind;
        this.render();
      });
      sites.appendChild(row);
    }
    fieldset.append(
      palette,
      button("Arm base tool", () => this.armBaseTool(), { active: this.desiredToolKind() === "base" }),
      button("Remove selected", () => this.removeSelectedSite(), { disabled: !this.selectedSiteId }),
      sites,
    );
    return fieldset;
  }

  renderSlots() {
    const fieldset = group("Player slots");
    const draft = this.session.draft;
    const mainSites = draft.sites.filter((site) => site.kind === "main");
    const naturalSites = draft.sites.filter((site) => site.kind === "natural");
    draft.layouts[0].slots.forEach((slot, index) => {
      const row = document.createElement("div");
      row.className = "lab-map-slot";
      const title = document.createElement("strong");
      title.textContent = `Player ${index + 1}`;
      const main = selectField("Main", mainSites, slot.main, false, (value) => {
        this.commitAndApply("Changed a player main", (next) => {
          const slots = next.layouts[0].slots;
          const previous = slots[index].main;
          const other = slots.findIndex((candidate, candidateIndex) => (
            candidateIndex !== index && candidate.main === value
          ));
          slots[index].main = value;
          if (other >= 0) slots[other].main = previous;
        });
      });
      const naturals = selectField("Naturals", naturalSites, slot.naturals, true, (values) => {
        this.commitAndApply("Changed player naturals", (next) => {
          const selected = values.slice(0, 3);
          next.layouts[0].slots[index].naturals = selected;
          next.layouts[0].slots.forEach((candidate, candidateIndex) => {
            if (candidateIndex !== index) {
              candidate.naturals = candidate.naturals.filter((id) => !selected.includes(id));
            }
          });
        });
      });
      row.append(title, main, naturals);
      fieldset.appendChild(row);
    });
    return fieldset;
  }

  renderSaveActions() {
    const fieldset = group("Save / export");
    fieldset.append(
      button("Apply to battle", () => this.applyDraft(), { disabled: this.applyPending }),
      button("Save local draft", () => this.saveLocal()),
      button("Load local draft", () => this.loadLocal()),
      button("Export map JSON", () => this.exportJson()),
    );
    return fieldset;
  }

  renderStatus() {
    const status = document.createElement("p");
    status.className = "lab-result";
    status.dataset.state = this.lastStatusError ? "error" : "ok";
    status.setAttribute("aria-live", "polite");
    status.textContent = this.lastStatus;
    return status;
  }

  armTerrainTool() {
    this.session.setDesiredTool({ kind: "terrain", terrain: this.selectedTerrain });
    return this.match?.armLabTool?.({
      kind: "editMapTerrain",
      label: `Paint ${terrainLabel(this.selectedTerrain)} terrain`,
      payload: { terrain: this.selectedTerrain },
      keepArmedOnWorldClick: true,
      paintOnDrag: true,
    }, {
      onWorldClick: (event) => this.paintWorldClick(event),
    });
  }

  armBaseTool() {
    this.session.setDesiredTool({ kind: "base", siteKind: this.selectedSiteKind });
    return this.match?.armLabTool?.({
      kind: "editMapBase",
      label: `Place ${this.selectedSiteKind} base`,
      payload: { siteKind: this.selectedSiteKind },
      keepArmedOnWorldClick: true,
    }, { onWorldClick: (event) => this.placeBase(event) });
  }

  restoreDesiredTool() {
    const desired = this.session.desiredTool;
    if (!desired) return;
    if (desired.kind === "terrain") {
      this.selectedTerrain = desired.terrain;
      this.armTerrainTool();
    } else if (desired.kind === "base") {
      this.selectedSiteKind = desired.siteKind;
      this.armBaseTool();
    }
  }

  desiredToolKind() {
    return this.session.desiredTool?.kind || "";
  }

  paintWorldClick(event) {
    const tile = this.worldTile(event?.x, event?.y);
    if (!tile) return;
    this.commitAndApply("Painted terrain tile", (draft) => {
      paintDraftRect(draft, {
        x0: tile.x,
        y0: tile.y,
        x1: tile.x,
        y1: tile.y,
      }, event.tool.payload.terrain);
      protectDraftBaseTerrain(draft);
    });
  }

  placeBase(event) {
    const clicked = this.worldTile(event?.x, event?.y);
    const size = this.session.draft?.terrain?.length || 0;
    const radius = event?.tool?.payload?.siteKind === "natural" ? 0 : 3;
    const tile = clicked && size > radius * 2 ? {
      x: Math.max(radius, Math.min(size - radius - 1, clicked.x)),
      y: Math.max(radius, Math.min(size - radius - 1, clicked.y)),
    } : null;
    if (!tile) return;
    let placedId = "";
    this.commitAndApply(`Placed ${this.selectedSiteKind} base`, (draft) => {
      placedId = placeDraftSite(draft, { kind: event.tool.payload.siteKind, ...tile });
      protectDraftBaseTerrain(draft);
    });
    this.selectedSiteId = placedId;
  }

  removeSelectedSite() {
    const id = this.selectedSiteId;
    if (!id) return;
    this.selectedSiteId = "";
    this.commitAndApply("Removed base site", (draft) => removeDraftSite(draft, id));
  }

  worldTile(x, y) {
    const tileSize = Number(this.startPayload?.map?.tileSize);
    const size = this.session.draft?.terrain?.length || 0;
    if (!Number.isFinite(x) || !Number.isFinite(y) || !tileSize || !size) return null;
    return {
      x: Math.max(0, Math.min(size - 1, Math.floor(x / tileSize))),
      y: Math.max(0, Math.min(size - 1, Math.floor(y / tileSize))),
    };
  }

  commitAndApply(label, mutation) {
    if (!this.session.mutate(label, mutation)) {
      this.setStatus("No tiles changed. Main-base footprints and natural centers must remain grass.");
      return Promise.resolve(null);
    }
    return this.applyDraft();
  }

  async applyDraft() {
    if (this.applyPending) {
      this.applyQueued = true;
      return null;
    }
    let materialized;
    try {
      materialized = this.session.materialized();
    } catch (error) {
      this.setStatus(error.message || String(error), true);
      return null;
    }
    this.applyPending = true;
    this.setStatus("Applying map to the battle…");
    const result = await this.labClient.applyMapDraft(materialized);
    this.applyPending = false;
    if (!result?.ok) {
      this.setStatus(result?.error || "Map apply failed.", true);
      return result;
    }
    if (!this.applyLabMapReset?.(result.outcome)) {
      this.setStatus("Map applied, but the local renderer could not refresh in place.", true);
      return result;
    }
    if (this.applyQueued) {
      this.applyQueued = false;
      return this.applyDraft();
    }
    this.setStatus(result.outcome?.battleReset
      ? "Base layout applied. The battle was reset on the edited map."
      : "Terrain applied without resetting the battle.");
    return result;
  }

  undo() {
    if (!this.session.undo()) return;
    void this.applyDraft();
  }

  redo() {
    if (!this.session.redo()) return;
    void this.applyDraft();
  }

  saveLocal() {
    const ok = this.session.saveLocal(this.mapStorageKey());
    this.setStatus(ok ? "Saved this draft in the browser." : "Local save is unavailable.", !ok);
  }

  loadLocal() {
    const ok = this.session.loadLocal(this.mapStorageKey());
    this.setStatus(ok ? "Loaded the browser draft." : "No compatible local draft was found.", !ok);
    if (ok) void this.applyDraft();
  }

  exportJson() {
    try {
      const draft = this.session.exportMap();
      const blob = new Blob([`${JSON.stringify(draft, null, 2)}\n`], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${slug(draft.name)}.json`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
      this.setStatus(`Exported ${anchor.download}.`);
    } catch (error) {
      this.setStatus(error.message || String(error), true);
    }
  }

  mapStorageKey() {
    return this.startPayload?.lab?.room || this.mapName || "default";
  }

  handleKeyDown(event) {
    if (this.destroyed || event.defaultPrevented || isTextEntry(event.target)) return;
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    const key = String(event.key || "").toLowerCase();
    const redo = key === "y" || (key === "z" && event.shiftKey);
    const undo = key === "z" && !event.shiftKey;
    if (!undo && !redo) return;
    event.preventDefault();
    redo ? this.redo() : this.undo();
  }

  setStatus(message, error = false) {
    this.lastStatus = String(message || "");
    this.lastStatusError = !!error;
    this.render();
  }

  applyLabToolChange(change) {
    const kind = change?.tool?.kind || "";
    if (change?.type === "armed" && kind !== "editMapTerrain" && kind !== "editMapBase") {
      this.session.setDesiredTool(null);
    }
    if (change?.type === "cancelled" && !["panelDestroy", "teardown"].includes(change.reason)) {
      this.session.setDesiredTool(null);
    }
    this.render();
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    globalThis.window?.removeEventListener?.("keydown", this.onKeyDown);
    this.unsubscribe?.();
    this.chrome.destroy();
    this.el.remove();
  }
}

function group(title) {
  const fieldset = document.createElement("fieldset");
  fieldset.className = "lab-tool-group lab-map-group";
  const legend = document.createElement("legend");
  legend.textContent = title;
  fieldset.appendChild(legend);
  return fieldset;
}

function button(label, onClick, { disabled = false, active = false, title = "" } = {}) {
  const el = document.createElement("button");
  el.type = "button";
  el.className = "lab-btn";
  el.textContent = label;
  el.disabled = !!disabled;
  el.dataset.active = active ? "true" : "false";
  if (title) el.title = title;
  el.addEventListener("click", onClick);
  return el;
}

function terrainPaletteButton(code, label, onClick, { active = false } = {}) {
  const el = button("", onClick, {
    active,
    title: `Paint ${label.toLowerCase()} terrain`,
  });
  el.className = "lab-btn lab-map-terrain-option";
  el.dataset.terrain = terrainName(code);
  el.setAttribute("aria-label", `Paint ${label.toLowerCase()} terrain`);

  const icon = document.createElement("span");
  icon.className = "lab-terrain-icon";
  icon.dataset.terrain = terrainName(code);
  icon.setAttribute("aria-hidden", "true");
  const text = document.createElement("span");
  text.className = "lab-terrain-label";
  text.textContent = label;
  el.append(icon, text);
  return el;
}

function terrainName(code) {
  if (code === TERRAIN.ROCK) return "stone";
  if (code === TERRAIN.WATER) return "water";
  return "grass";
}

function terrainLabel(code) {
  if (code === TERRAIN.ROCK) return "Stone";
  if (code === TERRAIN.WATER) return "Water";
  return "Grass";
}

function textField(labelText, value, onChange) {
  const label = document.createElement("label");
  label.className = "lab-field";
  label.dataset.wide = "true";
  const text = document.createElement("span");
  text.textContent = labelText;
  const input = document.createElement("input");
  input.value = value;
  input.addEventListener("change", () => onChange(input.value));
  label.append(text, input);
  return label;
}

function selectField(labelText, sites, selected, multiple, onChange) {
  const label = document.createElement("label");
  label.className = "lab-field";
  const text = document.createElement("span");
  text.textContent = labelText;
  const select = document.createElement("select");
  select.multiple = multiple;
  if (multiple) select.size = Math.min(3, Math.max(2, sites.length));
  if (!multiple) {
    const blank = document.createElement("option");
    blank.value = "";
    blank.textContent = "Choose…";
    select.appendChild(blank);
  }
  const selectedValues = new Set(Array.isArray(selected) ? selected : [selected]);
  for (const site of sites) {
    const option = document.createElement("option");
    option.value = site.id;
    option.textContent = `${site.id} (${site.x},${site.y})`;
    option.selected = selectedValues.has(site.id);
    select.appendChild(option);
  }
  select.addEventListener("change", () => {
    const values = [...select.selectedOptions].map((option) => option.value).filter(Boolean);
    onChange(multiple ? values : values[0] || "");
  });
  label.append(text, select);
  return label;
}

function readout(text) {
  const node = document.createElement("p");
  node.className = "lab-readout";
  node.textContent = text;
  return node;
}

function isTextEntry(target) {
  const tag = String(target?.tagName || "").toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || !!target?.isContentEditable;
}

function slug(value) {
  return String(value || "lab-map")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "lab-map";
}
