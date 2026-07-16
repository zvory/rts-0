import assert from "node:assert/strict";

import {
  HOTKEY_PRESET_CLASSIC,
  HOTKEY_PRESET_GRID,
  HOTKEY_COMMAND_SELECT_IDLE_WORKERS,
  HOTKEY_PROFILE_SCHEMA_VERSION,
  HOTKEY_STORAGE_ACTIVE_KEY,
  HOTKEY_STORAGE_PROFILES_KEY,
  HotkeyProfileService,
  buildHotkeyCommandCatalog,
} from "../client/src/hotkey_profiles.js";
import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  factionCommandId,
} from "../client/src/hud_command_card.js";
import { ABILITY, KIND, UPGRADE } from "../client/src/protocol.js";

const kriegsiaCommandId = (family, subject) => factionCommandId("kriegsia", family, subject);
const ekatCommandId = (family, subject) => factionCommandId("ekat", family, subject);

function memoryStorage(seed = {}) {
  const data = new Map(Object.entries(seed));
  return {
    getItem(key) {
      return data.has(key) ? data.get(key) : null;
    },
    setItem(key, value) {
      data.set(key, String(value));
    },
    removeItem(key) {
      data.delete(key);
    },
    data,
  };
}

function service(storage = memoryStorage()) {
  return new HotkeyProfileService({
    storage,
    catalog: buildHotkeyCommandCatalog(buildCommandCardContextCatalog()),
  });
}

