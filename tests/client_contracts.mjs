// tests/client_contracts.mjs
// Lightweight dependency-free checks that the client modules export the expected
// constructors and pure methods documented in docs/design/client-ui.md §4.1.
//
// This does NOT spin up a browser or a server. Modules that require DOM / Pixi
// (Renderer, Input, HUD, Minimap, Lobby) are not instantiated here.

import { Net } from "../client/src/net.js";
import { PredictionController, PREDICTION_STATE } from "../client/src/prediction_controller.js";
import { GameState } from "../client/src/state.js";
import { Camera } from "../client/src/camera.js";
import { Fog } from "../client/src/fog.js";
import { MatchHealth } from "../client/src/match_health.js";
import {
  AT_GUN_DEPLOYED_RANGE_TILES,
  AT_GUN_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  ARTILLERY_SHELL_DELAY_TICKS,
  MINING_CC_RANGE_TILES,
  RIFLEMAN_CHARGE_COOLDOWN_TICKS,
  SMOKE_ABILITY_COST,
  ABILITIES,
  STATS,
  UPGRADES,
} from "../client/src/config.js";
import {
  HUD,
  formatTankOilUsed,
  groupCooldownClocks,
  playerHasCompletedKind,
} from "../client/src/hud.js";
import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  duplicateCommandIdsForCard,
} from "../client/src/hud_command_card.js";
import { Audio, noticeSoundId } from "../client/src/audio.js";
import {
  attackKindHasCombatSound,
  machineGunnerHasAudibleTarget,
} from "../client/src/combat_audio.js";
import {
  COMPACT_SNAPSHOT_VERSION,
  PREDICTION_PROTOCOL_VERSION,
  ABILITY,
  ABILITY_CODE,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  NOTICE_SEVERITY,
  ORDER_STAGE,
  ORDER_STAGE_CODE,
  SETUP,
  SETUP_CODE,
  STATE,
  STATE_CODE,
  TERRAIN,
  UPGRADE,
  UPGRADE_CODE,
  cmd,
  decodeServerMessage,
  msg,
} from "../client/src/protocol.js";
import { Input, footprintValidAgainstEntities } from "../client/src/input/index.js";
import { CommandComposer } from "../client/src/command_composer.js";
import { _controlGroupSaveModifierActive } from "../client/src/input/control_groups.js";
import { ReplayCameraInput } from "../client/src/replay_camera_input.js";
import {
  automaticPointerLockDisabledForTests,
  cursorLockSupported,
  enterCursorLock,
  exitCursorLock,
  installedAppRuntime,
  shouldRequestPointerLock,
} from "../client/src/input/cursor_lock.js";
import { DomClickInputZone, MatchInputRouter } from "../client/src/input/router.js";
import { _drawUnit, _tankMotionVisual } from "../client/src/renderer/units.js";
import { _drawAbilityTargetPreview } from "../client/src/renderer/feedback.js";
import { buildGiveUpAction, buildSettingsTabs } from "../client/src/settings_panels.js";
import { readPredictionEnabled, writePredictionEnabled } from "../client/src/prediction_settings.js";
import {
  HOTKEY_PRESET_CLASSIC,
  HOTKEY_PROFILE_SCHEMA_VERSION,
  HotkeyProfileService,
  buildHotkeyCommandCatalog,
} from "../client/src/hotkey_profiles.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function assertApprox(actual, expected, epsilon, msg) {
  assert(
    Math.abs(actual - expected) <= epsilon,
    `${msg}: expected ${expected}, got ${actual}`,
  );
}

function assertThrows(fn, msg) {
  let threw = false;
  try {
    fn();
  } catch (err) {
    threw = true;
  }
  assert(threw, msg);
}

function assertHasMethod(obj, name, msgPrefix = "") {
  assert(
    typeof obj[name] === "function",
    `${msgPrefix || "Object"} missing method "${name}"`,
  );
}

function assertHasGetter(obj, name, msgPrefix = "") {
  const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(obj) || obj, name);
  assert(
    d && typeof d.get === "function",
    `${msgPrefix || "Object"} missing getter "${name}"`,
  );
}

function commandCardCtx({
  selection = [],
  entities = selection,
  resources = { steel: 1000, oil: 1000 },
  upgrades = [],
  playerId = 1,
  commandCardMode = null,
  commandTarget = null,
  spectator = false,
} = {}) {
  return {
    spectator,
    playerId,
    selection,
    resources,
    upgrades,
    commandCardMode,
    commandTarget,
    groupCooldownClocks,
    playerHasCompleteKind: (kind) => playerHasCompletedKind(entities, playerId, kind),
  };
}

function commandButtons(card) {
  return card.slots.filter(Boolean);
}

function buttonByAction(card, action) {
  return commandButtons(card).find((button) => button.action === action);
}

function buttonByLabel(card, label) {
  return commandButtons(card).find((button) => button.label === label);
}

function withFakeDocument(fn) {
  const priorDocument = globalThis.document;
  const created = [];
  globalThis.document = {
    createElement(tagName) {
      const el = {
        tagName: String(tagName).toUpperCase(),
        className: "",
        dataset: {},
        disabled: false,
        title: "",
        type: "",
        innerHTML: "",
        listeners: {},
        style: { setProperty() {} },
        addEventListener(type, handler) {
          this.listeners[type] = handler;
        },
        querySelectorAll() {
          return [];
        },
      };
      created.push(el);
      return el;
    },
    createDocumentFragment() {
      return { children: [], appendChild(child) { this.children.push(child); } };
    },
  };
  try {
    return fn(created);
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  }
}

function fakeClassList() {
  const values = new Set();
  return {
    add(value) { values.add(value); },
    remove(value) { values.delete(value); },
    contains(value) { return values.has(value); },
    toggle(value, enabled) {
      if (enabled) values.add(value);
      else values.delete(value);
    },
  };
}

function fakeHudRootWithoutResourceSpans() {
  const ids = new Map();
  const hud = {
    _html: "",
    get innerHTML() {
      return this._html;
    },
    set innerHTML(value) {
      this._html = String(value);
      ids.clear();
      for (const id of ["res-steel", "res-oil", "res-supply"]) {
        if (this._html.includes(`id="${id}"`)) {
          ids.set(id, { id, textContent: "", classList: fakeClassList() });
        }
      }
    },
    querySelector(selector) {
      if (selector.startsWith("#")) return ids.get(selector.slice(1)) || null;
      return null;
    },
  };
  return {
    ids,
    root: {
      querySelector(selector) {
        if (selector === "#hud") return hud;
        return hud.querySelector(selector);
      },
    },
  };
}

function withFakeSettingsDocument(fn) {
  const priorDocument = globalThis.document;
  const priorHTMLElement = globalThis.HTMLElement;
  const priorWindow = globalThis.window;
  const windowListeners = {};
  class FakeElement {
    constructor(tagName) {
      this.tagName = String(tagName).toUpperCase();
      this.id = "";
      this.type = "";
      this.className = "";
      this.textContent = "";
      this.innerHTML = "";
      this.hidden = false;
      this.disabled = false;
      this.value = "";
      this.dataset = {};
      this.children = [];
      this.attributes = new Map();
      this.listeners = {};
      this.classList = {
        add: (value) => {
          this.className = this.className ? `${this.className} ${value}` : value;
        },
      };
    }
    append(...children) {
      this.children.push(...children);
    }
    appendChild(child) {
      this.children.push(child);
      return child;
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) || null;
    }
    addEventListener(type, handler) {
      this.listeners[type] = handler;
    }
    replaceChildren(...children) {
      this.children = [...children];
    }
    click(init = {}) {
      this.listeners.click?.({ preventDefault() {}, ...init });
    }
  }
  globalThis.HTMLElement = FakeElement;
  globalThis.document = {
    createElement(tagName) {
      return new FakeElement(tagName);
    },
  };
  globalThis.window = {
    addEventListener(type, handler) {
      windowListeners[type] = handler;
    },
    removeEventListener(type, handler) {
      if (windowListeners[type] === handler) delete windowListeners[type];
    },
    listeners: windowListeners,
  };
  try {
    return fn(windowListeners);
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorHTMLElement === undefined) delete globalThis.HTMLElement;
    else globalThis.HTMLElement = priorHTMLElement;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

function findFakeById(root, id) {
  if (root.id === id) return root;
  for (const child of root.children || []) {
    const found = findFakeById(child, id);
    if (found) return found;
  }
  return null;
}

function findFakes(root, predicate, out = []) {
  if (predicate(root)) out.push(root);
  for (const child of root.children || []) findFakes(child, predicate, out);
  return out;
}

function memoryStorage(seed = {}) {
  const data = new Map(Object.entries(seed));
  return {
    getItem(key) {
      return data.has(key) ? data.get(key) : null;
    },
    setItem(key, value) {
      data.set(key, String(value));
    },
    data,
  };
}

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
// HUD resource bar
// ---------------------------------------------------------------------------
{
  const { root, ids } = fakeHudRootWithoutResourceSpans();
  const state = {
    resources: { steel: 325, oil: 80, supplyUsed: 7, supplyCap: 14 },
    playerResources: [],
  };
  const hud = new HUD(root, state, {}, null);
  assert(ids.has("res-steel"), "HUD constructor restores steel span after replay resource rows");
  assert(ids.has("res-oil"), "HUD constructor restores oil span after replay resource rows");
  assert(ids.has("res-supply"), "HUD constructor restores supply span after replay resource rows");
  hud._renderSinglePlayerResources();
  assert(ids.get("res-steel").textContent === "325", "restored HUD steel span updates from live resources");
  assert(ids.get("res-oil").textContent === "80", "restored HUD oil span updates from live resources");
  assert(ids.get("res-supply").textContent === "7 / 14", "restored HUD supply span updates from live supply");
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
    let giveUpOpened = false;
    const action = buildGiveUpAction({ visible: true, onOpen: () => { giveUpOpened = true; } });
    const button = action.render();
    assert(button.id === "give-up-open", "settings: live give-up action keeps pinned id");
    button.listeners.click();
    assert(giveUpOpened, "settings: live give-up action calls injected opener");
    assert(buildGiveUpAction({ visible: false, onOpen: () => {} }).render() === null,
      "settings: spectator/replay contexts omit give-up action");
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
  }

  withFakeSettingsDocument(() => {
    let predictionToggled = false;
    const [gameTab] = buildSettingsTabs({
      game: {
        kind: "match",
        prediction: {
          state: () => ({ enabled: true, active: true, available: true }),
          onToggle: () => { predictionToggled = true; },
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

// ---------------------------------------------------------------------------
// Command card descriptors
// ---------------------------------------------------------------------------

{
  const spectatorCard = buildCommandCardDescriptors(commandCardCtx({ spectator: true }));
  assert(spectatorCard.kind === "spectator", "spectator command card should be hidden");
  assert(spectatorCard.slots.length === 0, "spectator command card should emit no slots");

  const worker = { id: 10, owner: 1, kind: KIND.WORKER };
  const cityCentre = { id: 11, owner: 1, kind: KIND.CITY_CENTRE };
  const buildCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [worker],
    entities: [worker, cityCentre],
    commandCardMode: "workerBuild",
    resources: { steel: 175, oil: 0 },
  }));
  assert(buildCard.kind === "workerBuild", "worker build menu should use build descriptor card");
  assert(buildCard.slots.length === 9, "worker build card keeps a 3x3 grid");
  assert(buildCard.slots[0].intent.type === "beginPlacement", "worker build button should start placement");
  assert(buildCard.slots[0].commandId === `build.${KIND.CITY_CENTRE}`, "worker build button should expose stable command identity");
  assert(buildCard.slots[0].slotIndex === 0, "worker build button should expose rendered slot index");
  assert(buildCard.slots[0].label === "City Centre", "worker build first slot should stay City Centre");
  assert(buildCard.slots[0].hotkey === "Q", "worker build hotkey Q should be preserved");
  assert(buildCard.slots[0].unaffordable, "unaffordable build buttons stay clickable for feedback");
  assert(buildCard.slots[3].label === "Training Centre", "worker build menu should include Training Centre");
  assert(!buildCard.slots[3].enabled, "locked build buttons should be disabled");
  assert(buildCard.slots[3].title === "Requires Barracks", "locked build tooltip should explain requirement");
  assert(buildCard.slots[8].intent.type === "closeCommandCardMenu", "worker return button should close submenu");
  assert(buildCard.slots[8].commandId === "worker.return", "worker return should expose stable command identity");

  const barracks = { id: 20, owner: 1, kind: KIND.BARRACKS, buildProgress: null };
  const producingBarracks = {
    id: 21,
    owner: 1,
    kind: KIND.BARRACKS,
    buildProgress: null,
    state: STATE.TRAIN,
    prodQueue: 1,
  };
  const trainCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [barracks, producingBarracks],
    entities: [cityCentre, barracks, producingBarracks],
    resources: { steel: 60, oil: 0 },
  }));
  assert(trainCard.kind === "train", "production building should use train descriptor card");
  assert(trainCard.slots[0].label === "Rifleman", "Barracks first train slot should be Rifleman");
  assert(trainCard.slots[0].commandId === `train.${KIND.RIFLEMAN}`, "train button should expose stable train identity");
  assert(trainCard.slots[0].slotIndex === 0, "train button should expose rendered slot index");
  assert(trainCard.slots[0].repeatable, "train hotkeys should remain repeatable");
  assert(trainCard.slots[0].intent.type === "train", "train button should carry train intent");
  assert(trainCard.slots[8].label === "Cancel", "production cancel should stay in C slot");
  assert(trainCard.slots[8].hotkey === "C", "cancel hotkey should stay C");
  assert(trainCard.slots[8].repeatable, "cancel hotkey should remain repeatable");
  assert(trainCard.slots[8].commandId === `production.cancel.${KIND.BARRACKS}`, "cancel button should expose stable production cancel identity");
  assert(trainCard.slots[1].label === "Machine Gunner", "Barracks second train slot should be Machine Gunner");
  assert(!trainCard.slots[1].enabled, "requirement-gated train button should be disabled");
  assert(trainCard.slots[1].title === "Requires Training Centre", "train locked tooltip should name requirement");

  const scoutCar = {
    id: 30,
    owner: 1,
    kind: KIND.SCOUT_CAR,
    abilities: [{ ability: ABILITY.SMOKE, cooldownLeft: 0, remainingUses: 2 }],
  };
  const abilityCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [scoutCar],
    commandTarget: { kind: "ability", ability: ABILITY.SMOKE },
  }));
  const smoke = buttonByAction(abilityCard, "ability");
  assert(smoke.label === "Smoke", "ability button should expose ability label");
  assert(smoke.commandId === `ability.${ABILITY.SMOKE}`, "ability button should expose stable ability identity");
  assert(smoke.slotIndex === 5, "ability button should expose rendered slot index");
  assert(smoke.hotkey === "D", "ability preferred hotkey should be preserved");
  assert(smoke.intent.targetMode === "worldPoint", "world-point ability should carry targeting intent");
  assert(smoke.cls.includes("active"), "active ability targeting should keep active class");
  assert(smoke.enabled, "ready affordable ability should be enabled");

  const artillery = {
    id: 31,
    owner: 1,
    kind: KIND.ARTILLERY,
    setupState: SETUP.DEPLOYED,
    abilities: [{ ability: ABILITY.POINT_FIRE, cooldownLeft: 0, remainingUses: null }],
  };
  const pointFireCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [artillery],
    entities: [{ id: 40, owner: 1, kind: KIND.STEELWORKS }, artillery],
    resources: { steel: 0, oil: 0 },
  }));
  const pointFire = buttonByLabel(pointFireCard, "Point Fire");
  assert(pointFire.unaffordable, "unaffordable ability should stay clickable");
  assert(pointFire.onUnavailableIntent.type === "playNotEnough", "unaffordable ability should play resource notice");

  const packedArtillery = {
    ...artillery,
    id: 32,
    setupState: SETUP.PACKED,
  };
  const packedPointFireCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [packedArtillery],
    entities: [{ id: 40, owner: 1, kind: KIND.STEELWORKS }, packedArtillery],
    resources: { steel: 1000, oil: 1000 },
  }));
  const packedPointFire = buttonByLabel(packedPointFireCard, "Point Fire");
  assert(!packedPointFire.enabled, "packed artillery Point Fire should be visible but disabled");
  assert(
    packedPointFire.title === "Set up artillery before using Point Fire",
    "packed artillery Point Fire should explain the setup requirement",
  );
  const redeployingArtillery = {
    ...artillery,
    id: 33,
    setupState: SETUP.TEARING_DOWN,
    orderPlan: [{ kind: ORDER_STAGE.POINT_FIRE, x: 720, y: 360 }],
  };
  const redeployingPointFireCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [redeployingArtillery],
    entities: [{ id: 40, owner: 1, kind: KIND.STEELWORKS }, redeployingArtillery],
    resources: { steel: 1000, oil: 1000 },
  }));
  const redeployingPointFire = buttonByLabel(redeployingPointFireCard, "Point Fire");
  assert(
    redeployingPointFire.enabled,
    "artillery already redeploying for Point Fire should allow retargeting",
  );

  const steelworks = { id: 50, owner: 1, kind: KIND.STEELWORKS, buildProgress: null };
  const upgradeCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [steelworks],
    entities: [
      { id: 51, owner: 1, kind: KIND.CITY_CENTRE },
      { id: 52, owner: 1, kind: KIND.TRAINING_CENTRE },
      steelworks,
    ],
    resources: { steel: 125, oil: 125 },
  }));
  const atGun = buttonByLabel(upgradeCard, "AT Gun");
  assert(atGun && !atGun.enabled, "upgrade-gated unit should be disabled before research");
  assert(atGun.title === "Requires research in R&D Complex", "upgrade-gated unit tooltip should name R&D research");
  assert(!buttonByLabel(upgradeCard, "AT Gun Crews"), "Gun Works should not expose R&D research");

  const researchComplex = { id: 53, owner: 1, kind: KIND.RESEARCH_COMPLEX, buildProgress: null };
  const researchCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [researchComplex],
    entities: [
      { id: 51, owner: 1, kind: KIND.CITY_CENTRE },
      { id: 52, owner: 1, kind: KIND.TRAINING_CENTRE },
      researchComplex,
    ],
    resources: { steel: 200, oil: 200 },
  }));
  const atUnlock = buttonByLabel(researchCard, "AT Gun Crews");
  const artilleryUnlock = buttonByLabel(researchCard, "Unlock Artillery");
  assert(atUnlock && atUnlock.enabled, "available affordable upgrade should be enabled");
  assert(atUnlock.commandId === `research.${UPGRADE.AT_GUN_UNLOCK}`, "research button should expose stable research identity");
  assert(atUnlock.intent.type === "research", "upgrade button should carry research intent");
  assert(artilleryUnlock && !artilleryUnlock.enabled, "Artillery research should show disabled before AT Gun research");
  assert(artilleryUnlock.title === "Requires AT Gun Research", "Artillery research should name missing AT prerequisite");

  const catalog = buildCommandCardContextCatalog();
  assert(catalog.some((entry) => entry.id === "worker-build"), "command-card context catalog includes worker build context");
  assert(catalog.every((entry) => duplicateCommandIdsForCard(entry.card).length === 0), "catalog contexts have unique command identities");

  withFakeDocument(() => {
    let clicked = false;
    const button = HUD.prototype._cmdButton({
      icon: "RF",
      label: "Rifleman",
      commandId: "train.rifleman",
      slotIndex: 0,
      hotkey: "Q",
      cost: { steel: 50, oil: 0 },
      enabled: true,
      repeatable: true,
      tooltipHtml: `<span class="cmd-tooltip-title">Rifleman</span>`,
      onClick: () => { clicked = true; },
    });
    assert(button.type === "button", "command button type should remain button");
    assert(button.className === "cmd-btn", "enabled command button class should remain cmd-btn");
    assert(button.dataset.commandId === "train.rifleman", "command button should expose command identity dataset");
    assert(button.dataset.slotIndex === "0", "command button should expose rendered slot dataset");
    assert(button.dataset.hotkey === "Q", "command button should expose hotkey dataset");
    assert(button.dataset.repeatable === "true", "repeatable command button should expose repeatable dataset");
    assert(!button.disabled, "enabled command button should not be disabled");
    assert(button.innerHTML.includes("cmd-icon"), "command button should render icon span");
    assert(button.innerHTML.includes("cmd-label"), "command button should render label span");
    assert(button.innerHTML.includes("cmd-hotkey"), "command button should render hotkey span");
    assert(button.innerHTML.includes("cmd-cost"), "command button should render cost span");
    assert(button.innerHTML.includes("cmd-tooltip"), "command button should render rich tooltip span");
    button.listeners.click({ preventDefault() {} });
    assert(clicked, "enabled command button click should dispatch handler");
  });

  withFakeDocument(() => {
    let unavailable = false;
    const button = HUD.prototype._cmdButton({
      icon: "TK",
      label: "Tank",
      hotkey: "W",
      enabled: false,
      unaffordable: true,
      title: "Not enough resources",
      onUnavailable: () => { unavailable = true; },
    });
    assert(button.className === "cmd-btn unaffordable", "unaffordable command class should be preserved");
    assert(!button.disabled, "unaffordable command should stay clickable");
    assert(button.title === "Not enough resources", "command title should preserve disabled reason");
    button.listeners.click({ preventDefault() {} });
    assert(unavailable, "unaffordable command click should dispatch unavailable handler");
  });
}

