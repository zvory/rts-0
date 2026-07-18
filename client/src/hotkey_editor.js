import { HUD } from "./hud.js";
import {
  HOTKEY_PRESET_CLASSIC,
  HOTKEY_PRESET_GRID,
  HOTKEY_PROFILE_SCHEMA_VERSION,
  hotkeyLabelForCode,
  normalizeHotkeyCode,
  profileBindingForCommand,
  setProfileBindingForCommand,
} from "./hotkey_profiles.js";

const CONTEXT_LABELS = Object.freeze({
  "hud-shortcuts": "HUD Shortcuts",
  empty: "Empty Selection",
  "worker-main": "Worker Commands",
  "worker-build": "Worker Build Menu",
  "mixed-army-support": "Army Abilities",
  artillery: "Artillery Commands",
  "city-centre-train": "City Centre",
  "factory-train": "Vehicle Works",
  "gun-works-train": "Gun Works",
  "research-complex": "R&D Complex",
});

export function renderHotkeyEditor(root, hotkeyProfiles, context = {}) {
  const editor = new HotkeyEditor(root, hotkeyProfiles, context);
  editor.render();
  return () => editor.destroy();
}

export class HotkeyEditor {
  constructor(root, hotkeyProfiles, context = {}) {
    this.root = root;
    this.hotkeyProfiles = hotkeyProfiles;
    this.context = context;
    this.selectedProfileId = hotkeyProfiles?.getActiveProfile()?.id || HOTKEY_PRESET_GRID;
    this.selectedContextId = hotkeyProfiles?.catalog?.contexts?.find((entry) => hasCommands(entry))?.id || "empty";
    this.pendingCommandId = "";
    this.invalidCapture = "";
    this.importStatus = null;
    this.draft = this._draftFromProfile(this.hotkeyProfiles?.profileById(this.selectedProfileId));
    this.onKeyDown = (ev) => this._handleKeyDown(ev);
    globalThis.window?.addEventListener?.("keydown", this.onKeyDown, true);
  }

  render() {
    if (!this.root) return;
    this.root.classList.add("settings-hotkeys-panel");
    replaceChildren(this.root);
    if (!this.hotkeyProfiles) {
      this.root.appendChild(mutedText("Grid"));
      return;
    }

    const profile = this.hotkeyProfiles.profileById(this.selectedProfileId) || this.draft || this.hotkeyProfiles.getActiveProfile();
    const editingCustom = this.draft?.type === "custom";
    const validation = this._validation();

    this.root.appendChild(this._profileRow());
    this.root.appendChild(this._metaRow(profile));
    this.root.appendChild(this._editFields(editingCustom));
    this.root.appendChild(this._contextRow());
    this.root.appendChild(this._previewCard());
    this.root.appendChild(this._diagnostics(validation));
    this.root.appendChild(this._actions(validation, editingCustom));
  }

  destroy() {
    globalThis.window?.removeEventListener?.("keydown", this.onKeyDown, true);
  }

  _profileRow() {
    const row = document.createElement("label");
    row.className = "settings-select-row hotkey-profile-row";

    const label = document.createElement("span");
    label.textContent = "Profile";

    const select = document.createElement("select");
    select.id = "hotkey-profile-select";
    for (const profile of this.hotkeyProfiles.allProfiles()) {
      const option = document.createElement("option");
      option.value = profile.id;
      option.textContent = profile.name;
      select.appendChild(option);
    }
    if (this.draft?.type === "custom" && !this.hotkeyProfiles.hasProfile(this.draft.id)) {
      const option = document.createElement("option");
      option.value = this.draft.id;
      option.textContent = `${this.draft.name || "Custom Hotkeys"} *`;
      select.appendChild(option);
    }
    select.value = this.selectedProfileId;
    select.addEventListener("change", () => {
      this.selectedProfileId = select.value;
      this.hotkeyProfiles.setActiveProfile(select.value);
      this.draft = this._draftFromProfile(this.hotkeyProfiles.profileById(select.value));
      this.pendingCommandId = "";
      this.invalidCapture = "";
      this.importStatus = null;
      this.render();
    });

    row.append(label, select);
    return row;
  }