function workerCard() {
  const worker = { id: 20, owner: 1, kind: KIND.WORKER };
  return buildCommandCardDescriptors({
    playerId: 1,
    selection: [worker],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
}

function workerBuildCard(factionId = "kriegsia") {
  const worker = { id: 20, owner: 1, kind: KIND.WORKER };
  return buildCommandCardDescriptors({
    playerId: 1,
    factionId,
    selection: [worker],
    commandCardMode: "workerBuild",
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
}

function commandCarCard() {
  const cityCentre = { id: 21, owner: 1, kind: KIND.CITY_CENTRE, buildProgress: null };
  const commandCar = {
    id: 22,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    abilities: [
      { ability: ABILITY.BREAKTHROUGH, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.SCOUT_PLANE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  return buildCommandCardDescriptors({
    playerId: 1,
    selection: [commandCar],
    entities: [cityCentre, commandCar],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 20 },
    upgrades: [],
    playerHasCompleteKind: (kind) => kind === KIND.CITY_CENTRE,
    groupCooldownClocks: () => [],
  });
}

{
  const catalog = buildHotkeyCommandCatalog(buildCommandCardContextCatalog());
  assert(
    catalog.commands.some((command) =>
      command.commandId === kriegsiaCommandId("research", UPGRADE.PANZERFAUSTS) &&
        command.slotIndex === 1
    ),
    "hotkey command catalog includes Panzerfausts research in the Training Centre W slot",
  );
  assert(
    catalog.commands.some((command) =>
      command.commandId === kriegsiaCommandId("ability", ABILITY.SCOUT_PLANE) &&
        command.slotIndex === 8
    ),
    "hotkey command catalog includes the exposed Scout Plane Command Car ability in the C slot",
  );
  assert(
    catalog.commands.some((command) =>
      command.commandId === kriegsiaCommandId("ability", ABILITY.POINT_FIRE) &&
        command.slotIndex === 7
    ) && catalog.commands.some((command) =>
      command.commandId === kriegsiaCommandId("ability", ABILITY.BLANKET_FIRE) &&
        command.slotIndex === 8
    ),
    "hotkey command catalog keeps lower-priority artillery abilities discoverable",
  );
  assert(
    catalog.commands.some((command) =>
      command.commandId === kriegsiaCommandId("research", UPGRADE.ARTILLERY_UNLOCK) &&
        command.label === "Heavy Guns" &&
        command.slotIndex === 0
    ),
    "hotkey command catalog includes Heavy Guns research after Medium Guns unlocks its replacement slot",
  );
  assert(
    catalog.commands.some((command) =>
      command.commandId === HOTKEY_COMMAND_SELECT_IDLE_WORKERS &&
        command.gridHotkey === "T" &&
        command.classicHotkey === "I" &&
        command.global
    ),
    "hotkey command catalog includes the global idle-worker action with per-preset defaults",
  );
}

{
  const hotkeys = service();
  assert(Object.isFrozen(hotkeys.profileById(HOTKEY_PRESET_GRID)), "Grid preset is immutable");
  const classic = hotkeys.profileById(HOTKEY_PRESET_CLASSIC);
  assert(Object.isFrozen(classic.bindings), "Classic preset bindings are immutable");
  assert.equal(
    hotkeys.runtimeDiagnostics(classic).ok,
    true,
    "Classic preset has no conflicts in any cataloged command-card context",
  );
  assert.equal(hotkeys.getActiveProfile().id, HOTKEY_PRESET_GRID, "Grid is the default active profile");
  assert.equal(
    hotkeys.hotkeyForCommand(HOTKEY_COMMAND_SELECT_IDLE_WORKERS),
    "T",
    "Grid preset binds idle-worker selection to T",
  );
  hotkeys.setActiveProfile(HOTKEY_PRESET_CLASSIC);
  assert.equal(
    hotkeys.hotkeyForCommand(HOTKEY_COMMAND_SELECT_IDLE_WORKERS),
    "I",
    "Classic RTS uses conflict-free I because T is already assigned",
  );
  assert.equal(
    hotkeys.resolveCard(commandCarCard()).slots[8].hotkey,
    "C",
    "Classic RTS preset binds Scout Plane ability to C",
  );
}

{
  const hotkeys = service();
  const grid = hotkeys.profileById(HOTKEY_PRESET_GRID);
  const conflicting = {
    ...grid,
    id: "custom.grid-global-conflict",
    type: "custom",
    bindings: { ...grid.bindings, [HOTKEY_COMMAND_SELECT_IDLE_WORKERS]: "Q" },
  };
  assert.equal(
    hotkeys.validateDraftProfile(conflicting).ok,
    false,
    "grid profiles reject an idle-worker key that shadows the command-card grid",
  );
}

{
  const hotkeys = service();
  const saved = hotkeys.saveCustomProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.pre-idle-action",
    type: "custom",
    mode: "direct",
    name: "Existing Custom",
    bindings: { "unit.move": "I" },
    factionBindings: {},
  });
  assert.equal(saved.ok, true, "existing custom profiles gain a conflict-free idle-worker binding");
  assert.notEqual(
    saved.profile.bindings[HOTKEY_COMMAND_SELECT_IDLE_WORKERS],
    "I",
    "idle-worker migration does not displace an existing I binding",
  );
}

{
  const hotkeys = service();
  const classic = hotkeys.profileById(HOTKEY_PRESET_CLASSIC);
  const conflicting = {
    ...classic,
    id: "custom.global-conflict",
    type: "custom",
    bindings: { ...classic.bindings, [HOTKEY_COMMAND_SELECT_IDLE_WORKERS]: "T" },
    factionBindings: Object.fromEntries(Object.entries(classic.factionBindings)
      .map(([factionId, bindings]) => [factionId, { ...bindings }])),
  };
  const validation = hotkeys.validateDraftProfile(conflicting);
  assert.equal(validation.ok, false, "a global idle-worker binding cannot shadow a command-card key");
  assert(
    validation.errors.some((error) =>
      error.code === "duplicateKey" && error.commandIds.includes(HOTKEY_COMMAND_SELECT_IDLE_WORKERS)
    ),
    "global hotkey conflicts identify the idle-worker action",
  );
}

{
  const hotkeys = service();
  const cloned = hotkeys.createCustomFromPreset(HOTKEY_PRESET_CLASSIC, {
    id: "custom.classic",
    name: "My Classic",
  });
  assert.equal(cloned.ok, true, "preset cloning succeeds");
  assert.equal(cloned.profile.type, "custom", "cloned preset is editable custom profile");
  assert.equal(cloned.profile.basePresetId, HOTKEY_PRESET_CLASSIC, "clone keeps preset ancestry");
  assert.equal(hotkeys.setActiveProfile("custom.classic"), true, "custom profile can become active");
  assert.equal(hotkeys.getActiveProfile().id, "custom.classic", "active profile selection updates");
}

{
  const hotkeys = service();
  const editedPreset = hotkeys.saveCustomProfile({
    ...hotkeys.exportProfile(HOTKEY_PRESET_GRID),
    bindings: { "unit.move": "M" },
  });
  assert.equal(editedPreset.ok, false, "presets cannot be edited directly");
  assert(editedPreset.errors.some((error) => error.code === "presetImmutable"), "preset edit rejection is reported");
}

{
  const storage = memoryStorage();
  const hotkeys = service(storage);
  const saved = hotkeys.saveCustomProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.direct",
    type: "custom",
    mode: "direct",
    name: "Direct",
    bindings: {
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
    },
  });
  assert.equal(saved.ok, true, "custom direct profile saves");
  hotkeys.setActiveProfile("custom.direct");

  const reloaded = service(storage);
  assert.equal(reloaded.getActiveProfile().id, "custom.direct", "active profile persists through local storage");
  assert(storage.data.has(HOTKEY_STORAGE_PROFILES_KEY), "custom profiles are written to storage");
  assert.equal(storage.data.get(HOTKEY_STORAGE_ACTIVE_KEY), "custom.direct", "active id is written to storage");
}

{
  const hotkeys = service(memoryStorage({
    [HOTKEY_STORAGE_PROFILES_KEY]: "{bad json",
    [HOTKEY_STORAGE_ACTIVE_KEY]: "missing",
  }));
  assert.equal(hotkeys.getActiveProfile().id, HOTKEY_PRESET_GRID, "bad storage falls back to Grid");
  assert(hotkeys.diagnostics.errors.some((error) => error.code === "storageParseFailed"), "bad storage is diagnosed");
}

{
  const hotkeys = service();
  const invalid = hotkeys.saveCustomProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.bad",
    type: "custom",
    mode: "direct",
    name: "Bad",
    bindings: {
      "unit.move": "1",
      "unit.attack": "A",
      "unknown.command": "U",
    },
  });
  assert.equal(invalid.ok, false, "invalid keys block saving");
  assert(invalid.errors.some((error) => error.code === "invalidKey"), "invalid key is reported");
  assert(invalid.warnings.some((warning) => warning.code === "unknownCommand"), "unknown command is a warning");
}