// ---------------------------------------------------------------------------
// Match health
// ---------------------------------------------------------------------------

{
  const net = { latency: null, latencyUpdatedAt: 0 };
  let badgePayload = null;
  const health = new MatchHealth({
    net,
    statusBadge: { setMatchMetrics(metrics) { badgePayload = metrics; } },
    snapshotMs: 33,
  });

  net.latency = 179;
  net.latencyUpdatedAt = 1;
  health.refreshLatency();
  assert(health.metrics().latencyMs === 179, "MatchHealth records latest latency sample");
  assert(!health.metrics().issues.latency.active, "latency below threshold stays inactive");
  assert(health.metrics().issues.latency.count === 0, "latency below threshold does not count as bad RTT");

  net.latency = 180;
  net.latencyUpdatedAt = 2;
  health.refreshLatency();
  assert(health.metrics().issues.latency.active, "latency at threshold marks active issue");
  assert(health.metrics().issues.latency.count === 1, "latency issue count increments on bad sample");
  assert(health.reportStats.badRttSamples === 1, "bad RTT samples feed net report stats");
  health.refreshLatency();
  assert(health.metrics().issues.latency.count === 1, "unchanged latency timestamp does not double-count");

  health.noteSnapshotArrival(100, false);
  health.noteSnapshotArrival(133, false);
  assert(health.metrics().jitterMs === 0, "on-cadence snapshots report zero jitter");
  health.noteSnapshotArrival(187, false);
  assert(health.metrics().jitterMs === 21, "snapshot jitter records max delta in window");
  assert(health.metrics().issues.jitter.active, "snapshot jitter threshold marks active issue");
  assert(health.metrics().issues.jitter.count === 1, "snapshot jitter issue count increments");
  assert(health.reportStats.jitterSamples === 1, "jitter samples feed net report stats");

  let now = 187;
  for (let i = 0; i < 8; i += 1) {
    now += 34;
    health.noteSnapshotArrival(now, false);
  }
  assert(health.metrics().jitterMs === 1, "snapshot jitter window drops old outlier samples");
  assert(!health.metrics().issues.jitter.active, "jitter active state follows the latest visible delta");
  const jitterBeforeHidden = health.metrics().jitterMs;
  health.noteSnapshotArrival(now + 500, true);
  assert(health.metrics().jitterMs === jitterBeforeHidden, "hidden document snapshots do not update jitter");

  health.applyServerNetStatus({
    tickMs: 44,
    serverLagMs: 120,
    slowTick: true,
    slowTickCount: 3,
    headOfLine: true,
    headOfLineCount: 4,
  });
  assert(health.metrics().serverTickMs === 44, "server tick timing propagates to metrics");
  assert(health.metrics().serverLagMs === 120, "server lag timing propagates to metrics");
  assert(health.metrics().issues.slowTick.active, "slow tick status propagates to issues");
  assert(health.metrics().issues.slowTick.count === 3, "slow tick count propagates to issues");
  assert(health.metrics().issues.headOfLine.active, "head-of-line status propagates to issues");
  assert(health.metrics().issues.headOfLine.count === 4, "head-of-line count propagates to issues");

  health.publish();
  assert(badgePayload !== null, "MatchHealth publishes status badge payload");
  assert(
    Object.keys(badgePayload).join(",") === "latencyMs,serverTickMs,serverLagMs,jitterMs,issues",
    "status badge payload shape stays unchanged",
  );
  assert(
    Object.keys(badgePayload.issues).join(",") === "latency,slowTick,headOfLine,jitter",
    "status badge issue payload shape stays unchanged",
  );
}

async function testDevWatchScenarioConfig() {
  const priorDocument = globalThis.document;
  const priorWindow = globalThis.window;
  globalThis.document = {
    getElementById: () => null,
  };
  globalThis.window = {
    location: new URL(
      "http://localhost/?watchScenario=1&id=vehicle_small_block_baseline&unit=scout_car&count=5",
    ),
    localStorage: { getItem: () => null },
  };
  try {
    const { devWatchConfig } = await import("../client/src/bootstrap.js");
    let config = devWatchConfig();
    assert(config, "vehicle_small_block_baseline dev scenario should be recognized");
    assert(config.kind === "scenario", "dev scenario should set scenario kind");
    assert(
      config.room === "__dev_scenario__:vehicle_small_block_baseline:unit=scout_car:count=5",
      "dev scenario should auto-join the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=vehicle_small_block_baseline&unit=scout_car&count=5&blocker=machine_gunner",
    );
    config = devWatchConfig();
    assert(config, "vehicle_small_block_baseline blocker variant should be recognized");
    assert(
      config.room ===
        "__dev_scenario__:vehicle_small_block_baseline:unit=scout_car:count=5:blocker=machine_gunner",
      "dev scenario should include blocker variants in the server scenario room",
    );

    globalThis.window.location = new URL(
      "http://localhost/?watchScenario=1&id=bad/scenario&unit=scout_car&count=5",
    );
    config = devWatchConfig();
    assert(config === null, "dev scenario parser should reject unsafe scenario ids");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

class FakeGraphics {
  constructor() {
    this.position = { set() {} };
  }
  lineStyle() {}
  beginFill() {}
  endFill() {}
  drawPolygon() {}
  drawCircle() {}
  drawRect() {}
  moveTo() {}
  lineTo() {}
  arc() {}
}

class RecordingGraphics extends FakeGraphics {
  constructor() {
    super();
    this.calls = [];
  }
  lineStyle(width, color, alpha) {
    this.calls.push(["lineStyle", width, color, alpha]);
  }
  moveTo(x, y) {
    this.calls.push(["moveTo", x, y]);
  }
  lineTo(x, y) {
    this.calls.push(["lineTo", x, y]);
  }
}

await testDevWatchScenarioConfig();

assert(noticeSoundId("alert:under_attack") === "notice_under_attack", "under-attack notice has dedicated sound id");
assert(noticeSoundId("Not enough supply") === "notice_supply", "supply notice routes to supply voice line");
assert(noticeSoundId("Build more depots") === "notice_supply", "depot notice routes to supply voice line");
assert(noticeSoundId("Not enough steel") === "notice_steel", "steel notice routes to steel voice line");
assert(noticeSoundId("Not enough oil") === "notice_oil", "oil notice routes to oil voice line");
assert(noticeSoundId("Cannot build there") === "notice_cannot_build", "cannot-build notice routes to cannot-build voice line");
assert(noticeSoundId("Requirement not met") === null, "generic invalid notices stay silent");
assert(noticeSoundId("Unknown unit") === null, "unknown-unit notices stay silent");
assert(noticeSoundId("Not enough resources") === null, "generic resource notices stay silent");

// ---------------------------------------------------------------------------
// Control groups
// ---------------------------------------------------------------------------
{
  const ev = (mods) => ({
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    ...mods,
  });

  assert(
    _controlGroupSaveModifierActive(ev({ altKey: true }), { isWindows: true, isInstalledApp: false }),
    "Windows browser control-group save uses Alt+number",
  );
  assert(
    !_controlGroupSaveModifierActive(ev({ ctrlKey: true }), { isWindows: true, isInstalledApp: false }),
    "Windows browser control-group save does not use Ctrl+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ ctrlKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save uses Ctrl+number",
  );
  assert(
    !_controlGroupSaveModifierActive(ev({ altKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save does not use Alt+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ metaKey: true }), { isWindows: false, isInstalledApp: false }),
    "non-Windows control-group save keeps the existing modifier set",
  );
  assert(
    !_controlGroupSaveModifierActive(
      ev({ altKey: true, ctrlKey: true }),
      { isWindows: true, isInstalledApp: false },
    ),
    "Windows browser control-group save requires a clean Alt modifier",
  );
}

// ---------------------------------------------------------------------------
// Match input router
// ---------------------------------------------------------------------------
{
  const viewport = {
    getBoundingClientRect() {
      return { left: 10, top: 20, right: 810, bottom: 620, width: 800, height: 600 };
    },
  };
  const router = new MatchInputRouter(viewport);
  const calls = [];
  const lowZone = {
    priority: 1,
    contains: () => true,
    pointerDown: () => {
      calls.push("lowDown");
      return true;
    },
  };
  const highZone = {
    priority: 10,
    contains: (ev) => ev.clientX >= 100 && ev.clientX <= 200,
    pointerDown: (ev) => {
      calls.push(["highDown", ev.viewportX, ev.viewportY]);
      return true;
    },
    pointerMove: (ev) => {
      calls.push(["highMove", ev.clientX, ev.clientY]);
      return true;
    },
    pointerUp: () => {
      calls.push("highUp");
      return true;
    },
  };
  router.registerZone(lowZone);
  const unregisterHigh = router.registerZone(highZone);

  assert(router.pointerDown({ clientX: 150, clientY: 70, button: 0, source: "locked" }), "router consumes highest matching zone");
  assert(calls[0][0] === "highDown", "higher priority matching zone receives pointerDown first");
  assert(calls[0][1] === 140 && calls[0][2] === 50, "router computes viewport-local coords");
  assert(!router.pointerMove({ clientX: 500, clientY: 500, source: "dom" }), "capture ignores different event source");
  assert(router.pointerMove({ clientX: 500, clientY: 500, source: "locked" }), "captured zone receives pointerMove outside bounds");
  assert(calls[1][0] === "highMove", "pointerDown capture is retained for moves");
  assert(!router.pointerUp({ clientX: 500, clientY: 500, source: "dom" }), "capture is not released by a different source");
  assert(router.pointerUp({ clientX: 500, clientY: 500, source: "locked" }), "captured zone receives pointerUp");
  assert(calls[2] === "highUp", "pointerUp releases the captured zone");

  unregisterHigh();
  assert(router.pointerDown({ clientX: 150, clientY: 70, button: 0 }), "router falls back after unregister");
  assert(calls.at(-1) === "lowDown", "unregistered zone no longer receives events");
}

{
  const viewport = {
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600 };
    },
  };
  const button = {
    disabled: false,
    clickCount: 0,
    click() {
      this.clickCount += 1;
    },
    dispatchEvent(ev) {
      if (ev.type === "click") this.click();
      return true;
    },
    getAttribute() {
      return null;
    },
    closest() {
      return this;
    },
  };
  const root = {
    hidden: false,
    getBoundingClientRect() {
      return { left: 600, top: 420, right: 780, bottom: 580, width: 180, height: 160 };
    },
    contains(el) {
      return el === this || el === button;
    },
  };
  const doc = {
    elementFromPoint(x, y) {
      return x >= 620 && x <= 700 && y >= 440 && y <= 520 ? button : root;
    },
  };
  const router = new MatchInputRouter(viewport);
  router.registerZone(new DomClickInputZone(root, { documentRef: doc }));

  assert(router.pointerDown({ clientX: 640, clientY: 460, button: 0, source: "locked" }), "DOM zone consumes locked pointerDown over HUD button");
  assert(router.pointerUp({ clientX: 640, clientY: 460, button: 0, source: "locked" }), "DOM zone consumes locked pointerUp over HUD button");
  assert(button.clickCount === 1, "DOM zone forwards locked pointer click to the HUD button");
  assert(router.pointerDown({ clientX: 760, clientY: 560, button: 0, source: "locked" }), "DOM zone consumes empty HUD panel space");
  assert(router.pointerUp({ clientX: 760, clientY: 560, button: 0, source: "locked" }), "empty HUD panel click releases capture");
  assert(button.clickCount === 1, "empty HUD panel space does not click the prior button");
}

// ---------------------------------------------------------------------------
// Pointer lock bridge
// ---------------------------------------------------------------------------
{
  const priorMatchMedia = globalThis.matchMedia;
  const priorNavigatorDescriptor = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  globalThis.matchMedia = (query) => ({ matches: query === "(display-mode: standalone)" });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { standalone: false },
  });
  assert(installedAppRuntime(), "standalone display mode marks an installed app runtime");
  globalThis.matchMedia = () => ({ matches: false });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { standalone: false },
  });
  assert(!installedAppRuntime(), "regular browser tabs are not installed app runtimes");
  if (priorMatchMedia === undefined) delete globalThis.matchMedia;
  else globalThis.matchMedia = priorMatchMedia;
  if (priorNavigatorDescriptor) Object.defineProperty(globalThis, "navigator", priorNavigatorDescriptor);
  else delete globalThis.navigator;

  assert(cursorLockSupported(true), "browser pointer lock keeps cursor lock available");
  let browserFallbackCalled = 0;
  const mode = await enterCursorLock(
    async () => {
      browserFallbackCalled += 1;
      return true;
    },
    { x: 42, y: 64 },
  );
  assert(mode === "browser", "cursor lock uses browser Pointer Lock");
  assert(browserFallbackCalled === 1, "browser Pointer Lock is invoked once");

  let browserExitCalled = false;
  await exitCursorLock("browser", () => {
    browserExitCalled = true;
  });
  assert(browserExitCalled, "cursor lock exits through browser Pointer Lock");

  const priorDocument = globalThis.document;
  const prefixedDom = {
    webkitRequestPointerLock() {},
  };
  let webkitExitCalled = false;
  globalThis.document = {
    webkitPointerLockElement: prefixedDom,
    webkitExitPointerLock() {
      webkitExitCalled = true;
    },
  };
  const prefixedInput = Object.create(Input.prototype);
  prefixedInput.dom = prefixedDom;
  assert(prefixedInput._browserPointerLockSupported(), "WebKit-prefixed Pointer Lock is supported");
  assert(prefixedInput._browserPointerLockElement() === prefixedDom, "WebKit-prefixed lock element is detected");
  prefixedInput._exitBrowserPointerLock();
  assert(webkitExitCalled, "WebKit-prefixed Pointer Lock exit is called");
  globalThis.document = priorDocument;
}

{
  const viewport = { requestPointerLock() {} };
  const canvas = { requestPointerLock() {} };
  const canvasInput = Object.create(Input.prototype);
  canvasInput.dom = viewport;
  canvasInput.renderer = { app: { view: canvas } };
  assert(canvasInput._pointerLockTarget() === canvas, "Pointer Lock prefers the Pixi canvas target");
}

{
  let focused = false;
  let windowFocused = false;
  const priorWindow = globalThis.window;
  globalThis.window = {
    focus() {
      windowFocused = true;
    },
  };
  const focusInput = Object.create(Input.prototype);
  focusInput.dom = {
    clientWidth: 100,
    clientHeight: 80,
    focus(opts) {
      focused = !!opts?.preventScroll;
    },
  };
  focusInput.mouse = null;
  focusInput._setPointerLockCursor = () => {};
  focusInput._prepareCursorLock();
  assert(windowFocused, "Pointer Lock preparation asks the window to focus before requesting lock");
  assert(focused, "Pointer Lock preparation focuses the viewport before requesting lock");
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  let timeoutCallback = null;
  globalThis.window = {
    setTimeout(fn) {
      timeoutCallback = fn;
      return 1;
    },
  };
  globalThis.document = {
    hasFocus() { return true; },
    activeElement: { tagName: "DIV", id: "viewport", className: "" },
  };
  const pendingInput = Object.create(Input.prototype);
  pendingInput.dom = {};
  pendingInput._pointerLockAttempt = 3;
  pendingInput._lastPointerLockRequest = { attempt: 3, outcome: "pending" };
  pendingInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  pendingInput._browserPointerLockElement = () => null;
  const pending = pendingInput._waitForPointerLockPromise(new Promise(() => {}));
  assert(typeof timeoutCallback === "function", "promise Pointer Lock requests install a timeout");
  timeoutCallback();
  assert((await pending) === false, "pending Pointer Lock promise resolves false on timeout");
  assert(pendingInput._lastPointerLockRequest.outcome === "timeout", "pending Pointer Lock timeout is recorded");
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}

{
  let locked = false;
  const requests = [];
  const target = {};
  const rawOnlyInput = Object.create(Input.prototype);
  rawOnlyInput._pointerLockAttempt = 4;
  rawOnlyInput._browserPointerLockSupported = () => true;
  rawOnlyInput._browserPointerLockElement = () => locked ? target : null;
  rawOnlyInput._pointerLockTarget = () => target;
  rawOnlyInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  rawOnlyInput._browserRequestPointerLock = () => (options) => {
    requests.push(options);
    if (options?.unadjustedMovement) return Promise.reject(new Error("raw input unavailable"));
    locked = true;
    return Promise.resolve();
  };
  rawOnlyInput._waitForPointerLockPromise = async (promise) => {
    try {
      await promise;
      rawOnlyInput._finishPointerLockRequest("resolved");
      return rawOnlyInput._browserPointerLockElement() === target;
    } catch (err) {
      rawOnlyInput._finishPointerLockRequest("rejected", err);
      return false;
    }
  };
  assert(!(await rawOnlyInput._requestBrowserPointerLock()), "Pointer Lock fails closed after raw input rejection");
  assert(requests.length === 1, "Pointer Lock does not request plain fallback after raw rejection");
  assert(requests[0]?.unadjustedMovement === true, "first Pointer Lock request asks for unadjusted movement");
  assert(rawOnlyInput._lastPointerLockRequest.rawInputRequested === true, "raw rejection records the raw request");
  assert(rawOnlyInput._lastPointerLockRequest.outcome === "rejected", "raw rejection outcome is recorded");
}

{
  const rawSuccessRequests = [];
  const target = {};
  const rawSuccessInput = Object.create(Input.prototype);
  rawSuccessInput._pointerLockAttempt = 5;
  rawSuccessInput._browserPointerLockSupported = () => true;
  rawSuccessInput._browserPointerLockElement = () => target;
  rawSuccessInput._pointerLockTarget = () => target;
  rawSuccessInput._focusDebugState = () => ({ documentHasFocus: true, activeElement: null });
  rawSuccessInput._browserRequestPointerLock = () => (options) => {
    rawSuccessRequests.push(options);
    return Promise.resolve();
  };
  rawSuccessInput._waitForPointerLockPromise = async (promise) => {
    await promise;
    rawSuccessInput._finishPointerLockRequest("resolved");
    return true;
  };
  assert(await rawSuccessInput._requestBrowserPointerLock(), "Pointer Lock succeeds with raw input");
  assert(rawSuccessRequests.length === 1, "raw Pointer Lock success does not make a fallback request");
  assert(rawSuccessInput._lastPointerLockRequest.rawInputRequested === true, "raw request is recorded for diagnostics");
}

{
  const quietMoveInput = Object.create(Input.prototype);
  let routedMoves = 0;
  let previewRefreshes = 0;
  quietMoveInput.pointerLocked = true;
  quietMoveInput._panDrag = null;
  quietMoveInput._drag = null;
  quietMoveInput._lockedMovementDelta = () => ({ x: 0, y: 0 });
  quietMoveInput._routeLockedPointerMove = () => {
    routedMoves += 1;
    return false;
  };
  quietMoveInput._refreshResourceMiningPreview = () => {
    previewRefreshes += 1;
  };
  quietMoveInput._handleMouseMove({});
  assert(routedMoves === 0 && previewRefreshes === 0, "zero-delta locked mousemove does no hover work");
}

