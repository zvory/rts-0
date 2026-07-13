// tests/client_contracts/settings_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertHasMethod,
} from "./assertions.mjs";
import {
  findFakeById,
  findFakes,
  memoryStorage,
  withFakeSettingsDocument,
} from "./fakes.mjs";
import { buildCommandCardContextCatalog } from "../../client/src/hud_command_card.js";
import {
  buildBackToLobbyAction,
  buildGiveUpAction,
  buildPauseAction,
  buildSettingsTabs,
} from "../../client/src/settings_panels.js";
import { SettingsContainer } from "../../client/src/settings_container.js";
import {
  readPredictionEnabled,
  writePredictionEnabled,
} from "../../client/src/prediction_settings.js";
import {
  readUnitRangesEnabled,
  writeUnitRangesEnabled,
} from "../../client/src/unit_range_settings.js";
import {
  readAutoSpectatorEnabled,
  writeAutoSpectatorEnabled,
} from "../../client/src/auto_spectator_settings.js";
import {
  HOTKEY_PRESET_CLASSIC,
  HOTKEY_PROFILE_SCHEMA_VERSION,
  HotkeyProfileService,
  buildHotkeyCommandCatalog,
} from "../../client/src/hotkey_profiles.js";

function hotkeyService() {
  return new HotkeyProfileService({
    storage: memoryStorage(),
    catalog: buildHotkeyCommandCatalog(buildCommandCardContextCatalog()),
  });
}

{
  const hotkeys = hotkeyService();
  for (const method of [
    "allProfiles",
    "getActiveProfile",
    "profileById",
    "setActiveProfile",
    "createCustomFromPreset",
    "saveCustomProfile",
    "validateDraftProfile",
    "runtimeDiagnostics",
    "importProfile",
    "exportProfile",
    "exportProfileJson",
    "parseImportText",
    "resolveCard",
    "resolveSlot",
  ]) {
    assertHasMethod(hotkeys, method, "HotkeyProfileService");
  }
  const exported = hotkeys.exportProfile(HOTKEY_PRESET_CLASSIC);
  assert(exported.profileId === HOTKEY_PRESET_CLASSIC, "hotkeys: export uses profileId metadata");
  assert(typeof exported.createdWithBuild === "string", "hotkeys: export includes build metadata");
  const imported = hotkeys.importProfile(exported);
  assert(imported.ok && imported.profile.type === "custom", "hotkeys: imports are stored as custom profiles");
}

// ---------------------------------------------------------------------------
// Unified settings tabs
// ---------------------------------------------------------------------------