{
  const hotkeys = service();
  const conflict = hotkeys.saveCustomProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.conflict",
    type: "custom",
    mode: "direct",
    name: "Conflict",
    bindings: {
      "unit.move": "A",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
    },
  });
  assert.equal(conflict.ok, false, "same-context duplicate keys block saving");
  assert(conflict.errors.some((error) => error.code === "duplicateKey"), "duplicate key is reported");
}

{
  const hotkeys = service();
  const classic = hotkeys.profileById(HOTKEY_PRESET_CLASSIC);
  const draft = hotkeys.validateDraftProfile({
    ...classic,
    id: "custom.draft",
    type: "custom",
    bindings: { ...classic.bindings, "unit.move": "A", "unit.attack": "A" },
  });
  assert.equal(draft.ok, false, "draft validator reports same-context duplicate keys");
  assert(draft.errors.some((error) =>
    error.code === "duplicateKey" &&
    error.contextId === "worker-main" &&
    error.commandIds.includes("unit.move") &&
    error.commandIds.includes("unit.attack")
  ), "draft duplicate key names the affected context and commands");
}

{
  const hotkeys = service();
  const classic = hotkeys.profileById(HOTKEY_PRESET_CLASSIC);
  const draft = hotkeys.validateDraftProfile({
    ...classic,
    id: "custom.exclusive",
    type: "custom",
    bindings: { ...classic.bindings, "worker.buildMenu": "Y", "worker.return": "Y" },
  });
  assert.equal(draft.ok, true, "same key is allowed across mutually exclusive command-card contexts");
}

{
  const hotkeys = service();
  const draft = hotkeys.validateDraftProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.blank",
    type: "custom",
    mode: "direct",
    name: "Blank",
    bindings: {},
  });
  assert.equal(draft.ok, false, "draft validator blocks unresolved direct profiles");
  assert(draft.errors.some((error) => error.code === "unresolvedCommand"), "unresolved commands are reported");
}

