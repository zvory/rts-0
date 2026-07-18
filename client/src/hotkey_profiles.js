import {
  factionCommandId,
  gridHotkeyForSlot,
  parsedFactionCommandId,
} from "./hud_command_card.js";
import { ABILITY, DEFAULT_FACTION_ID, KIND, UPGRADE } from "./protocol.js";

export const HOTKEY_PROFILE_SCHEMA_VERSION = 2;
export const HOTKEY_STORAGE_PROFILES_KEY = "rts.hotkeyProfiles.v2";
export const HOTKEY_STORAGE_ACTIVE_KEY = "rts.activeHotkeyProfile.v2";
export const HOTKEY_PRESET_GRID = "preset.grid";
export const HOTKEY_PRESET_CLASSIC = "preset.classicRts";
export const HOTKEY_COMMAND_SELECT_IDLE_WORKERS = "hud.selectIdleWorkers";

const VALID_HOTKEY_CODE_RE = /^Key[A-Z]$/;
const DEFAULT_EXPORT_BUILD = "unknown";

// Direct-profile bindings must not depend on a command card slot. Keep the
// familiar action keys here, including the mnemonic conflict resolutions for
// commands that can share a card.
const CLASSIC_DIRECT_BINDINGS = Object.freeze({
  "unit.move": "KeyM",
  "unit.attack": "KeyA",
  "unit.holdPosition": "KeyH",
  "unit.stop": "KeyS",
  "unit.setupSupportWeapon": "KeyU",
  "worker.buildMenu": "KeyB",
  "worker.return": "KeyW",
  [factionCommandId(DEFAULT_FACTION_ID, "build", KIND.TANK_TRAP)]: "KeyK",
  [factionCommandId(DEFAULT_FACTION_ID, "ability", ABILITY.SMOKE)]: "KeyD",
  [factionCommandId(DEFAULT_FACTION_ID, "ability", ABILITY.SCOUT_PLANE)]: "KeyP",
  [factionCommandId(DEFAULT_FACTION_ID, "train", KIND.ARTILLERY)]: "KeyR",
  [factionCommandId(DEFAULT_FACTION_ID, "research", UPGRADE.MORTAR_AUTOCAST)]: "KeyO",
});

const GLOBAL_HOTKEY_CONTEXTS = Object.freeze([Object.freeze({
  id: "hud-shortcuts",
  label: "HUD Shortcuts",
  global: true,
  card: Object.freeze({
    kind: "hotkeys",
    signature: "hotkeys:hud-shortcuts",
    slots: Object.freeze([Object.freeze({
      commandId: HOTKEY_COMMAND_SELECT_IDLE_WORKERS,
      slotIndex: 0,
      icon: "IDLE",
      label: "Select Idle Workers",
      title: "Select all idle workers",
      gridHotkey: "T",
      classicHotkey: "I",
    })]),
  }),
})]);

export function normalizeHotkeyCode(value) {
  if (typeof value !== "string") return "";
  const code = value.trim();
  return VALID_HOTKEY_CODE_RE.test(code) ? code : "";
}

export function hotkeyLabelForCode(value) {
  const code = normalizeHotkeyCode(value);
  return code ? code.slice(3) : "";
}

function hotkeyCodeForLabel(value) {
  if (typeof value !== "string") return "";
  const label = value.trim().toUpperCase();
  return /^[A-Z]$/.test(label) ? `Key${label}` : "";
}

export function buildHotkeyCommandCatalog(cards = []) {
  const commands = new Map();
  const contexts = [];
  for (const entry of [...(cards || []), ...GLOBAL_HOTKEY_CONTEXTS]) {
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
          gridHotkey: hotkeyCodeForLabel(slot.gridHotkey),
          classicHotkey: hotkeyCodeForLabel(slot.classicHotkey),
          global: !!entry?.global,
        });
      }
    }
    contexts.push({ id: contextId, label: contextLabel, card, commandIds, global: !!entry?.global });
  }
  return Object.freeze({
    commands: Object.freeze([...commands.values()].map(Object.freeze)),
    contexts: Object.freeze(contexts.map((ctx) => Object.freeze({
      id: ctx.id,
      label: ctx.label,
      card: ctx.card,
      commandIds: Object.freeze([...ctx.commandIds]),
      global: ctx.global,
    }))),
  });
}