{
  const tabs = buildSettingsTabs({ audio: {}, game: { kind: "lobby" } }).filter((tab) => tab.visible !== false);
  assert(tabs.map((tab) => tab.id).join(",") === "game,hotkeys,audio", "settings: lobby shows game, hotkeys, and audio tabs");

  const debugTabs = buildSettingsTabs({
    audio: {},
    game: { kind: "match" },
    debug: { available: true },
  }).filter((tab) => tab.visible !== false);
  assert(debugTabs.map((tab) => tab.id).join(",") === "game,hotkeys,audio,debug", "settings: debug tab is conditional");

  withFakeSettingsDocument(() => {
    let enabled = false;
    const tabs = buildSettingsTabs({
      audio: {},
      game: { kind: "replay" },
      replayControls: {
        state: () => ({ available: true, enabled }),
        onToggle: () => { enabled = !enabled; },
      },
    }).filter((tab) => tab.visible !== false);
    assert(
      tabs.map((tab) => tab.id).join(",") === "game,hotkeys,audio,replay-controls",
      "settings: spectator and replay contexts expose a dedicated Replay Controls tab",
    );
    const root = document.createElement("div");
    tabs.find((tab) => tab.id === "replay-controls").render(root);
    const toggle = root.children[0];
    assert(toggle.textContent === "Enable Auto Spectator: off", "settings: auto spectator defaults off");
    toggle.listeners.click();
    assert(toggle.textContent === "Enable Auto Spectator: on", "settings: Replay Controls toggles auto spectator");
    assert(toggle.getAttribute("aria-checked") === "true", "settings: auto spectator exposes switch state");
  });

  withFakeSettingsDocument(() => {
    let giveUpOpened = false;
    const action = buildGiveUpAction({ visible: true, onOpen: () => { giveUpOpened = true; } });
    const button = action.render();
    assert(button.id === "give-up-open", "settings: live give-up action keeps pinned id");
    button.listeners.click();
    assert(giveUpOpened, "settings: live give-up action calls injected opener");
    assert(buildGiveUpAction({ visible: false, onOpen: () => {} }).render() === null,
      "settings: spectator/replay contexts omit give-up action");
  });

  withFakeSettingsDocument(() => {
    let returned = false;
    const action = buildBackToLobbyAction({ visible: true, onBackToLobby: () => { returned = true; } });
    const button = action.render();
    assert(button.id === "back-to-lobby-open", "settings: back-to-lobby action keeps pinned id");
    assert(button.textContent === "Back to Lobby", "settings: back-to-lobby action uses expected label");
    button.listeners.click();
    assert(returned, "settings: back-to-lobby action calls injected return handler");
    assert(buildBackToLobbyAction({ visible: false, onBackToLobby: () => {} }).render() === null,
      "settings: contexts without lobby return omit back-to-lobby action");
  });

  withFakeSettingsDocument(() => {
    let pauseSent = false;
    const action = buildPauseAction({
      visible: true,
      disabled: false,
      label: "Pause (3)",
      onPause: () => { pauseSent = true; },
    });
    const button = action.render();
    assert(button.id === "live-pause-open", "settings: live pause action keeps pinned id");
    assert(button.textContent === "Pause (3)", "settings: live pause action shows remaining count");
    button.listeners.click();
    assert(pauseSent, "settings: live pause action calls injected sender");
    assert(buildPauseAction({ visible: false }).render() === null,
      "settings: non-live contexts omit pause action");
  });

  withFakeSettingsDocument(() => {
    const shell = document.createElement("div");
    const button = document.createElement("button");
    const menu = document.createElement("div");
    button.parentElement = shell;
    const settings = new SettingsContainer({ button, menu });
    const tabs = buildSettingsTabs({ audio: {}, game: { kind: "match" } });
    const pauseButton = () => menu.children[0]?.children[1]?.children[0] || null;

    settings.setContext({
      kind: "match",
      actions: [buildPauseAction({ visible: true, disabled: true, label: "Pause (0)" })],
      tabs,
    });
    assert(pauseButton()?.disabled, "settings: closed menu can render an initially disabled pause action");

    settings.setContext({
      kind: "match",
      actions: [buildPauseAction({ visible: true, disabled: false, label: "Pause (3)" })],
      tabs,
    });
    settings.open({ focus: false });
    assert(pauseButton()?.textContent === "Pause (3)", "settings: closed context refresh updates pause label before open");
    assert(pauseButton()?.disabled === false, "settings: closed context refresh enables pause before open");
    settings.destroy();
  });

  {
    const values = new Map();
    const storage = {
      getItem(key) {
        return values.has(key) ? values.get(key) : null;
      },
      setItem(key, value) {
        values.set(key, value);
      },
      removeItem(key) {
        values.delete(key);
      },
    };
    assert(readPredictionEnabled(storage), "prediction setting defaults on");
    writePredictionEnabled(false, storage);
    assert(!readPredictionEnabled(storage), "prediction setting persists disabled state");
    writePredictionEnabled(true, storage);
    assert(readPredictionEnabled(storage), "prediction setting clears override when re-enabled");
    assert(readUnitRangesEnabled(storage), "unit range setting defaults on");
    writeUnitRangesEnabled(false, storage);
    assert(!readUnitRangesEnabled(storage), "unit range setting persists disabled state");
    writeUnitRangesEnabled(true, storage);
    assert(readUnitRangesEnabled(storage), "unit range setting clears override when re-enabled");
    assert(!readAutoSpectatorEnabled(storage), "auto spectator defaults off");
    writeAutoSpectatorEnabled(true, storage);
    assert(readAutoSpectatorEnabled(storage), "auto spectator persists enabled state");
    writeAutoSpectatorEnabled(false, storage);
    assert(!readAutoSpectatorEnabled(storage), "auto spectator clears its enabled override");
  }

  withFakeSettingsDocument(() => {
    let predictionToggled = false;
    let unitRangeToggled = false;
    const [gameTab] = buildSettingsTabs({
      game: {
        kind: "match",
        prediction: {
          state: () => ({ enabled: true, active: true, available: true }),
          onToggle: () => { predictionToggled = true; },
        },
        unitRanges: {
          state: () => ({ enabled: true, available: true }),
          onToggle: () => { unitRangeToggled = true; },
        },
      },
    }).filter((tab) => tab.id === "game");
    const root = document.createElement("div");
    gameTab.render(root, {});
    const toggle = findFakeById(root, "prediction-toggle");
    assert(toggle, "settings: game tab renders movement prediction control with pinned id");
    assert(toggle.getAttribute("aria-checked") === "true", "settings: prediction toggle reflects enabled state");
    toggle.listeners.click();
    assert(predictionToggled, "settings: prediction control calls injected toggle");
    const rangeToggle = findFakeById(root, "unit-range-toggle");
    assert(rangeToggle, "settings: game tab renders unit range control with pinned id");
    assert(rangeToggle.textContent === "Show Unit Ranges: on", "settings: unit range toggle uses expected label");
    rangeToggle.listeners.click();
    assert(unitRangeToggled, "settings: unit range control calls injected toggle");
  });

  withFakeSettingsDocument(() => {
    let debugToggled = false;
    const [debugTab] = buildSettingsTabs({
      debug: {
        available: true,
        state: () => ({ available: true, enabled: false }),
        onToggle: () => { debugToggled = true; },
      },
    }).filter((tab) => tab.id === "debug");
    const root = document.createElement("div");
    debugTab.render(root, {});
    const toggle = findFakeById(root, "debug-path-toggle");
    assert(toggle, "settings: debug tab renders movement waypoint control with pinned id");
    toggle.listeners.click();
    assert(debugToggled, "settings: debug waypoint control calls injected toggle");
  });

  withFakeSettingsDocument((windowListeners) => {
    const hotkeys = hotkeyService();
    const hotkeyTab = buildSettingsTabs({ hotkeyProfiles: hotkeys }).find((tab) => tab.id === "hotkeys");
    const root = document.createElement("div");
    const cleanup = hotkeyTab.render(root, { kind: "match" });

    const preview = findFakeById(root, "hotkey-command-card-preview");
    assert(preview, "hotkey editor: renders command-card preview");
    assert(findFakes(preview, (el) => el.tagName === "BUTTON").length > 0,
      "hotkey editor: preview exposes clickable command buttons");

    const clone = findFakeById(root, "hotkey-clone-profile");
    clone.listeners.click();
    const moveButton = findFakes(root, (el) => el.dataset?.commandId === "unit.move")[0];
    assert(moveButton?.dataset.slotIndex === "0", "hotkey editor: command slot stays fixed before rebind");
    moveButton.listeners.click({ preventDefault() {} });
    assert(findFakes(root, (el) => /Press a letter/.test(el.textContent || "")).length > 0,
      "hotkey editor: clicking a command starts key capture");
    windowListeners.keydown({
      key: "1",
      code: "Digit1",
      preventDefault() {},
      stopPropagation() {},
    });
    assert(findFakeById(root, "hotkey-save-profile").disabled,
      "hotkey editor: unsupported keys keep valid save blocked");
    assert(findFakes(root, (el) => /Use a single A-Z letter/.test(el.textContent || "")).length > 0,
      "hotkey editor: unsupported key warning is visible");

    moveButton.listeners.click({ preventDefault() {} });
    windowListeners.keydown({
      key: "M",
      code: "KeyM",
      preventDefault() {},
      stopPropagation() {},
    });
    const reboundMove = findFakes(root, (el) => el.dataset?.commandId === "unit.move")[0];
    assert(reboundMove?.dataset.hotkey === "M", "hotkey editor: valid rebind updates preview label");
    assert(reboundMove?.dataset.slotIndex === "0", "hotkey editor: rebind does not move the command slot");

    const save = findFakeById(root, "hotkey-save-profile");
    assert(!save.disabled, "hotkey editor: valid cloned profile can be saved");
    save.listeners.click();
    assert(hotkeys.getActiveProfile().bindings["unit.move"] === "M",
      "hotkey editor: saved profile applies immediately as the active profile");

    cleanup();
  });

  withFakeSettingsDocument(() => {
    const hotkeys = hotkeyService();
    const hotkeyTab = buildSettingsTabs({ hotkeyProfiles: hotkeys }).find((tab) => tab.id === "hotkeys");
    const root = document.createElement("div");
    hotkeyTab.render(root, {});
    findFakeById(root, "hotkey-new-blank-profile").listeners.click();
    assert(findFakeById(root, "hotkey-save-profile").disabled,
      "hotkey editor: blank direct profiles cannot save with unresolved commands");
    assert(findFakes(root, (el) => /is unbound/.test(el.textContent || "")).length > 0,
      "hotkey editor: unresolved bindings are displayed");
  });

  withFakeSettingsDocument(() => {
    const hotkeys = hotkeyService();
    const classic = hotkeys.profileById(HOTKEY_PRESET_CLASSIC);
    hotkeys.customProfiles = [{
      schemaVersion: HOTKEY_PROFILE_SCHEMA_VERSION,
      id: "custom.conflict-editor",
      type: "custom",
      mode: "direct",
      name: "Conflict Editor",
      description: "",
      basePresetId: HOTKEY_PRESET_CLASSIC,
      bindings: { ...classic.bindings, "unit.move": "A", "unit.attack": "A" },
    }];
    hotkeys.setActiveProfile("custom.conflict-editor");

    const hotkeyTab = buildSettingsTabs({ hotkeyProfiles: hotkeys }).find((tab) => tab.id === "hotkeys");
    const root = document.createElement("div");
    hotkeyTab.render(root, {});
    assert(findFakeById(root, "hotkey-save-profile").disabled,
      "hotkey editor: same-context duplicate keys block save");
    assert(findFakes(root, (el) => /Worker Commands/.test(el.textContent || "") && /Move/.test(el.textContent || "")).length > 0,
      "hotkey editor: conflict messages name affected commands and context");
  });
}
