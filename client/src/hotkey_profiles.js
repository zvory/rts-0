import { GRID_HOTKEYS, gridHotkeyForSlot } from "./hud_command_card.js";

export const HOTKEY_PROFILE_SCHEMA_VERSION = 1;
export const HOTKEY_STORAGE_PROFILES_KEY = "rts.hotkeyProfiles.v1";
export const HOTKEY_STORAGE_ACTIVE_KEY = "rts.activeHotkeyProfile.v1";
export const HOTKEY_PRESET_GRID = "preset.grid";
export const HOTKEY_PRESET_CLASSIC = "preset.classicRts";

const VALID_KEY_RE = /^[A-Z]$/;

const CORE_CLASSIC_BINDINGS = Object.freeze({
  "unit.move": "M",
  "unit.attack": "A",
  "unit.stop": "S",
  "worker.buildMenu": "B",
  "worker.return": "W",
});

export function normalizeHotkey(value) {
  if (typeof value !== "string") return "";
  const key = value.trim().toUpperCase();
  return VALID_KEY_RE.test(key) ? key : "";
}

export function buildHotkeyCommandCatalog(cards = []) {
  const commands = new Map();
  const contexts = [];
  for (const entry of cards || []) {
    const card = entry?.card || entry;
    const contextId = entry?.id || card?.signature || `context-${contexts.length}`;
    const contextLabel = entry?.label || labelFromContextId(contextId);
    const commandIds = [];
    for (const slot of card?.slots || []) {
      if (!slot?.commandId) continue;
      commandIds.push(slot.commandId);
      if (!commands.has(slot.commandId)) {
        commands.set(slot.commandId, {
          commandId: slot.commandId,
          label: slot.label || slot.commandId,
          slotIndex: Number.isInteger(slot.slotIndex) ? slot.slotIndex : null,
        });
      }
    }
    contexts.push({ id: contextId, label: contextLabel, card, commandIds });
  }
  return Object.freeze({
    commands: Object.freeze([...commands.values()].map(Object.freeze)),
    contexts: Object.freeze(contexts.map((ctx) => Object.freeze({
      id: ctx.id,
      label: ctx.label,
      card: ctx.card,
      commandIds: Object.freeze([...ctx.commandIds]),
    }))),
  });
}

export function createGridPreset() {
  return freezeProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: HOTKEY_PRESET_GRID,
    type: "preset",
    mode: "grid",
    name: "Grid",
    description: "Command-card hotkeys follow the rendered QWE/ASD/ZXC grid.",
    bindings: {},
  });
}

export function createClassicPreset(catalog) {
  return freezeProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: HOTKEY_PRESET_CLASSIC,
    type: "preset",
    mode: "direct",
    name: "Classic RTS",
    description: "Stable command hotkeys that do not move when command-card slots move.",
    bindings: buildClassicBindings(catalog),
  });
}

export class HotkeyProfileService {
  constructor({
    storage = globalThis.localStorage,
    catalog = buildHotkeyCommandCatalog([]),
    profilesKey = HOTKEY_STORAGE_PROFILES_KEY,
    activeKey = HOTKEY_STORAGE_ACTIVE_KEY,
  } = {}) {
    this.storage = storage || null;
    this.catalog = catalog;
    this.profilesKey = profilesKey;
    this.activeKey = activeKey;
    this.presets = Object.freeze([createGridPreset(), createClassicPreset(catalog)]);
    this.customProfiles = [];
    this.activeProfileId = HOTKEY_PRESET_GRID;
    this.revision = 0;
    this.diagnostics = { errors: [], warnings: [] };
    this.load();
  }

  load() {
    const loaded = this._readStoredProfiles();
    this.customProfiles = loaded.profiles;
    const active = this._storageGet(this.activeKey);
    this.activeProfileId = this.hasProfile(active) ? active : HOTKEY_PRESET_GRID;
    this.diagnostics = { errors: loaded.errors, warnings: loaded.warnings };
    this.revision += 1;
  }

  allProfiles() {
    return [...this.presets, ...this.customProfiles.map((profile) => ({ ...profile, bindings: { ...profile.bindings } }))];
  }

  getActiveProfile() {
    return this.profileById(this.activeProfileId) || this.profileById(HOTKEY_PRESET_GRID);
  }

  hasProfile(id) {
    return !!this.profileById(id);
  }

  profileById(id) {
    if (!id) return null;
    return this.presets.find((profile) => profile.id === id) ||
      this.customProfiles.find((profile) => profile.id === id) ||
      null;
  }

  setActiveProfile(id) {
    if (!this.hasProfile(id)) return false;
    this.activeProfileId = id;
    this._storageSet(this.activeKey, id);
    this.revision += 1;
    return true;
  }