export function createGridPreset(catalog = buildHotkeyCommandCatalog([])) {
  const bindings = {};
  for (const command of catalog?.commands || []) {
    if (command.gridHotkey) bindings[command.commandId] = command.gridHotkey;
  }
  return freezeProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: HOTKEY_PRESET_GRID,
    type: "preset",
    mode: "grid",
    name: "Grid",
    description: "Command-card hotkeys follow the rendered QWE/ASD/ZXC grid.",
    bindings,
    factionBindings: {},
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
    ...buildClassicBindingMaps(catalog),
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
    this.presets = Object.freeze([createGridPreset(catalog), createClassicPreset(catalog)]);
    this.customProfiles = [];
    this.activeProfileId = HOTKEY_PRESET_GRID;
    this.revision = 0;
    this.diagnostics = { errors: [], warnings: [] };
    this.load();
  }

  load() {
    const loaded = this._readStoredProfiles();
    this.customProfiles = loaded.profiles;
    // Schema v1 stored layout-produced letters rather than canonical physical
    // codes. Its separate storage keys are intentionally left unread.
    const active = this._storageGet(this.activeKey);
    this.activeProfileId = this.hasProfile(active) ? active : HOTKEY_PRESET_GRID;
    this.diagnostics = { errors: loaded.errors, warnings: loaded.warnings };
    this.revision += 1;
  }

  allProfiles() {
    return [...this.presets, ...this.customProfiles.map((profile) => cloneProfile(profile))];
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
      factionBindings: cloneFactionBindings(preset.factionBindings),
    };
    const result = this.saveCustomProfile(profile);
    return result.ok ? { ...result, profile: this.profileById(id) } : result;
  }

  saveCustomProfile(profile) {
    const parsed = this.parseProfilePayload(profile, { allowUnresolved: false });
    if (!parsed.ok) return parsed;
    if (parsed.profile.type === "preset" || this.presets.some((preset) => preset.id === parsed.profile.id)) {
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
        if (!profileBindingForCommand(parsed.profile, command.commandId)) {
          errors.push({ code: "unresolvedCommand", commandId: command.commandId });
        }
      }
    }
    return { ...parsed, ok: errors.length === 0, errors, warnings };
  }

  runtimeDiagnostics(profile = this.getActiveProfile()) {
    const parsed = this.parseProfilePayload(profile, { allowUnresolved: true, fillMissing: false });
    return {
      ...parsed,
      ok: parsed.errors.length === 0,
      errors: parsed.errors,
      warnings: parsed.warnings,
    };
  }

  importProfile(payload, { targetId = null, activate = false } = {}) {
    const parsed = this.parseProfilePayload(payload, { allowUnresolved: false });
    if (!parsed.ok) return parsed;
    const id = targetId || this._uniqueProfileId(parsed.profile.id || "custom.imported");
    const name = targetId
      ? parsed.profile.name
      : this._uniqueProfileName(parsed.profile.name || "Imported Hotkeys", id);
    const profile = {
      ...parsed.profile,
      id,
      type: "custom",
      name,
      bindings: { ...parsed.profile.bindings },
      factionBindings: cloneFactionBindings(parsed.profile.factionBindings),
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
    const basePreset = profile.basePresetId || (profile.type === "preset" ? profile.id : null);
    return {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      profileId: profile.id,
      mode: profile.mode,
      name: profile.name,
      description: profile.description || "",
      createdWithBuild: createdWithBuild(),
      basePreset,
      bindings: { ...profile.bindings },
      factionBindings: cloneFactionBindings(profile.factionBindings),
    };
  }

  exportProfileJson(id = this.activeProfileId) {
    const payload = this.exportProfile(id);
    return payload ? `${JSON.stringify(payload, null, 2)}\n` : "";
  }

  parseImportText(text, options = {}) {
    try {
      return this.importProfile(JSON.parse(String(text || "")), options);
    } catch {
      return {
        ok: false,
        profile: null,
        errors: [{ code: "importParseFailed" }],
        warnings: [],
      };
    }
  }

  storedProfilePayload(profile) {
    return {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id: profile.id,
      type: "custom",
      mode: profile.mode,
      name: profile.name,
      description: profile.description || "",
      basePresetId: profile.basePresetId || null,
      bindings: { ...profile.bindings },
      factionBindings: cloneFactionBindings(profile.factionBindings),
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
    const idSource = typeof raw.id === "string" && raw.id.trim()
      ? raw.id
      : typeof raw.profileId === "string" && raw.profileId.trim()
        ? raw.profileId
        : "";
    const id = idSource.trim();
    if (!id) errors.push({ code: "missingId" });
    const type = raw.type === "preset" ? "preset" : "custom";
    const sourceBindings = raw.bindings && typeof raw.bindings === "object" ? raw.bindings : {};
    const sourceFactionBindings = raw.factionBindings && typeof raw.factionBindings === "object"
      ? raw.factionBindings
      : {};
    const bindingMaps = { bindings: {}, factionBindings: {} };

    for (const [commandId, value] of Object.entries(sourceBindings)) {
      const code = normalizeHotkeyCode(value);
      if (!code) {
        errors.push({ code: "invalidKey", commandId, key: value });
        continue;
      }
      const migrated = this._canonicalImportedCommandId(commandId, warnings);
      if (!migrated) continue;
      setBindingForCommand(bindingMaps, migrated, code);
    }

    for (const [factionId, entries] of Object.entries(sourceFactionBindings)) {
      if (!validFactionId(factionId) || !entries || typeof entries !== "object" || Array.isArray(entries)) {
        warnings.push({ code: "invalidFactionBindings", factionId });
        continue;
      }
      for (const [commandId, value] of Object.entries(entries)) {
        const code = normalizeHotkeyCode(value);
        if (!code) {
          errors.push({ code: "invalidKey", commandId, key: value });
          continue;
        }
        const parsed = parsedFactionCommandId(commandId);
        if (!parsed || parsed.factionId !== factionId) {
          warnings.push({ code: "unknownCommand", commandId });
          continue;
        }
        if (!this._knownCommandIds().has(commandId)) {
          warnings.push({ code: "unavailableFactionCommand", commandId });
        }
        setBindingForCommand(bindingMaps, commandId, code);
      }
    }

    if (mode === "direct") {
      for (const command of this.catalog.commands || []) {
        if (bindingForCommand(bindingMaps, command.commandId)) continue;
        if (!fillMissing) {
          warnings.push({ code: "missingCommandUnresolved", commandId: command.commandId });
          if (!allowUnresolved) errors.push({ code: "unresolvedCommand", commandId: command.commandId });
          continue;
        }
        const fallback = this._fallbackKeyForMissingCommand(command, bindingMaps);
        if (fallback) {
          setBindingForCommand(bindingMaps, command.commandId, fallback);
          warnings.push({ code: "missingCommandFallback", commandId: command.commandId, key: fallback });
        } else {
          warnings.push({ code: "missingCommandUnresolved", commandId: command.commandId });
          if (!allowUnresolved) errors.push({ code: "unresolvedCommand", commandId: command.commandId });
        }
      }
      this._appendConflictErrors(bindingMaps, errors);
    } else if (mode === "grid") {
      this._appendGridGlobalConflictErrors(bindingMaps, errors);
    }

    const profile = {
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id,
      type,
      mode,
      name: typeof raw.name === "string" && raw.name.trim() ? raw.name.trim() : "Custom Hotkeys",
      description: typeof raw.description === "string" ? raw.description : "",
      basePresetId: typeof raw.basePresetId === "string"
        ? raw.basePresetId
        : typeof raw.basePreset === "string"
          ? raw.basePreset
          : null,
      bindings: bindingMaps.bindings,
      factionBindings: bindingMaps.factionBindings,
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
    const gridHotkey = hotkeyCodeForLabel(slot.gridHotkey);
    const hotkeyCode = profile?.mode === "direct"
      ? normalizeHotkeyCode(profileBindingForCommand(profile, slot.commandId)) || this._fallbackKeyForCommand(slot)
      : gridHotkey
        ? normalizeHotkeyCode(profileBindingForCommand(profile, slot.commandId)) || gridHotkey
        : hotkeyCodeForLabel(gridHotkeyForSlot(slot.slotIndex));
    return { ...slot, hotkeyCode, hotkey: hotkeyLabelForCode(hotkeyCode) };
  }

  hotkeyForCommand(commandId, profile = this.getActiveProfile()) {
    return hotkeyLabelForCode(this.hotkeyCodeForCommand(commandId, profile));
  }

  hotkeyCodeForCommand(commandId, profile = this.getActiveProfile()) {
    const command = (this.catalog.commands || []).find((entry) => entry.commandId === commandId);
    if (!command) return "";
    const bound = normalizeHotkeyCode(profileBindingForCommand(profile, commandId));
    if (bound) return bound;
    if (profile?.mode === "grid") {
      return command.gridHotkey || hotkeyCodeForLabel(gridHotkeyForSlot(command.slotIndex));
    }
    return this._fallbackKeyForCommand(command);
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
        errors.push(...parsed.errors.map((error) => ({ ...error, profileId: entry?.id || entry?.profileId || null })));
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
      profiles: this.customProfiles.map((profile) => this.storedProfilePayload(profile)),
    }));
  }

  _knownCommandIds() {
    if (!this._knownIds) {
      this._knownIds = new Set((this.catalog.commands || []).map((command) => command.commandId));
    }
    return this._knownIds;
  }

  _canonicalImportedCommandId(commandId, warnings) {
    if (this._knownCommandIds().has(commandId)) return commandId;
    const legacy = parsedLegacyFactionCommandId(commandId);
    if (legacy) {
      warnings.push({ code: "legacyCommandMigrated", commandId, migratedCommandId: legacy.commandId });
      return legacy.commandId;
    }
    if (parsedFactionCommandId(commandId)) {
      warnings.push({ code: "unavailableFactionCommand", commandId });
      return commandId;
    }
    warnings.push({ code: "unknownCommand", commandId });
    return "";
  }

  _fallbackKeyForCommand(command) {
    const classicKey = normalizeHotkeyCode(command.classicHotkey);
    if (classicKey) return classicKey;
    const labelKey = hotkeyCodeForLabel((command.label || "").trim().charAt(0));
    if (labelKey) return labelKey;
    return Number.isInteger(command.slotIndex) ? hotkeyCodeForLabel(gridHotkeyForSlot(command.slotIndex)) : "";
  }

  _fallbackKeyForMissingCommand(command, bindingMaps) {
    const preferred = this._fallbackKeyForCommand(command);
    const used = new Set();
    const globalCommandIds = new Set((this.catalog.contexts || [])
      .filter((context) => context.global)
      .flatMap((context) => context.commandIds));
    const commandIsGlobal = globalCommandIds.has(command.commandId);
    for (const context of this.catalog.contexts || []) {
      if (!commandIsGlobal && !context.commandIds.includes(command.commandId) && !context.global) continue;
      for (const contextCommandId of context.commandIds) {
        if (contextCommandId === command.commandId) continue;
        const key = bindingForCommand(bindingMaps, contextCommandId);
        if (key) used.add(key);
      }
    }
    if (preferred && !used.has(preferred)) return preferred;
    return firstFreeKey(used);
  }

  _appendConflictErrors(bindingMaps, errors) {
    for (const context of this.catalog.contexts || []) {
      const byKey = new Map();
      for (const commandId of context.commandIds) {
        const key = bindingForCommand(bindingMaps, commandId);
        if (!key) continue;
        const prior = byKey.get(key);
        if (prior) {
          errors.push({ code: "duplicateKey", contextId: context.id, key, commandIds: [prior, commandId] });
        } else {
          byKey.set(key, commandId);
        }
      }
    }

    const globalCommandIds = new Set((this.catalog.contexts || [])
      .filter((context) => context.global)
      .flatMap((context) => context.commandIds));
    for (const globalCommandId of globalCommandIds) {
      const globalKey = bindingForCommand(bindingMaps, globalCommandId);
      if (!globalKey) continue;
      for (const context of this.catalog.contexts || []) {
        if (context.global) continue;
        for (const commandId of context.commandIds) {
          if (bindingForCommand(bindingMaps, commandId) !== globalKey) continue;
          errors.push({
            code: "duplicateKey",
            contextId: context.id,
            key: globalKey,
            commandIds: [globalCommandId, commandId],
          });
        }
      }
    }
  }

  _appendGridGlobalConflictErrors(bindingMaps, errors) {
    const commandById = new Map((this.catalog.commands || [])
      .map((command) => [command.commandId, command]));
    const globalCommandIds = new Set((this.catalog.contexts || [])
      .filter((context) => context.global)
      .flatMap((context) => context.commandIds));
    const globalKeys = new Map();
    for (const commandId of globalCommandIds) {
      const command = commandById.get(commandId);
      const key = bindingForCommand(bindingMaps, commandId) || command?.gridHotkey || "";
      if (key) globalKeys.set(commandId, key);
    }
    for (const context of this.catalog.contexts || []) {
      if (context.global) continue;
      for (const commandId of context.commandIds) {
        const command = commandById.get(commandId);
        const key = command?.gridHotkey || hotkeyCodeForLabel(gridHotkeyForSlot(command?.slotIndex));
        for (const [globalCommandId, globalKey] of globalKeys) {
          if (!key || key !== globalKey) continue;
          errors.push({
            code: "duplicateKey",
            contextId: context.id,
            key,
            commandIds: [globalCommandId, commandId],
          });
        }
      }
    }
  }

  _uniqueProfileId(seed) {
    const cleanSeed = String(seed || "custom.imported").trim() || "custom.imported";
    const base = cleanSeed.startsWith("custom.") ? cleanSeed : `custom.${cleanSeed.replace(/^preset\./, "")}`;
    let candidate = base;
    let idx = 2;
    while (this.hasProfile(candidate)) {
      candidate = `${base}.${idx}`;
      idx += 1;
    }
    return candidate;
  }

  _uniqueProfileName(seed, id) {
    const base = String(seed || "Imported Hotkeys").trim() || "Imported Hotkeys";
    const names = new Set(this.allProfiles().map((profile) => profile.name));
    if (!names.has(base)) return base;
    const suffix = id.match(/\.([0-9]+)$/)?.[1];
    let idx = suffix ? Number(suffix) : 2;
    let candidate = `${base} ${idx}`;
    while (names.has(candidate)) {
      idx += 1;
      candidate = `${base} ${idx}`;
    }
    return candidate;
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

function createdWithBuild() {
  return String(globalThis.__RTS_BUILD__ || globalThis.__RTS_VERSION__ || DEFAULT_EXPORT_BUILD);
}

function buildClassicBindingMaps(catalog) {
  const bindingMaps = { bindings: {}, factionBindings: {} };
  for (const command of catalog?.commands || []) {
    setBindingForCommand(bindingMaps, command.commandId,
      CLASSIC_DIRECT_BINDINGS[command.commandId] ||
      command.classicHotkey ||
      hotkeyCodeForLabel((command.label || "").trim().charAt(0)));
  }
  return resolveContextConflicts(bindingMaps, catalog);
}

function resolveContextConflicts(bindingMaps, catalog) {
  for (const context of catalog?.contexts || []) {
    if (context.global) continue;
    const used = new Set();
    for (const commandId of context.commandIds) {
      const current = bindingForCommand(bindingMaps, commandId);
      if (current && !used.has(current)) {
        used.add(current);
        continue;
      }
      const next = firstFreeKey(used);
      if (next) {
        setBindingForCommand(bindingMaps, commandId, next);
        used.add(next);
      }
    }
  }
  const globalCommandIds = new Set((catalog?.contexts || [])
    .filter((context) => context.global)
    .flatMap((context) => context.commandIds));
  const globallyUsed = new Set((catalog?.commands || [])
    .filter((command) => !globalCommandIds.has(command.commandId))
    .map((command) => bindingForCommand(bindingMaps, command.commandId))
    .filter(Boolean));
  for (const commandId of globalCommandIds) {
    const preferred = bindingForCommand(bindingMaps, commandId);
    const key = preferred && !globallyUsed.has(preferred)
      ? preferred
      : firstFreeKey(globallyUsed);
    if (!key) continue;
    setBindingForCommand(bindingMaps, commandId, key);
    globallyUsed.add(key);
  }
  return bindingMaps;
}

function firstFreeKey(used) {
  for (const key of "ABCDEFGHIJKLMNOPQRSTUVWXYZ") {
    const code = `Key${key}`;
    if (!used.has(code)) return code;
  }
  return "";
}

function freezeProfile(profile) {
  return Object.freeze({
    ...profile,
    bindings: Object.freeze({ ...profile.bindings }),
    factionBindings: freezeFactionBindings(profile.factionBindings),
  });
}

function labelFromContextId(id) {
  return String(id || "")
    .split(/[-_.:]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ") || "Command Card";
}

export function profileBindingForCommand(profile, commandId) {
  return bindingForCommand(profile || {}, commandId);
}

export function setProfileBindingForCommand(profile, commandId, key) {
  if (!profile || typeof profile !== "object") return;
  setBindingForCommand(profile, commandId, key);
}

function bindingForCommand(bindingMaps, commandId) {
  const parsed = parsedFactionCommandId(commandId);
  if (parsed) return bindingMaps.factionBindings?.[parsed.factionId]?.[commandId] || "";
  return bindingMaps.bindings?.[commandId] || "";
}

function setBindingForCommand(bindingMaps, commandId, key) {
  const parsed = parsedFactionCommandId(commandId);
  if (parsed) {
    if (!bindingMaps.factionBindings || typeof bindingMaps.factionBindings !== "object") {
      bindingMaps.factionBindings = {};
    }
    bindingMaps.factionBindings[parsed.factionId] = {
      ...(bindingMaps.factionBindings[parsed.factionId] || {}),
      [commandId]: key,
    };
    return;
  }
  if (!bindingMaps.bindings || typeof bindingMaps.bindings !== "object") {
    bindingMaps.bindings = {};
  }
  bindingMaps.bindings[commandId] = key;
}

function parsedLegacyFactionCommandId(commandId) {
  const match = /^(build|train|research|ability)\.([A-Za-z0-9_]+)$/.exec(String(commandId || ""));
  return match
    ? {
        family: match[1],
        subject: match[2],
        commandId: factionCommandId(DEFAULT_FACTION_ID, match[1], match[2]),
      }
    : null;
}

function validFactionId(factionId) {
  return /^[a-z0-9_]+$/.test(String(factionId || ""));
}

function cloneProfile(profile) {
  return {
    ...profile,
    bindings: { ...profile.bindings },
    factionBindings: cloneFactionBindings(profile.factionBindings),
  };
}

function cloneFactionBindings(factionBindings = {}) {
  const clone = {};
  for (const [factionId, bindings] of Object.entries(factionBindings || {})) {
    if (!bindings || typeof bindings !== "object") continue;
    clone[factionId] = { ...bindings };
  }
  return clone;
}

function freezeFactionBindings(factionBindings = {}) {
  const clone = {};
  for (const [factionId, bindings] of Object.entries(factionBindings || {})) {
    clone[factionId] = Object.freeze({ ...bindings });
  }
  return Object.freeze(clone);
}