  _metaRow(profile) {
    const row = document.createElement("div");
    row.className = "settings-context-row hotkey-profile-meta";
    const scope = document.createElement("span");
    scope.textContent = this.context?.replay ? "Replay" : this.context?.spectator ? "Spectator" : "Player";
    const mode = document.createElement("span");
    mode.className = "settings-badge";
    mode.textContent = profile?.mode === "direct" ? "Direct" : "Grid";
    row.append(scope, mode);
    return row;
  }

  _editFields(editingCustom) {
    const wrap = document.createElement("div");
    wrap.className = "hotkey-edit-fields";

    const name = document.createElement("label");
    name.className = "hotkey-text-row";
    const nameText = document.createElement("span");
    nameText.textContent = "Name";
    const nameInput = document.createElement("input");
    nameInput.id = "hotkey-profile-name";
    nameInput.type = "text";
    nameInput.value = this.draft?.name || "";
    nameInput.disabled = !editingCustom;
    nameInput.addEventListener("input", () => { this.draft.name = nameInput.value; });
    name.append(nameText, nameInput);

    const description = document.createElement("label");
    description.className = "hotkey-text-row";
    const descriptionText = document.createElement("span");
    descriptionText.textContent = "Description";
    const descriptionInput = document.createElement("input");
    descriptionInput.id = "hotkey-profile-description";
    descriptionInput.type = "text";
    descriptionInput.value = this.draft?.description || "";
    descriptionInput.disabled = !editingCustom;
    descriptionInput.addEventListener("input", () => { this.draft.description = descriptionInput.value; });
    description.append(descriptionText, descriptionInput);

    wrap.append(name, description);
    return wrap;
  }

  _contextRow() {
    const row = document.createElement("label");
    row.className = "settings-select-row hotkey-context-row";
    const label = document.createElement("span");
    label.textContent = "Card";
    const select = document.createElement("select");
    select.id = "hotkey-context-select";
    for (const ctx of this.hotkeyProfiles.catalog.contexts || []) {
      if (!hasCommands(ctx)) continue;
      const option = document.createElement("option");
      option.value = ctx.id;
      option.textContent = contextLabel(ctx);
      select.appendChild(option);
    }
    select.value = this.selectedContextId;
    select.addEventListener("change", () => {
      this.selectedContextId = select.value;
      this.pendingCommandId = "";
      this.invalidCapture = "";
      this.importStatus = null;
      this.render();
    });
    row.append(label, select);
    return row;
  }

  _previewCard() {
    const card = document.createElement("div");
    card.className = "hotkey-command-card";
    card.id = "hotkey-command-card-preview";
    const ctx = this._selectedContext();
    const resolved = this._resolvedCard(ctx?.card);
    const slots = Array.isArray(resolved?.slots) ? resolved.slots : [];
    for (let i = 0; i < 9; i++) {
      const slot = slots[i] || null;
      if (!slot) {
        const empty = document.createElement("div");
        empty.className = "cmd-empty";
        card.appendChild(empty);
        continue;
      }
      const button = HUD.prototype._cmdButton({
        commandId: slot.commandId,
        slotIndex: slot.slotIndex,
        icon: slot.icon,
        label: slot.label,
        hotkey: slot.hotkey === "?" ? "" : slot.hotkey,
        hotkeyCode: slot.hotkeyCode || "",
        cost: slot.cost,
        enabled: true,
        unaffordable: false,
        title: this._buttonTitle(slot),
        cls: slot.commandId === this.pendingCommandId ? `${slot.cls || ""} rebinding`.trim() : slot.cls,
        countBadge: slot.hotkey === "?" ? "?" : slot.countBadge,
        cooldownClocks: slot.cooldownClocks,
        repeatable: false,
        onClick: () => this._beginRebind(slot.commandId),
      });
      if (slot.hotkey === "?") button.dataset.unresolved = "true";
      card.appendChild(button);
    }
    return card;
  }