  createCustomFromPreset(presetId, metadata = {}) {
    const preset = this.profileById(presetId);
    if (!preset || preset.type !== "preset") {
      return { ok: false, errors: [{ code: "unknownPreset", profileId: presetId }], warnings: [] };
    }
    const id = metadata.id || `custom.${Date.now().toString(36)}`;
    const profile = {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id,
      type: "custom",
      mode: preset.mode,
      name: metadata.name || `${preset.name} Custom`,
      description: metadata.description || "",
      basePresetId: preset.id,
      bindings: { ...preset.bindings },
    };
    const result = this.saveCustomProfile(profile);
    return result.ok ? { ...result, profile: this.profileById(id) } : result;
  }

  saveCustomProfile(profile) {
    const parsed = this.parseProfilePayload(profile, { allowUnresolved: false });
    if (!parsed.ok) return parsed;
    if (parsed.profile.type === "preset") {
      return {
        ok: false,
        profile: parsed.profile,
        errors: [{ code: "presetImmutable", profileId: parsed.profile.id }],
        warnings: parsed.warnings,
      };
    }
    this._replaceCustomProfile(parsed.profile);
    this._writeCustomProfiles();
    this.revision += 1;
    return parsed;
  }

  validateDraftProfile(profile) {
    const parsed = this.parseProfilePayload(profile, { allowUnresolved: true, fillMissing: false });
    const errors = [...parsed.errors];
    const warnings = [...parsed.warnings];
    if (parsed.profile.mode === "direct") {
      for (const command of this.catalog.commands || []) {
        if (!parsed.profile.bindings[command.commandId]) {
          errors.push({ code: "unresolvedCommand", commandId: command.commandId });
        }
      }
    }
    return { ...parsed, ok: errors.length === 0, errors, warnings };
  }

  importProfile(payload, { targetId = null, activate = false } = {}) {
    const parsed = this.parseProfilePayload(payload, { allowUnresolved: false });
    if (!parsed.ok) return parsed;
    const profile = {
      ...parsed.profile,
      id: targetId || parsed.profile.id,
      type: "custom",
      bindings: { ...parsed.profile.bindings },
    };
    this._replaceCustomProfile(profile);
    this._writeCustomProfiles();
    if (activate) this.setActiveProfile(profile.id);
    else this.revision += 1;
    return { ...parsed, profile };
  }

  exportProfile(id = this.activeProfileId) {
    const profile = this.profileById(id);
    if (!profile) return null;
    return {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id: profile.id,
      type: profile.type,
      mode: profile.mode,
      name: profile.name,
      description: profile.description || "",
      basePresetId: profile.basePresetId || null,
      bindings: { ...profile.bindings },
    };
  }

  parseProfilePayload(payload, { allowUnresolved = true, fillMissing = true } = {}) {
    const errors = [];
    const warnings = [];
    const raw = payload && typeof payload === "object" ? payload : {};
    const schemaVersion = raw.schemaVersion;
    if (schemaVersion !== HOTKEY_PROFILE_SCHEMA_VERSION) {
      errors.push({ code: "unsupportedSchemaVersion", schemaVersion });
    }
    const mode = raw.mode === "grid" || raw.mode === "direct" ? raw.mode : "";
    if (!mode) errors.push({ code: "invalidMode", mode: raw.mode });
    const id = typeof raw.id === "string" && raw.id.trim() ? raw.id.trim() : "";
    if (!id) errors.push({ code: "missingId" });
    const type = raw.type === "preset" ? "preset" : "custom";
    const sourceBindings = raw.bindings && typeof raw.bindings === "object" ? raw.bindings : {};
    const bindings = {};

    for (const [commandId, value] of Object.entries(sourceBindings)) {
      if (!this._knownCommandIds().has(commandId)) {
        warnings.push({ code: "unknownCommand", commandId });
        continue;
      }
      const key = normalizeHotkey(value);
      if (!key) {
        errors.push({ code: "invalidKey", commandId, key: value });
        continue;
      }
      bindings[commandId] = key;
    }

    if (mode === "direct") {
      for (const command of this.catalog.commands || []) {
        if (bindings[command.commandId]) continue;
        if (!fillMissing) {
          warnings.push({ code: "missingCommandUnresolved", commandId: command.commandId });
          if (!allowUnresolved) errors.push({ code: "unresolvedCommand", commandId: command.commandId });
          continue;
        }
        const fallback = this._fallbackKeyForCommand(command);
        if (fallback) {
          bindings[command.commandId] = fallback;
          warnings.push({ code: "missingCommandFallback", commandId: command.commandId, key: fallback });
        } else {
          warnings.push({ code: "missingCommandUnresolved", commandId: command.commandId });
          if (!allowUnresolved) errors.push({ code: "unresolvedCommand", commandId: command.commandId });
        }
      }
      this._appendConflictErrors(bindings, errors);
    }

    const profile = {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id,
      type,
      mode,
      name: typeof raw.name === "string" && raw.name.trim() ? raw.name.trim() : "Custom Hotkeys",
      description: typeof raw.description === "string" ? raw.description : "",
      basePresetId: typeof raw.basePresetId === "string" ? raw.basePresetId : null,
      bindings,
    };
    return { ok: errors.length === 0, profile, errors, warnings };
  }