{
  const hotkeys = service();
  const parsed = hotkeys.parseProfilePayload({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "import.partial",
    type: "custom",
    mode: "direct",
    name: "Partial",
    bindings: {
      "unit.move": "M",
    },
  });
  assert.equal(parsed.ok, true, "missing known commands are filled during import parsing");
  assert.equal(parsed.profile.bindings["unit.attack"], "A", "missing command falls back to rendered grid slot");
  assert(parsed.warnings.some((warning) => warning.code === "missingCommandFallback"), "fallback is diagnosed");
}

{
  const hotkeys = service();
  const migrated = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.legacy-build",
    type: "custom",
    mode: "direct",
    name: "Legacy Build",
    bindings: {
      "build.city_centre": "B",
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "W",
    },
  }, { activate: true });
  const cityCentreCommandId = kriegsiaCommandId("build", KIND.CITY_CENTRE);
  assert.equal(migrated.ok, true, "legacy faction command ids import successfully");
  assert(migrated.warnings.some((warning) => warning.code === "legacyCommandMigrated"), "legacy command migration is diagnosed");
  assert.equal(migrated.profile.factionBindings.kriegsia[cityCentreCommandId], "B", "legacy build ids migrate into Kriegsia bindings");
  assert.equal(migrated.profile.bindings["build.city_centre"], undefined, "legacy build ids are not kept in the global binding map");
  assert.equal(hotkeys.resolveCard(workerBuildCard()).slots[0].hotkey, "B", "migrated Kriegsia build binding resolves in Kriegsia cards");
}

{
  const hotkeys = service();
  const cityCentreCommandId = kriegsiaCommandId("build", KIND.CITY_CENTRE);
  const imported = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.kriegsia-only",
    type: "custom",
    mode: "direct",
    name: "Kriegsia Only",
    factionBindings: {
      kriegsia: {
        [cityCentreCommandId]: "B",
      },
    },
    bindings: {
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "W",
    },
  }, { activate: true });
  assert.equal(imported.ok, true, "Kriegsia faction bindings import successfully");
  assert.equal(hotkeys.resolveCard(workerBuildCard()).slots[0].hotkey, "B", "Kriegsia custom build binding applies to Kriegsia");
  assert.equal(workerBuildCard("ekat").slots[0], null, "unknown future factions do not inherit Kriegsia build commands");
}

{
  const hotkeys = service();
  const futureCommandId = ekatCommandId("build", KIND.CITY_CENTRE);
  const imported = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.future-ekat",
    type: "custom",
    mode: "direct",
    name: "Future Ekat",
    factionBindings: {
      ekat: {
        [futureCommandId]: "E",
      },
    },
    bindings: {
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "W",
    },
  }, { activate: true });
  assert.equal(imported.ok, true, "unavailable faction commands are preserved on import");
  assert(imported.warnings.some((warning) => warning.code === "unavailableFactionCommand"), "unavailable faction command preservation is diagnosed");
  assert.equal(imported.profile.factionBindings.ekat[futureCommandId], "E", "future Ekat command binding is stored");
  assert.equal(hotkeys.resolveCard(workerBuildCard()).slots[0].hotkey, "Q", "future Ekat bindings are inactive for current Kriegsia cards");
  assert.equal(hotkeys.exportProfile(imported.profile.id).factionBindings.ekat[futureCommandId], "E", "future Ekat bindings round-trip through export");
}

{
  const hotkeys = service();
  const unsupported = hotkeys.parseProfilePayload({
    schemaVersion: 999,
    id: "future",
    type: "custom",
    mode: "grid",
    name: "Future",
    bindings: {},
  });
  assert.equal(unsupported.ok, false, "unsupported schema versions are rejected");
  assert(unsupported.errors.some((error) => error.code === "unsupportedSchemaVersion"), "schema error is reported");
}