  _diagnostics(validation) {
    const wrap = document.createElement("div");
    wrap.className = "hotkey-diagnostics";
    wrap.setAttribute("role", "status");
    wrap.setAttribute("aria-live", "polite");
    if (this.pendingCommandId) {
      wrap.appendChild(message("info", `Press a physical A-Z key position for ${this._commandLabel(this.pendingCommandId)}.`));
    }
    if (this.invalidCapture) {
      wrap.appendChild(message("error", this.invalidCapture));
    }
    if (this.importStatus) {
      wrap.appendChild(message(this.importStatus.kind, this.importStatus.text));
    }
    for (const entry of [...(this.hotkeyProfiles.diagnostics?.errors || []), ...(this.hotkeyProfiles.diagnostics?.warnings || [])].slice(0, 4)) {
      wrap.appendChild(message(entry.code === "storageParseFailed" ? "error" : "warn", this._diagnosticText(entry)));
    }
    for (const entry of [...validation.errors, ...validation.warnings].slice(0, 8)) {
      wrap.appendChild(message(entry.code === "unknownCommand" || entry.code === "missingCommandUnresolved" ? "warn" : "error",
        this._diagnosticText(entry)));
    }
    if (!wrap.children.length) {
      wrap.appendChild(message("ok", "No conflicts in this profile."));
    }
    return wrap;
  }

  _actions(validation, editingCustom) {
    const row = document.createElement("div");
    row.className = "hotkey-actions";

    const clone = document.createElement("button");
    clone.id = "hotkey-clone-profile";
    clone.type = "button";
    clone.textContent = "Clone";
    clone.addEventListener("click", () => {
      const source = this.hotkeyProfiles.profileById(this.selectedProfileId) || this.hotkeyProfiles.profileById(HOTKEY_PRESET_CLASSIC);
      this.draft = this._customDraftFrom(source, `${source?.name || "Hotkeys"} Custom`);
      this.selectedProfileId = this.draft.id;
      this.pendingCommandId = "";
      this.invalidCapture = "";
      this.importStatus = null;
      this.render();
    });

    const blank = document.createElement("button");
    blank.id = "hotkey-new-blank-profile";
    blank.type = "button";
    blank.textContent = "Blank";
    blank.addEventListener("click", () => {
      this.draft = this._customDraftFrom(null, "Custom Hotkeys", {});
      this.selectedProfileId = this.draft.id;
      this.pendingCommandId = "";
      this.invalidCapture = "";
      this.importStatus = null;
      this.render();
    });

    const save = document.createElement("button");
    save.id = "hotkey-save-profile";
    save.type = "button";
    save.textContent = "Save";
    save.disabled = !editingCustom || !validation.ok || !!this.pendingCommandId || !!this.invalidCapture;
    save.addEventListener("click", () => {
      const result = this.hotkeyProfiles.saveCustomProfile(this.draft);
      if (!result.ok) {
        this.invalidCapture = this._diagnosticText(result.errors[0] || { code: "invalidProfile" });
        this.render();
        return;
      }
      this.hotkeyProfiles.setActiveProfile(result.profile.id);
      this.selectedProfileId = result.profile.id;
      this.draft = this._draftFromProfile(result.profile);
      this.pendingCommandId = "";
      this.invalidCapture = "";
      this.importStatus = null;
      this.render();
    });

    const exportButton = document.createElement("button");
    exportButton.id = "hotkey-export-profile";
    exportButton.type = "button";
    exportButton.textContent = "Export";
    exportButton.disabled = !this.hotkeyProfiles.exportProfile(this.selectedProfileId);
    exportButton.addEventListener("click", () => this._exportSelectedProfile());

    const importInput = document.createElement("input");
    importInput.id = "hotkey-import-file";
    importInput.type = "file";
    importInput.accept = "application/json,.json";
    importInput.hidden = true;
    importInput.addEventListener("change", async () => {
      const file = importInput.files?.[0] || null;
      importInput.value = "";
      if (!file) return;
      await this._importFile(file);
    });

    const importButton = document.createElement("button");
    importButton.id = "hotkey-import-profile";
    importButton.type = "button";
    importButton.textContent = "Import";
    importButton.addEventListener("click", () => importInput.click());

    row.append(clone, blank, save, exportButton, importButton, importInput);
    return row;
  }