{
  let previewRefreshes = 0;
  const painted = { style: {} };
  const lockedMoveInput = Object.create(Input.prototype);
  lockedMoveInput.pointerLocked = true;
  lockedMoveInput.mouse = { x: 10, y: 20 };
  lockedMoveInput.dom = { clientWidth: 100, clientHeight: 100 };
  lockedMoveInput._panDrag = null;
  lockedMoveInput._drag = null;
  lockedMoveInput._pointerLockCursor = painted;
  lockedMoveInput._pendingPointerLockCursor = null;
  lockedMoveInput._routeLockedPointerMove = () => false;
  lockedMoveInput._refreshResourceMiningPreview = () => {
    previewRefreshes += 1;
  };
  lockedMoveInput._handleMouseMove({ movementX: 3, movementY: -4 });
  assert(lockedMoveInput.mouse.x === 13 && lockedMoveInput.mouse.y === 16, "locked mousemove updates virtual cursor state");
  assert(previewRefreshes === 0, "nonzero locked mousemove defers hover work to frame update");
  assert(painted.style.transform === undefined, "locked mousemove defers virtual cursor paint");
  lockedMoveInput._flushPointerLockCursor();
  assert(painted.style.transform === "translate(13px, 16px)", "virtual cursor paint flushes once per frame");
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const fallbackElement = {
    contains() { return true; },
    addEventListener() {},
    removeEventListener() {},
    setAttribute() {},
    querySelectorAll() { return []; },
    hidden: false,
    disabled: false,
    textContent: "",
    title: "",
  };
  globalThis.window = {
    location: { protocol: "http:", host: "localhost", search: "" },
    localStorage: { getItem() { return null; } },
    setTimeout(fn) {
      fn();
      return 1;
    },
  };
  globalThis.document = {
    hidden: false,
    hasFocus() { return true; },
    getElementById() { return fallbackElement; },
    createElement() { return { classList: { add() {} }, appendChild() {}, style: {} }; },
  };
  const { Match } = await import("../client/src/match.js");
  const { ReplayViewer } = await import("../client/src/replay_viewer.js");
  const { ReplayControls } = await import("../client/src/replay_controls.js");
  const { shouldWarnBeforeUnload } = await import("../client/src/app.js");
  const { dom } = await import("../client/src/bootstrap.js");
  assert(ReplayViewer.prototype instanceof Match, "ReplayViewer reuses Match rendering lifecycle");
  assert(!("command" in ReplayCameraInput.prototype), "Replay camera input has no gameplay command API");
  assert(!shouldWarnBeforeUnload(), "lobby state does not warn before unload");
  assert(shouldWarnBeforeUnload({ match: {} }), "live match warns before unload");
  assert(shouldWarnBeforeUnload({ inReplayPlayback: true }), "replay playback warns before unload");
  assert(
    !shouldWarnBeforeUnload({ match: {}, allowUnloadWithoutWarning: true }),
    "intentional app navigation bypasses unload warning",
  );

  function fakeEl(tag = "div") {
    const el = {
      tagName: tag.toUpperCase(),
      children: [],
      dataset: {},
      style: { setProperty(name, value) { this[name] = value; } },
      hidden: false,
      textContent: "",
      className: "",
      _listeners: new Map(),
      classList: {
        add(cls) {
          if (!el.className.split(/\s+/).includes(cls)) el.className = `${el.className} ${cls}`.trim();
        },
        remove(cls) {
          el.className = el.className.split(/\s+/).filter((c) => c && c !== cls).join(" ");
        },
        toggle(cls, force) {
          const active = force === undefined ? !this.contains(cls) : !!force;
          if (active) this.add(cls);
          else this.remove(cls);
          return active;
        },
        contains(cls) {
          return el.className.split(/\s+/).includes(cls);
        },
      },
      setAttribute(name, value) {
        this[name] = value;
      },
      appendChild(child) {
        child.parentNode = this;
        this.children.push(child);
        return child;
      },
      replaceChildren(...children) {
        this.children = [];
        for (const child of children) this.appendChild(child);
      },
      addEventListener(type, handler) {
        this._listeners.set(type, handler);
      },
      removeEventListener(type, handler) {
        if (this._listeners.get(type) === handler) this._listeners.delete(type);
      },
      dispatchEvent(ev) {
        this._listeners.get(ev.type)?.(ev);
      },
      remove() {
        if (!this.parentNode) return;
        this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
        this.parentNode = null;
      },
      closest(selector) {
        if (selector.startsWith(".") && this.classList.contains(selector.slice(1))) return this;
        return this.parentNode?.closest?.(selector) || null;
      },
      getBoundingClientRect() {
        return { left: 0, width: 200 };
      },
      querySelector(selector) {
        return this.querySelectorAll(selector)[0] || null;
      },
      querySelectorAll(selector) {
        const out = [];
        const matches = (node) => {
          if (selector.includes(",")) {
            return selector.split(",").some((part) => {
              const trimmed = part.trim();
              return trimmed.startsWith(".") && node.classList?.contains(trimmed.slice(1));
            });
          }
          if (selector === ".spd-btn:not(.seek-btn)") {
            return node.classList?.contains("spd-btn") && !node.classList?.contains("seek-btn");
          }
          if (selector.startsWith("#")) return node.id === selector.slice(1);
          if (selector.startsWith(".")) return node.classList?.contains(selector.slice(1));
          return false;
        };
        const walk = (node) => {
          for (const child of node.children || []) {
            if (matches(child)) out.push(child);
            walk(child);
          }
        };
        walk(this);
        return out;
      },
    };
    return el;
  }

  globalThis.document.createElement = fakeEl;
  const replayControls = fakeEl("div");
  const speed2 = fakeEl("button");
  speed2.className = "spd-btn";
  speed2.dataset.speed = "2";
  const speed0 = fakeEl("button");
  speed0.className = "spd-btn dev-pause-btn";
  speed0.dataset.speed = "0";
  const seekBack = fakeEl("button");
  seekBack.className = "spd-btn seek-btn";
  seekBack.dataset.seekBack = "90";
  const stepDev = fakeEl("button");
  stepDev.className = "spd-btn dev-step-btn";
  stepDev.dataset.stepDevTick = "";
  const concluded = fakeEl("span");
  concluded.id = "replay-concluded";
  replayControls.appendChild(speed2);
  replayControls.appendChild(speed0);
  replayControls.appendChild(seekBack);
  replayControls.appendChild(stepDev);
  replayControls.appendChild(concluded);
  dom.replaySpeed = replayControls;
  const replayNet = {
    speeds: [],
    seekBacks: [],
    seekTargets: [],
    visions: [],
    branches: 0,
    steps: 0,
    setReplaySpeed(speed) {
      this.speeds.push(speed);
    },
    seekReplay(ticksBack) {
      this.seekBacks.push(ticksBack);
    },
    seekReplayTo(tick) {
      this.seekTargets.push(tick);
    },
    setReplayVision(vision) {
      this.visions.push(vision);
    },
    requestReplayBranch() {
      this.branches += 1;
    },
    stepDevTick() {
      this.steps += 1;
    },
  };
  const replayState = {
    players: [
      { id: 1, name: "Alpha", color: "#f00" },
      { id: 2, name: "Bravo", color: "#0f0" },
    ],
  };
  const replayUi = new ReplayControls({
    net: replayNet,
    state: replayState,
    replayViewer: true,
    isReplay: true,
    isScenario: false,
  });
  assert(speed2.classList.contains("active"), "replay speed defaults can mark 2x active");
  assert(replayControls.classList.contains("replay-viewer-controls"), "replay viewer controls keep wrapper class");
  assert(!seekBack.hidden, "replay seek buttons stay visible in replay mode");
  assert(stepDev.hidden, "scenario step controls stay hidden in replay mode");
  const pauseReplay = replayControls.querySelector(".replay-pause-btn");
  assert(pauseReplay?.textContent === "Pause", "replay viewer builds a pause button");
  const branchReplay = replayControls.querySelector(".replay-branch-btn");
  assert(branchReplay?.textContent === "Resume play from here", "replay branch button describes resuming from the current tick");
  replayControls._listeners.get("click")({ target: speed2 });
  assert(replayNet.speeds.at(-1) === 2, "speed click sends net.setReplaySpeed");
  replayUi.applyReplayState({ currentTick: 120, durationTicks: 1_000, speed: 2 });
  replayControls._listeners.get("click")({ target: pauseReplay });
  assert(replayNet.speeds.at(-1) === 0, "replay pause button sends zero playback speed");
  assert(pauseReplay.textContent === "Resume", "paused replay button switches to resume");
  replayControls._listeners.get("click")({ target: pauseReplay });
  assert(replayNet.speeds.at(-1) === 2, "replay resume button restores the last non-zero speed");
  assert(pauseReplay.textContent === "Pause", "resumed replay button switches back to pause");
  replayControls._listeners.get("click")({ target: seekBack });
  assert(replayNet.seekBacks.at(-1) === 90, "seek click sends net.seekReplay");
  const visionButtons = replayControls.querySelectorAll(".vision-btn");
  assert(visionButtons.length === 3, "replay viewer builds all-player and per-player fog controls");
  replayUi.onReplayVisionClick({ target: visionButtons[1], shiftKey: false });
  assert(
    replayNet.visions.at(-1).mode === "player" &&
      replayNet.visions.at(-1).playerId === 1,
    "single replay fog click sends a per-viewer player vision request",
  );
  replayUi.onReplayVisionClick({ target: visionButtons[2], shiftKey: true });
  assert(
    replayNet.visions.at(-1).mode === "players" &&
      replayNet.visions.at(-1).playerIds.join(",") === "1,2",
    "shift-click replay fog controls send a selected-players request",
  );
  replayUi.onReplayVisionClick({ target: visionButtons[0], shiftKey: false });
  assert(replayNet.visions.at(-1).mode === "all", "all replay fog control restores union vision");
  replayUi.applyReplayState({
    currentTick: 100,
    durationTicks: 1_000,
    keyframeTicks: [0, 400, 800],
    speed: 2,
    paused: false,
    ended: false,
  });
  assert(
    replayControls.querySelectorAll(".replay-timeline-mark").length === 3,
    "replay timeline renders server keyframe marks",
  );
  const timelineTrack = replayControls.querySelector(".replay-timeline-track");
  replayUi.onReplayTimelineClick({ currentTarget: timelineTrack, clientX: 100 });
  assert(replayNet.seekTargets.at(-1) === 500, "replay timeline click seeks to the clicked tick");
  assert(
    replayControls.querySelector(".replay-tick-status").textContent.includes("Seeking 500"),
    "replay timeline shows a pending seek indicator",
  );
  replayUi.destroy();
  assert(replayControls.hidden, "destroy hides replay controls");
  assert(!replayControls.classList.contains("replay-viewer-controls"), "destroy clears replay wrapper class");
  assert(!seekBack.hidden, "destroy restores seek controls visible");
  assert(stepDev.hidden, "destroy restores scenario step controls hidden");
  assert(!replayControls.querySelector(".replay-pause-btn"), "destroy removes generated replay pause button");
  assert(!replayControls.querySelector(".replay-branch-btn"), "destroy removes generated replay branch button");
  assert(!replayControls.querySelector(".replay-vision-controls"), "destroy removes generated vision controls");
  assert(!replayControls.querySelector(".replay-tick-status"), "destroy removes generated status");
  assert(!replayControls.querySelector(".replay-timeline"), "destroy removes generated timeline");
  assert(replayControls._listeners.size === 0, "destroy removes replay speed click listener");

  const scenarioControls = fakeEl("div");
  const scenarioSpeed0 = fakeEl("button");
  scenarioSpeed0.className = "spd-btn dev-pause-btn";
  scenarioSpeed0.dataset.speed = "0";
  const scenarioStep = fakeEl("button");
  scenarioStep.className = "spd-btn dev-step-btn";
  scenarioStep.dataset.stepDevTick = "";
  const scenarioSeek = fakeEl("button");
  scenarioSeek.className = "spd-btn seek-btn";
  scenarioSeek.dataset.seekBack = "30";
  scenarioControls.appendChild(scenarioSpeed0);
  scenarioControls.appendChild(scenarioStep);
  scenarioControls.appendChild(scenarioSeek);
  dom.replaySpeed = scenarioControls;
  const scenarioUi = new ReplayControls({
    net: replayNet,
    state: replayState,
    replayViewer: false,
    isReplay: false,
    isScenario: true,
  });
  assert(scenarioSeek.hidden, "scenario mode hides replay seek buttons");
  assert(!scenarioStep.hidden, "scenario mode shows step controls");
  scenarioControls._listeners.get("click")({ target: scenarioStep });
  assert(replayNet.steps === 1, "scenario step sends net.stepDevTick");
  scenarioControls._listeners.get("click")({ target: scenarioSpeed0 });
  assert(replayNet.speeds.at(-1) === 0, "scenario pause speed sends net.setReplaySpeed");
  scenarioUi.destroy();

  const noticeAudioMatch = Object.create(Match.prototype);
  const playedNotices = [];
  let minimapPings = 0;
  noticeAudioMatch.toast = () => {};
  noticeAudioMatch.audio = {
    play(id, opts) {
      playedNotices.push({ id, opts });
    },
  };
  noticeAudioMatch.minimap = {
    ping() {
      minimapPings += 1;
    },
    pulseBorder() {},
  };
  noticeAudioMatch.camera = { x: 0, y: 0, viewW: 100, viewH: 100, zoom: 1 };
  noticeAudioMatch.replayViewer = true;
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 512,
    y: 768,
  });
  assert(playedNotices.length === 0, "replay notice alerts do not play audio");
  assert(minimapPings === 1, "replay notice alerts still ping the minimap");
  noticeAudioMatch.replayViewer = false;
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 512,
    y: 768,
  });
  assert(
    playedNotices[0]?.id === "notice_under_attack",
    "live notice alerts still play audio outside the current viewport",
  );

  const storageValues = new Map();
  globalThis.window.localStorage = {
    getItem(key) {
      return storageValues.has(key) ? storageValues.get(key) : null;
    },
    setItem(key, value) {
      storageValues.set(key, value);
    },
    removeItem(key) {
      storageValues.delete(key);
    },
  };
  const storagePolicyMatch = Object.create(Match.prototype);
  assert(!storagePolicyMatch.readPointerLockPanEnabled(), "lock cursor pan defaults off without stored opt-in");
  storagePolicyMatch.writePointerLockPanEnabled(true);
  assert(storagePolicyMatch.readPointerLockPanEnabled(), "lock cursor pan opt-in persists");
  storagePolicyMatch.writePointerLockPanEnabled(false);
  assert(!storagePolicyMatch.readPointerLockPanEnabled(), "lock cursor pan opt-out clears persisted opt-in");

  const predictionPolicyMatch = Object.create(Match.prototype);
  predictionPolicyMatch.replayViewer = false;
  predictionPolicyMatch.state = {
    spectator: false,
    clearPredictedSnapshot() {
      this.clearedPrediction = true;
    },
  };
  predictionPolicyMatch.prediction = {
    enabled: true,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  let predictionAdapterInit = 0;
  predictionPolicyMatch.predictionAdapter = {
    ready: false,
    diagnostics: () => ({ ready: false }),
    init: async () => {
      predictionAdapterInit += 1;
      return true;
    },
  };
  predictionPolicyMatch.publishPredictionDebug = () => {};
  predictionPolicyMatch.mountSettings = () => {};
  predictionPolicyMatch.logPredictionStatus = () => {};
  predictionPolicyMatch.setPredictionEnabled(false);
  assert(!predictionPolicyMatch.prediction.enabled, "prediction setting can disable live prediction");
  assert(predictionPolicyMatch.state.clearedPrediction, "disabling prediction clears local predicted overlay");
  predictionPolicyMatch.setPredictionEnabled(true);
  await Promise.resolve();
  assert(predictionPolicyMatch.prediction.enabled, "prediction setting can re-enable live prediction");
  assert(predictionAdapterInit === 1, "re-enabling prediction initializes the WASM adapter");

  const mismatchMatch = Object.create(Match.prototype);
  mismatchMatch.prediction = {
    enabled: true,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  mismatchMatch.predictionStateMismatchLogged = false;
  let mismatchStatus = null;
  mismatchMatch.logPredictionStatus = (status) => { mismatchStatus = status; };
  mismatchMatch.state = {};
  mismatchMatch.advancePredictionVisual();
  assert(!mismatchMatch.prediction.enabled, "stale cached state module disables prediction instead of crashing");
  assert(mismatchStatus === "disabled-state-mismatch", "state mismatch is logged for diagnostics");

  const lockedPolicyMatch = Object.create(Match.prototype);
  lockedPolicyMatch.input = {
    pointerLocked: true,
    pointerLockSupported: () => true,
    installedAppRuntime: () => false,
  };
  lockedPolicyMatch.pointerLockPanEnabled = true;
  lockedPolicyMatch.pointerLockRetryToken = 0;
  let requestedRetry = null;
  lockedPolicyMatch.runPointerLockRetryBurst = (token, maxAttempts) => {
    requestedRetry = { token, maxAttempts };
    return Promise.resolve();
  };
  lockedPolicyMatch.requestAutomaticPointerLock({ requireGesture: true });
  assert(requestedRetry === null, "automatic Pointer Lock does not churn an already locked raw session");

  const disabledPolicyMatch = Object.create(Match.prototype);
  disabledPolicyMatch.input = {
    pointerLocked: false,
    pointerLockSupported: () => true,
    installedAppRuntime: () => false,
  };
  disabledPolicyMatch.pointerLockPanEnabled = false;
  disabledPolicyMatch.pointerLockRetryToken = 0;
  requestedRetry = null;
  disabledPolicyMatch.runPointerLockRetryBurst = (token, maxAttempts) => {
    requestedRetry = { token, maxAttempts };
    return Promise.resolve();
  };
  disabledPolicyMatch.requestAutomaticPointerLock({ requireGesture: true });
  assert(requestedRetry === null, "automatic Pointer Lock is gated behind the lock cursor pan setting");

  const unlockedPolicyMatch = Object.create(Match.prototype);
  unlockedPolicyMatch.input = {
    pointerLocked: false,
    pointerLockSupported: () => true,
    installedAppRuntime: () => true,
  };
  unlockedPolicyMatch.pointerLockPanEnabled = true;
  unlockedPolicyMatch.autoPointerLockUntil = 0;
  unlockedPolicyMatch.pointerLockRetryToken = 0;
  requestedRetry = null;
  unlockedPolicyMatch.runPointerLockRetryBurst = (token, maxAttempts) => {
    requestedRetry = { token, maxAttempts };
    return Promise.resolve();
  };
  unlockedPolicyMatch.requestAutomaticPointerLock({ requireGesture: true });
  assert(requestedRetry?.maxAttempts === 4, "installed-app gesture retries raw Pointer Lock while unlocked");

  const retryMatch = Object.create(Match.prototype);
  retryMatch.running = true;
  retryMatch.input = {
    pointerLocked: false,
    pointerLockSupported: () => true,
    installedAppRuntime: () => false,
    async requestPointerLock() {
      return false;
    },
  };
  retryMatch.autoPointerLockUntil = 0;
  retryMatch.pointerLockRetryToken = 7;
  retryMatch.waitPointerLockRetryDelay = async () => {};
  await retryMatch.runPointerLockRetryBurst(7, 2);
  assert(retryMatch.pointerLockRetry.attempts === 2, "raw Pointer Lock retry keeps trying while unlocked");
  assert(retryMatch.pointerLockRetry.stopped === "exhausted", "raw Pointer Lock retry exhausts without plain fallback");

  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}

{
  assert(
    !shouldRequestPointerLock({ installedAppRuntime: true, requireGesture: false }),
    "installed-app Pointer Lock skips non-gesture automatic requests",
  );
  assert(
    shouldRequestPointerLock({ installedAppRuntime: true, requireGesture: true }),
    "installed-app Pointer Lock runs from user gesture requests",
  );
  assert(
    shouldRequestPointerLock({ installedAppRuntime: false, requireGesture: false }),
    "browser Pointer Lock keeps non-gesture automatic attempts",
  );
  const priorLocation = globalThis.location;
  globalThis.location = { search: "?rtsNoAutoPointerLock=1" };
  assert(automaticPointerLockDisabledForTests(), "test URL flag disables automatic Pointer Lock requests");
  globalThis.location = { search: "" };
  assert(!automaticPointerLockDisabledForTests(), "automatic Pointer Lock is enabled by default");
  if (priorLocation === undefined) delete globalThis.location;
  else globalThis.location = priorLocation;
}

function fakeAudioParam(value = 1) {
  return {
    value,
    cancelScheduledValues() {},
    setValueAtTime(v) { this.value = v; },
    linearRampToValueAtTime(v) { this.value = v; },
  };
}

class FakeAudioNode {
  connect() { return this; }
  disconnect() {}
}

class FakeBufferSource extends FakeAudioNode {
  constructor() {
    super();
    this.playbackRate = fakeAudioParam(1);
    this.buffer = null;
    this.onended = null;
    this.started = false;
    this.stopped = false;
  }
  start() {
    this.started = true;
  }
  stop() {
    this.stopped = true;
    if (this.onended) this.onended();
  }
}

function fakeGain() {
  const node = new FakeAudioNode();
  node.gain = fakeAudioParam(1);
  return node;
}

function fakeAudioContext() {
  return {
    state: "running",
    currentTime: 0,
    createBufferSource() { return new FakeBufferSource(); },
    createStereoPanner() {
      const node = new FakeAudioNode();
      node.pan = fakeAudioParam(0);
      return node;
    },
    createBiquadFilter() {
      const node = new FakeAudioNode();
      node.type = "";
      node.frequency = fakeAudioParam(0);
      return node;
    },
    createGain: fakeGain,
    close() {},
  };
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------
{
  const decoded = decodeServerMessage({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [42, 100, 25, 3, 10],
    n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 7, 42],
    e: [
      [
        1,
        1,
        KIND_CODE[KIND.WORKER],
        10,
        20,
        40,
        40,
        STATE_CODE[STATE.GATHER],
        1.5,
        1.75,
        null,
        null,
        null,
        null,
        200,
        9,
        null,
        null,
        null,
        null,
        null,
        [
          [ORDER_STAGE_CODE[ORDER_STAGE.MOVE], 96, 112],
          [ORDER_STAGE_CODE[ORDER_STAGE.SETUP_AT_GUNS], 128, 160],
          [ORDER_STAGE_CODE[ORDER_STAGE.CHARGE], 176, 208],
          [ORDER_STAGE_CODE[ORDER_STAGE.SMOKE], 192, 224],
          [ORDER_STAGE_CODE[ORDER_STAGE.POINT_FIRE], 320, 352],
        ],
        87,
        [[ABILITY_CODE[ABILITY.CHARGE], 87, 2]],
        true,
        [[[112, 128], [144, 160]], [192, 224], 12, 2, 1, 2],
      ],
      [
        2,
        1,
        KIND_CODE[KIND.MACHINE_GUNNER],
        30,
        40,
        55,
        55,
        STATE_CODE[STATE.ATTACK],
        null,
        0.3,
        null,
        null,
        null,
        null,
        null,
        7,
        SETUP_CODE[SETUP.DEPLOYED],
      ],
      [
        3,
        1,
        KIND_CODE[KIND.CITY_CENTRE],
        100,
        120,
        450,
        500,
        STATE_CODE[STATE.TRAIN],
        null,
        null,
        KIND_CODE[KIND.WORKER],
        0.25,
        2,
        0.75,
      ],
    ],
    r: [[200, 1498]],
    sm: [[50, 320, 352, 2, 120]],
    u: [1, UPGRADE_CODE[UPGRADE.ARTILLERY_UNLOCK]],
    fg: [1, 2, 3, 1],
    ev: [
      [EVENT_CODE[EVENT.ATTACK], 1, 7],
      [EVENT_CODE[EVENT.DEATH], 200, 64, 96, KIND_CODE[KIND.STEEL]],
      [EVENT_CODE[EVENT.BUILD], 3, KIND_CODE[KIND.CITY_CENTRE]],
      [EVENT_CODE[EVENT.NOTICE], "Not enough steel"],
      [EVENT_CODE[EVENT.NOTICE], "alert:under_attack", 3, 512, 768],
      [EVENT_CODE[EVENT.MORTAR_LAUNCH], 9, [256, 272], [320, 352], 1.5, 68],
      [EVENT_CODE[EVENT.ARTILLERY_TARGET], 10, [320, 352], 3, ARTILLERY_SHELL_DELAY_TICKS],
      [EVENT_CODE[EVENT.ARTILLERY_IMPACT], 336, 368, 3],
    ],
  });

  assert(decoded.t === "snapshot", "compact snapshot keeps the semantic tag");
  assert(decoded.upgrades[0] === UPGRADE.METHAMPHETAMINES, "compact upgrades decode");
  assert(decoded.upgrades[1] === UPGRADE.ARTILLERY_UNLOCK, "compact artillery upgrade decodes");
  assert(decoded.tick === 42 && decoded.steel === 100 && decoded.supplyCap === 10, "compact scalars decode");
  assert(decoded.netStatus.predictionVersion === PREDICTION_PROTOCOL_VERSION, "compact prediction version decodes");
  assert(decoded.netStatus.lastSimConsumedClientSeq === 7, "compact consumed client sequence decodes");
  assert(decoded.netStatus.lastSimConsumedClientTick === 42, "compact consumed client tick decodes");
  assert(decoded.entities.length === 3, "compact entities decode");
  assert(decoded.entities[0].kind === KIND.WORKER, "entity kind code decodes");
  assert(decoded.entities[0].state === STATE.GATHER, "entity state code decodes");
  assert(decoded.entities[0].weaponFacing === 1.75, "entity optional weaponFacing decodes");
  assert(decoded.entities[0].latchedNode === 200, "entity optional latchedNode decodes");
  assert(decoded.entities[0].orderPlan.length === 5, "entity order plan decodes");
  assert(decoded.entities[0].chargeCooldownLeft === 87, "legacy charge cooldown decodes");
  assert(
    decoded.entities[0].abilities[0].ability === ABILITY.CHARGE &&
      decoded.entities[0].abilities[0].cooldownLeft === 87 &&
      decoded.entities[0].abilities[0].remainingUses === 2,
    "entity ability cooldowns decode",
  );
  assert(
    decoded.entities[0].orderPlan[0].kind === ORDER_STAGE.MOVE &&
      decoded.entities[0].orderPlan[0].x === 96 &&
      decoded.entities[0].orderPlan[0].y === 112,
    "entity active order stage decodes",
  );
  assert(decoded.entities[0].visionOnly === true, "entity visionOnly flag decodes");
  assert(
    decoded.entities[0].debugPath.waypoints[0].x === 112 &&
      decoded.entities[0].debugPath.waypoints[1].y === 160 &&
      decoded.entities[0].debugPath.goal.x === 192 &&
      decoded.entities[0].debugPath.lastRepathTick === 12 &&
      decoded.entities[0].debugPath.stuckTicks === 2 &&
      decoded.entities[0].debugPath.staticBlockedTicks === 1 &&
      decoded.entities[0].debugPath.totalWaypoints === 2,
    "entity debug path decodes",
  );
  assert(
      decoded.entities[0].orderPlan[1].kind === ORDER_STAGE.SETUP_AT_GUNS &&
      decoded.entities[0].orderPlan[2].kind === ORDER_STAGE.CHARGE &&
      decoded.entities[0].orderPlan[3].kind === ORDER_STAGE.SMOKE &&
      decoded.entities[0].orderPlan[4].kind === ORDER_STAGE.POINT_FIRE,
    "order plan stage flavor decodes",
  );
  assert(
    decoded.entities[0].orderPlan[1].kind === ORDER_STAGE.SETUP_AT_GUNS &&
      decoded.entities[0].orderPlan[1].x === 128 &&
      decoded.entities[0].orderPlan[1].y === 160,
    "queued AT gun setup order stage decodes",
  );
  assert(
    decoded.entities[0].orderPlan[2].kind === ORDER_STAGE.CHARGE &&
      decoded.entities[0].orderPlan[2].x === 176 &&
      decoded.entities[0].orderPlan[2].y === 208,
    "queued Charge order stage decodes",
  );
  assert(decoded.entities[1].setupState === SETUP.DEPLOYED, "entity setupState code decodes");
  assert(decoded.entities[2].prodKind === KIND.WORKER, "entity prodKind code decodes");
  assert(decoded.entities[2].prodProgress === 0.25, "entity prodProgress decodes");
  assert(
    decoded.entities[2].orderPlan === undefined,
    "compact snapshot tolerates missing order plan fields",
  );
  assert(decoded.resourceDeltas[0].remaining === 1498, "resource deltas decode");
  assert(
    decoded.smokes[0].id === 50 &&
      decoded.smokes[0].radiusTiles === 2 &&
      decoded.smokes[0].expiresIn === 120,
    "smoke clouds decode",
  );
  assert(
    decoded.visibleTiles.join(",") === "1,1,0,0,0,1",
    "compact snapshot decodes server visibility grid",
  );
  assert(decoded.events[0].e === EVENT.ATTACK && decoded.events[0].to === 7, "attack event decodes");
  assert(decoded.events[1].kind === KIND.STEEL, "death event kind decodes");
  assert(decoded.events[3].msg === "Not enough steel", "notice event decodes");
  assert(decoded.events[3].severity === NOTICE_SEVERITY.INFO, "legacy notice defaults to info");
  assert(decoded.events[4].severity === NOTICE_SEVERITY.ALERT, "notice severity decodes");
  assert(decoded.events[4].x === 512 && decoded.events[4].y === 768, "notice position decodes");
  assert(
    decoded.events[5].e === EVENT.MORTAR_LAUNCH &&
      decoded.events[5].from === 9 &&
      decoded.events[5].fromX === 256 &&
      decoded.events[5].toY === 352 &&
      decoded.events[5].delayTicks === 68,
    "mortar launch event decodes",
  );
  assert(
    decoded.events[6].e === EVENT.ARTILLERY_TARGET &&
      decoded.events[6].from === 10 &&
      decoded.events[6].delayTicks === ARTILLERY_SHELL_DELAY_TICKS &&
      decoded.events[6].radiusTiles === 3,
    "artillery target event decodes",
  );
  assert(
    decoded.events[7].e === EVENT.ARTILLERY_IMPACT &&
      decoded.events[7].x === 336 &&
      decoded.events[7].y === 368,
    "artillery impact event decodes",
  );

  const abilityCommand = cmd.useAbility(ABILITY.SMOKE, [7, 8], 320, 384, true);
  assert(
    abilityCommand.c === "useAbility" &&
      abilityCommand.ability === ABILITY.SMOKE &&
      abilityCommand.units.length === 2 &&
      abilityCommand.x === 320 &&
      abilityCommand.y === 384 &&
      abilityCommand.queued === true,
    "useAbility command builder emits targeted ability wire shape",
  );
  const buildCommand = cmd.build([7, 8], KIND.DEPOT, 12, 14, true);
  assert(
    buildCommand.c === "build" &&
      buildCommand.units.join(",") === "7,8" &&
      buildCommand.building === KIND.DEPOT &&
      buildCommand.tileX === 12 &&
      buildCommand.tileY === 14 &&
      buildCommand.queued === true,
    "build command builder emits selected-worker wire shape",
  );
  assert(
    JSON.stringify(msg.command(cmd.stop([7]), 3)) ===
      JSON.stringify({ t: "command", clientSeq: 3, cmd: { c: "stop", units: [7] } }),
    "command message builder wraps gameplay commands with clientSeq",
  );
  const pointFireCommand = cmd.pointFire([11, 12], 512, 640, true);
  assert(
    pointFireCommand.c === "useAbility" &&
      pointFireCommand.ability === ABILITY.POINT_FIRE &&
      pointFireCommand.units.join(",") === "11,12" &&
      pointFireCommand.x === 512 &&
      pointFireCommand.y === 640 &&
      pointFireCommand.queued === true,
    "pointFire command builder emits targeted ability wire shape",
  );

  assertThrows(
    () => decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION, s: [1], e: [] }),
    "compact snapshot rejects malformed scalar count",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [[1, 1, 255, 0, 0, 1, 1, STATE_CODE[STATE.IDLE]]],
      }),
    "compact snapshot rejects unknown enum codes",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: new Array(20001),
      }),
    "compact snapshot enforces entity count bounds",
  );
  assertThrows(
    () =>
      decodeServerMessage({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [[
          1,
          1,
          KIND_CODE[KIND.WORKER],
          0,
          0,
          1,
          1,
          STATE_CODE[STATE.IDLE],
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          null,
          new Array(10),
        ]],
      }),
    "compact snapshot enforces order plan bounds",
  );
}