{
  const hotkeys = service();
  globalThis.__RTS_BUILD__ = "test-build";
  const exported = hotkeys.exportProfile(HOTKEY_PRESET_CLASSIC);
  assert.equal(exported.schemaVersion, HOTKEY_PROFILE_SCHEMA_VERSION, "export includes schema version");
  assert.equal(exported.profileId, HOTKEY_PRESET_CLASSIC, "export uses player-facing profileId metadata");
  assert.equal(exported.name, "Classic RTS", "export includes profile name");
  assert.equal(exported.description.length > 0, true, "export includes profile description");
  assert.equal(exported.createdWithBuild, "test-build", "export includes build metadata");
  assert.equal(exported.basePreset, HOTKEY_PRESET_CLASSIC, "preset exports name their base preset");
  assert.equal(exported.bindings["unit.move"], "M", "export includes hotkey bindings");
  delete globalThis.__RTS_BUILD__;
}

{
  const hotkeys = service();
  const imported = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    profileId: HOTKEY_PRESET_CLASSIC,
    mode: "direct",
    name: "Classic RTS",
    description: "Imported preset",
    basePreset: HOTKEY_PRESET_CLASSIC,
    bindings: hotkeys.profileById(HOTKEY_PRESET_CLASSIC).bindings,
  });
  assert.equal(imported.ok, true, "preset-shaped export imports as a custom profile");
  assert.equal(imported.profile.type, "custom", "imported payloads are stored as custom profiles");
  assert.equal(imported.profile.id, "custom.classicRts", "import rewrites preset ids away from preset namespace");
  assert.equal(imported.profile.name, "Classic RTS 2", "import rewrites colliding display names");
}

{
  const hotkeys = service();
  const imported = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.direct",
    type: "custom",
    mode: "direct",
    name: "Direct",
    bindings: {
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
      "unknown.command": "U",
    },
  });
  assert.equal(imported.ok, true, "unknown imported commands are non-fatal");
  assert(imported.warnings.some((warning) => warning.code === "unknownCommand"), "unknown imported commands are reported");
  assert.equal(imported.profile.bindings["unknown.command"], undefined, "unknown imported commands are ignored");
}

{
  const hotkeys = service();
  const invalid = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.bad-import",
    mode: "direct",
    name: "Bad Import",
    bindings: {
      "unit.move": "1",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
    },
  });
  assert.equal(invalid.ok, false, "invalid imported keys are fatal");
  assert(invalid.errors.some((error) => error.code === "invalidKey"), "invalid imported keys are reported");
}

{
  const hotkeys = service();
  const conflict = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.bad-conflict-import",
    mode: "direct",
    name: "Conflict Import",
    bindings: {
      "unit.move": "A",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
    },
  });
  assert.equal(conflict.ok, false, "same-context imported duplicate keys are fatal");
  assert(conflict.errors.some((error) => error.code === "duplicateKey"), "same-context imported duplicates are reported");
}

{
  const hotkeys = service();
  const parsed = hotkeys.parseImportText("{bad json");
  assert.equal(parsed.ok, false, "invalid import JSON is rejected");
  assert(parsed.errors.some((error) => error.code === "importParseFailed"), "invalid import JSON is diagnosed");
}

{
  const hotkeys = service();
  const imported = hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.direct",
    type: "custom",
    mode: "direct",
    name: "Direct",
    bindings: {
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
    },
  }, { activate: true });
  assert.equal(imported.ok, true, "profile import succeeds");
  const direct = hotkeys.resolveCard(workerCard());
  assert.equal(direct.slots[0].hotkey, "M", "direct profiles change command labels");
  assert.equal(direct.slots[0].slotIndex, 0, "direct profiles do not move command slots");

  hotkeys.setActiveProfile(HOTKEY_PRESET_GRID);
  const grid = hotkeys.resolveCard(workerCard());
  assert.equal(grid.slots[0].hotkey, "Q", "Grid follows the rendered slot");
}

{
  const hotkeys = service();
  hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.replace",
    type: "custom",
    mode: "direct",
    name: "Replace",
    bindings: {
      "unit.move": "M",
      "unit.attack": "A",
      "unit.holdPosition": "S",
      "worker.buildMenu": "B",
    },
  });
  hotkeys.importProfile({
    schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
    id: "custom.replace",
    type: "custom",
    mode: "grid",
    name: "Replace",
    bindings: {},
  }, { targetId: "custom.replace" });
  const profile = hotkeys.profileById("custom.replace");
  assert.equal(profile.mode, "grid", "imports replace the target profile payload");
  assert.deepEqual(profile.bindings, {}, "replacement import does not merge old bindings");
}