  _beginRebind(commandId) {
    if (this.draft?.type !== "custom") {
      const source = this.hotkeyProfiles.profileById(this.selectedProfileId);
      this.draft = this._customDraftFrom(source, `${source?.name || "Hotkeys"} Custom`);
      this.selectedProfileId = this.draft.id;
    }
    this.pendingCommandId = commandId;
    this.invalidCapture = "";
    this.importStatus = null;
    this.render();
  }

  _handleKeyDown(ev) {
    if (!this.pendingCommandId) return;
    if (ev.repeat) {
      ev.preventDefault?.();
      ev.stopPropagation?.();
      return;
    }
    ev.preventDefault?.();
    ev.stopPropagation?.();
    if (ev.code === "Escape") {
      this.pendingCommandId = "";
      this.invalidCapture = "";
      this.render();
      return;
    }
    const code = normalizeHotkeyCode(ev.code);
    if (!code) {
      this.invalidCapture = "Use a physical A-Z key position.";
      this.render();
      return;
    }
    setProfileBindingForCommand(this.draft, this.pendingCommandId, code);
    this.pendingCommandId = "";
    this.invalidCapture = "";
    this.render();
  }

  _exportSelectedProfile() {
    const json = this.hotkeyProfiles.exportProfileJson(this.selectedProfileId);
    if (!json) return;
    const filename = `${safeFilename(this.hotkeyProfiles.profileById(this.selectedProfileId)?.name || "hotkeys")}.json`;
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    link.rel = "noopener";
    document.body?.appendChild?.(link);
    link.click();
    link.remove?.();
    URL.revokeObjectURL(url);
    this.importStatus = { kind: "ok", text: "Profile exported." };
    this.render();
  }

  async _importFile(file) {
    let text = "";
    try {
      text = await file.text();
    } catch {
      this.importStatus = { kind: "error", text: "Import file could not be read." };
      this.render();
      return;
    }
    const result = this.hotkeyProfiles.parseImportText(text, { activate: true });
    if (!result.ok) {
      this.importStatus = {
        kind: "error",
        text: this._diagnosticText(result.errors[0] || { code: "importParseFailed" }),
      };
      this.render();
      return;
    }
    this.selectedProfileId = result.profile.id;
    this.draft = this._draftFromProfile(result.profile);
    this.pendingCommandId = "";
    this.invalidCapture = "";
    const warning = result.warnings[0] ? ` ${this._diagnosticText(result.warnings[0])}` : "";
    this.importStatus = { kind: result.warnings.length ? "warn" : "ok", text: `Imported ${result.profile.name}.${warning}` };
    this.render();
  }

  _validation() {
    if (!this.hotkeyProfiles || !this.draft) return { ok: false, errors: [], warnings: [] };
    return this.hotkeyProfiles.validateDraftProfile(this.draft);
  }

  _selectedContext() {
    return (this.hotkeyProfiles.catalog.contexts || []).find((entry) => entry.id === this.selectedContextId) ||
      (this.hotkeyProfiles.catalog.contexts || []).find((entry) => hasCommands(entry)) ||
      null;
  }

  _resolvedCard(card) {
    if (!card) return card;
    if (this.draft?.mode !== "direct") return this.hotkeyProfiles.resolveCard(card, this.draft);
    return {
      ...card,
      slots: card.slots.map((slot) => {
        if (!slot) return null;
        const hotkeyCode = normalizeHotkeyCode(profileBindingForCommand(this.draft, slot.commandId));
        return {
          ...slot,
          hotkeyCode,
          hotkey: hotkeyLabelForCode(hotkeyCode) || "?",
        };
      }),
    };
  }

  _buttonTitle(slot) {
    const base = slot.title || slot.label || slot.commandId;
    if (this.pendingCommandId === slot.commandId) return `${base} - press a physical A-Z key position`;
    if (slot.hotkey === "?") return `${base} - unbound`;
    return `${base} (${slot.hotkey})`;
  }