{
  assert(
    JSON.stringify(cmd.setupAtGuns([1, 2], 100, 200)) ===
      JSON.stringify({ c: "setupAtGuns", units: [1, 2], x: 100, y: 200 }),
    "setupAtGuns command builder emits the wire shape",
  );
  assert(
    JSON.stringify(cmd.tearDownAtGuns([3, 4])) ===
      JSON.stringify({ c: "tearDownAtGuns", units: [3, 4] }),
    "tearDownAtGuns command builder emits the wire shape",
  );
  assert(
    JSON.stringify(cmd.move([1], 100, 200, true)) ===
      JSON.stringify({ c: "move", units: [1], x: 100, y: 200, queued: true }),
    "queued move command builder emits the queued flag only when requested",
  );
  assert(AT_GUN_DEPLOYED_RANGE_TILES === 12, "client mirrors deployed AT gun range");
  assertApprox(
    AT_GUN_FIELD_OF_FIRE_RAD,
    Math.PI / 4,
    0.000001,
    "client mirrors AT gun field of fire",
  );
}

// ---------------------------------------------------------------------------
// Net
// ---------------------------------------------------------------------------
{
  const net = new Net("ws://example.test/ws");
  assert(net instanceof Net, "Net constructor should return an instance");
  assertHasMethod(net, "connect", "Net");
  assertHasMethod(net, "on", "Net");
  assertHasMethod(net, "off", "Net");
  assertHasMethod(net, "join", "Net");
  assertHasMethod(net, "ready", "Net");
  assertHasMethod(net, "start", "Net");
  assertHasMethod(net, "giveUp", "Net");
  assertHasMethod(net, "returnToLobby", "Net");
  assertHasMethod(net, "command", "Net");
  assertHasMethod(net, "ping", "Net");
  assertHasMethod(net, "netReport", "Net");
  assertHasGetter(net, "playerId", "Net");
  assert(net.playerId === null, "Net.playerId should be null before welcome");
  assertHasMethod(net, "addAi", "Net");
  assertHasMethod(net, "removeAi", "Net");
  assertHasMethod(net, "setQuickstart", "Net");
  assertHasMethod(net, "setReplaySpeed", "Net");
  assertHasMethod(net, "setReplayVision", "Net");
  assertHasMethod(net, "requestReplayBranch", "Net");
  assertHasMethod(net, "claimBranchSeat", "Net");
  assertHasMethod(net, "releaseBranchSeat", "Net");
  assertHasMethod(net, "startBranch", "Net");
  const sent = [];
  net.ws = {
    readyState: WebSocket.OPEN,
    bufferedAmount: 0,
    send(json) {
      sent.push(JSON.parse(json));
    },
  };
  assertThrows(() => net.command(cmd.stop([1])), "Net.command requires controller-provided clientSeq");
  net.command(cmd.stop([1]), 7);
  assert(sent[0].clientSeq === 7, "Net.command sends the provided clientSeq");
  assert(!("replayOk" in msg.join("A", "main")), "join builder omits replayOk by default");
  assert(
    msg.join("A", "main", false, true).replayOk === true,
    "join builder can confirm replay joins",
  );
  assert(msg.netReport({ schemaVersion: 1 }).t === "netReport", "net-report builder tag");
  assert(msg.netReport({ schemaVersion: 1 }).report.schemaVersion === 1, "net-report builder payload");
  assert(msg.returnToLobby().t === "returnToLobby", "return-to-lobby builder tag");
  assert(msg.requestReplayBranch().t === "requestReplayBranch", "replay branch builder tag");
  assert(msg.claimBranchSeat(7).t === "claimBranchSeat", "branch seat claim builder tag");
  assert(msg.releaseBranchSeat(7).t === "releaseBranchSeat", "branch seat release builder tag");
  assert(msg.startBranch().t === "startBranch", "branch start builder tag");
  assert(msg.replayVisionAll().t === "setReplayVision", "replay all-vision builder tag");
  assert(msg.replayVisionAll().vision.mode === "all", "replay all-vision builder payload");
  assert(
    msg.replayVisionPlayer(7).vision.playerId === 7,
    "replay single-player vision builder payload",
  );
  assert(
    msg.replayVisionPlayers([1, 2]).vision.playerIds.join(",") === "1,2",
    "replay subset vision builder payload",
  );
}