  resolveCard(card, profile = this.getActiveProfile()) {
    if (!card || !Array.isArray(card.slots)) return card;
    return {
      ...card,
      hotkeyProfileId: profile?.id || HOTKEY_PRESET_GRID,
      slots: card.slots.map((slot) => slot ? this.resolveSlot(slot, profile) : null),
    };
  }

  resolveSlot(slot, profile = this.getActiveProfile()) {
    const hotkey = profile?.mode === "direct"
      ? normalizeHotkey(profile.bindings?.[slot.commandId]) || this._fallbackKeyForCommand(slot)
      : gridHotkeyForSlot(slot.slotIndex);
    return { ...slot, hotkey };
  }

  _readStoredProfiles() {
    const errors = [];
    const warnings = [];
    const text = this._storageGet(this.profilesKey);
    if (!text) return { profiles: [], errors, warnings };
    let raw;
    try {
      raw = JSON.parse(text);
    } catch {
      return { profiles: [], errors: [{ code: "storageParseFailed" }], warnings };
    }
    const entries = Array.isArray(raw?.profiles) ? raw.profiles : Array.isArray(raw) ? raw : [];
    const profiles = [];
    for (const entry of entries) {
      const parsed = this.parseProfilePayload(entry, { allowUnresolved: false });
      warnings.push(...parsed.warnings);
      if (parsed.ok && parsed.profile.type === "custom") {
        profiles.push(parsed.profile);
      } else {
        errors.push(...parsed.errors.map((error) => ({ ...error, profileId: entry?.id || null })));
      }
    }
    return { profiles, errors, warnings };
  }

  _replaceCustomProfile(profile) {
    const idx = this.customProfiles.findIndex((entry) => entry.id === profile.id);
    if (idx >= 0) this.customProfiles.splice(idx, 1, profile);
    else this.customProfiles.push(profile);
  }

  _writeCustomProfiles() {
    this._storageSet(this.profilesKey, JSON.stringify({
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      profiles: this.customProfiles,
    }));
  }

  _knownCommandIds() {
    if (!this._knownIds) {
      this._knownIds = new Set((this.catalog.commands || []).map((command) => command.commandId));
    }
    return this._knownIds;
  }

  _fallbackKeyForCommand(command) {
    const slotKey = Number.isInteger(command.slotIndex) ? gridHotkeyForSlot(command.slotIndex) : "";
    if (slotKey) return slotKey;
    return normalizeHotkey((command.label || "").trim().charAt(0));
  }

  _appendConflictErrors(bindings, errors) {
    for (const context of this.catalog.contexts || []) {
      const byKey = new Map();
      for (const commandId of context.commandIds) {
        const key = bindings[commandId];
        if (!key) continue;
        const prior = byKey.get(key);
        if (prior) {
          errors.push({ code: "duplicateKey", contextId: context.id, key, commandIds: [prior, commandId] });
        } else {
          byKey.set(key, commandId);
        }
      }
    }
  }

  _storageGet(key) {
    try {
      return this.storage?.getItem?.(key) || "";
    } catch {
      return "";
    }
  }

  _storageSet(key, value) {
    try {
      this.storage?.setItem?.(key, value);
      return true;
    } catch {
      return false;
    }
  }
}

function buildClassicBindings(catalog) {
  const bindings = {};
  for (const command of catalog?.commands || []) {
    bindings[command.commandId] =
      CORE_CLASSIC_BINDINGS[command.commandId] ||
      normalizeHotkey((command.label || "").trim().charAt(0)) ||
      (Number.isInteger(command.slotIndex) ? GRID_HOTKEYS[command.slotIndex] : "");
  }
  return resolveContextConflicts(bindings, catalog);
}

function resolveContextConflicts(bindings, catalog) {
  for (const context of catalog?.contexts || []) {
    const used = new Set();
    for (const commandId of context.commandIds) {
      const current = bindings[commandId];
      if (current && !used.has(current)) {
        used.add(current);
        continue;
      }
      const next = firstFreeKey(used);
      if (next) {
        bindings[commandId] = next;
        used.add(next);
      }
    }
  }
  return bindings;
}

function firstFreeKey(used) {
  for (const key of "ABCDEFGHIJKLMNOPQRSTUVWXYZ") {
    if (!used.has(key)) return key;
  }
  return "";
}

function freezeProfile(profile) {
  return Object.freeze({ ...profile, bindings: Object.freeze({ ...profile.bindings }) });
}

function labelFromContextId(id) {
  return String(id || "")
    .split(/[-_.:]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ") || "Command Card";
}