  _diagnosticText(entry) {
    switch (entry?.code) {
      case "duplicateKey":
        return `${hotkeyLabelForCode(entry.key) || entry.key} is used by ${entry.commandIds.map((id) => this._commandLabel(id)).join(" and ")} in ${this._contextLabel(entry.contextId)}.`;
      case "invalidKey":
        return `${this._commandLabel(entry.commandId)} has unsupported key ${String(entry.key || "").toUpperCase() || "(blank)"}.`;
      case "unknownCommand":
        return `Unknown command ${entry.commandId} was ignored.`;
      case "missingCommandUnresolved":
      case "unresolvedCommand":
        return `${this._commandLabel(entry.commandId)} is unbound.`;
      case "presetImmutable":
        return "Preset profiles cannot be overwritten.";
      case "missingId":
        return "Profile id is missing.";
      case "invalidMode":
        return "Profile mode is invalid.";
      case "storageParseFailed":
        return "Stored hotkey profiles could not be read.";
      case "importParseFailed":
        return "Import file is not valid JSON.";
      default:
        return "Profile has unresolved hotkey issues.";
    }
  }

  _commandLabel(commandId) {
    const command = (this.hotkeyProfiles.catalog.commands || []).find((entry) => entry.commandId === commandId);
    return command?.label || commandId;
  }

  _contextLabel(contextId) {
    const ctx = (this.hotkeyProfiles.catalog.contexts || []).find((entry) => entry.id === contextId);
    return contextLabel(ctx || { id: contextId });
  }

  _draftFromProfile(profile) {
    if (!profile) return this._customDraftFrom(null, "Custom Hotkeys", {});
    return {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id: profile.id,
      type: profile.type,
      mode: profile.mode,
      name: profile.name,
      description: profile.description || "",
      basePresetId: profile.basePresetId || (profile.type === "preset" ? profile.id : null),
      bindings: { ...profile.bindings },
      factionBindings: cloneFactionBindings(profile.factionBindings),
    };
  }

  _customDraftFrom(source, name, bindings = null) {
    const now = Date.now().toString(36);
    const sourceBindingMaps = bindings
      ? { bindings, factionBindings: {} }
      : source?.mode === "direct"
        ? {
            bindings: { ...source.bindings },
            factionBindings: cloneFactionBindings(source.factionBindings),
          }
        : source?.mode === "grid"
          ? this._gridBindingMaps()
          : { bindings: {}, factionBindings: {} };
    return {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id: `custom.${now}`,
      type: "custom",
      mode: "direct",
      name,
      description: source?.description || "",
      basePresetId: source?.id || null,
      bindings: { ...sourceBindingMaps.bindings },
      factionBindings: cloneFactionBindings(sourceBindingMaps.factionBindings),
    };
  }

  _gridBindingMaps() {
    const profile = { bindings: {}, factionBindings: {} };
    for (const command of this.hotkeyProfiles.catalog.commands || []) {
      if (command.gridHotkey) {
        setProfileBindingForCommand(profile, command.commandId, command.gridHotkey);
        continue;
      }
      if (Number.isInteger(command.slotIndex)) {
        const slotKey = ["Q", "W", "E", "A", "S", "D", "Z", "X", "C"][command.slotIndex] || "";
        if (slotKey) setProfileBindingForCommand(profile, command.commandId, `Key${slotKey}`);
      }
    }
    return profile;
  }
}

function hasCommands(ctx) {
  return (ctx?.commandIds || []).length > 0;
}

function contextLabel(ctx) {
  return CONTEXT_LABELS[ctx?.id] || ctx?.label || ctx?.id || "Command Card";
}

function replaceChildren(root, ...children) {
  if (typeof root.replaceChildren === "function") root.replaceChildren(...children);
  else {
    root.children.length = 0;
    root.append?.(...children);
  }
}

function mutedText(text) {
  const el = document.createElement("div");
  el.className = "settings-muted";
  el.textContent = text;
  return el;
}

function message(kind, text) {
  const row = document.createElement("div");
  row.className = `hotkey-message ${kind}`;
  row.textContent = text;
  return row;
}

function safeFilename(value) {
  return String(value || "hotkeys")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "") || "hotkeys";
}

function cloneFactionBindings(factionBindings = {}) {
  const clone = {};
  for (const [factionId, bindings] of Object.entries(factionBindings || {})) {
    if (!bindings || typeof bindings !== "object") continue;
    clone[factionId] = { ...bindings };
  }
  return clone;
}