// ---------------------------------------------------------------------------
// PredictionController
// ---------------------------------------------------------------------------
{
  let clock = 100;
  const sent = [];
  const prediction = new PredictionController({
    now: () => clock,
    commandTimeoutMs: 50,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  assert(prediction.debugSummary().mode === PREDICTION_STATE.TRACKING, "PredictionController starts tracking");
  prediction.issueCommand(cmd.stop([1]));
  prediction.issueCommand(cmd.stop([2]));
  prediction.issueCommand(cmd.stop([3]));
  assert(sent.map((entry) => entry.clientSeq).join(",") === "1,2,3", "PredictionController allocates sequences");
  prediction.applyAuthoritativeSnapshot({
    tick: 10,
    netStatus: { lastSimConsumedClientSeq: 1, lastSimConsumedClientTick: 9 },
  });
  assert(prediction.pendingCommandCount === 2, "PredictionController drops acknowledged commands");
  assert(prediction.debugSummary().pendingClientSeqs.join(",") === "2,3", "ack 1 leaves 2 and 3 pending");
  prediction.applyAuthoritativeSnapshot({ tick: 10, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(prediction.debugSummary().duplicateSnapshotCount === 1, "duplicate snapshots are tracked");
  prediction.applyAuthoritativeSnapshot({ tick: 12, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(prediction.debugSummary().skippedSnapshotCount === 1, "skipped authoritative ticks are tolerated");
  prediction.applyAuthoritativeSnapshot({ tick: 11, netStatus: { lastSimConsumedClientSeq: 3 } });
  assert(prediction.pendingCommandCount === 2, "stale snapshots do not ack commands");
  assert(prediction.debugSummary().staleSnapshotCount === 1, "stale snapshot is counted");
  prediction.issueCommand(cmd.stop([4]));
  prediction.issueCommand(cmd.stop([5]));
  prediction.applyAuthoritativeSnapshot({ tick: 13, netStatus: { lastSimConsumedClientSeq: 3 } });
  assert(prediction.debugSummary().pendingClientSeqs.join(",") === "4,5", "ack 3 drops older commands");
  prediction.recordSocketReceipt(4, { serverTick: 13 });
  assert(prediction.pendingCommandCount === 2, "socket receipt does not reconcile command 4");
  prediction.recordCommandRejection(5, "invalid target");
  assert(prediction.pendingCommandCount === 2, "command rejection notice alone does not consume sim ack");
  clock = 200;
  assert(prediction.expireTimedOutCommands() === 2, "timed out pending commands are marked");
  prediction.applyAuthoritativeSnapshot({ tick: 14, netStatus: { lastSimConsumedClientSeq: 5 } });
  assert(prediction.pendingCommandCount === 0, "later sim ack clears timed-out/rejected pending commands");
  prediction.beginResync({ dx: 3 });
  assert(prediction.debugSummary().mode === PREDICTION_STATE.RESYNCING, "resync state is exposed");
  prediction.finishResync();
  assert(prediction.debugSummary().mode === PREDICTION_STATE.TRACKING, "resync returns to tracking");
  prediction.reset();
  assert(prediction.debugSummary().nextClientSeq === 1, "PredictionController reset restarts sequence ids");
}

// ---------------------------------------------------------------------------
// Replay branch staging
// ---------------------------------------------------------------------------
{
  const { BranchStaging } = await import("../client/src/branch_staging.js");
  function fakeEl(tag = "div") {
    const el = {
      tagName: tag.toUpperCase(),
      children: [],
      dataset: {},
      style: {},
      hidden: false,
      disabled: false,
      textContent: "",
      className: "",
      classList: {
        add(cls) {
          if (!el.className.split(/\s+/).includes(cls)) el.className = `${el.className} ${cls}`.trim();
        },
        remove(cls) {
          el.className = el.className.split(/\s+/).filter((c) => c && c !== cls).join(" ");
        },
        contains(cls) {
          return el.className.split(/\s+/).includes(cls);
        },
      },
      setAttribute(name, value) {
        this[name] = value;
      },
      appendChild(child) {
        child.parentNode = this;
        this.children.push(child);
        return child;
      },
      append(...children) {
        for (const child of children) this.appendChild(child);
      },
      replaceChildren(...children) {
        this.children = [];
        for (const child of children) this.appendChild(child);
      },
      addEventListener(type, handler) {
        this[`on${type}`] = handler;
      },
      remove() {
        if (!this.parentNode) return;
        this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
        this.parentNode = null;
      },
    };
    return el;
  }
  const priorDocument = globalThis.document;
  const priorSetTimeout = globalThis.setTimeout;
  const priorClearTimeout = globalThis.clearTimeout;
  let nextTimer = 1;
  const timers = new Map();
  globalThis.document = { createElement: fakeEl };
  globalThis.setTimeout = (fn) => {
    const id = nextTimer++;
    timers.set(id, fn);
    return id;
  };
  globalThis.clearTimeout = (id) => timers.delete(id);
  const sent = [];
  const handlers = new Map();
  const net = {
    playerId: 10,
    on(type, handler) { handlers.set(type, handler); },
    off(type) { handlers.delete(type); },
    claimBranchSeat(id) { sent.push(["claim", id]); },
    releaseBranchSeat(id) { sent.push(["release", id]); },
    startBranch() { sent.push(["start"]); },
  };
  const root = fakeEl("section");
  const staging = new BranchStaging(root, net);
  staging.show();
  handlers.get("branchStaging")({
    t: "branchStaging",
    room: "__replay_branch__:abc",
    sourceTick: 1200,
    hostId: 10,
    canStart: false,
    seats: [
      { playerId: 1, name: "Alpha", color: "#4878c8" },
      { playerId: 2, name: "Bravo", color: "#c84848", claimantId: 11, claimantName: "Other" },
    ],
    occupants: [{ id: 10, name: "Me" }, { id: 11, name: "Other" }],
  });
  assert(root.classList.contains("branch-staging-active"), "branch staging marks active root");
  const box = root.children[0];
  assert(box.className === "branch-staging-box", "branch staging renders focused room box");
  const seatList = box.children.find((child) => child.className === "branch-seat-list");
  assert(seatList.children.length === 2, "branch staging renders original seats");
  const claimButton = seatList.children[0].children[2];
  claimButton.onclick();
  assert(sent[0][0] === "claim" && sent[0][1] === 1, "claim button sends branch seat claim");
  const startButton = box.children.find((child) => child.className === "branch-actions").children[0];
  assert(startButton.hidden === false, "host sees start button");
  assert(startButton.disabled === true, "start disabled until all seats claimed");
  handlers.get("matchCountdown")({
    t: "matchCountdown",
    durationMs: 3000,
    words: ["Drei!", "Zwei!", "Eins!"],
  });
  const countdown = root.children.find((child) => child.className.includes("match-countdown"));
  assert(countdown?.textContent === "Drei!", "branch staging renders the visible countdown overlay");
  staging.hide();
  assert(
    !root.children.some((child) => child.className.includes("match-countdown")),
    "branch staging clears countdown overlay when hidden",
  );
  staging.destroy();
  globalThis.document = priorDocument;
  globalThis.setTimeout = priorSetTimeout;
  globalThis.clearTimeout = priorClearTimeout;
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
{
  assert(MINING_CC_RANGE_TILES === 7, "client mirrors the server mining City Centre range");
  assert(STATS[KIND.CITY_CENTRE].cost.steel === 200, "City Centre cost mirrors server");
  assert(
    Array.isArray(STATS[KIND.FACTORY].requires),
    "Vehicle Works should expose all server-side build prerequisites",
  );
  assert(
    STATS[KIND.FACTORY].label === "Vehicle Works",
    "factory protocol kind should present as Vehicle Works",
  );
  assert(
    STATS[KIND.STEELWORKS].label === "Gun Works",
    "steelworks protocol kind should present as Gun Works",
  );
  assert(
    Array.isArray(STATS[KIND.TRAINING_CENTRE].requires),
    "Training Centre should expose all server-side build prerequisites",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].requires.includes(KIND.CITY_CENTRE),
    "Training Centre should require a City Centre in the command card",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].requires.includes(KIND.BARRACKS),
    "Training Centre should require a Barracks in the command card",
  );
  assert(STATS[KIND.TRAINING_CENTRE].buildTicks === 560, "Training Centre build time mirrors server");
  assert(
    STATS[KIND.FACTORY].requires.includes(KIND.CITY_CENTRE),
    "Vehicle Works should require a City Centre in the command card",
  );
  assert(
    STATS[KIND.FACTORY].requires.includes(KIND.TRAINING_CENTRE),
    "Vehicle Works should require a Training Centre in the command card",
  );
  assert(
    STATS[KIND.FACTORY].trains[0] === KIND.SCOUT_CAR,
    "Vehicle Works should put Scout Car in the leftmost train slot",
  );
  assert(
    STATS[KIND.FACTORY].trains.includes(KIND.TANK),
    "Vehicle Works should train Tanks after the unlock",
  );
  assert(STATS[KIND.SCOUT_CAR].cost.steel === 125, "Scout Car steel cost mirrors server");
  assert(STATS[KIND.SCOUT_CAR].cost.oil === 50, "Scout Car oil cost mirrors server");
  assert(STATS[KIND.SCOUT_CAR].sight === 10, "Scout Car has the largest mobile sight radius");
  assert(SMOKE_ABILITY_COST.steel === 0 && SMOKE_ABILITY_COST.oil === 0, "Scout Car smoke has no resource cost");
  assert(!("requires" in ABILITIES[ABILITY.SMOKE]), "Scout Car smoke should be available without Gun Works");
  assert(STATS[KIND.SCOUT_CAR].body.length === 40.8, "Scout Car client body length mirrors server");
  assert(STATS[KIND.SCOUT_CAR].body.width === 21.6, "Scout Car client body width mirrors server");
  assert(KIND_CODE[KIND.SCOUT_CAR] === 14, "Scout Car compact kind code should follow steelworks protocol kind");
  assert(KIND_CODE[KIND.ARTILLERY] === 16, "Artillery compact kind code should be reserved");
  assert(KIND_CODE[KIND.RESEARCH_COMPLEX] === 17, "R&D Complex compact kind code should be reserved");
  assert(ABILITY_CODE[ABILITY.POINT_FIRE] === 4, "Point Fire compact ability code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.POINT_FIRE] === 10, "Point Fire compact order stage code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_TARGET] === 7, "Artillery target compact event code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_IMPACT] === 8, "Artillery impact compact event code should be reserved");
  assert(EVENT_CODE[EVENT.MORTAR_LAUNCH] === 9, "Mortar launch compact event code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.MORTAR_AUTOCAST] === 5, "Mortar Autocast compact upgrade code should be reserved");
  assert(
    STATS[KIND.ARTILLERY].cost.steel === 300 &&
      STATS[KIND.ARTILLERY].cost.oil === 100 &&
      STATS[KIND.ARTILLERY].supply === 5,
    "Artillery cost and supply mirror server",
  );
  assert(STATS[KIND.ARTILLERY].upgradeRequires === UPGRADE.ARTILLERY_UNLOCK, "Artillery training requires its unlock");
  assert(
    ABILITIES[ABILITY.POINT_FIRE].carriers.includes(KIND.ARTILLERY) &&
      ABILITIES[ABILITY.POINT_FIRE].rangeTiles === ARTILLERY_MAX_RANGE_TILES &&
      ABILITIES[ABILITY.POINT_FIRE].minRangeTiles === ARTILLERY_MIN_RANGE_TILES &&
      ABILITIES[ABILITY.POINT_FIRE].delayTicks === ARTILLERY_SHELL_DELAY_TICKS &&
      ARTILLERY_SHELL_DELAY_TICKS === 150,
    "Point Fire ability exposes Artillery carrier, max range, minimum range, and 5-second delay",
  );
  assert(
    STATS[KIND.STEELWORKS].footW === 3 && STATS[KIND.STEELWORKS].footH === 3,
    "Gun Works should be a 3x3 building",
  );
  assert(
    STATS[KIND.STEELWORKS].cost.steel === 125 && STATS[KIND.STEELWORKS].cost.oil === 125,
    "Gun Works cost mirrors server",
  );
  assert(STATS[KIND.STEELWORKS].buildTicks === 620, "Gun Works build time mirrors server");
  assert(
    STATS[KIND.STEELWORKS].trains.includes(KIND.AT_TEAM),
    "Gun Works should train AT Guns after the unlock",
  );
  assert(
    !STATS[KIND.STEELWORKS].researches,
    "Gun Works should no longer expose advanced unlock research",
  );
  assert(
    !STATS[KIND.BARRACKS].trains.includes(KIND.AT_TEAM),
    "Barracks should no longer train AT Guns",
  );
  assert(
    STATS[KIND.STEELWORKS].requires.includes(KIND.TRAINING_CENTRE),
    "Gun Works should require Training Centre tech in the command card",
  );
  assert(
    STATS[KIND.RESEARCH_COMPLEX].label === "R&D Complex" &&
      STATS[KIND.RESEARCH_COMPLEX].footW === 3 &&
      STATS[KIND.RESEARCH_COMPLEX].footH === 3,
    "R&D Complex should be a 3x3 command-card building",
  );
  assert(
    STATS[KIND.RESEARCH_COMPLEX].cost.steel === 100 &&
      STATS[KIND.RESEARCH_COMPLEX].cost.oil === 100 &&
      STATS[KIND.RESEARCH_COMPLEX].buildTicks === 450,
    "R&D Complex cost and build time mirror server",
  );
  assert(
    STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.AT_GUN_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.ARTILLERY_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.TANK_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.MORTAR_AUTOCAST),
    "R&D Complex should expose AT Gun, Artillery, Tank, and Mortar Autocast research",
  );
  assert(!ABILITIES[ABILITY.CHARGE], "client no longer exposes Rifleman Charge as a command-card ability");
  assert(
    STATS[KIND.TRAINING_CENTRE].researches.includes(UPGRADE.METHAMPHETAMINES),
    "Training Centre should expose Methamphetamines research",
  );
  assert(
    UPGRADES[UPGRADE.METHAMPHETAMINES].cost.steel === 100 &&
      UPGRADES[UPGRADE.METHAMPHETAMINES].cost.oil === 100 &&
      UPGRADES[UPGRADE.METHAMPHETAMINES].researchTicks === 600,
    "Methamphetamines research cost and time mirror server",
  );
  assert(
    UPGRADES[UPGRADE.MORTAR_AUTOCAST].cost.steel === 150 &&
      UPGRADES[UPGRADE.MORTAR_AUTOCAST].cost.oil === 150 &&
      UPGRADES[UPGRADE.MORTAR_AUTOCAST].researchTicks === 600,
    "Mortar Autocast research cost and time mirror server",
  );
  assert(
    STATS[KIND.AT_TEAM].upgradeRequiresText === "Requires research in R&D Complex",
    "AT Gun training should explain the R&D Complex research requirement",
  );
  assert(
    STATS[KIND.TANK].upgradeRequiresText === "Requires research in R&D Complex",
    "Tank training should explain the R&D Complex research requirement",
  );
  const playerId = 1;
  const underConstructionTrainingCentre = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: 0.5 },
  ];
  assert(
    !playerHasCompletedKind(underConstructionTrainingCentre, playerId, KIND.TRAINING_CENTRE),
    "Vehicle Works should not unlock while the Training Centre is still under construction",
  );
  const underConstructionBarracks = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.BARRACKS, buildProgress: 0.5 },
  ];
  assert(
    !playerHasCompletedKind(underConstructionBarracks, playerId, KIND.BARRACKS),
    "Training Centre should not unlock while the Barracks is still under construction",
  );
  const completedTrainingCentre = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: null },
  ];
  assert(
    playerHasCompletedKind(completedTrainingCentre, playerId, KIND.TRAINING_CENTRE),
    "Vehicle Works should unlock once the Training Centre is complete",
  );
  assert(formatTankOilUsed(0.04) === "0.0", "tank oil panel rounds tiny values to tenths");
  assert(formatTankOilUsed(9.94) === "9.9", "tank oil panel keeps tenths below ten oil");
  assert(formatTankOilUsed(10.4) === "10", "tank oil panel rounds whole values above ten oil");
  assert(formatTankOilUsed(-2) === "0.0", "tank oil panel clamps negative values");
  assert(formatTankOilUsed(Number.NaN) === "0.0", "tank oil panel tolerates missing oilUsed");
  const groupedNearlySameCooldowns = groupCooldownClocks([150, 149, 146], RIFLEMAN_CHARGE_COOLDOWN_TICKS);
  assert(groupedNearlySameCooldowns.length === 1, "nearby rifleman cooldowns share one clock arm");
  assert(groupedNearlySameCooldowns[0].count === 3, "clock grouping keeps the grouped unit count");
  const groupedDistinctCooldowns = groupCooldownClocks([150, 120, 60], RIFLEMAN_CHARGE_COOLDOWN_TICKS);
  assert(groupedDistinctCooldowns.length === 3, "visibly different rifleman cooldowns get separate clock arms");
  const groupedIgnoringReady = groupCooldownClocks([0, 0, 30, 31], RIFLEMAN_CHARGE_COOLDOWN_TICKS);
  assert(groupedIgnoringReady.length === 1 && groupedIgnoringReady[0].count === 2, "ready riflemen do not create cooldown clocks");

  const trained = [];
  let selectedProductionBuildings = [
    { id: 20, owner: playerId, kind: KIND.BARRACKS },
    { id: 22, owner: playerId, kind: KIND.BARRACKS, buildProgress: 0.5 },
    { id: 21, owner: playerId, kind: KIND.BARRACKS },
    { id: 30, owner: playerId, kind: KIND.FACTORY },
  ];
  const hud = Object.create(HUD.prototype);
  hud.state = {
    playerId,
    selectedEntities: () => selectedProductionBuildings,
  };
  hud.commandIssuer = {
    command: (command) => trained.push(command),
  };
  hud._trainRoundRobin = new Map();
  hud._cancelRoundRobin = new Map();

  hud._issueTrain(KIND.RIFLEMAN);
  hud._issueTrain(KIND.MACHINE_GUNNER);
  hud._issueTrain(KIND.RIFLEMAN);
  hud._issueTrain(KIND.SCOUT_CAR);
  assert(
    trained.map((command) => command.building).join(",") === "20,21,20,30",
    "selected production buildings should receive train commands round-robin by compatible producer set",
  );

  selectedProductionBuildings = [
    { id: 21, owner: playerId, kind: KIND.BARRACKS },
    { id: 20, owner: playerId, kind: KIND.BARRACKS },
  ];
  hud._issueTrain(KIND.RIFLEMAN);
  assert(
    trained[4].building === 21,
    "changing selected producer order should start the new round-robin set at its first building",
  );

  selectedProductionBuildings = [
    { id: 20, owner: playerId, kind: KIND.BARRACKS, prodQueue: 1 },
    { id: 21, owner: playerId, kind: KIND.BARRACKS, prodQueue: 2 },
    { id: 30, owner: playerId, kind: KIND.FACTORY, prodQueue: 1 },
  ];
  hud._issueCancelProduction(KIND.BARRACKS);
  hud._issueCancelProduction(KIND.BARRACKS);
  hud._issueCancelProduction(KIND.BARRACKS);
  assert(
    trained.slice(5).map((command) => command.building).join(",") === "21,20,21",
    "selected producing buildings should receive cancel commands reverse round-robin by producer kind",
  );

  const priorDocument = globalThis.document;
  const priorMouseEvent = globalThis.MouseEvent;
  const renderedButtons = [];
  function fakeElement(tagName) {
    const listeners = new Map();
    return {
      tagName: tagName.toUpperCase(),
      children: [],
      className: "",
      dataset: {},
      disabled: false,
      innerHTML: "",
      style: {
        values: {},
        setProperty(name, value) {
          this.values[name] = value;
        },
      },
      appendChild(child) {
        if (child?.nodeType === "fragment") this.children.push(...child.children);
        else this.children.push(child);
      },
      querySelector(selector) {
        const abilityMatch = selector.match(/^button\[data-ability="([^"]+)"\]$/);
        if (abilityMatch) {
          return this.children.find((child) => child.dataset?.ability === abilityMatch[1]) || null;
        }
        return null;
      },
      querySelectorAll() {
        return [];
      },
      addEventListener(type, listener) {
        listeners.set(type, listener);
      },
      dispatchEvent(ev) {
        listeners.get(ev.type)?.(ev);
        return true;
      },
      click(ev = {}) {
        listeners.get("click")?.({
          type: "click",
          preventDefault() {},
          shiftKey: !!ev.shiftKey,
        });
      },
    };
  }
  function renderCommandCard(hud) {
    if (!hud.elCommand) hud.elCommand = fakeElement("div");
    hud._renderCommandCard();
    return hud.elCommand;
  }
  try {
    globalThis.document = {
      createDocumentFragment() {
        return {
          nodeType: "fragment",
          children: [],
          appendChild(child) {
            this.children.push(child);
          },
        };
      },
      createElement(tagName) {
        const el = fakeElement(tagName);
        if (tagName === "button") renderedButtons.push(el);
        return el;
      },
    };
    globalThis.MouseEvent = class {
      constructor(type, init = {}) {
        this.type = type;
        this.altKey = !!init.altKey;
        this.ctrlKey = !!init.ctrlKey;
        this.metaKey = !!init.metaKey;
        this.shiftKey = !!init.shiftKey;
        this.bubbles = !!init.bubbles;
        this.cancelable = !!init.cancelable;
      }
      preventDefault() {}
    };

    const sent = [];
    const selectedTrainingCentre = {
      id: 77,
      owner: playerId,
      kind: KIND.TRAINING_CENTRE,
      buildProgress: null,
    };
    const researchHud = Object.create(HUD.prototype);
    researchHud.state = {
      playerId,
      resources: { steel: 100, oil: 100 },
      upgrades: [],
      commandTarget: null,
      selectedEntities: () => [selectedTrainingCentre],
      entitiesInterpolated: () => [selectedTrainingCentre],
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    researchHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    researchHud._cardSig = null;
    researchHud._resourceIcons = {};

    renderCommandCard(researchHud);
    const researchButton = renderedButtons.find((button) => button.innerHTML.includes("Methamphetamines"));
    assert(researchButton && !researchButton.disabled, "Methamphetamines command-card button renders enabled");
    assert(researchButton.dataset.hotkey === "Q", "Methamphetamines command-card button uses Q as its hotkey");
    assert(researchButton.innerHTML.includes("Research time"), "Methamphetamines tooltip includes research time");
    researchButton.click({ shiftKey: true });
    assert(
      sent.length === 1 &&
        sent[0].c === "research" &&
        sent[0].building === 77 &&
        sent[0].upgrade === UPGRADE.METHAMPHETAMINES,
      "Clicking Methamphetamines should send a research command",
    );

    const mortarButtonsBefore = renderedButtons.length;
    const selectedMortar = {
      id: 501,
      owner: playerId,
      kind: KIND.MORTAR_TEAM,
      abilities: [{
        ability: ABILITY.MORTAR_FIRE,
        cooldownLeft: 30,
        autocastEnabled: true,
      }],
    };
    const mortarHud = Object.create(HUD.prototype);
    mortarHud.state = {
      playerId,
      resources: { steel: 100, oil: 100 },
      commandTarget: null,
      selectedEntities: () => [selectedMortar],
      entitiesInterpolated: () => [selectedMortar],
      beginCommandTarget(target) {
        this.commandTarget = target;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    mortarHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    mortarHud.audio = null;
    mortarHud._cardSig = null;
    renderCommandCard(mortarHud);
    const mortarButtonCount = renderedButtons.length;
    assert(
      mortarButtonCount > mortarButtonsBefore,
      "selected Mortar Team should render an ability command button",
    );
    selectedMortar.abilities[0].cooldownLeft = 29;
    renderCommandCard(mortarHud);
    assert(
      renderedButtons.length === mortarButtonCount,
      "Mortar Fire cooldown ticks should update in place without rebuilding the command button",
    );

    globalThis.document.getElementById = (id) => {
      assert(id === "command-card", "Methamphetamines hotkey should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "Methamphetamines hotkey should query hotkey buttons");
          return [researchButton];
        },
      };
    };
    const input = Object.create(Input.prototype);
    input.state = researchHud.state;
    const hotkeyEv = {
      code: "KeyQ",
      shiftKey: false,
      repeat: false,
      preventDefault() {},
    };
    const hotkeyResult = input._activateCommandHotkey(hotkeyEv);
    assert(hotkeyResult?.handled === true, "Methamphetamines hotkey should activate the command-card button");
    assert(
      sent.length === 2 &&
        sent[1].c === "research" &&
        sent[1].building === 77 &&
        sent[1].upgrade === UPGRADE.METHAMPHETAMINES,
      "Methamphetamines hotkey should send a research command",
    );

    renderedButtons.length = 0;
    const selectedFactory = {
      id: 78,
      owner: playerId,
      kind: KIND.FACTORY,
      buildProgress: null,
    };
    const factoryHud = Object.create(HUD.prototype);
    factoryHud.state = {
      playerId,
      resources: { steel: 300, oil: 150 },
      upgrades: [],
      selectedEntities: () => [selectedFactory],
      entitiesInterpolated: () => [selectedFactory],
    };
    factoryHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    factoryHud._cardSig = null;
    factoryHud._trainRoundRobin = new Map();
    factoryHud._cancelRoundRobin = new Map();
    factoryHud._resourceIcons = {};
    renderCommandCard(factoryHud);
    const scoutCarButton = renderedButtons.find((button) => button.innerHTML.includes("Scout Car"));
    const tankButton = renderedButtons.find((button) => button.innerHTML.includes("Tank"));
    const tankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("TK+"));
    assert(scoutCarButton?.dataset.hotkey === "Q", "Scout Car training should keep the Q slot");
    assert(tankButton?.dataset.hotkey === "W", "Tank training should occupy the top-middle W slot");
    assert(!tankResearchButton, "Tank Production research should move out of Vehicle Works");

    renderedButtons.length = 0;
    factoryHud.state.upgrades = [UPGRADE.TANK_UNLOCK];
    factoryHud._cardSig = null;
    renderCommandCard(factoryHud);
    assert(
      !renderedButtons.some((button) => button.innerHTML.includes("TK+")),
      "completed Tank Production research should disappear from the command card",
    );

    renderedButtons.length = 0;
    const selectedGunWorks = {
      id: 79,
      owner: playerId,
      kind: KIND.STEELWORKS,
      buildProgress: null,
    };
    const gunWorksHud = Object.create(HUD.prototype);
    gunWorksHud.state = {
      playerId,
      resources: { steel: 300, oil: 200 },
      upgrades: [],
      selectedEntities: () => [selectedGunWorks],
      entitiesInterpolated: () => [selectedGunWorks],
    };
    gunWorksHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    gunWorksHud._cardSig = null;
    gunWorksHud._trainRoundRobin = new Map();
    gunWorksHud._cancelRoundRobin = new Map();
    gunWorksHud._resourceIcons = {};
    renderCommandCard(gunWorksHud);
    const mortarButton = renderedButtons.find((button) => button.innerHTML.includes("Mortar Team"));
    const atGunButton = renderedButtons.find((button) => button.innerHTML.includes("AT Gun"));
    const artilleryButton = renderedButtons.find((button) => button.innerHTML.includes("Artillery"));
    const atResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AT+"));
    const artilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    assert(mortarButton?.dataset.hotkey === "Q", "Mortar Team training should occupy the top-left Q slot");
    assert(atGunButton?.dataset.hotkey === "W", "AT Gun training should occupy the top-middle W slot");
    assert(artilleryButton?.dataset.hotkey === "E", "Artillery training should occupy the top-right E slot");
    assert(!atResearchButton, "AT Gun Crews research should move out of Gun Works");
    assert(!artilleryResearchButton, "Unlock Artillery research should move out of Gun Works");

    renderedButtons.length = 0;
    const selectedResearchComplex = {
      id: 80,
      owner: playerId,
      kind: KIND.RESEARCH_COMPLEX,
      buildProgress: null,
    };
    const rdHud = Object.create(HUD.prototype);
    rdHud.state = {
      playerId,
      resources: { steel: 500, oil: 500 },
      upgrades: [],
      selectedEntities: () => [selectedResearchComplex],
      entitiesInterpolated: () => [selectedResearchComplex],
    };
    rdHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    rdHud._cardSig = null;
    rdHud._trainRoundRobin = new Map();
    rdHud._cancelRoundRobin = new Map();
    rdHud._resourceIcons = {};
    renderCommandCard(rdHud);
    const rdAtResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AT+"));
    const rdArtilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    const rdTankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("TK+"));
    const rdMortarAutocastButton = renderedButtons.find((button) => button.innerHTML.includes("MT+"));
    assert(rdAtResearchButton?.dataset.hotkey === "Q", "AT Gun Crews research should appear in R&D Complex");
    assert(rdTankResearchButton?.dataset.hotkey === "E", "Tank Production research should appear in R&D Complex");
    assert(rdMortarAutocastButton?.dataset.hotkey === "A", "Mortar Autocast research should appear in R&D Complex");
    assert(rdArtilleryResearchButton?.dataset.hotkey === "W", "Unlock Artillery research should appear in R&D Complex");
    assert(rdArtilleryResearchButton?.disabled, "Artillery research should be disabled before AT Gun research");
    assert(rdArtilleryResearchButton?.title === "Requires AT Gun Research", "Artillery research should name AT Gun prerequisite");

    renderedButtons.length = 0;
    rdHud.state.upgrades = [UPGRADE.AT_GUN_UNLOCK];
    rdHud._cardSig = null;
    renderCommandCard(rdHud);
    const unlockedArtilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    assert(unlockedArtilleryResearchButton && !unlockedArtilleryResearchButton.disabled, "Artillery research should enable after AT Gun research");

    renderedButtons.length = 0;
    const playedNotices = [];
    let placements = 0;
    const selectedWorker = { id: 90, owner: playerId, kind: KIND.WORKER };
    const completedCityCentre = { id: 91, owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null };
    const shortResourceHud = Object.create(HUD.prototype);
    shortResourceHud.state = {
      playerId,
      resources: { steel: 100, oil: 0 },
      selectedEntities: () => [selectedWorker],
      entitiesInterpolated: () => [selectedWorker, completedCityCentre],
      beginPlacement() {
        placements += 1;
      },
    };
    shortResourceHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    shortResourceHud.audio = {
      play(id) {
        playedNotices.push(id);
      },
    };
    shortResourceHud._cardSig = null;
    shortResourceHud._resourceIcons = {};

    shortResourceHud.state.commandCardMode = "workerBuild";
    renderCommandCard(shortResourceHud);
    const barracksButton = renderedButtons.find((button) => button.innerHTML.includes("Barracks"));
    const factoryButton = renderedButtons.find((button) => button.innerHTML.includes("Vehicle Works"));
    assert(barracksButton && !barracksButton.disabled, "unlocked unaffordable build button stays clickable");
    assert(
      barracksButton.className.includes("unaffordable"),
      "unlocked unaffordable build button gets the intermediate visual class",
    );
    assert(factoryButton?.disabled, "tech-locked build button stays hard-disabled");

    barracksButton.click();
    assert(placements === 0, "clicking an unaffordable build button should not enter placement");
    assert(
      playedNotices[0] === "notice_steel",
      "clicking an unaffordable build button plays the missing-steel voice line",
    );

    globalThis.document.getElementById = (id) => {
      assert(id === "command-card", "unaffordable build hotkey should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "unaffordable build hotkey should query hotkey buttons");
          return [barracksButton];
        },
      };
    };
    input.state = shortResourceHud.state;
    input._activateCommandHotkey({
      code: `Key${barracksButton.dataset.hotkey}`,
      shiftKey: false,
      repeat: false,
      preventDefault() {},
    });
    assert(placements === 0, "unaffordable build hotkey should not enter placement");
    assert(playedNotices[1] === "notice_steel", "unaffordable build hotkey plays the missing-steel voice line");

    renderedButtons.length = 0;
    sent.length = 0;
    const selectedAtGun = { id: 88, owner: playerId, kind: KIND.AT_TEAM, setupState: SETUP.DEPLOYED };
    const selectedArtillery = { id: 89, owner: playerId, kind: KIND.ARTILLERY, setupState: SETUP.PACKED };
    const atGunHud = Object.create(HUD.prototype);
    atGunHud.state = {
      playerId,
      resources: { steel: 0, oil: 0 },
      commandTarget: null,
      selectedEntities: () => [selectedAtGun, selectedArtillery],
      entitiesInterpolated: () => [selectedAtGun, selectedArtillery],
      beginCommandTarget(kind) {
        this.commandTarget = kind;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    atGunHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    atGunHud._cardSig = null;

    renderCommandCard(atGunHud);
    const setupButton = renderedButtons.find((button) => button.innerHTML.includes("Set Up"));
    const tearDownButton = renderedButtons.find((button) => button.innerHTML.includes("Tear Down"));
    assert(setupButton?.dataset.hotkey, "AT gun Set Up button should keep its command-card hotkey");
    assert(!tearDownButton, "AT gun Tear Down should not occupy a command-card slot");

    const setupCommands = [];
    const setupInput = Object.create(Input.prototype);
    setupInput.state = {
      playerId,
      commandTarget: "setupAtGuns",
      selectedEntities: () => [selectedAtGun, selectedArtillery],
      addCommandFeedback() {},
    };
    setupInput.commandIssuer = { issueCommand: (command) => setupCommands.push(command) };
    setupInput._worldAt = (x, y) => ({ x, y });
    setupInput._selectedOwnUnitIds = () => [selectedAtGun.id, selectedArtillery.id];
    setupInput._issueTargetedCommand({ x: 160, y: 192 }, { shiftKey: true });
    assert(
      setupCommands[0]?.c === "setupAtGuns" &&
        setupCommands[0].units.includes(selectedAtGun.id) &&
        setupCommands[0].units.includes(selectedArtillery.id) &&
        setupCommands[0].queued === true,
      "setupAtGuns targeting includes selected artillery as setup-capable support weapons",
    );
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorMouseEvent === undefined) delete globalThis.MouseEvent;
    else globalThis.MouseEvent = priorMouseEvent;
  }
}

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------
{
  const start = {
    playerId: 1,
    tick: 0,
    map: {
      width: 4,
      height: 4,
      tileSize: 32,
      terrain: new Array(16).fill(0),
      resources: [
        { id: 200, kind: KIND.STEEL, x: 64, y: 96 },
        { id: 201, kind: KIND.OIL, x: 96, y: 96 },
      ],
    },
    players: [
      { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
    ],
  };
  const state = new GameState(start);
  assert(state instanceof GameState, "GameState constructor should return an instance");
  assert(state.playerId === 1, "GameState.playerId");
  assert(state.startInfo === start, "GameState.startInfo");
  assert(state.map.width === 4, "GameState.map");
  assert(state.map.resources.length === 2, "GameState keeps start payload resources");
  assert(state.resourceById.get(200).kind === KIND.STEEL, "GameState indexes resources by id");
  assert(state.resourceById.get(200).remaining === 1500, "steel defaults to full known amount");
  assert(state.resourceById.get(201).remaining === 5000, "oil defaults to full known amount");
  assert(Array.isArray(state.players), "GameState.players");
  assertHasMethod(state, "applySnapshot", "GameState");
  assertHasMethod(state, "entitiesInterpolated", "GameState");
  assertHasGetter(state, "prevRecvTime", "GameState");
  assertHasGetter(state, "currRecvTime", "GameState");
  assert(state.prevRecvTime === null, "prevRecvTime null before snapshots");
  assert(state.currRecvTime === null, "currRecvTime null before snapshots");
  assert(state.resources !== undefined, "GameState.resources");
  assert(Array.isArray(state.events), "GameState.events");
  assert(state.resourceMiningPreview === null, "GameState.resourceMiningPreview initially null");
  assert(state.atGunSetupPreview === null, "GameState.atGunSetupPreview initially null");
  assertHasMethod(state, "updateResourceMiningPreview", "GameState");
  assert(state.selection instanceof Set, "GameState.selection");
  assert(state.debugPathOverlaysAvailable === false, "GameState hides waypoint diagnostics by default");
  assert(state.debugPathOverlaysEnabled === false, "GameState leaves waypoint diagnostics off by default");
  assertHasMethod(state, "setSelection", "GameState");
  assertHasMethod(state, "addToSelection", "GameState");
  assertHasMethod(state, "clearSelection", "GameState");
  assertHasMethod(state, "selectedEntities", "GameState");
  assertHasMethod(state, "entityById", "GameState");
  assert(state.commandCardMode === null, "GameState.commandCardMode initially null");
  assertHasMethod(state, "openWorkerBuildMenu", "GameState");
  assertHasMethod(state, "closeCommandCardMenu", "GameState");
  assert(state.placement === null, "GameState.placement initially null");
  assertHasMethod(state, "beginPlacement", "GameState");
  assertHasMethod(state, "updatePlacement", "GameState");
  assertHasMethod(state, "endPlacement", "GameState");

  const debugState = new GameState({
    ...start,
    debugMode: true,
    map: {
      ...start.map,
      resources: start.map.resources.map((resource) => ({ ...resource })),
    },
  });
  assert(debugState.debugPathOverlaysAvailable === true, "GameState exposes waypoint diagnostics in debug mode");
  assert(debugState.debugPathOverlaysEnabled === true, "GameState enables waypoint diagnostics in debug mode");

  // Snapshot buffering
  const t0 = performance.now();
  state.applySnapshot({
    tick: 0,
    steel: 10,
    oil: 5,
    supplyUsed: 2,
    supplyCap: 10,
    entities: [{ id: 1, owner: 1, kind: "worker", x: 10, y: 20, hp: 40, maxHp: 40, state: "idle" }],
    resourceDeltas: [{ id: 200, remaining: 1498 }],
    events: [],
  });
  assert(state.currRecvTime !== null, "currRecvTime set after first snapshot");
  assert(state.prevRecvTime === null, "prevRecvTime still null after one snapshot");
  assert(state.resources.steel === 10, "resources updated");
  assert(state.entityById(200).kind === KIND.STEEL, "static resources are available as local entities");
  assert(state.entityById(200).remaining === 1498, "resourceDeltas update known resource state");

  state.applySnapshot({
    tick: 1,
    steel: 12,
    oil: 5,
    supplyUsed: 2,
    supplyCap: 10,
    entities: [{ id: 1, owner: 1, kind: "worker", x: 15, y: 25, hp: 40, maxHp: 40, state: "idle" }],
    events: [{ e: "death", id: 200, x: 64, y: 96, kind: KIND.STEEL }],
  });
  assert(state.prevRecvTime !== null, "prevRecvTime set after two snapshots");
  assert(state.entityById(200).remaining === 0, "visible resource death tombstones known resource");
  assert(state.entityById(201).remaining === 5000, "untouched resources keep their last-known amount");
  state.updateResourceMiningPreview({
    resourceId: 200,
    resourceX: 64,
    resourceY: 96,
    ccId: 3,
    ccX: 48,
    ccY: 48,
    inRange: true,
  });
  assert(state.resourceMiningPreview?.resourceId === 200, "resource mining preview stores hover link");
  state.updateResourceMiningPreview(null);
  assert(state.resourceMiningPreview === null, "resource mining preview can be cleared");
  state.updateAtGunSetupPreview({ mouseX: 1, mouseY: 2, guns: [{ id: 9 }] });
  assert(state.atGunSetupPreview?.guns?.[0]?.id === 9, "AT setup preview stores selected guns");
  state.endCommandTarget();
  assert(state.atGunSetupPreview === null, "ending command target clears AT setup preview");

  const artilleryState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  artilleryState.applySnapshot({
    tick: 10,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{ id: 10, owner: 1, kind: KIND.ARTILLERY, x: 300, y: 340, hp: 100, maxHp: 100, state: STATE.IDLE, weaponFacing: 0 }],
    events: [
      { e: EVENT.ARTILLERY_TARGET, from: 10, x: 320, y: 352, radiusTiles: 3, delayTicks: ARTILLERY_SHELL_DELAY_TICKS },
      { e: EVENT.ARTILLERY_IMPACT, x: 336, y: 368, radiusTiles: 3 },
    ],
  });
  assert(artilleryState.liveArtilleryTargets(performance.now()).length === 1, "artillery target event creates a live marker");
  assert(artilleryState.liveArtilleryLaunches(performance.now()).length === 1, "artillery target event creates launch dust");
  assert(artilleryState.weaponRecoil(10, KIND.ARTILLERY, performance.now()) > 0, "artillery target event starts firing-gun recoil");
  assert(artilleryState.liveArtilleryImpacts(performance.now()).length === 1, "artillery impact event creates a live explosion");
  assert(
    artilleryState.visibleTiles.length === 0,
    "artillery visual events do not stamp or extend client fog visibility",
  );

  const artilleryRevealState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  artilleryRevealState.applySnapshot({
    tick: 11,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: [{
      e: EVENT.ATTACK,
      from: 99,
      to: 99,
      reveal: {
        owner: 2,
        kind: KIND.ARTILLERY,
        x: 512,
        y: 544,
        facing: 0,
        weaponFacing: 0,
        setupState: SETUP.DEPLOYED,
      },
    }],
  });
  assert(artilleryRevealState.entityById(99)?.shotReveal === true, "artillery self-attack event creates a fog shot reveal");
  assert(artilleryRevealState.liveMuzzleFlashes(performance.now()).length === 0, "artillery self-reveal does not draw a tracer");
  assert(artilleryRevealState.weaponRecoil(99, KIND.ARTILLERY, performance.now()) > 0, "artillery self-reveal still recoils the gun");

  // Interpolation clamps alpha to [0,1]
  const entsNeg = state.entitiesInterpolated(-0.5);
  const entsOver = state.entitiesInterpolated(1.5);
  const entsMid = state.entitiesInterpolated(0.5);
  const midWorker = entsMid.find((e) => e.id === 1);
  assert(entsMid.length === 3 && midWorker, "entitiesInterpolated returns units and known resources");
  assert(midWorker.x >= 10 && midWorker.x <= 15, "interpolation works for moving units");
  assert(!("facing" in midWorker), "entitiesInterpolated does not add missing facing");

  const angleState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  angleState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "move", facing: 0 },
      {
        id: 11,
        owner: 1,
        kind: "tank",
        x: 0,
        y: 0,
        hp: 100,
        maxHp: 100,
        state: "move",
        facing: (170 * Math.PI) / 180,
        weaponFacing: (170 * Math.PI) / 180,
      },
      { id: 13, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle", facing: 0.5 },
      { id: 14, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle" },
    ],
    events: [],
  });
  angleState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 10, y: 20, hp: 40, maxHp: 40, state: "move", facing: Math.PI / 2 },
      {
        id: 11,
        owner: 1,
        kind: "tank",
        x: 0,
        y: 0,
        hp: 100,
        maxHp: 100,
        state: "move",
        facing: (-170 * Math.PI) / 180,
        weaponFacing: (-170 * Math.PI) / 180,
      },
      { id: 12, owner: 1, kind: "worker", x: 5, y: 5, hp: 40, maxHp: 40, state: "idle", facing: 1.25 },
      { id: 13, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle" },
      { id: 14, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle", facing: 0.75 },
    ],
    events: [],
  });
  const angleEnts = angleState.entitiesInterpolated(0.5);
  const quarterTurn = angleEnts.find((e) => e.id === 10);
  const wrapTurn = angleEnts.find((e) => e.id === 11);
  const newFacing = angleEnts.find((e) => e.id === 12);
  const missingCurrentFacing = angleEnts.find((e) => e.id === 13);
  const missingPriorFacing = angleEnts.find((e) => e.id === 14);
  assertApprox(quarterTurn.x, 5, 0.001, "x interpolation still works");
  assertApprox(quarterTurn.y, 10, 0.001, "y interpolation still works");
  assertApprox(quarterTurn.facing, Math.PI / 4, 0.001, "facing interpolates between snapshots");
  assertApprox(
    Math.abs(wrapTurn.facing),
    Math.PI,
    0.001,
    "facing interpolation uses the short path across angle wrap",
  );
  assertApprox(
    Math.abs(wrapTurn.weaponFacing),
    Math.PI,
    0.001,
    "weaponFacing interpolation uses the short path across angle wrap",
  );
  assertApprox(newFacing.facing, 1.25, 0.001, "missing prior entity keeps current facing");
  assert(!("facing" in missingCurrentFacing), "missing current facing does not add a field");
  assertApprox(missingPriorFacing.facing, 0.75, 0.001, "missing prior facing keeps current facing");

  // Selection resolves against current snapshot
  state.setSelection([1, 999]);
  const sel = state.selectedEntities();
  assert(sel.length === 1 && sel[0].id === 1, "selectedEntities drops stale ids");

  // Command-card submenu is local-only and is closed by mode-changing actions.
  state.openWorkerBuildMenu();
  assert(state.commandCardMode === "workerBuild", "worker build submenu opens");
  assert(state.closeCommandCardMenu() === true, "closeCommandCardMenu reports an open submenu");
  assert(state.closeCommandCardMenu() === false, "closeCommandCardMenu reports when no submenu was open");
  state.openWorkerBuildMenu();
  state.beginCommandTarget("attack");
  assert(state.commandCardMode === null, "command targeting closes the worker build submenu");
  assert(state.commandTarget === "attack", "command targeting mirrors the composer target");
  const queuedIssue = state.issueCommandTarget({ shiftKey: true });
  assert(queuedIssue.keepArmed && state.commandTarget === "attack", "Shift-issued command remains armed");
  state.releaseCommandTargetShift();
  assert(state.commandTarget === null, "Shift release clears a Shift-preserved command target");
  state.openWorkerBuildMenu();
  state.beginPlacement(KIND.DEPOT);
  assert(state.commandCardMode === null, "build placement closes the worker build submenu");
  state.openWorkerBuildMenu();
  state.setSelection([1]);
  assert(state.commandCardMode === null, "selection replacement closes the worker build submenu");

  // Control groups are local-only, own controllable entities only, and capped like selection.
  const cgState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  const ownControllables = Array.from({ length: 14 }, (_, i) => ({
    id: 100 + i,
    owner: 1,
    kind: i === 12 ? KIND.BARRACKS : KIND.WORKER,
    x: i * 10,
    y: 0,
    hp: 40,
    maxHp: 40,
    state: "idle",
  }));
  cgState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 20,
    entities: [
      ...ownControllables,
      { id: 160, owner: 2, kind: KIND.WORKER, x: 0, y: 20, hp: 40, maxHp: 40, state: "idle" },
      { id: 161, owner: 0, kind: KIND.STEEL, x: 0, y: 40, remaining: 100 },
    ],
    events: [],
  });
  assert(Array.isArray(cgState.controlGroups) && cgState.controlGroups.length === 10, "GameState has ten control groups");
  assertHasMethod(cgState, "setControlGroup", "GameState");
  assertHasMethod(cgState, "addToControlGroup", "GameState");
  assertHasMethod(cgState, "selectControlGroup", "GameState");
  assertHasMethod(cgState, "controlGroupEntities", "GameState");
  cgState.setControlGroup(0, [100, 160, 101, 161, 112, 113, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111]);
  assert(
    cgState.controlGroups[0].join(",") === "100,101,112,113,102,103,104,105,106,107,108,109",
    "control groups store own units/buildings only in selection order up to 12",
  );
  cgState.addToControlGroup(0, [110, 111, 112, 113]);
  assert(cgState.controlGroups[0].length === 12, "adding to a full control group ignores overflow");
  cgState.setControlGroup(1, [100, 101]);
  cgState.addToControlGroup(1, [101, 102, 103]);
  assert(cgState.controlGroups[1].join(",") === "100,101,102,103", "adding to a control group dedupes existing ids");
  cgState.selectControlGroup(1);
  assert(Array.from(cgState.selection).join(",") === "100,101,102,103", "selectControlGroup recalls live group ids");
  cgState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 20,
    entities: ownControllables.filter((e) => e.id !== 101),
    events: [{ e: "death", id: 101, x: 10, y: 0, kind: KIND.WORKER }],
  });
  assert(cgState.controlGroups[1].join(",") === "100,102,103", "dead entities disappear from control groups");

  // Placement is local-only
  state.beginPlacement("barracks");
  assert(state.placement !== null, "placement started");
  state.updatePlacement(2, 3, true);
  assert(state.placement.tileX === 2, "updatePlacement sets tileX");
  assert(state.placement.tileY === 3, "updatePlacement sets tileY");
  assert(state.placement.valid === true, "updatePlacement sets valid");
  state.endPlacement();
  assert(state.placement === null, "endPlacement clears placement");

  const map = { width: 6, height: 6, tileSize: 32, terrain: new Array(36).fill(0) };
  const worker = { id: 7, owner: 1, kind: "worker", x: 80, y: 80 };
  const other = { id: 8, owner: 1, kind: "worker", x: 80, y: 80 };
  assert(
    footprintValidAgainstEntities([worker], new Set([7]), 1, 1, 2, 2, map) === true,
    "client_preview_allows_chosen_worker_body_inside_footprint",
  );
  assert(
    footprintValidAgainstEntities([other], new Set([7]), 1, 1, 2, 2, map) === false,
    "client_preview_rejects_other_unit_body_inside_footprint",
  );
  const tank = { id: 9, owner: 1, kind: KIND.TANK, x: 116, y: 64 };
  assert(
    footprintValidAgainstEntities([tank], new Set(), 1, 1, 2, 2, map) === false,
    "client preview should reject a tank body touching a footprint edge",
  );
  assert(STATS[KIND.TANK].body.length === 50.4, "tank client body length mirrors server");
  assert(STATS[KIND.TANK].body.width === 28.8, "tank client body width mirrors server");
  assert(STATS[KIND.AT_TEAM].body.length === 42.0, "AT gun client body length mirrors server");
  assert(STATS[KIND.AT_TEAM].body.width === 24.0, "AT gun client body width mirrors server");
  assert(STATS[KIND.ARTILLERY].size === STATS[KIND.TANK].size, "Artillery selection size should match tank size");
  assert(
    STATS[KIND.ARTILLERY].body.length === STATS[KIND.TANK].body.length &&
      STATS[KIND.ARTILLERY].body.width === STATS[KIND.TANK].body.width,
    "Artillery client body should match tank footprint",
  );

  const input = Object.create(Input.prototype);
  input.state = {
    entitiesInterpolated: () => [worker, other],
  };
  input._selectedWorkerIds = () => [7, 8];
  assert(
    input._footprintValid(1, 1, 2, 2, map) === false,
    "preview should not ignore every selected worker",
  );
  input.state.entitiesInterpolated = () => [worker];
  assert(
    input._footprintValid(1, 1, 2, 2, map) === true,
    "preview should ignore one selected worker body as an advisory build-placement allowance",
  );

  const clickableTank = { id: 10, owner: 1, kind: KIND.TANK, x: 0, y: 0, facing: 0 };
  assert(
    input._worldPointHitsEntity(clickableTank, 25.2, 0, 32) === true,
    "tank hit testing should reach the long hull axis",
  );
  assert(
    input._worldPointHitsEntity(clickableTank, 0, 20, 32) === false,
    "tank hit testing should not use a stale circular side radius",
  );
  const clickableAtGun = { id: 11, owner: 1, kind: KIND.AT_TEAM, x: 0, y: 0, facing: 0 };
  assert(
    input._worldPointHitsEntity(clickableAtGun, 22, 0, 32) === true,
    "AT gun hit testing should reach the wheeled body axis",
  );
  assert(
    input._worldPointHitsEntity(clickableAtGun, 0, 18, 32) === false,
    "AT gun hit testing should not use the old circular radius",
  );

  const overlappingWorker = { id: 30, owner: 1, kind: KIND.WORKER, x: 100, y: 100 };
  const overlappingSteel = { id: 31, owner: 0, kind: KIND.STEEL, x: 104, y: 100, remaining: 1500 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [overlappingWorker, overlappingSteel],
    selectedEntities: () => [overlappingWorker],
    addCommandFeedback() {},
  };
  const rightClickCommands = [];
  input.commandIssuer = { issueCommand(command) { rightClickCommands.push(command); } };
  input._worldAt = (x, y) => ({ x, y });
  input._onRightClick({ x: 100, y: 100 });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "gather" &&
      rightClickCommands[0].node === overlappingSteel.id,
    "worker right-click should prioritize an overlapped resource patch over the worker body",
  );

  const moveUnit = { id: 40, owner: 1, kind: KIND.RIFLEMAN, x: 120, y: 120 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [moveUnit],
    selectedEntities: () => [moveUnit],
    addCommandFeedback() {},
  };
  rightClickCommands.length = 0;
  input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "move" &&
      rightClickCommands[0].queued === true,
    "Shift terrain right-click should send queued move",
  );

  const enemyUnit = { id: 41, owner: 2, kind: KIND.RIFLEMAN, x: 180, y: 180 };
  input.state.entitiesInterpolated = () => [moveUnit, enemyUnit];
  rightClickCommands.length = 0;
  input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "attack" &&
      rightClickCommands[0].queued === true,
    "Shift right-click on enemies should send queued attack",
  );

  input.dom = { clientWidth: 800, clientHeight: 600 };
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  const deployedAtGun = {
    id: 21,
    owner: 1,
    kind: KIND.AT_TEAM,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
  };
  const otherDeployedAtGun = {
    id: 22,
    owner: 1,
    kind: KIND.AT_TEAM,
    x: 120,
    y: 100,
    setupState: SETUP.DEPLOYED,
  };
  const packedAtGun = {
    id: 23,
    owner: 1,
    kind: KIND.AT_TEAM,
    x: 110,
    y: 100,
    setupState: SETUP.PACKED,
  };
  input.state = {
    playerId: 1,
    entitiesInterpolated: () => [deployedAtGun, otherDeployedAtGun, packedAtGun],
  };
  assert(
    input
      ._closestOwnUnitKindInViewport(
        KIND.AT_TEAM,
        deployedAtGun.x,
        deployedAtGun.y,
        deployedAtGun,
      )
      .join(",") === "21,22",
    "selecting set-up AT guns should not include packed AT guns",
  );
  assert(
    input
      ._closestOwnUnitKindInViewport(KIND.AT_TEAM, packedAtGun.x, packedAtGun.y, packedAtGun)
      .join(",") === "23",
    "selecting packed AT guns should not include set-up AT guns",
  );
  assert(
    input._closestOwnUnitKindInViewport(KIND.AT_TEAM, deployedAtGun.x, deployedAtGun.y).join(",") ===
      "21,23,22",
    "kind-only AT selection helper calls should keep legacy all-AT behavior",
  );

  assert(input._controlGroupSlotFromKey({ code: "Digit1" }) === 0, "Digit1 maps to control group slot 0");
  assert(input._controlGroupSlotFromKey({ code: "Digit0" }) === 9, "Digit0 maps to control group slot 9");
  assert(input._controlGroupSlotFromKey({ code: "Numpad5" }) === 4, "Numpad5 maps to control group slot 4");
  assert(input._controlGroupSlotFromKey({ code: "KeyQ" }) === null, "non-number keys do not map to control groups");

  const hotkeyCalls = [];
  const hotkeyInput = Object.create(Input.prototype);
  hotkeyInput.state = {
    spectator: false,
    selection: new Set([1, 2]),
    setControlGroup(slot, ids) {
      hotkeyCalls.push({ type: "set", slot, ids: Array.from(ids) });
      return Array.from(ids);
    },
    addToControlGroup(slot, ids) {
      hotkeyCalls.push({ type: "add", slot, ids: Array.from(ids) });
      return Array.from(ids);
    },
    selectControlGroup(slot) {
      hotkeyCalls.push({ type: "select", slot });
      return [1, 2];
    },
  };
  hotkeyInput._lastControlGroupTap = null;
  hotkeyInput._jumpToControlGroupCluster = (slot) => hotkeyCalls.push({ type: "jump", slot });
  const keyEvent = (code, mods = {}) => ({
    code,
    altKey: !!mods.altKey,
    ctrlKey: !!mods.ctrlKey,
    metaKey: !!mods.metaKey,
    shiftKey: !!mods.shiftKey,
    repeat: !!mods.repeat,
    preventDefault() { this.prevented = true; },
    stopPropagation() { this.stopped = true; },
  });
  const saveEvent = keyEvent("Digit2", { altKey: true });
  assert(hotkeyInput._handleControlGroupHotkey(saveEvent) === true, "Alt+number saves a control group");
  assert(saveEvent.prevented && saveEvent.stopped, "handled control-group hotkeys prevent browser handling");
  const addEvent = keyEvent("Digit2", { shiftKey: true });
  assert(hotkeyInput._handleControlGroupHotkey(addEvent) === true, "Shift+number adds to a control group");
  hotkeyInput._handleControlGroupHotkey(keyEvent("Digit2"));
  hotkeyInput._handleControlGroupHotkey(keyEvent("Digit2"));
  assert(
    hotkeyCalls.map((c) => c.type).join(",") === "set,add,select,select,jump",
    "plain number recalls, and double-tap recalls then jumps",
  );

  const repeatHotkeyInput = Object.create(Input.prototype);
  repeatHotkeyInput.keys = {};
  repeatHotkeyInput.pointerLocked = false;
  repeatHotkeyInput._handleControlGroupHotkey = () => false;
  let repeatClicks = 0;
  let repeatable = true;
  globalThis.document = {
    getElementById(id) {
      assert(id === "command-card", "repeated command hotkeys should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "repeated command hotkeys should query hotkey buttons");
          return [{
            dataset: { hotkey: "W", repeatable: repeatable ? "true" : "false" },
            disabled: false,
            click() {
              repeatClicks += 1;
            },
          }];
        },
      };
    },
  };
  repeatHotkeyInput._handleKeyDown(keyEvent("KeyW", { repeat: true }));
  repeatable = false;
  repeatHotkeyInput._handleKeyDown(keyEvent("KeyW", { repeat: true }));
  assert(repeatClicks === 1, "only repeatable command-card buttons respond to native key repeat");

  const menuCancelInput = Object.create(Input.prototype);
  let menuClosed = 0;
  let selectionCleared = 0;
  menuCancelInput.state = {
    placement: null,
    commandTarget: null,
    closeCommandCardMenu() {
      menuClosed += 1;
      return true;
    },
    clearSelection() {
      selectionCleared += 1;
    },
  };
  menuCancelInput._cancel();
  assert(menuClosed === 1, "Esc closes the worker build submenu first");
  assert(selectionCleared === 0, "Esc returning to worker commands does not clear selection");

  const clusterInput = Object.create(Input.prototype);
  let centered = null;
  clusterInput.camera = {
    viewW: 100,
    viewH: 100,
    zoom: 1,
    x: 0,
    y: 0,
    centerOn(x, y) { centered = { x, y }; },
  };
  clusterInput.state = {
    controlGroupEntities: () => [
      { id: 1, x: 0, y: 0 },
      { id: 2, x: 20, y: 0 },
      { id: 3, x: 500, y: 500 },
    ],
  };
  assert(clusterInput._jumpToControlGroupCluster(0) === true, "control-group double-tap jumps to a cluster");
  assert(centered.x < 100 && centered.y < 100, "control-group jump chooses the dense cluster, not the all-entity centroid");

  const ownBuilding = {
    id: 31,
    owner: 1,
    kind: KIND.BARRACKS,
    x: 200,
    y: 200,
  };
  const targetedInput = Object.create(Input.prototype);
  const sentCommands = [];
  const selectionClicks = [];
  const feedback = [];
  targetedInput.state = {
    placement: null,
    commandTarget: "attack",
    commandComposer: new CommandComposer(),
    playerId: 1,
    addCommandFeedback(kind, x, y) {
      feedback.push({ kind, x, y });
    },
    endCommandTarget() {
      this.commandComposer.cancel();
      this.commandTarget = null;
    },
    issueCommandTarget(ev = {}) {
      const issued = this.commandComposer.issue(ev);
      this.commandTarget = this.commandComposer.target;
      return issued;
    },
    holdCommandTarget(kind, key, shiftKey = false) {
      this.commandComposer.hold(kind, key, { shiftKey });
      this.commandTarget = this.commandComposer.target;
    },
    releaseCommandTargetKey(key, shiftKey = false) {
      this.commandComposer.releaseKey(key, { shiftKey });
      this.commandTarget = this.commandComposer.target;
    },
  };
  targetedInput.state.commandComposer.arm("attack");
  targetedInput.renderer = { drawSelectionBox() {} };
  targetedInput.commandIssuer = { issueCommand: (command) => sentCommands.push(command) };
  targetedInput._worldAt = (x, y) => ({ x, y });
  targetedInput._entityAtWorld = () => ownBuilding;
  targetedInput._selectedOwnUnitIds = () => [7];
  targetedInput._commitClickSelection = (p) => selectionClicks.push(p);
  targetedInput._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  targetedInput._trackMouse = () => {};
  targetedInput._onLeftDown({ x: 200, y: 200 }, {});
  assert(targetedInput.state.commandTarget === null, "attack targeting clears after one click");
  assert(sentCommands.length === 1, "own click while attack targeting should issue one command");
  assert(sentCommands[0].c === "attackMove", "own click while attack targeting should attack-move");
  assert(sentCommands[0].units.join(",") === "7", "attack-move should use selected own units");
  assert(sentCommands[0].x === 200 && sentCommands[0].y === 200, "attack-move should go to the clicked own position");
  assert(feedback.length === 1 && feedback[0].kind === "attack", "own attack-move click should show attack feedback");
  assert(targetedInput._drag == null, "attack targeting should not fall through to selection on the same click");
  targetedInput._handleMouseUp({
    button: 0,
    clientX: 200,
    clientY: 200,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(selectionClicks.length === 0, "attack targeting click should not also select on mouse-up");

  targetedInput.state.commandTarget = null;
  targetedInput._drag = null;
  targetedInput._entityAtWorld = () => null;
  targetedInput._onLeftDown({ x: 240, y: 240 }, {});
  targetedInput._handleMouseUp({
    button: 0,
    clientX: 240,
    clientY: 240,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(sentCommands.length === 1, "a second click without another A press should not issue attack-move");
  assert(selectionClicks.length === 1, "a second click without another A press should be normal selection");

  targetedInput.state.commandTarget = "move";
  targetedInput.state.commandComposer.arm("move");
  targetedInput._onLeftDown({ x: 260, y: 260 }, { shiftKey: true });
  let lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "move", "move targeting should issue a move command");
  assert(lastSent.queued === true, "Shift move targeting should queue movement");

  targetedInput.state.commandTarget = "attack";
  targetedInput.state.commandComposer.arm("attack");
  targetedInput._entityAtWorld = () => null;
  targetedInput._onLeftDown({ x: 280, y: 280 }, { shiftKey: true });
  lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "attackMove", "attack targeting terrain should attack-move");
  assert(lastSent.queued === true, "Shift attack-move targeting should queue attack-move");

  targetedInput.state.commandTarget = "attack";
  targetedInput.state.commandComposer.arm("attack");
  targetedInput._entityAtWorld = () => ({ id: 99, owner: 2, kind: KIND.RIFLEMAN, x: 300, y: 300 });
  targetedInput._onLeftDown({ x: 300, y: 300 }, { shiftKey: true });
  lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "attack", "attack targeting an enemy should issue attack");
  assert(
    lastSent.queued === true,
    "Shift enemy attack targeting should queue attack",
  );

  targetedInput.state.commandTarget = "attack";
  targetedInput.state.commandComposer.hold("attack", "KeyA", { shiftKey: true });
  targetedInput._entityAtWorld = () => null;
  targetedInput._onLeftDown({ x: 320, y: 320 }, { shiftKey: true });
  assert(
    targetedInput.state.commandTarget === "attack",
    "Shift attack targeting should stay armed while A is held",
  );
  targetedInput._onLeftDown({ x: 340, y: 340 }, { shiftKey: true });
  assert(
    sentCommands.at(-2).c === "attackMove" &&
      sentCommands.at(-2).queued === true &&
      sentCommands.at(-1).c === "attackMove" &&
      sentCommands.at(-1).queued === true,
    "held A plus Shift should queue multiple attack-move orders",
  );
  targetedInput._onLeftDown({ x: 360, y: 360 }, { shiftKey: false });
  assert(
    targetedInput.state.commandTarget === "attack",
    "held A keeps attack targeting armed after an unqueued click",
  );

  targetedInput.state.commandComposer.cancel();
  targetedInput.state.commandTarget = "attack";
  targetedInput.state.commandComposer.hold("attack", "KeyA");
  targetedInput._handleKeyUp({ code: "KeyA", preventDefault() {} });
  assert(targetedInput.state.commandTarget === null, "A keyup exits sticky attack targeting");

  const originalDocument = globalThis.document;
  const hotkeyTargetedInput = Object.create(Input.prototype);
  const hotkeyIssues = [];
  hotkeyTargetedInput.mouse = { x: 420, y: 260 };
  hotkeyTargetedInput._handleControlGroupHotkey = () => false;
  hotkeyTargetedInput._quickCastCommandTarget = (ev) => {
    hotkeyIssues.push({ shiftKey: !!ev.shiftKey, mouse: hotkeyTargetedInput.mouse });
    return Input.prototype._quickCastCommandTarget.call(hotkeyTargetedInput, ev);
  };
  hotkeyTargetedInput._issueTargetedCommand = (p, ev) => {
    hotkeyIssues.push({ issuedAt: p, queued: !!ev.shiftKey });
  };
  hotkeyTargetedInput.state = {
    commandTarget: null,
    commandComposer: new CommandComposer(),
    lastCommandTargetArm: null,
    beginCommandTarget(kind, options = {}) {
      const armed = this.commandComposer.arm(kind, options);
      this.lastCommandTargetArm = armed;
      this.commandTarget = this.commandComposer.target;
      return armed;
    },
    endCommandTarget() {
      this.commandComposer.cancel();
      this.commandTarget = null;
      this.lastCommandTargetArm = null;
    },
    issueCommandTarget(ev = {}) {
      const issued = this.commandComposer.issue(ev);
      this.commandTarget = this.commandComposer.target;
      return issued;
    },
    holdCommandTarget(kind, key, shiftKey = false) {
      this.commandComposer.hold(kind, key, { shiftKey });
      this.commandTarget = this.commandComposer.target;
    },
    releaseCommandTargetKey(key, shiftKey = false) {
      this.commandComposer.releaseKey(key, { shiftKey });
      this.commandTarget = this.commandComposer.target;
    },
    releaseCommandTargetShift() {
      this.commandComposer.releaseShift();
      this.commandTarget = this.commandComposer.target;
    },
  };
  globalThis.document = {
    getElementById(id) {
      assert(id === "command-card", "command hotkeys should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "command hotkeys should query hotkey buttons");
          return [{
            dataset: { hotkey: "Y" },
            disabled: false,
            click() {
              hotkeyTargetedInput.state.beginCommandTarget("attack", { now: 100 + hotkeyIssues.length * 100 });
            },
          }];
        },
      };
    },
  };
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyA"));
  assert(
    hotkeyTargetedInput.state.commandTarget === null && hotkeyIssues.length === 0,
    "unbound legacy A key should not arm attack when Attack is rebound",
  );
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY"));
  hotkeyTargetedInput._handleKeyUp({ code: "KeyY", shiftKey: false, preventDefault() {} });
  assert(
    hotkeyTargetedInput.state.commandTarget === "attack",
    "plain targeted-order hotkey tap should stay armed after keyup",
  );
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY"));
  assert(
    hotkeyIssues.some((entry) => entry.issuedAt === hotkeyTargetedInput.mouse && entry.queued === false),
    "second same targeted-order hotkey should quick-cast at the cursor",
  );
  assert(
    hotkeyTargetedInput.state.commandTarget === null,
    "unqueued quick-cast should consume the armed targeted order",
  );

  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY", { shiftKey: true }));
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY", { shiftKey: true }));
  assert(
    hotkeyIssues.some((entry) => entry.issuedAt === hotkeyTargetedInput.mouse && entry.queued === true),
    "Shift double-tap targeted-order hotkey should quick-cast a queued order at the cursor",
  );
  assert(
    hotkeyTargetedInput.state.commandTarget === "attack",
    "Shift quick-cast should keep the targeted order armed until Shift is released",
  );
  hotkeyTargetedInput._handleKeyUp({ code: "KeyY", shiftKey: true, preventDefault() {} });
  hotkeyTargetedInput._handleKeyUp({ code: "ShiftLeft", preventDefault() {} });
  assert(hotkeyTargetedInput.state.commandTarget === null, "Shift release clears the queued hotkey target");
  globalThis.document = originalDocument;

  const placementKeyInput = Object.create(Input.prototype);
  let placementEnded = 0;
  let commandTargetShiftReleased = 0;
  let shiftKeyupPrevented = false;
  placementKeyInput.state = {
    placement: { building: KIND.DEPOT, tileX: 2, tileY: 3, valid: true },
    releaseCommandTargetShift() {
      commandTargetShiftReleased += 1;
    },
    endPlacement() {
      placementEnded += 1;
      this.placement = null;
    },
  };
  placementKeyInput._handleKeyUp({
    code: "ShiftRight",
    preventDefault() {
      shiftKeyupPrevented = true;
    },
  });
  assert(commandTargetShiftReleased === 1, "Shift release still clears command-target preservation");
  assert(placementEnded === 1 && placementKeyInput.state.placement === null, "Shift release clears build placement");
  assert(shiftKeyupPrevented === true, "Shift placement release prevents browser default");

  const placementBlurInput = Object.create(Input.prototype);
  let blurPlacementEnded = 0;
  placementBlurInput.pointerLocked = false;
  placementBlurInput.keys = { up: true, down: true, left: true, right: true };
  placementBlurInput.mouse = { x: 1, y: 2 };
  placementBlurInput._spacePan = true;
  placementBlurInput._panDrag = { x: 1, y: 2, button: 1 };
  placementBlurInput._drag = null;
  placementBlurInput.state = {
    placement: { building: KIND.DEPOT, tileX: 2, tileY: 3, valid: true },
    endCommandTarget() {},
    endPlacement() {
      blurPlacementEnded += 1;
      this.placement = null;
    },
  };
  placementBlurInput._handleBlur();
  assert(blurPlacementEnded === 1 && placementBlurInput.state.placement === null, "window blur clears build placement");

  const placementConfirmInput = Object.create(Input.prototype);
  const placementCommands = [];
  let confirmedPlacementEnded = 0;
  placementConfirmInput.commandIssuer = {
    command(command) {
      placementCommands.push(command);
    },
  };
  placementConfirmInput.state = {
    placement: { building: KIND.DEPOT, tileX: 4, tileY: 5, valid: true },
    endPlacement() {
      confirmedPlacementEnded += 1;
      this.placement = null;
    },
  };
  placementConfirmInput._selectedWorkerIds = () => [77];
  placementConfirmInput._confirmPlacement();
  assert(
    placementCommands.length === 1 &&
      placementCommands[0].c === "build" &&
      placementCommands[0].building === KIND.DEPOT &&
      placementCommands[0].tileX === 4 &&
      placementCommands[0].tileY === 5,
    "build placement confirm should send through a legacy one-argument command sender",
  );
  assert(confirmedPlacementEnded === 1, "build placement confirm exits placement after sending");

  const artilleryCommands = [];
  const artilleryFeedback = [];
  const selectedArtillery = { id: 44, owner: 1, kind: KIND.ARTILLERY, x: 100, y: 100 };
  const pointFireInput = Object.create(Input.prototype);
  pointFireInput.mouse = { x: 900, y: 100 };
  pointFireInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    commandTarget: { kind: "ability", ability: ABILITY.POINT_FIRE },
    selectedEntities: () => [selectedArtillery],
    updateAbilityTargetPreview(preview) {
      this.abilityTargetPreview = preview;
    },
    addCommandFeedback(kind, x, y, queued, radiusTiles) {
      artilleryFeedback.push({ kind, x, y, queued, radiusTiles });
    },
  };
  pointFireInput.commandIssuer = { issueCommand: (command) => artilleryCommands.push(command) };
  pointFireInput._worldAt = (x, y) => ({ x, y });
  pointFireInput._selectedOwnUnitIds = () => [selectedArtillery.id];
  pointFireInput._issueTargetedCommand({ x: 920, y: 116 }, { shiftKey: true });
  assert(
    artilleryCommands[0]?.c === "useAbility" &&
      artilleryCommands[0].ability === ABILITY.POINT_FIRE &&
      artilleryCommands[0].units[0] === selectedArtillery.id &&
      artilleryCommands[0].queued === true,
    "Point Fire targeting issues the dedicated pointFire ability command",
  );
  assert(
    artilleryFeedback[0]?.kind === "artillery" && artilleryFeedback[0].radiusTiles === ABILITIES[ABILITY.POINT_FIRE].radiusTiles,
    "Point Fire targeting shows artillery command feedback with splash radius",
  );

  pointFireInput.mouse = { x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8, y: selectedArtillery.y };
  pointFireInput._refreshAbilityTargetPreview();
  assert(pointFireInput.state.abilityTargetPreview?.hoverInRange === false, "Point Fire preview rejects the minimum range dead zone");
  assert(pointFireInput.state.abilityTargetPreview?.hoverInsideMinRange === true, "Point Fire preview identifies minimum range invalidity");
  assert(
    pointFireInput.state.abilityTargetPreview?.minRangePx === ARTILLERY_MIN_RANGE_TILES * 32,
    "Point Fire preview exposes minimum range in pixels",
  );
  pointFireInput.mouse = { x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 + 16, y: selectedArtillery.y };
  pointFireInput._refreshAbilityTargetPreview();
  assert(pointFireInput.state.abilityTargetPreview?.hoverInRange === true, "Point Fire preview accepts targets past minimum range");
  assert(pointFireInput.state.abilityTargetPreview?.hoverInsideMinRange === false, "Point Fire preview clears minimum range invalidity outside the dead zone");

  const previewGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: previewGfx },
    { abilityTargetPreview: { ...pointFireInput.state.abilityTargetPreview, carriers: [] } },
  );
  const validHorizontalStroke = previewGfx.calls.some(
    (call, i, calls) =>
      call[0] === "moveTo" &&
      call[2] === pointFireInput.state.abilityTargetPreview.mouseY &&
      calls[i + 1]?.[0] === "lineTo" &&
      calls[i + 1]?.[2] === pointFireInput.state.abilityTargetPreview.mouseY,
  );
  assert(validHorizontalStroke, "Point Fire valid cursor keeps the crosshair stroke");

  pointFireInput.mouse = { x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8, y: selectedArtillery.y };
  pointFireInput._refreshAbilityTargetPreview();
  const invalidGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: invalidGfx },
    { abilityTargetPreview: { ...pointFireInput.state.abilityTargetPreview, carriers: [] } },
  );
  const invalidDiagonalStroke = invalidGfx.calls.some(
    (call, i, calls) =>
      call[0] === "moveTo" &&
      call[2] < pointFireInput.state.abilityTargetPreview.mouseY &&
      calls[i + 1]?.[0] === "lineTo" &&
      calls[i + 1]?.[2] > pointFireInput.state.abilityTargetPreview.mouseY,
  );
  assert(invalidDiagonalStroke, "Point Fire invalid minimum-range cursor draws an X");
}

{
  const artilleryEntity = {
    id: 700,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 128,
    y: 160,
    facing: 0,
    weaponFacing: 0,
    setupState: SETUP.PACKED,
    state: STATE.IDLE,
  };
  const fakePools = new Map();
  const fakeRenderer = {
    _tankMotion: new Map(),
    _tankMotionVisual,
    _slot(pool, id) {
      const key = `${pool}:${id}`;
      if (!fakePools.has(key)) fakePools.set(key, new FakeGraphics());
      return fakePools.get(key);
    },
    _tintFor() {
      return 0x4878c8;
    },
    _vehicleShadow() {},
    _shadow() {},
    _deployedWeaponSetupVisual() {
      return { prongFactor: 0, barrel: false };
    },
  };
  _drawUnit.call(fakeRenderer, artilleryEntity, new Map([[1, 0x4878c8]]), {
    playerId: 1,
    resources: { oil: 10 },
  });
  assert(fakePools.has("units:700"), "Artillery renderer draws without a null vehicle body");
}

// ---------------------------------------------------------------------------
// Command composer
// ---------------------------------------------------------------------------
{
  const composer = new CommandComposer();
  let armed = composer.arm("attack", { now: 100 });
  assert(!armed.quickCast, "first command tap arms without quick-casting");
  armed = composer.arm("attack", { now: 220 });
  assert(armed.quickCast, "second same command tap inside the window requests quick-cast");

  let issued = composer.issue({ shiftKey: true });
  assert(issued.queued === true && issued.keepArmed === true, "Shift-click queues and preserves a tapped command");
  issued = composer.issue({ shiftKey: true });
  assert(issued.keepArmed === true, "Shift-preserved command can issue repeatedly");
  composer.releaseShift();
  assert(composer.target === null, "releasing Shift clears a Shift-preserved tapped command");

  composer.arm({ kind: "ability", ability: ABILITY.SMOKE }, { source: "hold", key: "KeyQ" });
  issued = composer.issue({ shiftKey: false });
  assert(
    issued.target.kind === "ability" &&
      issued.target.ability === ABILITY.SMOKE &&
      issued.keepArmed === true,
    "held ability key keeps the target armed after a click",
  );
  composer.releaseKey("KeyQ", { shiftKey: true });
  assert(composer.target?.ability === ABILITY.SMOKE, "Shift preserves the last held ability after key release");
  composer.releaseShift();
  assert(composer.target === null, "Shift release clears the preserved held ability");

  composer.arm("move");
  composer.cancel();
  assert(composer.target === null, "cancel clears the armed command");
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------
{
  const cam = new Camera(800, 600);
  assert(cam instanceof Camera, "Camera constructor should return an instance");
  assert(typeof cam.x === "number", "Camera.x");
  assert(typeof cam.y === "number", "Camera.y");
  assert(typeof cam.zoom === "number", "Camera.zoom");
  assertHasMethod(cam, "update", "Camera");
  assertHasMethod(cam, "worldToScreen", "Camera");
  assertHasMethod(cam, "screenToWorld", "Camera");
  assertHasMethod(cam, "centerOn", "Camera");
  assertHasMethod(cam, "setBounds", "Camera");
  assertHasMethod(cam, "setView", "Camera");

  cam.setBounds(1000, 800, 800, 600);
  cam.centerOn(500, 400);
  assert(cam.x >= 0 && cam.y >= 0, "Camera clamped after centerOn");

  // Inverse check
  const world = { x: 123, y: 456 };
  const screen = cam.worldToScreen(world.x, world.y);
  const back = cam.screenToWorld(screen.x, screen.y);
  assert(Math.abs(back.x - world.x) < 0.001, "worldToScreen / screenToWorld inverse x");
  assert(Math.abs(back.y - world.y) < 0.001, "worldToScreen / screenToWorld inverse y");

  cam.setView({ x: 120, y: 140, zoom: 1.25 });
  assertApprox(cam.x, 120, 0.001, "Camera.setView restores x");
  assertApprox(cam.y, 140, 0.001, "Camera.setView restores y");
  assertApprox(cam.zoom, 1.25, 0.001, "Camera.setView restores zoom");
}

// ---------------------------------------------------------------------------
// Fog
// ---------------------------------------------------------------------------
{
  const fog = new Fog(8, 8);
  assert(fog instanceof Fog, "Fog constructor should return an instance");
  assert(fog.width === 8 && fog.height === 8, "Fog dimensions");
  assert(fog.visibleGrid instanceof Uint8Array, "Fog.visibleGrid is Uint8Array");
  assert(fog.exploredGrid instanceof Uint8Array, "Fog.exploredGrid is Uint8Array");
  assertHasMethod(fog, "update", "Fog");
  assertHasMethod(fog, "isVisible", "Fog");
  assertHasMethod(fog, "isExplored", "Fog");

  // Out of bounds returns false
  assert(fog.isVisible(-1, 0) === false, "isVisible out-of-bounds left");
  assert(fog.isVisible(0, -1) === false, "isVisible out-of-bounds top");
  assert(fog.isVisible(8, 0) === false, "isVisible out-of-bounds right");
  assert(fog.isVisible(0, 8) === false, "isVisible out-of-bounds bottom");
  assert(fog.isExplored(-1, 0) === false, "isExplored out-of-bounds");

  // Visibility accumulation
  fog.update(
    [{ kind: "worker", x: 64, y: 64 }], // center of tile (2,2) at ts=32
    32,
  );
  assert(fog.isVisible(2, 2) === true, "tile under entity should be visible");
  assert(fog.isExplored(2, 2) === true, "tile under entity should be explored");

  // After clearing visible, explored should persist
  fog.update([], 32);
  assert(fog.isVisible(2, 2) === false, "tile should no longer be visible");
  assert(fog.isExplored(2, 2) === true, "tile should still be explored");

  const terrain = new Array(8 * 8).fill(TERRAIN.GRASS);
  terrain[2 * 8 + 3] = TERRAIN.ROCK;
  const blockedFog = new Fog(8, 8, terrain);
  blockedFog.update(
    [{ kind: "worker", x: 48, y: 80 }], // center of tile (1,2)
    32,
  );
  assert(blockedFog.isVisible(3, 2) === true, "stone tile itself should be visible");
  assert(blockedFog.isVisible(4, 2) === false, "stone should block fog behind it");
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------
{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorLocalStorage = globalThis.localStorage;
  globalThis.window = {
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.document = {
    hidden: false,
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.localStorage = {
    getItem() { return null; },
    setItem() {},
  };

  const audio = new Audio();
  assertHasMethod(audio, "play", "Audio");
  assertHasMethod(audio, "playUI", "Audio");
  assertHasMethod(audio, "stopByKey", "Audio");
  assertHasMethod(audio, "preload", "Audio");
  assertHasMethod(audio, "setListener", "Audio");
  assertHasMethod(audio, "pickVariant", "Audio");
  audio.setListener(100, 100, 2, 800);
  assertApprox(audio.listener.refDist, 400, 0.001, "Audio listener refDist derives from zoom");

  const near = audio._computeSpatial(300, 100);
  assert(near !== null, "Audio spatial near emitter should play");
  assertApprox(near.gain, 1, 0.001, "Audio spatial gain is flat inside refDist");
  assertApprox(near.pan, 0.5, 0.001, "Audio spatial pan uses dx/refDist");

  const mid = audio._computeSpatial(900, 100);
  assert(mid !== null, "Audio spatial off-viewport emitter should play");
  assertApprox(mid.gain, 1 / 3, 0.001, "Audio spatial gain doubles far-distance attenuation");

  const far = audio._computeSpatial(1300, 100);
  assert(far !== null, "Audio spatial max-distance edge should play");
  assertApprox(far.gain, 1 / 5, 0.001, "Audio spatial gain attenuates harder at maxDist");
  assertApprox(far.lpHz, 1200, 0.001, "Audio spatial lowpass reaches far cutoff");
  assert(audio._computeSpatial(1301, 100) === null, "Audio drops sounds beyond maxDist");

  const priorPerformance = globalThis.performance;
  let now = 0;
  globalThis.performance = { now: () => now };

  let stopped = 0;
  let disconnected = 0;
  const keyedVoice = (key) => ({
    key,
    node: {
      onended: () => {},
      stop() { stopped += 1; },
    },
    trail: [{ disconnect() { disconnected += 1; } }],
  });
  audio.voices = [keyedVoice("mg:1"), keyedVoice("other"), keyedVoice("mg:1")];
  assert(audio.stopByKey("mg:1") === 2, "Audio.stopByKey reports stopped voices");
  assert(stopped === 2, "Audio.stopByKey stops matching voices");
  assert(disconnected === 2, "Audio.stopByKey disconnects matching voice nodes");
  assert(
    audio.voices.length === 1 && audio.voices[0].key === "other",
    "Audio.stopByKey keeps unrelated voices active",
  );
  audio.voices = [];

  audio.ctx = fakeAudioContext();
  audio.master = fakeGain();
  audio.gains = {
    ui: fakeGain(),
    alert: fakeGain(),
    combat_self: fakeGain(),
    combat_other: fakeGain(),
    unit_voice: fakeGain(),
    ambient: fakeGain(),
  };
  for (const [cat, gain] of Object.entries(audio.gains)) {
    gain.gain.value = audio.getCategoryVolume(cat);
  }

  for (let i = 0; i < 200; i++) audio.buffers.set(`pool_${i}`, { duration: 0.1 });
  for (let i = 0; i < 120; i++) {
    audio.play(`pool_${i}`, { category: "ambient" });
    assert(audio.voices.length <= 48, "ambient voice pool stays capped");
    now += 1;
  }
  for (let i = 120; i < 200; i++) {
    audio.play(`pool_${i}`, { category: "alert" });
    assert(audio.voices.length <= 48, "alert voice pool stays capped");
    now += 1;
  }
  assert(audio.voices.length <= 48, "Audio voice pool stays capped");
  assert(audio.voices.every((v) => v.category === "alert"), "Audio priority eviction keeps highest-priority voices");

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("notice_under_attack", { duration: 0.5 });
  now = 10_000;
  assert(
    audio.play("notice_under_attack", {
      category: "alert",
      alertId: "under_attack",
      alertX: 100,
      alertY: 100,
    }),
    "first under-attack alert plays",
  );
  assert(
    !audio.play("notice_under_attack", {
      category: "alert",
      alertId: "under_attack",
      alertX: 120,
      alertY: 140,
    }),
    "under-attack alert dedups within the same spatial bucket",
  );
  assert(
    audio.play("notice_under_attack", {
      category: "alert",
      alertId: "under_attack",
      alertX: 2000,
      alertY: 100,
    }),
    "under-attack alert plays in a different spatial bucket",
  );

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("notice_supply", { duration: 2.3 });
  now = 30_000;
  assert(audio.play("notice_supply", { category: "alert" }), "first spoken alert plays");
  now += 1500;
  assert(!audio.play("notice_supply", { category: "alert" }), "spoken alert cooldown honors buffer duration");
  now += 801;
  assert(audio.play("notice_supply", { category: "alert" }), "spoken alert plays after buffer-duration cooldown");

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("duck_alert", { duration: 0.1 });
  now = 40_000;
  const ambientBefore = audio.gains.ambient.gain.value;
  const combatBefore = audio.gains.combat_self.gain.value;
  assert(audio.play("duck_alert", { category: "alert" }), "ducking alert plays");
  assert(audio.gains.ambient.gain.value < ambientBefore, "alert ducks ambient bus");
  assert(audio.gains.combat_self.gain.value < combatBefore, "alert ducks combat bus");
  audio.voices.slice().forEach((v) => v.node.stop());
  assertApprox(audio.gains.ambient.gain.value, audio.getCategoryVolume("ambient"), 0.0001, "ambient bus restores");
  assertApprox(audio.gains.combat_self.gain.value, audio.getCategoryVolume("combat_self"), 0.0001, "combat bus restores");

  audio.destroy();
  globalThis.window = priorWindow;
  globalThis.document = priorDocument;
  globalThis.localStorage = priorLocalStorage;
  globalThis.performance = priorPerformance;
}

// ---------------------------------------------------------------------------
// Combat audio
// ---------------------------------------------------------------------------
{
  assert(
    machineGunnerHasAudibleTarget({
      kind: KIND.MACHINE_GUNNER,
      state: STATE.MOVE,
      setupState: SETUP.TEARING_DOWN,
      targetId: 7,
    }),
    "MG combat loop stays active while the machine gunner still has a target",
  );
  assert(
    !machineGunnerHasAudibleTarget({
      kind: KIND.MACHINE_GUNNER,
      state: STATE.ATTACK,
      setupState: SETUP.DEPLOYED,
    }),
    "MG combat loop stops once the machine gunner has no target",
  );
  assert(
    !machineGunnerHasAudibleTarget({
      kind: KIND.RIFLEMAN,
      targetId: 7,
    }),
    "non-MG targets do not hold the MG combat loop",
  );
  assert(
    !attackKindHasCombatSound(KIND.WORKER),
    "worker attacks are silent instead of falling back to rifle shots",
  );
  assert(attackKindHasCombatSound(KIND.RIFLEMAN), "rifleman attacks still play combat sounds");
}

console.log("✅ client_contracts.mjs: all contract assertions passed");
