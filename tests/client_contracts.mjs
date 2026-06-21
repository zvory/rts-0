// tests/client_contracts.mjs
// Lightweight dependency-free checks that the client modules export the expected
// constructors and pure methods documented in docs/design/client-ui.md §4.1.
//
// This does NOT spin up a browser or a server. Modules that require DOM / Pixi
// (Input, HUD, Minimap, Lobby) are not instantiated here; renderer resilience
// is covered with a tiny fake Pixi harness.

import fs from "node:fs";
import { CLIENT_NET_REPORT_FIELDS } from "./client_net_report_fields.mjs";
import {
  assertMalformedBinaryRejected,
  encodeMessagePack,
  fixtureSnapshotFrames,
  runSnapshotCodecBakeoff,
} from "../scripts/snapshot-codec-bakeoff.mjs";
import {
  RENDER_FRAME_BUDGET_MS,
  RENDER_FRAME_BUDGET_TARGETS,
  buildRenderStressMatrixCells,
  buildRenderStressMatrixSummary,
  buildRenderDiagnosticsReport,
  buildRenderBudgetReport,
  formatRenderBudgetConsole,
  formatRenderStressMatrixMarkdown,
  parseMatrixViewportList,
  parsePositiveNumberList,
} from "../scripts/client-perf-harness.mjs";
import { Net, SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES } from "../client/src/net.js";
import {
  DEFAULT_AI_PROFILE_ID,
  MAX_LOBBY_TEAMS,
  PLAYABLE_FACTIONS,
  betaFactionSelectEnabledForLocation,
  shouldAcceptSpectatorDrop,
  shouldAcceptTeamDrop,
  teamSlotsForLobby,
} from "../client/src/lobby.js";
import {
  LOBBY_BROWSER_POLL_MS,
  LobbyBrowserView,
  LobbyCreateModal,
  formatLobbyAge,
  lobbyJoinIntent,
  lobbyActionLabel,
  lobbyStatusLabel,
  sortLobbySummaries,
  suggestLobbyName,
  validateLobbyName,
} from "../client/src/lobby_browser_view.js";
import { PredictionController, PREDICTION_STATE } from "../client/src/prediction_controller.js";
import { formatTeamLabel, scoreRowIsWinner } from "../client/src/scoreboard.js";
import { GameState } from "../client/src/state.js";
import { Camera } from "../client/src/camera.js";
import { Fog } from "../client/src/fog.js";
import { buildFrameEntityViews } from "../client/src/frame_entity_views.js";
import { FrameProfiler, collectMatchFrameContext } from "../client/src/frame_profiler.js";
import { MatchHealth } from "../client/src/match_health.js";
import {
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  ARTILLERY_SHELL_DELAY_TICKS,
  COLORS,
  MINING_CC_RANGE_TILES,
  RIFLEMAN_CHARGE_COOLDOWN_TICKS,
  SMOKE_ABILITY_COST,
  ABILITIES,
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  STATS,
  TICK_HZ,
  UPGRADES,
  WORKER_BUILDABLE,
} from "../client/src/config.js";
import { commandWithinBudget } from "../client/src/command_budget.js";
import {
  HUD,
  formatTankOilUsed,
  groupCooldownClocks,
  playerHasCompletedKind,
  selectionBudgetBlockShape,
  selectionBudgetGridModel,
} from "../client/src/hud.js";
import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  duplicateCommandIdsForCard,
  factionCommandId,
} from "../client/src/hud_command_card.js";
import { Audio, noticeSoundId } from "../client/src/audio.js";
import {
  attackKindHasCombatSound,
  machineGunnerHasAudibleTarget,
} from "../client/src/combat_audio.js";
import {
  COMPACT_SNAPSHOT_VERSION,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  DEFAULT_FACTION_ID,
  PREDICTION_PROTOCOL_VERSION,
  ABILITY,
  ABILITY_CODE,
  ABILITY_OBJECT_KIND,
  ABILITY_OBJECT_KIND_CODE,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  LAB_ROLE,
  MOVEMENT_PATH_DIAGNOSTICS,
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
  parseServerFrame,
  msg,
} from "../client/src/protocol.js";
import { Input, footprintValidAgainstEntities } from "../client/src/input/index.js";
import {
  footprintPlacementBlocker,
  movementBodyClass,
  placementPolicyForBuilding,
} from "../client/src/input/placement.js";
import {
  buildTankTrapLineSites,
  tankTrapBuildCommands,
  tankTrapLineTiles,
  validTankTrapLineSites,
} from "../client/src/input/tank_trap_line.js";
import { armPostQuickCastSelectionGuard } from "../client/src/input/quick_cast_selection_guard.js";
import { CameraNavigationInput } from "../client/src/input/camera_navigation.js";
import { CommandComposer } from "../client/src/command_composer.js";
import { ClientIntent } from "../client/src/client_intent.js";
import { LabClient, labVision, labVisionLabel } from "../client/src/lab_client.js";
import { createDefaultControlPolicy, createLabControlPolicy } from "../client/src/lab_control_policy.js";
import { LabPanel, labSpawnFactionOptions, labSpawnUnitKindsForFaction } from "../client/src/lab_panel.js";
import { LabPanelWindowChrome } from "../client/src/lab_panel_window.js";
import { _controlGroupSaveModifierActive } from "../client/src/input/control_groups.js";
import { Minimap } from "../client/src/minimap.js";
import { ReplayCameraInput } from "../client/src/replay_camera_input.js";
import {
  cursorLockSupported,
  enterCursorLock,
  exitCursorLock,
  installedAppRuntime,
} from "../client/src/input/cursor_lock.js";
import { DomClickInputZone, MatchInputRouter } from "../client/src/input/router.js";
import { Renderer } from "../client/src/renderer/index.js";
import { _drawBuilding } from "../client/src/renderer/buildings.js";
import { buildRendererFeedbackView } from "../client/src/renderer/feedback_view_model.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
  _drawCommandFeedback,
  _drawMortarImpacts,
  _drawPlacement,
  _drawResourceMiningPreview,
} from "../client/src/renderer/feedback.js";
import { LivePauseOverlay } from "../client/src/live_pause_overlay.js";
import { buildGiveUpAction, buildPauseAction, buildSettingsTabs } from "../client/src/settings_panels.js";
import { readPredictionEnabled, writePredictionEnabled } from "../client/src/prediction_settings.js";
import {
  HOTKEY_PRESET_CLASSIC,
  HOTKEY_PROFILE_SCHEMA_VERSION,
  HotkeyProfileService,
  buildHotkeyCommandCatalog,
} from "../client/src/hotkey_profiles.js";
import {
  OBSERVER_ANALYSIS_TABS,
  ObserverAnalysisOverlay,
  calculateViewportArmyValue,
  createObserverAnalysisOverlayPreferences,
  shouldMountObserverAnalysisOverlay,
} from "../client/src/observer_analysis_overlay.js";
import { createRoomCapabilities } from "../client/src/room_capabilities.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function assertDeepEqual(actual, expected, msg) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), msg);
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

function messagePackSnapshotFrame(raw) {
  const payload = encodeMessagePack(raw);
  const frame = new Uint8Array(5 + payload.byteLength);
  frame.set([0x52, 0x54, 0x53, 0x4d, SNAPSHOT_CODEC_VERSION], 0);
  frame.set(payload, 5);
  return frame;
}

function commandCardCtx({
  selection = [],
  entities = selection,
  resources = { steel: 1000, oil: 1000 },
  optimisticProduction = [],
  upgrades = [],
  playerId = 1,
  commandCardMode = null,
  commandTarget = null,
  spectator = false,
  commandSurfaceEnabled = undefined,
  factionId = DEFAULT_FACTION_ID,
  state = null,
  controlPolicy = null,
} = {}) {
  return {
    spectator,
    ...(typeof commandSurfaceEnabled === "boolean" ? { commandSurfaceEnabled } : {}),
    playerId,
    factionId,
    selection,
    state,
    resources,
    optimisticProduction,
    upgrades,
    commandCardMode,
    commandTarget,
    controlPolicy,
    groupCooldownClocks,
    playerHasCompleteKind: (kind) => playerHasCompletedKind(entities, playerId, kind),
  };
}

function defaultFactionCommandId(family, subject) {
  return factionCommandId(DEFAULT_FACTION_ID, family, subject);
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
  const restore = () => {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  };
  const docListeners = {};
  globalThis.document = {
    activeElement: null,
    listeners: docListeners,
    addEventListener(type, handler) {
      docListeners[type] = handler;
    },
    removeEventListener(type, handler) {
      if (docListeners[type] === handler) delete docListeners[type];
    },
    createElement(tagName) {
      const el = {
        tagName: String(tagName).toUpperCase(),
        className: "",
        classList: fakeClassList(),
        children: [],
        dataset: {},
        disabled: false,
        hidden: false,
        title: "",
        type: "",
        value: "",
        innerHTML: "",
        listeners: {},
        style: { setProperty() {} },
        addEventListener(type, handler) {
          this.listeners[type] = handler;
        },
        removeEventListener(type, handler) {
          if (this.listeners[type] === handler) delete this.listeners[type];
        },
        append(...children) {
          this.children.push(...children);
        },
        appendChild(child) {
          this.children.push(child);
          return child;
        },
        replaceChildren(...children) {
          this.children = [...children];
        },
        setAttribute(name, value) {
          this[name] = String(value);
        },
        remove() {
          this.removed = true;
        },
        focus() {
          globalThis.document.activeElement = this;
        },
        click() {
          this.listeners.click?.({ target: this, preventDefault() {}, stopPropagation() {} });
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
    const result = fn(created);
    if (result && typeof result.finally === "function") return result.finally(restore);
    restore();
    return result;
  } catch (err) {
    restore();
    throw err;
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
  const resourceSpan = (id) => {
    let text = "";
    return {
      id,
      textWrites: 0,
      classList: fakeClassList(),
      get textContent() {
        return text;
      },
      set textContent(value) {
        this.textWrites += 1;
        text = String(value);
      },
    };
  };
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
          ids.set(id, resourceSpan(id));
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

function withFakeHudDocument(fn) {
  const priorDocument = globalThis.document;
  class FakeElement {
    constructor(tagName) {
      this.tagName = String(tagName).toUpperCase();
      this.type = "";
      this.className = "";
      this.textContent = "";
      this.title = "";
      this.children = [];
      this.parentNode = null;
      this.dataset = {};
      this.listeners = {};
      this.attributes = new Map();
      this.style = {
        values: new Map(),
        setProperty: (name, value) => {
          this.style.values.set(name, String(value));
        },
      };
      this._innerHTML = "";
    }
    set innerHTML(value) {
      this._innerHTML = String(value);
      if (value === "") {
        for (const child of this.children || []) child.parentNode = null;
        this.children = [];
      }
    }
    get innerHTML() {
      return this._innerHTML;
    }
    appendChild(child) {
      child.parentNode = this;
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
    removeEventListener(type, handler) {
      if (this.listeners[type] === handler) delete this.listeners[type];
    }
    contains(node) {
      for (let cur = node; cur; cur = cur.parentNode) {
        if (cur === this) return true;
      }
      return false;
    }
    closest(selector) {
      for (let cur = this; cur; cur = cur.parentNode) {
        if (matches(cur, selector)) return cur;
      }
      return null;
    }
    querySelectorAll(selector) {
      const results = [];
      const visit = (node) => {
        if (matches(node, selector)) results.push(node);
        for (const child of node.children || []) visit(child);
      };
      visit(this);
      return results;
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] || null;
    }
  }
  function matches(node, selector) {
    if (!node) return false;
    if (selector.startsWith(".")) return node.className.split(/\s+/).includes(selector.slice(1));
    if (selector.startsWith("[")) return node.attributes.has(selector.slice(1, -1));
    return node.tagName === selector.toUpperCase();
  }
  globalThis.document = {
    createElement(tagName) {
      return new FakeElement(tagName);
    },
    createDocumentFragment() {
      return new FakeElement("fragment");
    },
  };
  try {
    return fn({ FakeElement });
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  }
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

function withFakeOverlayDocument(fn) {
  const priorDocument = globalThis.document;
  const priorElement = globalThis.Element;

  class FakeElement {
    constructor(tagName) {
      this.tagName = String(tagName).toUpperCase();
      this.id = "";
      this.type = "";
      this.className = "";
      this.textContent = "";
      this.title = "";
      this.hidden = false;
      this.tabIndex = 0;
      this.dataset = {};
      this.children = [];
      this.parentNode = null;
      this.focused = false;
      this.replaceChildrenCount = 0;
      this.listeners = {};
      this.attributes = new Map();
      this.classList = {
        add: (value) => this.setClass(value, true),
        remove: (value) => this.setClass(value, false),
        toggle: (value, enabled) => this.setClass(value, !!enabled),
        contains: (value) => this.className.split(/\s+/).includes(value),
      };
    }
    setClass(value, enabled) {
      const classes = new Set(this.className.split(/\s+/).filter(Boolean));
      if (enabled) classes.add(value);
      else classes.delete(value);
      this.className = [...classes].join(" ");
    }
    append(...children) {
      for (const child of children) this.appendChild(child);
    }
    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
      return child;
    }
    replaceChildren(...children) {
      this.replaceChildrenCount += 1;
      for (const child of this.children) child.parentNode = null;
      this.children = [];
      this.append(...children);
    }
    remove() {
      if (!this.parentNode) return;
      const siblings = this.parentNode.children;
      const index = siblings.indexOf(this);
      if (index >= 0) siblings.splice(index, 1);
      this.parentNode = null;
    }
    addEventListener(type, handler) {
      this.listeners[type] = handler;
    }
    removeEventListener(type, handler) {
      if (this.listeners[type] === handler) delete this.listeners[type];
    }
    focus() {
      this.focused = true;
    }
    setAttribute(name, value) {
      this.attributes.set(name, String(value));
    }
    getAttribute(name) {
      return this.attributes.get(name) || null;
    }
    contains(node) {
      for (let cur = node; cur; cur = cur.parentNode) {
        if (cur === this) return true;
      }
      return false;
    }
    closest(selector) {
      for (let cur = this; cur; cur = cur.parentNode) {
        if (matchesSelector(cur, selector)) return cur;
      }
      return null;
    }
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] || null;
    }
    querySelectorAll(selector) {
      const results = [];
      const visit = (node) => {
        if (matchesSelector(node, selector)) results.push(node);
        for (const child of node.children) visit(child);
      };
      for (const child of this.children) visit(child);
      return results;
    }
  }

  function matchesSelector(node, selector) {
    if (!node) return false;
    if (selector === "button") return node.tagName === "BUTTON";
    if (selector.startsWith(".")) return node.classList.contains(selector.slice(1));
    if (selector.startsWith("#")) return node.id === selector.slice(1);
    return false;
  }

  globalThis.Element = FakeElement;
  globalThis.document = {
    createElement(tagName) {
      return new FakeElement(tagName);
    },
  };

  try {
    return fn({ FakeElement });
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorElement === undefined) delete globalThis.Element;
    else globalThis.Element = priorElement;
  }
}

function fakeStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem(key) {
      return values.has(key) ? values.get(key) : null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
    values,
  };
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
  const steelWrites = ids.get("res-steel").textWrites;
  const oilWrites = ids.get("res-oil").textWrites;
  const supplyWrites = ids.get("res-supply").textWrites;
  hud._renderSinglePlayerResources();
  assert(ids.get("res-steel").textWrites === steelWrites, "unchanged HUD steel skips duplicate text writes");
  assert(ids.get("res-oil").textWrites === oilWrites, "unchanged HUD oil skips duplicate text writes");
  assert(ids.get("res-supply").textWrites === supplyWrites, "unchanged HUD supply skips duplicate text writes");
  state.resources.steel = 326;
  hud._renderSinglePlayerResources();
  assert(ids.get("res-steel").textContent === "326", "changed HUD steel still updates after dirty guard");
}

// ---------------------------------------------------------------------------
// HUD selection budget grid
// ---------------------------------------------------------------------------
{
  const riflemen = Array.from({ length: 24 }, (_, index) => ({
    id: 1000 + index,
    owner: 1,
    kind: KIND.RIFLEMAN,
  }));
  const tanks = Array.from({ length: 3 }, (_, index) => ({
    id: 1100 + index,
    owner: 1,
    kind: KIND.TANK,
  }));
  const commandCar = { id: 1200, owner: 1, kind: KIND.COMMAND_CAR };
  const artillery = { id: 1300, owner: 1, kind: KIND.ARTILLERY };

  const infantryModel = selectionBudgetGridModel(riflemen);
  assert(infantryModel.used === 24 && infantryModel.cap === BASE_COMMAND_SUPPLY_CAP, "HUD budget grid reports 24/24 infantry supply");
  assert(infantryModel.cols === 12, "HUD base budget grid uses two rows of twelve cells");
  assert(infantryModel.blocks.every((block) => block.weight === 1 && block.cols === 1 && block.rows === 1 && block.placed),
    "HUD infantry blocks occupy one fixed cell each");

  const tankModel = selectionBudgetGridModel(tanks);
  assert(tankModel.used === 24 && tankModel.cap === BASE_COMMAND_SUPPLY_CAP, "HUD budget grid reports three Tanks as 24/24");
  assert(tankModel.blocks.every((block) => block.weight === 8 && block.cols === 4 && block.rows === 2 && block.placed),
    "HUD Tank blocks occupy a two-row by four-column shape");

  const commandCarModel = selectionBudgetGridModel(tanks.concat(commandCar));
  assert(commandCarModel.used === 28 &&
    commandCarModel.cap === BASE_COMMAND_SUPPLY_CAP + COMMAND_CAR_SUPPLY_CAP_BONUS + STATS[KIND.COMMAND_CAR].supply,
    "HUD budget grid includes Command Car net-zero cap expansion");
  assert(commandCarModel.cols === 24, "HUD budget grid grows visible columns for Command Car cap");

  const artilleryShape = selectionBudgetBlockShape(STATS[KIND.ARTILLERY].supply);
  assert(artilleryShape.cols === 3 && artilleryShape.rows === 2 && artilleryShape.reservedCells === 1,
    "HUD five-supply shape uses a deterministic near-rectangle with one reserved cell");
  const artilleryModel = selectionBudgetGridModel([artillery]);
  assert(artilleryModel.blocks[0].reservedCells === 1, "HUD five-supply blocks expose their reserved visual cell");

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const state = {
      selectionBudgetOverflow: { used: 24, cap: BASE_COMMAND_SUPPLY_CAP, seq: 1 },
      selectedEntities() {
        return tanks;
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    const grid = panel.querySelector(".sel-budget-grid");
    const blocks = panel.querySelectorAll(".sel-budget-block");
    const overflow = panel.querySelector(".sel-budget-overflow");
    assert(grid && grid.style.values.get("--sel-budget-cols") === "12", "HUD renders grid columns into selected panel DOM");
    assert(blocks.length === 3 && blocks.every((block) => block.className.includes("weight-8")),
      "HUD renders three Tank budget blocks into selected panel DOM");
    assert(overflow?.textContent === "Selection limit reached", "HUD renders overflow flash text near the budget counter");
    const stableChildren = panel.children;
    hud._renderSelectedPanel();
    assert(panel.children === stableChildren, "HUD selected budget grid skips unchanged DOM rebuilds");
  });

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const selected = { id: 2200, owner: 1, kind: KIND.TANK, hp: 80, maxHp: 100, oilUsed: 4.2 };
    const state = {
      selectionBudgetOverflow: null,
      selectedEntities() {
        return [selected];
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    const stableNode = panel.children[0];
    hud._renderSelectedPanel();
    assert(panel.children[0] === stableNode, "HUD selected detail skips unchanged DOM rebuilds");
    selected.hp = 40;
    hud._renderSelectedPanel();
    assert(panel.children[0] !== stableNode, "HUD selected detail updates when displayed health changes");
    assert(panel.children[0].innerHTML.includes("40 / 100"), "HUD selected detail renders changed health after dirty guard");
  });

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const selectedEntities = [
      { id: 2100, owner: 1, kind: KIND.WORKER },
      { id: 2101, owner: 1, kind: KIND.WORKER },
      { id: 2102, owner: 1, kind: KIND.RIFLEMAN },
      { id: 2103, owner: 1, kind: KIND.TANK },
    ];
    const byId = new Map(selectedEntities.map((entity) => [entity.id, entity]));
    let selectedIds = selectedEntities.map((entity) => entity.id);
    const state = {
      selectionBudgetOverflow: null,
      selectedEntities() {
        return selectedIds.map((id) => byId.get(id)).filter(Boolean);
      },
      entityById(id) {
        return byId.get(id) || null;
      },
      setSelection(ids) {
        selectedIds = Array.from(ids);
      },
      removeFromSelection(ids) {
        const removed = new Set(ids);
        selectedIds = selectedIds.filter((id) => !removed.has(id));
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    const blockFor = (id) => panel.querySelectorAll(".sel-budget-block")
      .find((block) => block.getAttribute("data-selection-entity-id") === String(id));
    const clickBlock = (id, modifiers = {}) => {
      panel.listeners.click({
        target: blockFor(id),
        preventDefault() {},
        ...modifiers,
      });
    };

    clickBlock(2100, { shiftKey: true });
    assert(selectedIds.join(",") === "2101,2102,2103", "HUD selection panel shift-click removes that unit");

    selectedIds = selectedEntities.map((entity) => entity.id);
    clickBlock(2102);
    assert(selectedIds.join(",") === "2102", "HUD selection panel left-click selects only that unit");

    selectedIds = selectedEntities.map((entity) => entity.id);
    clickBlock(2100, { ctrlKey: true });
    assert(selectedIds.join(",") === "2100,2101", "HUD selection panel ctrl-click filters selection to that unit kind");

    selectedIds = selectedEntities.map((entity) => entity.id);
    panel.listeners.contextmenu({
      target: blockFor(2103),
      ctrlKey: true,
      preventDefault() {},
    });
    assert(selectedIds.join(",") === "2103", "HUD selection panel control context-click filters selection to that unit kind");

    hud.destroy();
    assert(!panel.listeners.click && !panel.listeners.contextmenu, "HUD selection panel listeners are removed on destroy");
  });
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

  const labWorker = { id: 14, owner: 2, kind: KIND.WORKER };
  const labState = {
    selectedEntities() {
      return [labWorker];
    },
  };
  const labPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  const labOperatorCard = buildCommandCardDescriptors(commandCardCtx({
    spectator: true,
    commandSurfaceEnabled: labPolicy.canUseCommandSurface(labState),
    playerId: 99,
    selection: [labWorker],
    entities: [labWorker],
    state: labState,
    controlPolicy: labPolicy,
  }));
  assert(buttonByAction(labOperatorCard, "move"), "lab operator spectator-shaped starts expose unit command buttons");
  assert(
    buttonByAction(labOperatorCard, "stop").intent.unitIds.join(",") === String(labWorker.id),
    "lab operator command descriptors target the controllable selected owner",
  );
  const labViewerPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } });
  const labViewerCard = buildCommandCardDescriptors(commandCardCtx({
    spectator: true,
    commandSurfaceEnabled: labViewerPolicy.canUseCommandSurface(labState),
    selection: [labWorker],
    state: labState,
    controlPolicy: labViewerPolicy,
  }));
  assert(labViewerCard.kind === "spectator", "read-only lab viewers keep the command card hidden");
  const mixedLabSelection = [
    { id: 15, owner: 1, kind: KIND.RIFLEMAN },
    { id: 16, owner: 2, kind: KIND.RIFLEMAN },
  ];
  const mixedLabState = {
    selectedEntities() {
      return mixedLabSelection;
    },
  };
  const mixedLabCard = buildCommandCardDescriptors(commandCardCtx({
    spectator: true,
    commandSurfaceEnabled: labPolicy.canUseCommandSurface(mixedLabState),
    playerId: 99,
    selection: mixedLabSelection,
    entities: mixedLabSelection,
    state: mixedLabState,
    controlPolicy: labPolicy,
  }));
  assert(commandButtons(mixedLabCard).length === 0, "mixed-owner lab selections stay non-commandable");

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
  assert(buildCard.slots[0].commandId === defaultFactionCommandId("build", KIND.CITY_CENTRE), "worker build button should expose stable command identity");
  assert(buildCard.slots[0].slotIndex === 0, "worker build button should expose rendered slot index");
  assert(buildCard.slots[0].label === "City Centre", "worker build first slot should stay City Centre");
  assert(buildCard.slots[0].hotkey === "Q", "worker build hotkey Q should be preserved");
  assert(buildCard.slots[0].unaffordable, "unaffordable build buttons stay clickable for feedback");
  assert(buildCard.slots[3].label === "Training Centre", "worker build menu should include Training Centre");
  assert(!buildCard.slots[3].enabled, "locked build buttons should be disabled");
  assert(buildCard.slots[3].title === "Requires Barracks", "locked build tooltip should explain requirement");
  assert(buildCard.slots[7].label === "Tank Trap", "worker build menu should include Tank Trap in the next open slot");
  assert(buildCard.slots[7].hotkey === "X", "Tank Trap build hotkey should use the next open grid key");
  assert(!buildCard.slots[7].enabled, "Tank Trap build button should respect Training Centre requirements");
  assert(buildCard.slots[7].title === "Requires Training Centre", "Tank Trap build tooltip should explain requirement");
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
  assert(trainCard.slots[0].commandId === defaultFactionCommandId("train", KIND.RIFLEMAN), "train button should expose stable train identity");
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

  const supplyReservedTrainCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [cityCentre],
    entities: [cityCentre],
    resources: { steel: 100, oil: 0, supplyUsed: 9, supplyCap: 10 },
    optimisticProduction: [{ building: cityCentre.id, unit: KIND.WORKER, optimisticQueue: 1 }],
  }));
  assert(!supplyReservedTrainCard.slots[0].enabled, "pending train optimism should reserve supply for train buttons");
  assert(supplyReservedTrainCard.slots[0].unaffordable, "supply-blocked train button should stay clickable for feedback");
  assert(supplyReservedTrainCard.slots[0].title === "Not enough supply", "supply-blocked train tooltip should name supply");
  assert(
    supplyReservedTrainCard.slots[0].onUnavailableIntent.supply === STATS[KIND.WORKER].supply,
    "supply-blocked train button should carry supply for unavailable feedback",
  );

  const steelReservedTrainCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [cityCentre],
    entities: [cityCentre],
    resources: { steel: 75, oil: 0, supplyUsed: 4, supplyCap: 10 },
    optimisticProduction: [{ building: cityCentre.id, unit: KIND.WORKER, optimisticQueue: 1 }],
  }));
  assert(!steelReservedTrainCard.slots[0].enabled, "pending train optimism should reserve steel for train buttons");
  assert(steelReservedTrainCard.slots[0].title === "Not enough resources", "resource-blocked train tooltip should name resources");

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
  assert(smoke.commandId === defaultFactionCommandId("ability", ABILITY.SMOKE), "ability button should expose stable ability identity");
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
  const antiTankGun = buttonByLabel(upgradeCard, "Anti-Tank Gun");
  assert(antiTankGun && !antiTankGun.enabled, "upgrade-gated unit should be disabled before research");
  assert(antiTankGun.title === "Requires research in R&D Complex", "upgrade-gated unit tooltip should name R&D research");
  assert(!buttonByLabel(upgradeCard, "Anti-Tank Gun Crews"), "Gun Works should not expose R&D research");

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
  const antiTankGunUnlock = buttonByLabel(researchCard, "Anti-Tank Gun Crews");
  const artilleryUnlock = buttonByLabel(researchCard, "Unlock Artillery");
  assert(antiTankGunUnlock && antiTankGunUnlock.enabled, "available affordable upgrade should be enabled");
  assert(antiTankGunUnlock.commandId === defaultFactionCommandId("research", UPGRADE.ANTI_TANK_GUN_UNLOCK), "research button should expose stable research identity");
  assert(antiTankGunUnlock.intent.type === "research", "upgrade button should carry research intent");
  assert(artilleryUnlock && !artilleryUnlock.enabled, "Artillery research should show disabled before Anti-Tank Gun research");
  assert(artilleryUnlock.title === "Requires Anti-Tank Gun Research", "Artillery research should name missing anti-tank prerequisite");

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
// Frame profiler
// ---------------------------------------------------------------------------

{
  let clock = 0;
  const profiler = new FrameProfiler({
    now: () => clock,
    slowFrameMs: 20,
    slowPhaseMs: 5,
    maxRecentFrames: 2,
  });

  profiler.beginFrame({ at: 0, frameGapMs: 16, scheduledAt: -5 });
  profiler.recordPhase("match.camera", 3);
  profiler.recordPhase("renderer.units", 9);
  profiler.recordDiagnosticCounter("renderer.pixi.displayObject.created.units", 2);
  profiler.recordDiagnosticCounter("renderer.pixi.displayObject.created.units", 1);
  profiler.endFrame({ at: 25, context: { entityCount: 7, selectedCount: 2, hidden: false, focused: true } });

  clock = 40;
  profiler.beginFrame({ at: 40, frameGapMs: 40 });
  profiler.recordDiagnosticCounter("hud.dirty.resources.hit", 1);
  profiler.time("match.hud", () => { clock = 47; });
  profiler.endFrame({ at: 48, context: { visibleTileCount: 12 } });

  clock = 70;
  profiler.beginFrame({ at: 70, frameGapMs: 10 });
  profiler.recordPhase("renderer.units", 1);
  profiler.endFrame({ at: 72 });

  const summary = profiler.summary();
  assert(summary.schemaVersion === 1, "FrameProfiler exposes a versioned debug summary");
  assert(summary.frameCount === 3, "FrameProfiler counts completed frames");
  assert(summary.slowFrameCount === 2, "FrameProfiler counts slow frames by gap or work");
  assert(summary.recentFrames.length === 2, "FrameProfiler keeps recent frame history bounded");
  assert(summary.context.entityCount === 7, "FrameProfiler preserves latest entity count context");
  assert(summary.context.visibleTileCount === 12, "FrameProfiler merges later shape context");
  const unitsPhase = summary.phases.find((phase) => phase.label === "renderer.units");
  assert(unitsPhase?.count === 2, "FrameProfiler aggregates repeated renderer phases");
  assert(unitsPhase?.slowCount === 1, "FrameProfiler counts slow phase samples");
  assert(unitsPhase?.maxMs === 9, "FrameProfiler records phase max timing");
  assert(unitsPhase?.p50Ms === 1, "FrameProfiler reports bucketed p50 timing");
  assert(unitsPhase?.p95Ms === 12, "FrameProfiler reports bucketed p95 timing");
  const unattributedPhase = summary.phases.find((phase) => phase.label === "frame.unattributed");
  assert(unattributedPhase?.p95Ms === 24, "FrameProfiler records unattributed frame work");
  const rafDispatchPhase = summary.phases.find((phase) => phase.label === "frame.rafDispatch");
  assert(rafDispatchPhase?.p95Ms === 8, "FrameProfiler records RAF dispatch delay separately");
  assert(summary.worstPhase?.label === "frame.unattributed", "FrameProfiler can report missing frame attribution as the worst phase");
  assert(summary.recentLongFrames.length === 2, "FrameProfiler keeps bounded long-frame context");
  assert(summary.recentLongFrames[0].rafDispatchMs === 5, "FrameProfiler long-frame context includes RAF dispatch delay");
  assert(summary.recentLongFrames[0].unattributedFrameMs === 22, "FrameProfiler long-frame context includes unattributed work");
  assert(
    summary.recentLongFrames[0].rendererNestedPhase?.label === "renderer.units",
    "FrameProfiler long-frame context names the slowest nested renderer phase",
  );
  const createdCounter = summary.renderDiagnostics.counters.find(
    (counter) => counter.label === "renderer.pixi.displayObject.created.units",
  );
  assert(createdCounter?.total === 3, "FrameProfiler aggregates diagnostic counter totals");
  assert(createdCounter?.frames === 1, "FrameProfiler counts frames where a diagnostic counter appeared");
  assert(createdCounter?.maxFrame === 3, "FrameProfiler records diagnostic max per frame");
  assert(profiler.text().includes("renderer.units"), "FrameProfiler text summary is copyable");
  assert(profiler.text().includes("renderer.pixi.displayObject.created.units"), "FrameProfiler text includes diagnostics");
  const report = profiler.reportSummary();
  assert(report.frameCount === 3, "FrameProfiler report summary counts the report window");
  assert(report.slowFrameCount === 2, "FrameProfiler report summary counts slow frames");
  assert(report.frameWorkMaxMs === 25, "FrameProfiler report summary records max frame work");
  assert(report.frameWorkP95Ms === 33, "FrameProfiler report summary records bucketed frame work p95");
  assert(report.frameUnattributedMaxMs === 22, "FrameProfiler report summary records max unattributed frame work");
  assert(report.frameUnattributedP95Ms === 24, "FrameProfiler report summary records bucketed unattributed p95");
  assert(report.frameRafDispatchMaxMs === 5, "FrameProfiler report summary records max RAF dispatch delay");
  assert(report.frameRafDispatchP95Ms === 8, "FrameProfiler report summary records bucketed RAF dispatch p95");
  assert(report.worstFramePhase === "frame.unattributed", "FrameProfiler report summary names worst phase");
  assert(report.worstFramePhaseMs === 22, "FrameProfiler report summary records worst phase max");
  assert(report.rendererMaxMs === 0, "FrameProfiler report summary tolerates missing renderer phase");
  assert(
    report.renderDiagnostics.counters.some((counter) => counter.label === "hud.dirty.resources.hit"),
    "FrameProfiler report summary includes bounded diagnostics",
  );
  const surface = profiler.debugSurface();
  assert(typeof surface.summary === "function", "FrameProfiler debug surface exposes summary()");
  assert(typeof surface.copy === "function", "FrameProfiler debug surface exposes copy()");
  assert(typeof surface.reportSummary === "function", "FrameProfiler debug surface exposes reportSummary()");
  profiler.resetReportWindow();
  assert(profiler.reportSummary().frameCount === 0, "FrameProfiler can reset only report-window aggregates");
  assert(profiler.reportSummary().renderDiagnostics.counters.length === 0, "FrameProfiler report reset clears diagnostics");
  assert(profiler.summary().frameCount === 3, "FrameProfiler report-window reset preserves debug aggregates");
  surface.reset();
  assert(profiler.summary().frameCount === 0, "FrameProfiler debug surface reset clears aggregates");
  assert(profiler.summary().renderDiagnostics.counters.length === 0, "FrameProfiler reset clears diagnostics");
}

{
  const report = buildRenderBudgetReport({
    schemaVersion: 1,
    frameCount: 120,
    slowFrameCount: 2,
    worstPhase: { label: "match.minimap", count: 80 },
    context: { entityCount: 42, selectedCount: 4 },
    phases: [
      { label: "frame.work", count: 120, avgMs: 7.5, maxMs: 14.6, p50Ms: 8, p95Ms: 12, slowCount: 0 },
      { label: "frame.unattributed", count: 120, avgMs: 4.9, maxMs: 10, p50Ms: 4, p95Ms: 8, slowCount: 4 },
      { label: "match.minimap", count: 120, avgMs: 2.6, maxMs: 5.9, p50Ms: 2, p95Ms: 4, slowCount: 0 },
      { label: "renderer.units", count: 120, avgMs: 0.9, maxMs: 2.4, p50Ms: 1, p95Ms: 2, slowCount: 0 },
    ],
  });

  assert(report.target.frameBudgetMs === RENDER_FRAME_BUDGET_MS, "render budget report exposes the 120 FPS frame budget");
  assert(report.target.frameBudgets.length === RENDER_FRAME_BUDGET_TARGETS.length, "render budget report exposes all FPS frame budgets");
  assert(report.status === "warn", "render budget report warns without failing on over-budget frame work");
  assert(report.frameWork.avgMs === 7.5, "render budget report includes frame.work average");
  assert(report.frameWork.p95Ms === 12, "render budget report includes frame.work p95");
  assert(report.frameAttribution.topLevelAvgMs === 2.6, "render budget report sums top-level named work");
  assert(report.frameAttribution.unattributedP95Ms === 8, "render budget report includes unattributed p95");
  const budget120 = report.frameWork.budgetMargins.find((budget) => budget.fps === 120);
  assert(budget120.p95MarginMs === -3.67 && budget120.p95Clears === false, "render budget report shows p95 margin to 120 FPS");
  assert(report.frameWork.nextMissedBudget.fps === 120, "render budget report names the next missed p95 budget");
  assert(report.worstPhase.label === "match.minimap", "render budget report preserves worst-phase count context");
  assert(
    report.recurringPhaseWarnings.some((phase) => phase.label === "match.minimap" && phase.severity === "high"),
    "render budget report calls out recurring phases above 2 ms",
  );
  assert(
    report.groups.topLevel.some((phase) => phase.label === "match.minimap")
      && report.groups.rendererNested.some((phase) => phase.label === "renderer.units"),
    "render budget report separates top-level match phases from nested renderer phases",
  );
  assert(
    formatRenderBudgetConsole(report).includes("next missed=120 FPS"),
    "render budget console summary shows the next missed budget",
  );
  assert(
    formatRenderBudgetConsole(report).includes("advisory"),
    "render budget console summary labels warnings as advisory",
  );
  assert(
    formatRenderBudgetConsole(report).includes("frame attribution"),
    "render budget console summary includes frame attribution",
  );
}

{
  const report = buildRenderBudgetReport({
    schemaVersion: 1,
    frameCount: 120,
    slowFrameCount: 0,
    phases: [
      { label: "frame.work", count: 120, avgMs: 3.8, maxMs: 9, p50Ms: 4, p95Ms: 7.5, slowCount: 0 },
    ],
  });

  assert(report.frameWork.nextMissedBudget.fps === 240, "120 FPS work can still miss the next headroom target");
  assert(
    report.warnings.some((warning) => warning.kind === "frame_work_p95_misses_headroom_budget"),
    "render budget report warns when local p95 clears 120 but misses higher headroom",
  );
}

{
  const missing = buildRenderDiagnosticsReport(null, null);
  assert(missing.status === "missing", "render diagnostics report tolerates absent counters");

  const report = buildRenderDiagnosticsReport({
    schemaVersion: 1,
    context: { workloadId: "vehicle-wall-stress" },
    renderDiagnostics: {
      schemaVersion: 1,
      counters: [
        { label: "renderer.rig.redraw.completed", total: 20, frames: 5, maxFrame: 6 },
        { label: "minimap.invalidate.fog.fog-revision", total: 4, frames: 4, maxFrame: 1 },
        { label: "hud.dirty.resources.hit", total: 12, frames: 12, maxFrame: 1 },
      ],
    },
    recentLongFrames: [
      {
        at: 12,
        frameWorkMs: 34,
        topPhase: { label: "match.renderer", ms: 18 },
        rendererNestedPhase: { label: "renderer.units", ms: 14 },
      },
    ],
  });
  assert(report.status === "ok", "render diagnostics report summarizes present counters");
  assert(report.groups.rigRedraws.total === 20, "render diagnostics groups rig redraw counters");
  assert(report.groups.minimapInvalidations.total === 4, "render diagnostics groups minimap invalidations");
  assert(report.recentLongFrames[0].rendererNestedPhase.label === "renderer.units", "render diagnostics preserves long-frame context");
}

{
  const cpus = parsePositiveNumberList("1,2,4", "--matrix-cpu");
  const dprs = parsePositiveNumberList("1,1.5,2", "--matrix-dpr");
  const viewports = parseMatrixViewportList("small,1440x900,large");
  assert(cpus.length === 3 && cpus[2] === 4, "stress matrix parser accepts CPU throttle lists");
  assert(dprs.includes(1.5), "stress matrix parser accepts fractional DPR values");
  assert(
    viewports.some((viewport) => viewport.label === "small" && viewport.width === 1024)
      && viewports.some((viewport) => viewport.label === "1440x900" && viewport.height === 900),
    "stress matrix parser accepts presets and explicit viewport sizes",
  );

  const workloads = [{ id: "matt-alex-replay" }, { id: "fog-combat-replay-stress" }];
  const cells = buildRenderStressMatrixCells({
    workloads,
    cpuThrottles: [1, 4],
    viewports: viewports.slice(0, 2),
    deviceScaleFactors: [1, 2],
    repeatCount: 2,
  });
  assert(cells.length === 32, "stress matrix expands workloads, CPU, viewport, DPR, and repeats");
  assert(
    cells.some((cell) => cell.configLabel.includes("cpu4") && cell.configLabel.includes("dpr2")),
    "stress matrix cells include stable config labels",
  );

  const passingBudget = buildRenderBudgetReport({
    schemaVersion: 1,
    frameCount: 120,
    phases: [
      { label: "frame.work", count: 120, avgMs: 3, maxMs: 4, p95Ms: 3.5 },
      { label: "match.renderer", count: 120, avgMs: 0.8, maxMs: 1.2, p95Ms: 1 },
    ],
  });
  const failingBudget = buildRenderBudgetReport({
    schemaVersion: 1,
    frameCount: 120,
    worstPhase: { label: "match.renderer", count: 80 },
    phases: [
      { label: "frame.work", count: 120, avgMs: 12, maxMs: 22, p95Ms: 18 },
      { label: "frame.unattributed", count: 120, avgMs: 7, maxMs: 15, p95Ms: 14 },
      { label: "match.renderer", count: 120, avgMs: 5, maxMs: 12, p95Ms: 9 },
      { label: "renderer.units", count: 120, avgMs: 3, maxMs: 7, p95Ms: 5 },
    ],
  });
  const matrixSummary = buildRenderStressMatrixSummary([
    {
      status: "passed",
      workloadId: "matt-alex-replay",
      artifactDir: "target/client-perf/matt-alex-replay/a",
      renderBudget: passingBudget,
      matrixCell: {
        workloadId: "matt-alex-replay",
        configLabel: "cpu1-vpdefault-dpr1",
        cpuThrottleRate: 1,
        viewport: { label: "default", width: 1440, height: 900 },
        deviceScaleFactor: 1,
        repeatIndex: 1,
        repeatCount: 1,
      },
    },
    {
      status: "passed",
      workloadId: "fog-combat-replay-stress",
      artifactDir: "target/client-perf/fog-combat-replay-stress/a",
      renderBudget: failingBudget,
      matrixCell: {
        workloadId: "fog-combat-replay-stress",
        configLabel: "cpu4-vplarge-dpr2",
        cpuThrottleRate: 4,
        viewport: { label: "large", width: 1920, height: 1080 },
        deviceScaleFactor: 2,
        repeatIndex: 1,
        repeatCount: 1,
      },
    },
  ], { durationMs: 1000, matrixRepeatCount: 1 });
  assert(matrixSummary.cells.length === 2, "stress matrix summary groups runs into cells");
  assert(
    matrixSummary.firstFailingCell.workloadId === "fog-combat-replay-stress",
    "stress matrix summary ranks the first failing cell",
  );
  assert(
    matrixSummary.firstFailingCell.topMeasuredPhase.label === "frame.unattributed",
    "stress matrix summary reports unattributed work when it is the top measured phase",
  );
  assert(
    formatRenderStressMatrixMarkdown(matrixSummary).includes("fog-combat-replay-stress"),
    "stress matrix markdown includes failing workload rows",
  );
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  globalThis.window = { devicePixelRatio: 2, __rtsPerfWorkloadId: "selected-unit-hud-stress" };
  globalThis.document = { hidden: true, hasFocus: () => false };
  try {
    const context = collectMatchFrameContext({
      lastSnapshotTick: 123,
      state: {
        _curById: new Map([[1, {}], [2, {}], [3, {}]]),
        selection: new Set([1, 2]),
        rememberedBuildings: [{ id: 9 }],
        visibleTiles: Uint8Array.from([1, 0, 1, 1]),
      },
      camera: { viewW: 800, viewH: 600, zoom: 1.5 },
      renderer: { app: { view: { width: 1600, height: 1200 }, renderer: {} } },
      prediction: { debugSummary: () => ({ mode: "predicting" }) },
    });
    assert(context.matchMode === "live", "match frame context includes bounded mode");
    assert(context.workloadId === "selected-unit-hud-stress", "match frame context includes local workload id");
    assert(context.matchTick === 123, "match frame context includes latest match tick");
    assert(context.entityCount === 3, "match frame context includes current entity count");
    assert(context.selectedCount === 2, "match frame context includes selected count");
    assert(context.rememberedBuildingCount === 1, "match frame context includes remembered building count");
    assert(context.visibleTileCount === 3, "match frame context counts visible tiles");
    assert(context.canvasWidth === 1600, "match frame context includes canvas backing width");
    assert(context.devicePixelRatio === 2, "match frame context includes device pixel ratio");
    assert(context.predictionMode === "predicting", "match frame context includes prediction mode");
    assert(context.hidden === true && context.focused === false, "match frame context includes document state");
  } finally {
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  }
}

// ---------------------------------------------------------------------------
// Frame entity views
// ---------------------------------------------------------------------------

{
  const calls = [];
  const selected = [{ id: 10, owner: 1, kind: KIND.WORKER, x: 80, y: 80 }];
  const state = {
    playerId: 1,
    spectator: false,
    entitiesInterpolated(alpha, options = {}) {
      calls.push({ alpha, includePrediction: options.includePrediction !== false });
      if (options.includePrediction === false) {
        return [
          { id: 1, owner: 1, kind: KIND.WORKER, x: 10, y: 12 },
          { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 30, y: 32 },
          { id: 3, owner: 0, kind: KIND.STEEL, x: 40, y: 40 },
          { id: 4, owner: 1, kind: KIND.RIFLEMAN, x: 50, y: 50, shotReveal: true },
          { id: 5, owner: 1, kind: KIND.RIFLEMAN, x: 60, y: 60, visionOnly: true },
        ];
      }
      return [{ id: 1, owner: 1, kind: KIND.WORKER, x: alpha * 100, y: alpha * 100 }];
    },
    selectedEntities() {
      return selected;
    },
  };

  const frameViews = buildFrameEntityViews(state, { alpha: 0.5 });
  assert(frameViews.interpolatedEntities[0].x === 50, "frame entity views keep alpha-interpolated entities");
  assert(frameViews.currentEntities[0].x === 100, "frame entity views keep latest predicted current entities");
  assert(frameViews.authoritativeEntities.some((entity) => entity.id === 2), "frame entity views keep authoritative entities");
  assert(frameViews.selectedEntities === selected, "frame entity views reuse the selected entity array");
  assert(
    frameViews.fogSourceEntities.length === 1 && frameViews.fogSourceEntities[0].id === 1,
    "frame entity views filter fog sources to own non-shot-reveal non-vision entries",
  );
  assert(frameViews.debug.entitiesInterpolatedCalls === 3, "frame entity views cap interpolation calls for a mixed-alpha frame");
  assert(frameViews.debug.selectedEntitiesCalls === 1, "frame entity views resolve selection once");
  assert(
    calls.map((call) => `${call.alpha}:${call.includePrediction}`).join("|") === "0.5:true|1:true|1:false",
    "frame entity views request predicted alpha, predicted current, and no-prediction current views",
  );

  calls.length = 0;
  const currentFrameViews = buildFrameEntityViews(state, { alpha: 1 });
  assert(
    currentFrameViews.currentEntities === currentFrameViews.interpolatedEntities,
    "frame entity views reuse alpha-1 predicted entities for current predicted consumers",
  );
  assert(currentFrameViews.debug.entitiesInterpolatedCalls === 2, "alpha-1 frame skips duplicate predicted current interpolation");

  state.spectator = true;
  const spectatorViews = buildFrameEntityViews(state, { alpha: 1 });
  assert(
    spectatorViews.fogSourceEntities.map((entity) => entity.id).join(",") === "1,2",
    "spectator fog sources include non-neutral visible entities from the authoritative union",
  );
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

  health.noteFrameGap(16, 1000);
  health.noteFrameGap(16, 1016);
  assert(health.metrics().fps === 62.5, "MatchHealth records live FPS from the latest frame gap");
  assert(health.metrics().fpsOneMinute === 62.5, "MatchHealth records rolling one-minute FPS");
  health.noteFrameGap(32, 62017);
  assert(health.metrics().fps === 31.25, "live FPS follows the latest frame");
  assert(health.metrics().fpsOneMinute === 31.25, "one-minute FPS prunes stale frame samples");

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

  const cadenceHealth = new MatchHealth({ net, statusBadge: null, snapshotMs: 33 });
  cadenceHealth.noteSnapshotArrival(0, false, 10);
  cadenceHealth.noteSnapshotArrival(1, false, 10);
  cadenceHealth.noteSnapshotArrival(2, false, 13);
  cadenceHealth.noteSnapshotArrival(3, false, 12);
  assert(cadenceHealth.reportStats.duplicateSnapshotCount === 1, "duplicate snapshot ticks feed report stats");
  assert(cadenceHealth.reportStats.skippedSnapshotCount === 1, "skipped snapshot ticks feed report stats");
  assert(cadenceHealth.reportStats.staleSnapshotCount === 1, "stale snapshot ticks feed report stats");
  assert(cadenceHealth.reportStats.snapshotTickGapMax === 3, "snapshot tick gap max is reported");
  assert(cadenceHealth.reportStats.snapshotBurstCount === 1, "multiple snapshots before a frame count as one burst");
  assert(cadenceHealth.reportStats.snapshotBurstMax === 4, "snapshot burst max records per-frame receive pressure");
  cadenceHealth.noteFrameGap(16, 20);
  cadenceHealth.noteSnapshotArrival(21, false, 14);
  assert(cadenceHealth.reportStats.snapshotBurstMax === 4, "frame boundaries reset current burst without clearing report max");

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
    Object.keys(badgePayload).join(",") === "latencyMs,serverTickMs,serverLagMs,jitterMs,fps,fpsOneMinute,issues",
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
      "http://localhost/?watchScenario=1&id=tank_trap_pathing_matrix&unit=scout_car&count=1&case=enemy_vehicle_breach",
    );
    config = devWatchConfig();
    assert(config, "tank_trap_pathing_matrix case variant should be recognized");
    assert(
      config.room ===
        "__dev_scenario__:tank_trap_pathing_matrix:unit=scout_car:count=1:case=enemy_vehicle_breach",
      "dev scenario should include matrix case variants in the server scenario room",
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

async function testReplayArtifactLaunchConfig() {
  const priorDocument = globalThis.document;
  const priorWindow = globalThis.window;
  globalThis.document = {
    getElementById: () => null,
  };
  globalThis.window = {
    location: new URL("http://localhost/?replayArtifact=manual_worker_rush_latest"),
    localStorage: { getItem: () => null },
  };
  try {
    const { replayLaunchConfig } = await import("../client/src/bootstrap.js");
    let config = replayLaunchConfig();
    assert(config, "replay artifact launch config should be recognized");
    assert(
      config.room === "__replay_artifact__:manual_worker_rush_latest",
      "replay artifact launch should auto-join the neutral replay artifact room",
    );

    globalThis.window.location = new URL("http://localhost/?replayArtifact=bad/artifact");
    config = replayLaunchConfig();
    assert(config === null, "replay artifact launch rejects unsafe artifact names");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
}

async function testLabLaunchConfig() {
  const priorDocument = globalThis.document;
  const priorWindow = globalThis.window;
  globalThis.document = {
    getElementById: () => null,
  };
  globalThis.window = {
    location: new URL("http://localhost/lab?room=sandbox&map=low-econ&seed=1234"),
    localStorage: { getItem: () => null },
  };
  try {
    const { labLaunchConfig } = await import("../client/src/bootstrap.js");
    let config = labLaunchConfig();
    assert(config, "lab route launch config should be recognized");
    assert(config.publicRoom === "sandbox", "lab launch keeps public room label");
    assert(config.map === "low-econ", "lab launch keeps map label");
    assert(
      config.room === "__lab__:sandbox:map=low-econ:seed=1234",
      "lab launch should build the server lab room id",
    );

    globalThis.window.location = new URL("http://localhost/lab?room=bad/room&map=bad map");
    config = labLaunchConfig();
    assert(
      config.room === "__lab__:default:map=Default",
      "lab launch falls back for unsafe room and map tokens",
    );

    globalThis.window.location = new URL("http://localhost/?room=sandbox");
    assert(labLaunchConfig() === null, "non-lab route does not auto-join a lab");
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
  clear() {}
  lineStyle() {}
  beginFill() {}
  endFill() {}
  drawPolygon() {}
  drawCircle() {}
  drawRect() {}
  drawRoundedRect() {}
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
  beginFill(color, alpha) {
    this.calls.push(["beginFill", color, alpha]);
  }
  clear() {
    this.calls.push(["clear"]);
  }
  drawCircle(x, y, radius) {
    this.calls.push(["drawCircle", x, y, radius]);
  }
  arc(x, y, radius, start, end, anticlockwise) {
    this.calls.push(["arc", x, y, radius, start, end, anticlockwise]);
  }
  drawRect(x, y, width, height) {
    this.calls.push(["drawRect", x, y, width, height]);
  }
  drawPolygon(points) {
    this.calls.push(["drawPolygon", points]);
  }
  drawRoundedRect(x, y, width, height, radius) {
    this.calls.push(["drawRoundedRect", x, y, width, height, radius]);
  }
}

function installFakePixi() {
  const priorPixi = globalThis.PIXI;
  const priorWindow = globalThis.window;

  class FakeContainer {
    constructor() {
      this.children = [];
      this.position = { set: (x = 0, y = 0) => { this.x = x; this.y = y; } };
      this.scale = { set: (value = 1) => { this.scaleValue = value; } };
      this.visible = true;
    }
    addChild(child) {
      this.children.push(child);
      child.parent = this;
      return child;
    }
    removeChild(child) {
      this.children = this.children.filter((item) => item !== child);
      child.parent = null;
    }
    destroy() {}
  }

  class PixiGraphics extends RecordingGraphics {
    constructor() {
      super();
      this.visible = true;
      this.alpha = 1;
    }
    destroy() {
      this.destroyed = true;
    }
  }

  class FakeApplication {
    constructor(options = {}) {
      this.options = options;
      this.stage = new FakeContainer();
      this.view = { style: {}, parentNode: null };
      this.renderer = {
        roundPixels: false,
        resize: (w, h) => {
          this.width = w;
          this.height = h;
        },
      };
    }
    destroy() {
      this.destroyed = true;
    }
  }

  globalThis.window = {
    ...(priorWindow || {}),
    devicePixelRatio: 1,
    innerWidth: 800,
    innerHeight: 600,
  };
  globalThis.PIXI = {
    Application: FakeApplication,
    Container: FakeContainer,
    Graphics: PixiGraphics,
    SCALE_MODES: { NEAREST: "nearest" },
    settings: {},
  };

  return () => {
    if (priorPixi === undefined) delete globalThis.PIXI;
    else globalThis.PIXI = priorPixi;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  };
}

{
  const restorePixi = installFakePixi();
  const priorConsoleError = console.error;
  const consoleErrors = [];
  console.error = (...args) => consoleErrors.push(args);
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = new Renderer(parent);
    const profiler = new FrameProfiler();
    renderer._drawUnit = () => {
      throw new Error("broken worker art");
    };
    renderer._drawMortarImpacts = () => {
      throw new Error("broken mortar overlay");
    };

    let placementDraws = 0;
    const noOpOverlay = () => {};
    for (const name of [
      "_drawAbilityObjects",
      "_drawSmokes",
      "_drawFog",
      "_drawSmokeCanisters",
      "_drawCommandFeedback",
      "_drawMortarTargets",
      "_drawMortarLaunches",
      "_drawMortarShells",
      "_drawArtilleryLaunches",
      "_drawArtilleryTargets",
      "_drawArtilleryImpacts",
      "_drawSelectedMortarRanges",
      "_drawBreakthroughAuras",
      "_drawAbilityTargetPreview",
      "_drawAntiTankGunSetupPreview",
      "_drawOrderPlan",
      "_drawDebugPathOverlay",
      "_drawRallyPoints",
      "_drawResourceMiningPreview",
      "_drawMuzzleFlashes",
    ]) {
      renderer[name] = noOpOverlay;
    }
    renderer._drawPlacement = () => {
      placementDraws += 1;
    };

    renderer.render(
      {
        playerId: 1,
        players: [{ id: 1, color: "#4878c8" }],
        selection: new Set(),
        rememberedBuildings: [],
        map: { tileSize: 32 },
        entitiesInterpolated: () => [
          { id: 101, owner: 1, kind: KIND.WORKER, x: 100, y: 120, facing: 0 },
        ],
      },
      {
        x: 0,
        y: 0,
        zoom: 1,
      },
      null,
      1,
      { profiler },
    );

    const fallback = renderer._pools.units.get(101);
    const rendererPhases = new Set(profiler.summary().phases.map((phase) => phase.label));
    assert(placementDraws === 1, "renderer continues later overlays after a render helper throws");
    assert(renderer._renderErrors.get("unit:worker")?.count === 1, "renderer records entity render errors by kind");
    assert(renderer._renderErrors.get("mortarImpacts")?.count === 1, "renderer records overlay render errors by label");
    assert(rendererPhases.has("renderer.units"), "renderer records unit sub-phase timing");
    assert(rendererPhases.has("renderer.feedbackOverlays"), "renderer records feedback overlay sub-phase timing");
    assert(profiler.summary().context.entityCount === 1, "renderer profiler context includes entity count");
    assert(fallback?.calls.some((call) => call[0] === "drawRect"), "broken entity art draws a checkerboard fallback");
    assert(
      consoleErrors.some((args) => String(args[0]).includes("[RTS_RENDER] skipped unit:worker")),
      "renderer logs recovered render errors",
    );
    assert(globalThis.__rtsRenderErrors?.latest?.label === "mortarImpacts", "renderer exposes latest render error diagnostics");
  } finally {
    console.error = priorConsoleError;
    restorePixi();
    delete globalThis.__rtsRenderErrors;
  }
}

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorRequestAnimationFrame = globalThis.requestAnimationFrame;
  const priorConsoleError = console.error;
  const consoleErrors = [];
  const tickFn = () => {};
  console.error = (...args) => consoleErrors.push(args);
  globalThis.window = {
    ...(priorWindow || {}),
    location: { protocol: "http:", host: "localhost", search: "" },
    localStorage: { getItem() { return null; } },
  };
  globalThis.document = {
    hidden: false,
    getElementById: () => null,
  };
  globalThis.requestAnimationFrame = (fn) => {
    assert(fn === tickFn, "frame recovery schedules the match tick callback");
    return 77;
  };
  try {
    const { Match } = await import("../client/src/match.js");
    const match = Object.create(Match.prototype);
    Object.assign(match, {
      running: true,
      lastFrame: 1000,
      tickFn,
      frameErrors: { count: 0, lastLogAt: -Infinity },
      health: {
        noteFrameGap() {},
        refreshLatency() {},
        publish() {},
      },
      computeAlpha: () => 1,
      camera: {
        update() {
          throw new Error("camera update failed");
        },
      },
      input: { update() {} },
      advancePredictionVisual() {},
      fog: { update() {} },
      ownEntities: () => [],
      state: { map: { tileSize: 32 }, visibleTiles: null },
      renderer: { render() {} },
      hud: { update() {} },
      minimap: { render() {} },
      observerAnalysisOverlay: null,
    });

    match.frame(1016);

    assert(match.rafId === 77, "match frame schedules the next frame after a client error");
    assert(match.frameErrors.count === 1, "match frame records recovered client errors");
    assert(
      consoleErrors.some((args) => String(args[0]).includes("[RTS_FRAME] recovered")),
      "match frame logs recovered client errors",
    );
    assert(globalThis.__rtsFrameErrors?.count === 1, "match frame exposes recovered frame diagnostics");
  } finally {
    if (priorRequestAnimationFrame === undefined) delete globalThis.requestAnimationFrame;
    else globalThis.requestAnimationFrame = priorRequestAnimationFrame;
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
    console.error = priorConsoleError;
    delete globalThis.__rtsFrameErrors;
  }
}

await testDevWatchScenarioConfig();
await testReplayArtifactLaunchConfig();
await testLabLaunchConfig();

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
// Client boundary baseline contracts
// ---------------------------------------------------------------------------

{
  const intent = new ClientIntent({ now: () => 100 });
  intent.openWorkerBuildMenu();
  assert(intent.commandCardMode === "workerBuild", "ClientIntent owns command-card submenu state");
  intent.beginPlacement(KIND.DEPOT);
  assertDeepEqual(intent.placement, {
    building: KIND.DEPOT,
    tileX: 0,
    tileY: 0,
    valid: false,
  }, "ClientIntent seeds placement previews");
  intent.updatePlacement(3, 4, true);
  assertDeepEqual(intent.placement, {
    building: KIND.DEPOT,
    tileX: 3,
    tileY: 4,
    valid: true,
  }, "ClientIntent updates placement previews");
  intent.beginCommandTarget("attack", { now: 100, shiftKey: true });
  assert(intent.placement === null, "ClientIntent clears placement when targeting begins");
  assert(intent.commandTarget === "attack", "ClientIntent mirrors armed command targets");
  intent.updateAntiTankGunSetupPreview({ mouseX: 1, mouseY: 2, guns: [] });
  intent.updateAbilityTargetPreview({ ability: ABILITY.SMOKE, carriers: [], hoverInRange: true });
  intent.beginCommandTarget("move", { now: 200 });
  assert(intent.antiTankGunSetupPreview === null, "ClientIntent clears support previews on target changes");
  assert(intent.abilityTargetPreview === null, "ClientIntent clears ability previews on target changes");
  intent.addCommandFeedback("move", 10, 20, true, null, 100);
  assert(intent.liveCommandFeedback(700).length === 1, "ClientIntent keeps fresh command feedback");
  assert(intent.liveCommandFeedback(751).length === 0, "ClientIntent expires command feedback by TTL");
  intent.updateResourceMiningPreview({
    resourceId: 200,
    resourceX: 64,
    resourceY: 96,
    ccId: 3,
    ccX: 48,
    ccY: 48,
    inRange: true,
  });
  assert(intent.resourceMiningPreview?.resourceId === 200, "ClientIntent owns resource hover previews");
  const armedLabTool = intent.beginLabTool({ kind: "fieldPoint", payload: { xField: "spawn-x", yField: "spawn-y" } });
  assert(armedLabTool.id && armedLabTool.kind === "fieldPoint", "ClientIntent arms lab tools with stable ids");
  assert(intent.commandTarget === null && intent.placement === null, "ClientIntent lab tools clear placement and command targeting");
  assert(intent.resourceMiningPreview === null, "ClientIntent lab tools clear hover previews");
  intent.beginPlacement(KIND.DEPOT);
  assert(intent.activeLabTool === null, "ClientIntent placement cancels active lab tools");
  intent.beginLabTool({ kind: "fieldPoint" });
  intent.beginCommandTarget("move");
  assert(intent.activeLabTool === null, "ClientIntent command targeting cancels active lab tools");
  intent.beginLabTool({ kind: "fieldPoint" });
  assert(intent.cancelLabTool("escape")?.reason === "escape", "ClientIntent reports lab tool cancellation reason");
  assert(intent.activeLabTool === null, "ClientIntent clears lab tool cancellation state");

  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 12, height: 12, tileSize: 32, terrain: new Array(144).fill(0), resources: [] },
    players: [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
  assert(!("clientIntent" in state), "GameState no longer owns browser-local intent state");
  assert(!("placement" in state), "GameState no longer exposes placement intent shims");
  assert(!("commandTarget" in state), "GameState no longer exposes command-target intent shims");
  assert(!("activeLabTool" in state), "GameState no longer exposes lab-tool intent shims");

  const explicitHudIntent = new ClientIntent();
  const facadeHud = Object.create(HUD.prototype);
  facadeHud.state = {
    beginPlacement() {
      throw new Error("HUD must use injected ClientIntent for placement");
    },
    beginCommandTarget() {
      throw new Error("HUD must use injected ClientIntent for command targeting");
    },
  };
  facadeHud.clientIntent = explicitHudIntent;
  facadeHud.commandIssuer = { issueCommand() {} };
  facadeHud._dispatchCommandIntent({ type: "openWorkerBuildMenu" });
  assert(explicitHudIntent.commandCardMode === "workerBuild", "HUD dispatch opens build menu through injected ClientIntent");
  facadeHud._dispatchCommandIntent({ type: "beginPlacement", building: KIND.DEPOT });
  assert(explicitHudIntent.placement?.building === KIND.DEPOT, "HUD dispatch starts placement through injected ClientIntent");
  facadeHud._dispatchCommandIntent({ type: "beginCommandTarget", target: "attack" });
  assert(explicitHudIntent.commandTarget === "attack", "HUD dispatch arms command targets through injected ClientIntent");
  facadeHud._dispatchCommandIntent({ type: "beginCommandTarget", target: "move" }, { shiftKey: true });
  assert(explicitHudIntent.commandComposer.shiftPreserved === true, "HUD dispatch preserves Shift for command targeting");

  const explicitInputIntent = new ClientIntent();
  explicitInputIntent.beginPlacement(KIND.BARRACKS);
  const facadeInput = Object.create(Input.prototype);
  let facadeSelectionCleared = 0;
  facadeInput.state = {
    endPlacement() {
      throw new Error("Input must use injected ClientIntent for placement cancellation");
    },
    clearSelection() {
      facadeSelectionCleared += 1;
    },
  };
  facadeInput.clientIntent = explicitInputIntent;
  facadeInput._cancel();
  assert(explicitInputIntent.placement === null, "Input cancellation clears placement through injected ClientIntent");
  assert(facadeSelectionCleared === 0, "Input placement cancellation does not fall through to selection clear");

  const labIntent = new ClientIntent();
  labIntent.beginLabTool({ kind: "fieldPoint" });
  const labCancelInput = Object.create(Input.prototype);
  let labSelectionCleared = 0;
  let labCancelReason = null;
  labCancelInput.state = { clearSelection() { labSelectionCleared += 1; } };
  labCancelInput.clientIntent = labIntent;
  labCancelInput.labToolController = {
    cancel(reason) {
      labCancelReason = reason;
      return labIntent.cancelLabTool(reason);
    },
  };
  labCancelInput._cancel();
  assert(labIntent.activeLabTool === null, "Input cancellation clears active lab tools through ClientIntent");
  assert(labCancelReason === "escape", "Input cancellation reports active lab-tool cancellation through the controller");
  assert(labSelectionCleared === 0, "Input lab tool cancellation does not fall through to selection clear");
}

{
  let selectedReads = 0;
  let commandFeedbackNow = 0;
  const selected = [{
    id: 7,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 128,
    y: 128,
    facing: 0,
    setupState: SETUP.DEPLOYED,
  }];
  const mortarImpact = {
    x: 192,
    y: 208,
    radiusTiles: 3,
    seed: 91,
    createdAt: performance.now(),
  };
  const feedbackState = {
    playerId: 1,
    map: {
      tileSize: 32,
      resources: [{ id: 200, kind: KIND.STEEL, x: 80, y: 112, remaining: 900 }],
    },
    abilityObjects: [{
      id: 9,
      owner: 1,
      kind: ABILITY_OBJECT_KIND.RETURN_MARKER,
      ability: ABILITY.EKAT_TELEPORT,
      x: 220,
      y: 240,
    }],
    smokes: [{ id: 1, x: 64, y: 80, radiusTiles: 2 }],
    selectedEntities() {
      selectedReads += 1;
      return selected;
    },
    liveSmokeCanisters() { return []; },
    liveMortarLaunches() { return []; },
    liveMortarShells() { return []; },
    liveMortarTargets() { return []; },
    liveMortarImpacts() { return [mortarImpact]; },
    liveArtilleryTargets() { return []; },
    liveArtilleryLaunches() { return []; },
    liveArtilleryImpacts() { return []; },
    liveMuzzleFlashes() { return []; },
    isOwnOwner(owner) {
      return owner === 1;
    },
    isAllyOwner() {
      return false;
    },
  };
  const feedbackIntent = {
    placement: { building: KIND.CITY_CENTRE, tileX: 2, tileY: 3, valid: true },
    antiTankGunSetupPreview: {
      mouseX: 180,
      mouseY: 128,
      guns: [{ kind: KIND.ANTI_TANK_GUN, x: 128, y: 128 }],
    },
    abilityTargetPreview: {
      ability: ABILITY.SMOKE,
      mouseX: 180,
      mouseY: 128,
      carriers: [{ kind: KIND.SCOUT_CAR, x: 128, y: 128 }],
      rangePx: 96,
      radiusPx: 24,
      hoverInRange: true,
    },
    resourceMiningPreview: {
      resourceId: 200,
      resourceX: 80,
      resourceY: 112,
      ccId: 3,
      ccX: 220,
      ccY: 220,
      inRange: false,
    },
    liveCommandFeedback(now) {
      commandFeedbackNow = now;
      return [{ kind: "move", x: 96, y: 128, append: true, createdAt: now - 100 }];
    },
  };
  const feedbackView = buildRendererFeedbackView(feedbackState, {
    clientIntent: feedbackIntent,
    entities: selected,
    now: 1500,
  });

  assert(feedbackView.playerId === 1, "feedback view exposes player id");
  assert(feedbackView.placement?.building === KIND.CITY_CENTRE, "feedback view exposes placement shape");
  assert(feedbackView.commandFeedback.length === 1, "feedback view exposes live command feedback");
  assert(commandFeedbackNow === 1500, "feedback view samples live feedback at the requested frame time");
  assert(feedbackView.liveCommandFeedback(999) === feedbackView.commandFeedback, "feedback view returns stable command feedback for the frame");
  assert(feedbackView.selectedEntities() === selected, "feedback view exposes stable selected entities for the frame");
  assert(selectedReads === 1, "feedback view snapshots selected entities once per frame");
  assert(feedbackView.entityById(7) === selected[0], "feedback view exposes renderer entity lookup");
  assert(feedbackView.abilityTargetPreview?.ability === ABILITY.SMOKE, "feedback view exposes ability target preview");
  assert(feedbackView.resourceMiningPreview?.resourceId === 200, "feedback view exposes resource mining preview");
  assert(feedbackView.abilityObjects.length === 1, "feedback view exposes ability objects");

  const placementGfx = new RecordingGraphics();
  const feedbackGfx = new RecordingGraphics();
  const abilityObjectGfx = new RecordingGraphics();
  const renderer = {
    _placementGfx: placementGfx,
    _feedbackGfx: feedbackGfx,
    _abilityObjectGfx: abilityObjectGfx,
    _lineProjectileTrails: new Map(),
    _map: { tileSize: 32 },
  };
  _drawPlacement.call(renderer, feedbackView, null);
  _drawCommandFeedback.call(renderer, feedbackView);
  _drawAntiTankGunSetupPreview.call(renderer, feedbackView);
  _drawAbilityTargetPreview.call(renderer, feedbackView);
  _drawAbilityObjects.call(renderer, feedbackView);
  _drawResourceMiningPreview.call(renderer, feedbackView);
  _drawMortarImpacts.call(renderer, feedbackView);

  assert(placementGfx.calls.some((call) => call[0] === "drawRoundedRect"), "renderer feedback reads placement through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawCircle"), "renderer feedback reads command/preview state through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "lineTo"), "renderer feedback reads resource mining preview through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawPolygon"), "renderer feedback draws live mortar impacts without missing helper references");
  assert(abilityObjectGfx.calls.some((call) => call[0] === "drawCircle"), "renderer feedback reads ability objects through the feedback view");
}

{
  const placementGfx = new RecordingGraphics();
  _drawPlacement.call({
    _placementGfx: placementGfx,
    _map: { tileSize: 32 },
  }, {
    placement: {
      building: KIND.TANK_TRAP,
      tileX: 0,
      tileY: 0,
      valid: true,
      lineSites: [
        { tileX: 0, tileY: 0, valid: true },
        { tileX: 2, tileY: 0, valid: false },
        { tileX: 4, tileY: 0, valid: true },
      ],
    },
  }, null);
  const rects = placementGfx.calls.filter((call) => call[0] === "drawRoundedRect");
  const fills = placementGfx.calls.filter((call) => call[0] === "beginFill");
  assert(rects.length === 3, "Tank Trap line placement preview draws each candidate site");
  assert(
    fills.some((call) => call[1] === COLORS.placeOk) && fills.some((call) => call[1] === COLORS.placeBad),
    "Tank Trap line placement preview distinguishes valid and invalid sites",
  );
}

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
    _controlGroupSaveModifierActive(ev({ altKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save also uses Alt+number",
  );
  assert(
    _controlGroupSaveModifierActive(ev({ metaKey: true }), { isWindows: true, isInstalledApp: true }),
    "Windows installed-app control-group save also uses Cmd/Meta+number",
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
  globalThis.matchMedia = (query) => ({ matches: query === "(display-mode: fullscreen)" });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { standalone: false },
  });
  assert(!installedAppRuntime(), "browser fullscreen mode does not mark an installed app runtime");
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
  {
    const selectionArea = { hidden: false };
    const commandCard = { hidden: false };
    const giveUpConfirm = { hidden: false };
    dom.selectionArea = selectionArea;
    dom.commandCard = commandCard;
    dom.giveUpConfirm = giveUpConfirm;

    const replayMatch = Object.create(Match.prototype);
    replayMatch.replayViewer = true;
    replayMatch.state = { spectator: false };
    replayMatch.applySpectatorUi();
    assert(selectionArea.hidden, "replay viewer hides the selected-unit HUD area");
    assert(commandCard.hidden, "replay viewer keeps command card hidden");
    assert(giveUpConfirm.hidden, "replay viewer hides give-up confirmation");

    selectionArea.hidden = true;
    commandCard.hidden = true;
    const liveMatch = Object.create(Match.prototype);
    liveMatch.replayViewer = false;
    liveMatch.state = { spectator: false };
    liveMatch.applySpectatorUi();
    assert(!selectionArea.hidden, "live player match restores the selected-unit HUD area");
    assert(!commandCard.hidden, "live player match restores the command card");

    selectionArea.hidden = true;
    commandCard.hidden = true;
    const labOperatorMatch = Object.create(Match.prototype);
    labOperatorMatch.replayViewer = false;
    labOperatorMatch.state = {
      spectator: true,
      controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } }),
    };
    labOperatorMatch.applySpectatorUi();
    assert(!selectionArea.hidden, "lab operator keeps the selected-unit HUD area visible");
    assert(!commandCard.hidden, "lab operator keeps the command card visible");

    selectionArea.hidden = false;
    commandCard.hidden = false;
    const labViewerMatch = Object.create(Match.prototype);
    labViewerMatch.replayViewer = false;
    labViewerMatch.state = {
      spectator: true,
      controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } }),
    };
    labViewerMatch.applySpectatorUi();
    assert(selectionArea.hidden, "read-only lab viewer hides the selected-unit HUD area");
    assert(commandCard.hidden, "read-only lab viewer hides the command card");

    const labToolMatch = Object.create(Match.prototype);
    labToolMatch.clientIntent = new ClientIntent();
    const labToolChanges = [];
    labToolMatch.publishLabToolChange = (change) => labToolChanges.push(change);
    let labToolWorldClick = null;
    const active = labToolMatch.armLabTool(
      { kind: "fieldPoint", payload: { xField: "spawn-x" } },
      { onWorldClick: (event) => { labToolWorldClick = event; } },
    );
    assert(labToolChanges.at(-1)?.type === "armed", "Match lab tool controller publishes armed state");
    labToolMatch.consumeLabToolWorldClick({
      tool: active,
      x: 44.5,
      y: 88.25,
      world: { x: 44.5, y: 88.25 },
      screen: { x: 10, y: 20 },
    });
    assert(labToolWorldClick?.tool.id === active.id, "Match lab tool controller routes world clicks with the active tool");
    assert(labToolWorldClick.x === 44.5 && labToolWorldClick.y === 88.25, "Match lab tool controller preserves exact world coordinates");
    assert(labToolMatch.clientIntent.activeLabTool === null, "Match lab tool controller clears consumed tools");
    assert(labToolChanges.at(-1)?.reason === "worldClick", "Match lab tool controller publishes world-click cancellation");
    const persistent = labToolMatch.armLabTool(
      { kind: "spawnEntity", keepArmedOnWorldClick: true },
      { onWorldClick: () => {} },
    );
    labToolMatch.consumeLabToolWorldClick({
      tool: persistent,
      x: 12,
      y: 16,
      world: { x: 12, y: 16 },
      screen: { x: 1, y: 2 },
    });
    assert(labToolMatch.clientIntent.activeLabTool?.id === persistent.id, "Match lab tool controller keeps persistent tools armed after world clicks");
  }
  {
    const priorWindowForReplayInput = globalThis.window;
    const listeners = new Map();
    const options = new Map();
    const viewport = {
      addEventListener(type, handler, opts) {
        listeners.set(type, handler);
        options.set(type, opts);
      },
      removeEventListener(type, handler) {
        if (listeners.get(type) === handler) listeners.delete(type);
      },
      getBoundingClientRect() {
        return { left: 20, top: 30, width: 640, height: 480 };
      },
    };
    const camera = {
      zoom: 1,
      calls: [],
      pans: [],
      setZoom(zoom, x, y) {
        this.calls.push({ zoom, x, y });
        this.zoom = zoom;
      },
      panByScreenDelta(dx, dy) {
        this.pans.push({ dx, dy });
      },
    };
    globalThis.window = {
      addEventListener(type, handler) {
        listeners.set(`window:${type}`, handler);
      },
      removeEventListener(type, handler) {
        if (listeners.get(`window:${type}`) === handler) listeners.delete(`window:${type}`);
      },
    };
    try {
      const replayInput = new ReplayCameraInput(viewport, camera);
      assert(options.get("wheel")?.passive === false, "Replay camera wheel listener is non-passive");
      let prevented = 0;
      listeners.get("wheel")({
        deltaY: -100,
        clientX: 220,
        clientY: 180,
        preventDefault() {
          prevented += 1;
        },
      });
      assertApprox(camera.zoom, 1.12, 0.000001, "Replay mouse wheel zooms in");
      assert(camera.calls[0].x === 200 && camera.calls[0].y === 150, "Replay wheel zoom anchors on cursor");
      listeners.get("wheel")({
        deltaY: 100,
        clientX: 220,
        clientY: 180,
        preventDefault() {
          prevented += 1;
        },
      });
      assertApprox(camera.zoom, 1, 0.000001, "Replay mouse wheel zooms out");
      assert(prevented === 2, "Replay wheel zoom prevents page scroll");
      let dragPrevented = 0;
      listeners.get("mousedown")({
        button: 1,
        clientX: 120,
        clientY: 130,
        preventDefault() {
          dragPrevented += 1;
        },
      });
      listeners.get("window:mousemove")({
        clientX: 150,
        clientY: 160,
        preventDefault() {
          dragPrevented += 1;
        },
      });
      listeners.get("window:mouseup")({
        button: 1,
        preventDefault() {
          dragPrevented += 1;
        },
      });
      assert(camera.pans.length === 1, "Replay middle-drag pans through shared camera navigation");
      assert(camera.pans[0].dx === 30 && camera.pans[0].dy === 30, "Replay middle-drag uses screen delta");
      assert(dragPrevented === 3, "Replay middle-drag suppresses browser drag defaults");
      listeners.get("window:keydown")({
        code: "Space",
        preventDefault() {},
      });
      listeners.get("mousedown")({
        button: 0,
        clientX: 170,
        clientY: 175,
        preventDefault() {},
      });
      listeners.get("window:mousemove")({
        clientX: 160,
        clientY: 165,
        preventDefault() {},
      });
      listeners.get("window:mouseup")({
        button: 0,
        preventDefault() {},
      });
      listeners.get("window:keyup")({
        code: "Space",
        preventDefault() {},
      });
      assert(camera.pans.length === 2, "Replay Space+left-drag pans through shared camera navigation");
      assert(camera.pans[1].dx === -10 && camera.pans[1].dy === -10, "Replay Space+left-drag uses screen delta");
      replayInput.destroy();
      assert(!listeners.has("wheel"), "Replay camera input removes wheel listener on destroy");
    } finally {
      if (priorWindowForReplayInput === undefined) delete globalThis.window;
      else globalThis.window = priorWindowForReplayInput;
    }
  }
  {
    const listeners = new Map();
    const viewport = {
      addEventListener(type, handler) {
        listeners.set(type, handler);
      },
      removeEventListener(type, handler) {
        if (listeners.get(type) === handler) listeners.delete(type);
      },
      getBoundingClientRect() {
        return { left: 0, top: 0, width: 800, height: 600 };
      },
    };
    const windowRef = {
      addEventListener(type, handler) {
        listeners.set(`window:${type}`, handler);
      },
      removeEventListener(type, handler) {
        if (listeners.get(`window:${type}`) === handler) listeners.delete(`window:${type}`);
      },
    };
    const helper = new CameraNavigationInput(viewport, { zoom: 1 }, {
      installListeners: true,
      windowRef,
      panKeyCodes: CameraNavigationInput.replayPanKeyCodes(),
    });
    let prevented = 0;
    listeners.get("window:keydown")({
      code: "KeyW",
      preventDefault() {
        prevented += 1;
      },
    });
    assert(helper.keys.up, "Shared camera navigation maps configured pan keys");
    listeners.get("window:keyup")({
      code: "KeyW",
      preventDefault() {
        prevented += 1;
      },
    });
    assert(!helper.keys.up && prevented === 2, "Shared camera navigation releases configured pan keys");
    helper.destroy();
    assert(!listeners.has("window:keydown"), "Shared camera navigation removes key listeners on destroy");
  }
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
  stepDev.dataset.stepRoomTime = "";
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
    setRoomTimeSpeed(speed) {
      this.speeds.push(speed);
    },
    seekRoomTime(ticksBack) {
      this.seekBacks.push(ticksBack);
    },
    seekRoomTimeTo(tick) {
      this.seekTargets.push(tick);
    },
    setReplayVision(vision) {
      this.visions.push(vision);
    },
    requestReplayBranch() {
      this.branches += 1;
    },
    stepRoomTime() {
      this.steps += 1;
    },
  };
  const roomTimeState = {
    players: [
      { id: 1, name: "Alpha", color: "#f00" },
      { id: 2, name: "Bravo", color: "#0f0" },
    ],
  };
  const replayUi = new ReplayControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: true,
    capabilities: createRoomCapabilities({
      startPayload: {
        replay: { durationTicks: 1_000 },
        capabilities: {
          roomTime: {
            available: true,
            setSpeed: true,
            pause: true,
            seekRelative: true,
            seekAbsolute: true,
            timeline: true,
          },
          visibility: { replayVision: true },
        },
      },
    }),
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
  assert(replayNet.speeds.at(-1) === 2, "speed click sends net.setRoomTimeSpeed");
  replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2 });
  replayControls._listeners.get("click")({ target: pauseReplay });
  assert(replayNet.speeds.at(-1) === 0, "replay pause button sends zero playback speed");
  assert(pauseReplay.textContent === "Resume", "paused replay button switches to resume");
  replayControls._listeners.get("click")({ target: pauseReplay });
  assert(replayNet.speeds.at(-1) === 2, "replay resume button restores the last non-zero speed");
  assert(pauseReplay.textContent === "Pause", "resumed replay button switches back to pause");
  replayControls._listeners.get("click")({ target: seekBack });
  assert(replayNet.seekBacks.at(-1) === 90, "seek click sends net.seekRoomTime");
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
  replayUi.applyRoomTimeState({
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
  scenarioStep.dataset.stepRoomTime = "";
  const scenarioSeek = fakeEl("button");
  scenarioSeek.className = "spd-btn seek-btn";
  scenarioSeek.dataset.seekBack = "30";
  scenarioControls.appendChild(scenarioSpeed0);
  scenarioControls.appendChild(scenarioStep);
  scenarioControls.appendChild(scenarioSeek);
  dom.replaySpeed = scenarioControls;
  const scenarioUi = new ReplayControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: false,
    capabilities: createRoomCapabilities({
      startPayload: {
        spectator: true,
        capabilities: {
          roomTime: {
            available: true,
            setSpeed: true,
            pause: true,
            step: true,
          },
        },
      },
    }),
  });
  assert(scenarioSeek.hidden, "scenario mode hides replay seek buttons");
  assert(!scenarioStep.hidden, "scenario mode shows step controls");
  scenarioControls._listeners.get("click")({ target: scenarioStep });
  assert(replayNet.steps === 1, "scenario step sends net.stepRoomTime");
  scenarioControls._listeners.get("click")({ target: scenarioSpeed0 });
  assert(replayNet.speeds.at(-1) === 0, "scenario pause speed sends net.setRoomTimeSpeed");
  scenarioUi.destroy();

  const noCapabilityControls = fakeEl("div");
  dom.replaySpeed = noCapabilityControls;
  const noCapabilityUi = new ReplayControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: true,
    capabilities: createRoomCapabilities({ startPayload: { spectator: true, replay: {} } }),
  });
  assert(!noCapabilityControls._listeners.has("click"), "room-time controls need an advertised capability");
  assert(
    !noCapabilityControls.querySelector(".replay-vision-controls"),
    "replay identity alone does not build replay vision controls",
  );
  noCapabilityUi.destroy();

  const normalCapabilities = createRoomCapabilities({
    startPayload: { spectator: false, capabilities: { commands: { gameplay: true }, matchControls: { pause: true } } },
  });
  assert(!normalCapabilities.roomTime.available, "normal matches do not mount room-time controls");
  assert(normalCapabilities.commands.gameplay, "active players keep gameplay command affordances");
  assert(normalCapabilities.matchControls.pause, "active live players keep live pause affordances");

  const spectatorCapabilities = createRoomCapabilities({
    startPayload: {
      spectator: true,
      diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.ALL },
      capabilities: { commands: { gameplay: false } },
    },
  });
  assert(!spectatorCapabilities.commands.gameplay, "spectators get read-only command affordances");
  assert(
    spectatorCapabilities.diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.ALL,
    "capability parser keeps diagnostic affordances from the start payload",
  );
  assert(!spectatorCapabilities.matchControls.pause, "spectators do not get live pause controls by default");

  withFakeOverlayDocument(({ FakeElement }) => {
    const root = new FakeElement("section");
    let unpaused = false;
    const overlay = new LivePauseOverlay({ root, onUnpause: () => { unpaused = true; } });
    overlay.applyLivePauseState({ paused: true, pausedBy: 2, pauseLimit: 3, canUnpause: true });
    assert(root.children.length === 1, "live pause overlay mounts generated DOM");
    assert(!root.children[0].hidden, "live pause overlay shows when paused");
    const button = root.querySelector("#live-pause-unpause");
    assert(button && !button.hidden && !button.disabled, "live pause overlay enables unpause for active players");
    button.listeners.click();
    assert(unpaused, "live pause overlay calls injected unpause action");
    overlay.applyLivePauseState({ paused: true, canUnpause: false });
    assert(button.hidden && button.disabled, "live pause overlay hides spectator unpause control");
    overlay.applyLivePauseState({ paused: false });
    assert(root.children[0].hidden, "live pause overlay hides when running");
    overlay.destroy();
    assert(root.children.length === 0, "live pause overlay tears down DOM");
  });

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
  noticeAudioMatch.state = { spectator: false };
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
  noticeAudioMatch.state = { spectator: true };
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 512,
    y: 768,
  });
  assert(playedNotices.length === 0, "live spectator notice alerts do not play audio");
  assert(minimapPings === 2, "live spectator notice alerts still ping the minimap");
  noticeAudioMatch.state = { spectator: false };
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

  const artilleryMarkerMatch = Object.create(Match.prototype);
  const artilleryMarkers = [];
  artilleryMarkerMatch.audio = null;
  artilleryMarkerMatch.minimap = {
    markArtilleryFiring(ev) {
      artilleryMarkers.push(ev);
    },
  };
  artilleryMarkerMatch.handleSnapshotEvents([
    { e: EVENT.ARTILLERY_FIRING, owner: 2, x: 288, y: 304, facing: 0.25 },
  ]);
  assert(
    artilleryMarkers.length === 1 &&
      artilleryMarkers[0].owner === 2 &&
      artilleryMarkers[0].x === 288,
    "artillery firing events are forwarded to the minimap marker layer",
  );

  const predictionPolicyMatch = Object.create(Match.prototype);
  predictionPolicyMatch.replayViewer = false;
  predictionPolicyMatch.state = {
    spectator: false,
    applyPredictionDisplayOverlay(overlay) {
      if (Object.prototype.hasOwnProperty.call(overlay || {}, "predictedSnapshot")) {
        this.predictedSnapshot = overlay.predictedSnapshot;
      }
      if (Object.prototype.hasOwnProperty.call(overlay || {}, "optimisticCommands")) {
        this.optimisticCommands = overlay.optimisticCommands;
      }
    },
  };
  predictionPolicyMatch.prediction = {
    enabled: true,
    predictor: null,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  predictionPolicyMatch.predictionInitToken = 0;
  let predictionAdapterInit = 0;
  let predictionAdapterDestroy = 0;
  let predictionAdapterId = 0;
  const makePredictionAdapter = () => {
    const adapter = {
      id: ++predictionAdapterId,
      ready: false,
      loading: false,
      destroyed: false,
      diagnostics: () => ({ ready: adapter.ready, loading: adapter.loading }),
      init: async () => {
        predictionAdapterInit += 1;
        adapter.loading = true;
        await Promise.resolve();
        adapter.loading = false;
        adapter.ready = true;
        return true;
      },
      destroy: () => {
        predictionAdapterDestroy += 1;
        adapter.destroyed = true;
        adapter.ready = false;
        adapter.loading = false;
      },
    };
    return adapter;
  };
  predictionPolicyMatch.createPredictionAdapter = makePredictionAdapter;
  predictionPolicyMatch.predictionAdapter = {
    ready: false,
    loading: false,
    diagnostics: () => ({ ready: false }),
    init: async () => true,
    destroy: () => { predictionAdapterDestroy += 1; },
  };
  predictionPolicyMatch.prediction.predictor = predictionPolicyMatch.predictionAdapter;
  predictionPolicyMatch.publishPredictionDebug = () => {};
  predictionPolicyMatch.mountSettings = () => {};
  predictionPolicyMatch.logPredictionStatus = () => {};
  predictionPolicyMatch.setPredictionEnabled(false);
  assert(!predictionPolicyMatch.prediction.enabled, "prediction setting can disable live prediction");
  assert(predictionPolicyMatch.state.predictedSnapshot === null, "disabling prediction clears local predicted overlay");
  assert(predictionPolicyMatch.state.optimisticCommands === null, "disabling prediction clears optimistic command UI");
  assert(predictionPolicyMatch.prediction.predictor === predictionPolicyMatch.predictionAdapter,
    "disabling prediction replaces the controller predictor with a fresh inactive adapter");
  assert(predictionAdapterDestroy === 1, "disabling prediction destroys the active WASM adapter");
  predictionPolicyMatch.setPredictionEnabled(true);
  await Promise.resolve();
  await Promise.resolve();
  assert(predictionPolicyMatch.prediction.enabled, "prediction setting can re-enable live prediction");
  assert(predictionAdapterInit === 1, "re-enabling prediction initializes the WASM adapter");
  assert(predictionPolicyMatch.predictionAdapter.ready, "re-enabled prediction owns a ready fresh adapter");

  const staleInitMatch = Object.create(Match.prototype);
  staleInitMatch.replayViewer = false;
  staleInitMatch.state = { spectator: false };
  staleInitMatch.predictionInitToken = 0;
  staleInitMatch.prediction = {
    enabled: true,
    predictor: null,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  let resolveStaleInit = null;
  const staleAdapter = {
    destroyed: false,
    ready: false,
    loading: true,
    diagnostics: () => ({ ready: staleAdapter.ready, loading: staleAdapter.loading }),
    init: () => new Promise((resolve) => {
      resolveStaleInit = () => {
        staleAdapter.loading = false;
        staleAdapter.ready = true;
        resolve(true);
      };
    }),
    destroy: () => {
      staleAdapter.destroyed = true;
      staleAdapter.ready = false;
      staleAdapter.loading = false;
    },
  };
  let freshAdapter = null;
  staleInitMatch.createPredictionAdapter = () => {
    freshAdapter = {
      destroyed: false,
      ready: false,
      loading: false,
      diagnostics: () => ({ ready: freshAdapter.ready, loading: freshAdapter.loading }),
      init: async () => {
        freshAdapter.ready = true;
        return true;
      },
      destroy: () => {
        freshAdapter.destroyed = true;
        freshAdapter.ready = false;
      },
    };
    return freshAdapter;
  };
  staleInitMatch.predictionAdapter = staleAdapter;
  staleInitMatch.prediction.predictor = staleAdapter;
  staleInitMatch.publishPredictionDebug = () => {};
  staleInitMatch.mountSettings = () => {};
  staleInitMatch.logPredictionStatus = () => {};
  staleInitMatch.initPredictionAdapter();
  staleInitMatch.setPredictionEnabled(false);
  staleInitMatch.setPredictionEnabled(true);
  await Promise.resolve();
  resolveStaleInit();
  await Promise.resolve();
  assert(staleAdapter.destroyed, "stale in-flight prediction init is destroyed after the toggle-off token changes");
  assert(freshAdapter.ready && !freshAdapter.destroyed, "stale init completion does not destroy the re-enabled adapter");

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

  const manualPointerLockMatch = Object.create(Match.prototype);
  let toggledPointerLock = 0;
  let closedSettings = 0;
  manualPointerLockMatch.input = {
    pointerLocked: false,
    pointerLockSupported: () => true,
    togglePointerLock() {
      toggledPointerLock += 1;
    },
  };
  manualPointerLockMatch.closeSettingsMenu = () => {
    closedSettings += 1;
  };
  manualPointerLockMatch.syncPointerLockUi = () => {};
  manualPointerLockMatch.togglePointerLock();
  assert(toggledPointerLock === 1, "manual cursor-lock action is the only Pointer Lock request path");
  assert(closedSettings === 1, "manual cursor-lock request closes settings before requesting lock");

  let unsupportedToast = null;
  manualPointerLockMatch.input.pointerLockSupported = () => false;
  manualPointerLockMatch.toast = (msg) => {
    unsupportedToast = msg;
  };
  manualPointerLockMatch.togglePointerLock();
  assert(toggledPointerLock === 1, "unsupported cursor-lock action does not request Pointer Lock");
  assert(unsupportedToast === "Cursor lock is not supported by this browser.",
    "unsupported cursor lock surfaces the existing support message");

  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
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
          [ORDER_STAGE_CODE[ORDER_STAGE.SETUP_ANTI_TANK_GUNS], 128, 160],
          [ORDER_STAGE_CODE[ORDER_STAGE.CHARGE], 176, 208],
          [ORDER_STAGE_CODE[ORDER_STAGE.SMOKE], 192, 224],
          [ORDER_STAGE_CODE[ORDER_STAGE.POINT_FIRE], 320, 352],
        ],
        87,
        [[ABILITY_CODE[ABILITY.CHARGE], 87, 2, null, 77, 45, null, 90]],
        66,
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
        null,
        null,
        true,
      ],
    ],
    r: [[200, 1498]],
    sm: [[50, 320, 352, 2, 120]],
    ao: [
      [
        60,
        1,
        ABILITY_CODE[ABILITY.EKAT_TELEPORT],
        ABILITY_OBJECT_KIND_CODE[ABILITY_OBJECT_KIND.RETURN_MARKER],
        384,
        416,
        90,
        7,
        [45, null, 14, null, null, null],
      ],
    ],
    u: [1, UPGRADE_CODE[UPGRADE.ARTILLERY_UNLOCK]],
    fg: [1, 2, 3, 1],
    ev: [
      [EVENT_CODE[EVENT.ATTACK], 1, 7],
      [EVENT_CODE[EVENT.OVERPENETRATION], 22],
      [EVENT_CODE[EVENT.DEATH], 200, 64, 96, KIND_CODE[KIND.STEEL]],
      [EVENT_CODE[EVENT.BUILD], 3, KIND_CODE[KIND.CITY_CENTRE]],
      [EVENT_CODE[EVENT.NOTICE], "Not enough steel"],
      [EVENT_CODE[EVENT.NOTICE], "alert:under_attack", 3, 512, 768],
      [EVENT_CODE[EVENT.MORTAR_LAUNCH], 9, [256, 272], [320, 352], 1.5, 68],
      [EVENT_CODE[EVENT.ARTILLERY_TARGET], 10, [320, 352], 3, ARTILLERY_SHELL_DELAY_TICKS],
      [EVENT_CODE[EVENT.ARTILLERY_FIRING], 1, 288, 304, 0.25],
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
      decoded.entities[0].abilities[0].remainingUses === 2 &&
      decoded.entities[0].abilities[0].activeObjectId === 77 &&
      decoded.entities[0].abilities[0].availableTick === 45 &&
      decoded.entities[0].abilities[0].expiresIn === 90,
    "entity ability cooldowns decode",
  );
  assert(
    decoded.entities[0].orderPlan[0].kind === ORDER_STAGE.MOVE &&
      decoded.entities[0].orderPlan[0].x === 96 &&
      decoded.entities[0].orderPlan[0].y === 112,
    "entity active order stage decodes",
  );
  assert(decoded.entities[0].breakthroughTicks === 66, "entity breakthrough status decodes");
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
      decoded.entities[0].orderPlan[1].kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      decoded.entities[0].orderPlan[2].kind === ORDER_STAGE.CHARGE &&
      decoded.entities[0].orderPlan[3].kind === ORDER_STAGE.SMOKE &&
      decoded.entities[0].orderPlan[4].kind === ORDER_STAGE.POINT_FIRE,
    "order plan stage flavor decodes",
  );
  assert(
    decoded.entities[0].orderPlan[1].kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      decoded.entities[0].orderPlan[1].x === 128 &&
      decoded.entities[0].orderPlan[1].y === 160,
    "queued anti-tank gun setup order stage decodes",
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
  assert(decoded.entities[2].buildActive === true, "entity construction activity flag decodes");
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
    decoded.abilityObjects[0].id === 60 &&
      decoded.abilityObjects[0].kind === ABILITY_OBJECT_KIND.RETURN_MARKER &&
      decoded.abilityObjects[0].ownerState.earliestReturnTick === 45 &&
      decoded.abilityObjects[0].ownerState.radius === 14,
    "ability objects decode",
  );
  assert(
    decoded.visibleTiles.join(",") === "1,1,0,0,0,1",
    "compact snapshot decodes server visibility grid",
  );
  assert(decoded.events[0].e === EVENT.ATTACK && decoded.events[0].to === 7, "attack event decodes");
  assert(
    decoded.events[1].e === EVENT.OVERPENETRATION && decoded.events[1].to === 22,
    "overpenetration event decodes",
  );
  assert(decoded.events[2].kind === KIND.STEEL, "death event kind decodes");
  assert(decoded.events[4].msg === "Not enough steel", "notice event decodes");
  assert(decoded.events[4].severity === NOTICE_SEVERITY.INFO, "legacy notice defaults to info");
  assert(decoded.events[5].severity === NOTICE_SEVERITY.ALERT, "notice severity decodes");
  assert(decoded.events[5].x === 512 && decoded.events[5].y === 768, "notice position decodes");
  assert(
    decoded.events[6].e === EVENT.MORTAR_LAUNCH &&
      decoded.events[6].from === 9 &&
      decoded.events[6].fromX === 256 &&
      decoded.events[6].toY === 352 &&
      decoded.events[6].delayTicks === 68,
    "mortar launch event decodes",
  );
  assert(
    decoded.events[7].e === EVENT.ARTILLERY_TARGET &&
      decoded.events[7].from === 10 &&
      decoded.events[7].delayTicks === ARTILLERY_SHELL_DELAY_TICKS &&
      decoded.events[7].radiusTiles === 3,
    "artillery target event decodes",
  );
  assert(
    decoded.events[8].e === EVENT.ARTILLERY_FIRING &&
      decoded.events[8].owner === 1 &&
      decoded.events[8].x === 288 &&
      decoded.events[8].facing === 0.25,
    "artillery firing minimap event decodes",
  );
  assert(
    decoded.events[9].e === EVENT.ARTILLERY_IMPACT &&
      decoded.events[9].x === 336 &&
      decoded.events[9].y === 368,
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
  const recastCommand = cmd.recastAbility(ABILITY.EKAT_TELEPORT, [9], 77, true);
  assert(
    recastCommand.c === "recastAbility" &&
      recastCommand.ability === ABILITY.EKAT_TELEPORT &&
      recastCommand.units.length === 1 &&
      recastCommand.targetObjectId === 77 &&
      recastCommand.queued === true,
    "recastAbility command builder emits explicit recast wire shape",
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
  const deconstructCommand = cmd.deconstruct([7, 8], 55, true);
  assert(
    deconstructCommand.c === "deconstruct" &&
      deconstructCommand.units.join(",") === "7,8" &&
      deconstructCommand.target === 55 &&
      deconstructCommand.queued === true,
    "deconstruct command builder emits selected-worker target wire shape",
  );
  assert(
    JSON.stringify(msg.command(cmd.stop([7]), 3)) ===
      JSON.stringify({ t: "command", clientSeq: 3, cmd: { c: "stop", units: [7] } }),
    "command message builder wraps gameplay commands with clientSeq",
  );
  assert(
    JSON.stringify(msg.command(cmd.holdPosition([7]), 4)) ===
      JSON.stringify({ t: "command", clientSeq: 4, cmd: { c: "holdPosition", units: [7] } }),
    "holdPosition command builder emits the hold-position wire shape",
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
  assert(SNAPSHOT_CODEC.COMPACT_JSON === "compact-json", "client keeps compact JSON baseline codec name");
  assert(
    SNAPSHOT_CODEC.MESSAGEPACK_COMPACT === "messagepack-compact",
    "client mirrors MessagePack snapshot codec name",
  );
  assert(SNAPSHOT_CODEC_VERSION === 1, "client mirrors snapshot codec version");
  assert(SNAPSHOT_FRAME_KIND.BINARY === "binary", "client mirrors binary snapshot frame kind");
  assert(
    parseServerFrame(JSON.stringify({ t: "snapshot", tick: 1 })).t === "snapshot",
    "protocol frame parser accepts JSON text frames",
  );
  const binarySnapshot = messagePackSnapshotFrame({
    t: "snapshot",
    v: COMPACT_SNAPSHOT_VERSION,
    s: [77, 10, 20, 1, 5],
    e: [],
    n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 0, null],
  });
  assert(
    decodeServerMessage(parseServerFrame(binarySnapshot)).tick === 77,
    "protocol frame parser decodes MessagePack snapshot frames",
  );
  assertThrows(
    () => parseServerFrame(new Uint8Array([1, 2, 3])),
    "protocol frame parser rejects malformed binary frames",
  );
  assertThrows(
    () => parseServerFrame(new Uint8Array([0x52, 0x54, 0x53, 0x4d, 0xff])),
    "protocol frame parser rejects unsupported MessagePack frame versions",
  );
  const bakeoff = runSnapshotCodecBakeoff({ frames: fixtureSnapshotFrames(), iterations: 1 });
  assert(
    bakeoff.candidates.some((candidate) => candidate.id === "compact-json") &&
      bakeoff.candidates.some((candidate) => candidate.id === "custom-positional-binary"),
    "snapshot codec bake-off compares baseline and custom binary candidates",
  );
  assertMalformedBinaryRejected();
}

{
  assert(
    JSON.stringify(cmd.setupAntiTankGuns([1, 2], 100, 200)) ===
      JSON.stringify({ c: "setupAntiTankGuns", units: [1, 2], x: 100, y: 200 }),
    "setupAntiTankGuns command builder emits the wire shape",
  );
  assert(
    JSON.stringify(cmd.tearDownAntiTankGuns([3, 4])) ===
      JSON.stringify({ c: "tearDownAntiTankGuns", units: [3, 4] }),
    "tearDownAntiTankGuns command builder emits the wire shape",
  );
  assert(
    JSON.stringify(cmd.move([1], 100, 200, true)) ===
      JSON.stringify({ c: "move", units: [1], x: 100, y: 200, queued: true }),
    "queued move command builder emits the queued flag only when requested",
  );
  assert(ANTI_TANK_GUN_DEPLOYED_RANGE_TILES === 14, "client mirrors deployed anti-tank gun range");
  assertApprox(
    ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
    45 * Math.PI / 180,
    0.000001,
    "client mirrors anti-tank gun field of fire",
  );
}

// ---------------------------------------------------------------------------
// Lobby team UI helpers
// ---------------------------------------------------------------------------
{
  assert(MAX_LOBBY_TEAMS === 4, "lobby exposes four host-managed team slots");
  assert(
    PLAYABLE_FACTIONS.find((entry) => entry.id === "ekat")?.label === "Ekat",
    "lobby faction selector labels the ekat faction as Ekat",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "rts-0-zvorygin-beta.fly.dev", pathname: "/" }),
    "lobby faction select shows on beta host",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "localhost", pathname: "/" }),
    "lobby faction select shows on local runserver host",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "127.0.0.1", pathname: "/" }),
    "lobby faction select shows on loopback host",
  );
  assert(
    betaFactionSelectEnabledForLocation({ hostname: "0.0.0.0", pathname: "/" }),
    "lobby faction select shows on wildcard bind host",
  );
  assert(
    !betaFactionSelectEnabledForLocation({ hostname: "rts-0-zvorygin.fly.dev", pathname: "/" }),
    "lobby faction select stays hidden on mainline host",
  );
  const slots = teamSlotsForLobby([
    { id: 3, teamId: 1 },
    { id: 4, teamId: 2 },
    { id: 9, teamId: 0, isSpectator: true },
  ]);
  assert(
    slots.length === 3 && slots[0].id === 1 && slots[1].id === 2 && slots[2].id === 3 && slots[2].isNew,
    "lobby renders occupied teams plus the first empty new-team slot",
  );
  const fullSlots = teamSlotsForLobby([
    { id: 1, teamId: 1 },
    { id: 2, teamId: 2 },
    { id: 3, teamId: 3 },
    { id: 4, teamId: 4 },
  ]);
  assert(fullSlots.length === 4 && fullSlots.every((slot) => !slot.isNew),
    "lobby omits the new-team slot when all four teams are occupied");
  assert(
    shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 2, teamId: 2 },
      isHost: true,
      countdownActive: false,
    }),
    "host can drag an active human player to spectators",
  );
  assert(
    !shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 9, teamId: 2, isAi: true },
      isHost: true,
      countdownActive: false,
    }),
    "spectator drop rejects AI seats",
  );
  assert(
    !shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: true,
      countdownActive: false,
    }),
    "spectator drop rejects existing spectators",
  );
  assert(
    !shouldAcceptSpectatorDrop({
      draggedPlayer: { id: 2, teamId: 2 },
      isHost: false,
      countdownActive: false,
    }),
    "spectator drop is host-only",
  );
  assert(
    shouldAcceptTeamDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: true,
      countdownActive: false,
    }),
    "host can drag a spectator back into a team slot",
  );
  assert(
    !shouldAcceptTeamDrop({
      draggedPlayer: { id: 3, isSpectator: true },
      isHost: false,
      countdownActive: false,
    }),
    "team drop is host-only",
  );
}

// ---------------------------------------------------------------------------
// Lobby browser UI helpers
// ---------------------------------------------------------------------------
{
  const now = 200_000_000;
  assert(LOBBY_BROWSER_POLL_MS === 1500, "lobby browser polls inside the 1-2 second contract");
  assert(formatLobbyAge(now - 5_000, now) === "just now", "lobby browser formats fresh ages");
  assert(formatLobbyAge(now - 3 * 60_000, now) === "3m ago", "lobby browser formats minute ages");
  assert(formatLobbyAge(now - 2 * 60 * 60_000, now) === "2h ago", "lobby browser formats hour ages");
  assert(lobbyStatusLabel("fullSpectatorOnly") === "Full", "full lobby rows get a distinct status label");
  assert(lobbyActionLabel("fullSpectatorOnly") === "Join as spectator",
    "full lobby rows advertise spectator joining");
  assert(lobbyActionLabel("inGame") === "Spectate",
    "in-progress lobby rows advertise live spectating");
  assertDeepEqual(lobbyJoinIntent({ joinState: "open" }), { state: "open", joinable: true, spectator: false },
    "open lobby rows join as active players");
  assertDeepEqual(lobbyJoinIntent({ joinState: "fullSpectatorOnly" }),
    { state: "fullSpectatorOnly", joinable: true, spectator: true },
    "full waiting lobby rows join as spectators");
  assertDeepEqual(lobbyJoinIntent({ joinState: "inGame" }),
    { state: "inGame", joinable: true, spectator: true },
    "in-progress lobby rows join as spectators");
  assert(validateLobbyName(" Alpha ").ok, "lobby create accepts trimmed plain names");
  assert(!validateLobbyName("   ").ok, "lobby create rejects empty names");
  assert(!validateLobbyName("__lab__:sandbox").ok, "lobby create rejects reserved internal prefixes");
  assert(!validateLobbyName("x".repeat(65)).ok, "lobby create mirrors the server byte-length cap");
  assert(suggestLobbyName("Alex") === "Alex's lobby", "lobby create suggests a lobby from player name");
  assert(suggestLobbyName("") === "Commander's lobby", "lobby create suggestion falls back when player name is blank");
  assert(validateLobbyName(suggestLobbyName("x".repeat(120))).ok,
    "lobby create suggestion stays within the public lobby name limit");
  assert(validateLobbyName(suggestLobbyName("__lab__:sandbox")).ok,
    "lobby create suggestion avoids reserved internal prefixes");
  const indexHtml = fs.readFileSync(new URL("../client/index.html", import.meta.url), "utf8");
  assert(indexHtml.includes('class="lobby-manual-room" hidden'),
    "manual room-name join controls stay outside the normal pre-join product path");
  assert(indexHtml.includes("#lobby-room and #lobby-join remain hidden compatibility controls"),
    "DOM contract documents room-name controls as hidden compatibility only");
  assert(indexHtml.includes('id="lobby-lab-open"'),
    "normal lobby exposes a direct lab entry affordance");
  assert(indexHtml.includes('href="/lab?room=default&map=Default"'),
    "normal lobby lab entry uses the direct shared lab URL contract");
  assert(!indexHtml.includes('id="lobby-quickstart"'),
    "normal lobby does not render the legacy quickstart control");
  assert(!indexHtml.includes("Debug mode"),
    "normal lobby copy no longer advertises Debug mode as the experimentation path");

  const sorted = sortLobbySummaries([
    { room: "old-open", hostName: "A", createdAtUnixMs: 100, joinState: "open" },
    { room: "in-game", hostName: "B", createdAtUnixMs: 900, joinState: "inGame" },
    { room: "full", hostName: "C", createdAtUnixMs: 800, joinState: "fullSpectatorOnly" },
    { room: "new-open", hostName: "D", createdAtUnixMs: 700, joinState: "open" },
    { room: "starting", hostName: "E", createdAtUnixMs: 950, joinState: "starting" },
  ]);
  assertDeepEqual(
    sorted.map((row) => row.room),
    ["new-open", "old-open", "full", "in-game", "starting"],
    "lobby browser sorts open, full, in-game, and starting rows by joinability then age",
  );

  withFakeDocument(() => {
    const joins = [];
    let createClicks = 0;
    const rowsRoot = {
      children: [],
      replaceChildren(...children) {
        this.children = children;
      },
    };
    const statusEl = { textContent: "" };
    const root = {
      classList: fakeClassList(),
      querySelector(selector) {
        if (selector === "#lobby-browser-rows") return rowsRoot;
        if (selector === "#lobby-browser-status") return statusEl;
        return null;
      },
    };
    const view = new LobbyBrowserView(root);
    view.render({
      rows: [],
      nowMs: now,
      onCreateLobby: () => { createClicks += 1; },
      onJoinLobby: (row, options) => joins.push({ room: row.room, spectator: !!options?.spectator }),
    });
    assert(textWithin(rowsRoot).includes("No lobbies"), "lobby browser renders compact empty state");
    findFakes(rowsRoot, (el) => el.tagName === "BUTTON" && el.textContent === "Create lobby")[0]?.click();
    assert(createClicks === 1, "empty lobby browser create action opens the create flow");
    view.render({
      rows: [
        {
          room: "Open Lobby",
          hostName: "Host A",
          map: "Default",
          createdAtUnixMs: now - 30_000,
          occupiedSlots: 1,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "open",
        },
        {
          room: "Alpha Long Lobby",
          hostName: "Host",
          map: "No Terrain",
          createdAtUnixMs: now - 60_000,
          occupiedSlots: 4,
          maxSlots: 4,
          spectatorCount: 1,
          joinState: "fullSpectatorOnly",
        },
        {
          room: "Locked Match",
          hostName: "Host C",
          map: "Default",
          createdAtUnixMs: now - 90_000,
          occupiedSlots: 4,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "inGame",
        },
        {
          room: "Countdown Match",
          hostName: "Host D",
          map: "Default",
          createdAtUnixMs: now - 10_000,
          occupiedSlots: 2,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "starting",
        },
        {
          room: "Unknown State",
          hostName: "Host E",
          map: "Default",
          createdAtUnixMs: now - 20_000,
          occupiedSlots: 1,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "mystery",
        },
      ],
      nowMs: now,
    });
    assert(textWithin(rowsRoot).includes("Alpha Long Lobby"), "lobby browser renders lobby names");
    assert(textWithin(rowsRoot).includes("No Terrain"), "lobby browser renders map names");
    assert(textWithin(rowsRoot).includes("4 / 4 +1 obs"), "lobby browser renders active slots and spectators");
    assert(textWithin(rowsRoot).includes("Join as spectator"), "lobby browser renders row action state");
    const row = rowsRoot.children.find((child) => child.dataset.joinState === "fullSpectatorOnly");
    assert(row.dataset.joinState === "fullSpectatorOnly", "lobby browser pins row join-state data");
    const buttons = findFakes(rowsRoot, (el) => el.tagName === "BUTTON");
    const openButton = buttons.find((button) => button.textContent === "Join lobby");
    const spectatorButton = buttons.find((button) => button.textContent === "Join as spectator");
    const inGameButton = buttons.find((button) => button.textContent === "Spectate");
    const startingButton = buttons.find((button) => button.textContent === "Starting");
    const staleButton = buttons.find((button) => button.textContent === "Stale");
    assert(!openButton?.disabled, "open lobby row action is enabled");
    assert(!spectatorButton?.disabled, "full lobby row action joins as spectator");
    assert(!inGameButton?.disabled, "in-game lobby row action joins as spectator");
    assert(startingButton?.disabled, "countdown lobby row action stays disabled");
    assert(staleButton?.disabled, "unknown lobby row action stays disabled as stale");
    openButton.click();
    spectatorButton.click();
    inGameButton.click();
    assertDeepEqual(joins, [
      { room: "Open Lobby", spectator: false },
      { room: "Alpha Long Lobby", spectator: true },
      { room: "Locked Match", spectator: true },
    ], "lobby browser row actions carry active vs spectator join intent");
    view.render({
      rows: [
        {
          room: "Refresh Failed",
          hostName: "Host F",
          map: "Default",
          createdAtUnixMs: now,
          occupiedSlots: 1,
          maxSlots: 4,
          spectatorCount: 0,
          joinState: "open",
        },
      ],
      error: "Lobby list unavailable.",
    });
    const disabledAfterError = findFakes(rowsRoot,
      (el) => el.tagName === "BUTTON" && el.textContent === "Join lobby")[0];
    assert(disabledAfterError?.disabled, "failed lobby-list refresh disables stale row actions");
  });

  await withFakeDocument(async () => {
    const host = document.createElement("section");
    const trigger = document.createElement("button");
    let submitted = "";
    const modal = new LobbyCreateModal(host, {
      onSubmit: async (room) => {
        submitted = room;
        modal.setError("Lobby name is already in use.");
        return false;
      },
    });
    modal.open(trigger, { initialValue: "Alex's lobby" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    const input = findFakes(host, (el) => el.tagName === "INPUT")[0];
    const submit = findFakes(host, (el) => el.tagName === "BUTTON" && el.textContent === "Create lobby")[0];
    assert(document.activeElement === input, "create lobby modal moves focus to the name input");
    assert(input.value === "Alex's lobby", "create lobby modal prepopulates the suggested lobby name");
    assert(!submit.disabled, "create lobby modal enables submit when the suggested lobby name is valid");
    input.value = "taken";
    input.listeners.input?.({ target: input });
    assert(!submit.disabled, "create lobby modal enables submit for a valid name");
    submit.click();
    await Promise.resolve();
    assert(submitted === "taken", "create lobby modal submits the trimmed lobby name");
    assert(textWithin(host).includes("Lobby name is already in use."),
      "duplicate create failures are displayed inline");
    modal.close();
    assert(document.activeElement === trigger, "create lobby modal returns focus to the trigger");
    modal.destroy();
  });
}

// ---------------------------------------------------------------------------
// Scoreboard team helpers
// ---------------------------------------------------------------------------
{
  assert(formatTeamLabel(2) === "Team 2", "scoreboard formats numeric team labels");
  assert(formatTeamLabel(null) === "-", "scoreboard formats missing team labels");
  assert(scoreRowIsWinner({ id: 7, teamId: 2 }, 7, null), "scoreboard keeps winnerId fallback");
  assert(scoreRowIsWinner({ id: 8, teamId: 2 }, 7, 2), "scoreboard highlights all winning-team rows");
  assert(!scoreRowIsWinner({ id: 7, teamId: 1 }, 7, 2),
    "winnerTeamId takes precedence over singleton winnerId highlighting");
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
  assertHasMethod(net, "pauseGame", "Net");
  assertHasMethod(net, "unpauseGame", "Net");
  assertHasMethod(net, "returnToLobby", "Net");
  assertHasMethod(net, "command", "Net");
  assertHasMethod(net, "ping", "Net");
  assertHasMethod(net, "netReport", "Net");
  assertHasGetter(net, "playerId", "Net");
  assert(net.playerId === null, "Net.playerId should be null before welcome");
  assertHasMethod(net, "addAi", "Net");
  assertHasMethod(net, "removeAi", "Net");
  assertHasMethod(net, "setTeamPreset", "Net");
  assertHasMethod(net, "setTeam", "Net");
  assertHasMethod(net, "setFaction", "Net");
  assertHasMethod(net, "setQuickstart", "Net");
  assertHasMethod(net, "setRoomTimeSpeed", "Net");
  assertHasMethod(net, "stepRoomTime", "Net");
  assertHasMethod(net, "seekRoomTime", "Net");
  assertHasMethod(net, "seekRoomTimeTo", "Net");
  assertHasMethod(net, "setReplayVision", "Net");
  assertHasMethod(net, "lab", "Net");
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
  net.pauseGame();
  net.unpauseGame();
  assert(sent[1].t === "pauseGame" && sent[2].t === "unpauseGame", "Net live pause helpers send exact tags");
  net.lab(12, { op: "setVision", vision: msg.labVisionFullWorld() });
  assert(sent[3].t === "lab" && sent[3].requestId === 12, "Net.lab sends lab request envelopes");
  assert(
    msg.labExportScenario(13, "saved").op.name === "saved",
    "lab export builder includes a scenario name",
  );
  assert(
    msg.labImportScenario(14, { schemaVersion: 1 }).op.scenario.schemaVersion === 1,
    "lab import builder includes a scenario payload",
  );
  assert(!("replayOk" in msg.join("A", "main")), "join builder omits replayOk by default");
  assert(
    msg.join("A", "main", false, true).replayOk === true,
    "join builder can confirm replay joins",
  );
  const priorPerformance = globalThis.performance;
  let nowSamples = [0, 2, 2, 5, 10, 13, 13, 17];
  globalThis.performance = { now: () => nowSamples.shift() ?? 17 };
  try {
    const reportNet = new Net("ws://example.invalid");
    reportNet.ws = { extensions: "permessage-deflate; client_max_window_bits" };
    reportNet._onMessage({
      data: messagePackSnapshotFrame({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [],
        n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 0, null],
      }),
    });
    reportNet._onMessage({
      data: messagePackSnapshotFrame({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [2, 0, 0, 0, 0],
        e: [],
        n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 0, null],
      }),
    });
    const stats = reportNet.consumeSnapshotReportStats();
    assert(stats.snapshotMessageCount === 2, "Net reports snapshot message count");
    assert(stats.snapshotBytesTotal > stats.snapshotBytesMax, "Net reports bounded snapshot byte totals");
    assert(
      stats.snapshotByteSource === "messagepack-application-payload",
      "Net labels MessagePack payload byte measurement source",
    );
    assert(stats.snapshotCodec === SNAPSHOT_CODEC.MESSAGEPACK_COMPACT, "Net reports snapshot codec");
    assert(stats.snapshotCodecVersion === SNAPSHOT_CODEC_VERSION, "Net reports snapshot codec version");
    assert(stats.snapshotFrameKind === SNAPSHOT_FRAME_KIND.BINARY, "Net reports binary snapshot frame kind");
    assert(
      stats.websocketExtensions.includes("permessage-deflate"),
      "Net reports browser WebSocket extension string",
    );
    assert(
      stats.websocketCompression === "permessage-deflate",
      "Net reports negotiated permessage-deflate state",
    );
    assert(stats.snapshotSegmentBudgetBytes === SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES, "Net reports snapshot packet budget");
    assert(stats.snapshotBytesP95 >= stats.snapshotBytesAvg, "Net reports snapshot byte p95");
    assert(stats.snapshotOverSegmentBudgetCount === 0, "small snapshots stay within packet budget");
    assert(stats.snapshotParseMaxMs === 3, "Net reports snapshot frame parse max");
    assert(stats.snapshotDecodeMaxMs === 4, "Net reports compact decode max");
    const resetStats = reportNet.consumeSnapshotReportStats();
    assert(resetStats.snapshotMessageCount === 0, "Net snapshot report stats reset");
    assert(resetStats.websocketCompression === "permessage-deflate", "Net keeps compression state after stats reset");
    assert(resetStats.snapshotOverSegmentBudgetCount === 0, "Net snapshot packet-budget stats reset");
    assert(resetStats.snapshotCodec === SNAPSHOT_CODEC.MESSAGEPACK_COMPACT, "Net snapshot codec default resets");
    assert(resetStats.snapshotFrameKind === SNAPSHOT_FRAME_KIND.BINARY, "Net snapshot frame kind default resets");

    reportNet.noteSnapshotFrame({
      bytes: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES + 1,
      parseMs: 0,
      decodeMs: 0,
      snapshotCodec: SNAPSHOT_CODEC.MESSAGEPACK_COMPACT,
      snapshotCodecVersion: SNAPSHOT_CODEC_VERSION,
      frameKind: SNAPSHOT_FRAME_KIND.BINARY,
    });
    const overBudget = reportNet.consumeSnapshotReportStats();
    assert(overBudget.snapshotBytesP95 > SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES, "Net reports over-budget byte p95");
    assert(overBudget.snapshotOverSegmentBudgetCount === 1, "Net counts over-budget snapshot frames");
    assert(overBudget.snapshotOverSegmentBudgetPctX100 === 10000, "Net reports over-budget snapshot percentage");
  } finally {
    globalThis.performance = priorPerformance;
  }
  for (const field of [
    "snapshotBytesTotal",
    "snapshotByteSource",
    "snapshotCodec",
    "snapshotCodecVersion",
    "snapshotFrameKind",
    "snapshotBytesP95",
    "snapshotSegmentBudgetBytes",
    "snapshotOverSegmentBudgetCount",
    "snapshotOverSegmentBudgetPctX100",
    "snapshotParseMaxMs",
    "snapshotDecodeP95Ms",
    "websocketExtensions",
    "websocketCompression",
    "snapshotApplyMaxMs",
    "predictionApplyP95Ms",
    "snapshotTickGapMax",
    "snapshotBurstMax",
    "frameWorkMaxMs",
    "frameWorkP95Ms",
    "slowFrameCount",
    "worstFramePhase",
    "rendererMaxMs",
    "entityCount",
    "devicePixelRatioX100",
  ]) {
    assert(CLIENT_NET_REPORT_FIELDS.includes(field), `client net-report field list includes ${field}`);
  }
  assert(msg.netReport({ schemaVersion: 1 }).t === "netReport", "net-report builder tag");
  assert(msg.netReport({ schemaVersion: 1 }).report.schemaVersion === 1, "net-report builder payload");
  assert(msg.returnToLobby().t === "returnToLobby", "return-to-lobby builder tag");
  assert(msg.setRoomTimeSpeed(2).t === "setRoomTimeSpeed", "room-time speed builder tag");
  assert(msg.stepRoomTime().t === "stepRoomTime", "room-time step builder tag");
  assert(msg.seekRoomTime(90).ticksBack === 90, "room-time relative seek builder payload");
  assert(msg.seekRoomTimeTo(450).tick === 450, "room-time absolute seek builder payload");
  assert(msg.setTeamPreset("1v2").preset === "1v2", "team preset builder payload");
  assert(msg.setTeam(7, 2).teamId === 2, "team assignment builder payload");
  assert(msg.setFaction("ekat").factionId === "ekat", "faction selection builder payload");
  assert(DEFAULT_AI_PROFILE_ID === "ai_1_1_tank_mg", "lobby defaults to the highest AI profile version");
  assert(msg.addAi(2).teamId === 2, "addAi builder can include teamId");
  assert(
    msg.addAi(2, DEFAULT_AI_PROFILE_ID).aiProfileId === DEFAULT_AI_PROFILE_ID,
    "addAi builder can include default aiProfileId",
  );
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
// Lab client and panel
// ---------------------------------------------------------------------------
{
  const sent = [];
  const net = new Net("ws://example.test/ws");
  net.ws = {
    readyState: WebSocket.OPEN,
    bufferedAmount: 0,
    send(json) {
      sent.push(JSON.parse(json));
    },
  };
  const labClient = new LabClient(net, { timeoutMs: 1000 });
  let observedState = null;
  let observedResult = null;
  labClient.subscribeState((state) => {
    observedState = state;
  });
  labClient.subscribeResult((result) => {
    observedResult = result;
  });
  labClient.setInitialState({
    room: "__lab__:sandbox:map=Default",
    operatorId: 1,
    role: LAB_ROLE.OPERATOR,
    vision: labVision.fullWorld(),
    dirty: false,
    operationCount: 0,
  });
  assert(observedState.role === "operator", "LabClient publishes initial lab state");
  const resultPromise = labClient.setVision(labVision.team(2));
  assert(
    sent.at(-1).op.vision.mode === "team" && sent.at(-1).requestId === 1,
    "LabClient allocates request ids for vision operations",
  );
  net._emit("labResult", { t: "labResult", requestId: 1, ok: true, op: "setVision" });
  const result = await resultPromise;
  assert(result.ok && observedResult.ok, "LabClient resolves matching labResult messages");
  void labClient.spawnEntity({ owner: 2, kind: KIND.RIFLEMAN, x: 128, y: 160, completed: true });
  assert(sent.at(-1).op.op === "spawnEntity" && sent.at(-1).op.kind === KIND.RIFLEMAN, "LabClient sends spawn operations");
  void labClient.setCompletedResearch(1, UPGRADE.TANK_UNLOCK, true);
  assert(sent.at(-1).op.op === "setCompletedResearch" && sent.at(-1).op.upgrade === UPGRADE.TANK_UNLOCK, "LabClient sends research operations");
  void labClient.exportScenario("saved setup");
  assert(sent.at(-1).op.op === "exportScenario" && sent.at(-1).op.name === "saved setup", "LabClient sends scenario export requests");
  void labClient.importScenario({ schemaVersion: 1, kind: "labScenario" });
  assert(sent.at(-1).op.op === "importScenario" && sent.at(-1).op.scenario.kind === "labScenario", "LabClient sends scenario import requests");
  assert(labVisionLabel(labVision.teams([1, 2])) === "Teams 1, 2", "labVisionLabel formats team unions");
  labClient.destroy();
}

{
  const requests = [];
  const policy = createLabControlPolicy({
    labClient: { request: (op) => { requests.push(op); return Promise.resolve({ ok: true }); } },
    metadata: { role: LAB_ROLE.OPERATOR },
  });
  assert(policy.kind === "lab" && policy.canIssueAs(1), "lab control policy gates issue-as to operator");
  const state = {
    selectedEntities() {
      return [{ id: 11, owner: 2, kind: KIND.RIFLEMAN }];
    },
  };
  assert(policy.canControlOwner(2, state), "lab control policy controls a single selected owner");
  assert(!policy.canControlOwner(1, state), "lab control policy rejects non-selected owners");
  assert(policy.canUseCommandSurface(state), "lab operator can use the command surface");
  const issued = await policy.issueCommand(cmd.move([11], 20, 30), { state });
  assert(issued.sent && requests[0].playerId === 2, "lab control policy routes gameplay commands through issue-as");
  const mixedState = {
    selectedEntities() {
      return [{ id: 11, owner: 1, kind: KIND.RIFLEMAN }, { id: 12, owner: 2, kind: KIND.RIFLEMAN }];
    },
  };
  assert(!policy.canIssueGameplayCommand(cmd.stop([11, 12]), mixedState).ok, "lab policy rejects mixed-owner gameplay commands");
  assert(
    !createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } }).canUseCommandSurface(state),
    "read-only lab viewers cannot use the command surface",
  );
  assert(!createDefaultControlPolicy().canUseCommandSurface({ spectator: true }), "default spectators cannot use the command surface");
  assert(!createDefaultControlPolicy().canIssueAs(1), "default control policy does not issue-as");
}

{
  assertDeepEqual(
    labSpawnFactionOptions().map((entry) => entry.id),
    ["kriegsia", "ekat"],
    "LabPanel spawn palette exposes product-playable faction catalogs",
  );
  assert(
    labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.RIFLEMAN),
    "LabPanel spawn palette includes Kriegsia catalog units",
  );
  assert(
    !labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.CITY_CENTRE),
    "LabPanel spawn palette excludes buildings from primary unit options",
  );
  assertDeepEqual(
    labSpawnUnitKindsForFaction("ekat"),
    [KIND.EKAT],
    "LabPanel spawn palette filters Ekat to Ekat units",
  );
}

await withFakeDocument(async () => {
  const buildLabClient = (role) => {
    const net = new Net("ws://example.test/ws");
    const labClient = new LabClient(net);
    labClient.setInitialState({
      room: "__lab__:sandbox:map=Default",
      operatorId: 1,
      role,
      vision: labVision.fullWorld(),
      dirty: true,
      operationCount: 7,
    });
    return labClient;
  };
  const buildMatch = (panelRef) => ({
    clientIntent: new ClientIntent(),
    state: {
      map: { width: 64, height: 64 },
      playerResources: [],
      selectedEntities() {
        return [];
      },
    },
    armLabTool(tool) {
      const armed = this.clientIntent.beginLabTool({ id: `tool-${tool.kind}-${panelRef.root.children.length}`, ...tool });
      panelRef.panel?.applyLabToolChange({ type: "armed", tool: armed });
      return armed;
    },
    cancelLabTool(reason) {
      const cancelled = this.clientIntent.cancelLabTool(reason);
      if (cancelled) panelRef.panel?.applyLabToolChange({ type: "cancelled", reason, tool: cancelled });
      return cancelled;
    },
  });
  const startPayload = {
    map: { name: "Default" },
    players: [{ id: 1, teamId: 1 }, { id: 2, teamId: 2 }],
  };

  const rootA = document.createElement("div");
  const rootB = document.createElement("div");
  const refA = { root: rootA, panel: null };
  const refB = { root: rootB, panel: null };
  const operatorA = buildLabClient(LAB_ROLE.OPERATOR);
  const operatorB = buildLabClient(LAB_ROLE.OPERATOR);
  const matchA = buildMatch(refA);
  const matchB = buildMatch(refB);
  refA.panel = new LabPanel({ root: rootA, labClient: operatorA, startPayload, match: matchA });
  refB.panel = new LabPanel({ root: rootB, labClient: operatorB, startPayload, match: matchB });

  assert(textWithin(rootB).includes("Operator"), "later lab joiner operator role renders as Operator");
  assert(refB.panel.fields.has("lab-player"), "later lab joiner operator receives setup tools");
  refA.panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  assert(textWithin(rootA).includes("Armed: Spawn Rifleman"), "one lab tab can arm a setup tool locally");
  assert(textWithin(rootB).includes("No setup tool armed"), "another lab tab keeps its own setup tool state");

  const readOnlyRoot = document.createElement("div");
  const readOnlyClient = buildLabClient(LAB_ROLE.READ_ONLY);
  const readOnlyRef = { root: readOnlyRoot, panel: null };
  readOnlyRef.panel = new LabPanel({
    root: readOnlyRoot,
    labClient: readOnlyClient,
    startPayload,
    match: buildMatch(readOnlyRef),
  });
  assert(textWithin(readOnlyRoot).includes("Read-only"), "read-only lab role renders read-only status");
  assert(!readOnlyRef.panel.fields.has("lab-player"), "read-only lab role does not expose setup tools");

  refA.panel.destroy();
  refB.panel.destroy();
  readOnlyRef.panel.destroy();
  operatorA.destroy();
  operatorB.destroy();
  readOnlyClient.destroy();
});

await withFakeDocument(async () => {
  const sent = [];
  const net = new Net("ws://example.test/ws");
  net.ws = {
    readyState: WebSocket.OPEN,
    bufferedAmount: 0,
    send(json) {
      sent.push(JSON.parse(json));
    },
  };
  const root = document.createElement("div");
  const labClient = new LabClient(net);
  let armedTool = null;
  let armedCallbacks = null;
  let cancelledToolReason = null;
  let selectedEntities = [];
  let panel = null;
  const match = {
    clientIntent: new ClientIntent(),
    camera: { x: 320, y: 352 },
    state: {
      map: { width: 64, height: 64 },
      playerResources: [{ steel: 500, oil: 200 }],
      selectedEntities() {
        return selectedEntities;
      },
    },
    armLabTool(tool, callbacks) {
      armedTool = this.clientIntent.beginLabTool({ id: "lab-tool-test", ...tool });
      const toolAtArm = armedTool;
      armedCallbacks = {
        onWorldClick: (event) => {
          const result = callbacks.onWorldClick(event);
          if (!toolAtArm.keepArmedOnWorldClick) this.cancelLabTool("worldClick");
          return result;
        },
      };
      panel?.applyLabToolChange({ type: "armed", tool: armedTool });
      return armedTool;
    },
    cancelLabTool(reason) {
      cancelledToolReason = reason;
      const cancelled = this.clientIntent.cancelLabTool(reason);
      if (cancelled) panel?.applyLabToolChange({ type: "cancelled", reason, tool: cancelled });
      return cancelled;
    },
  };
  labClient.setInitialState({
    room: "__lab__:sandbox:map=Default",
    operatorId: 1,
    role: LAB_ROLE.OPERATOR,
    vision: labVision.fullWorld(),
    dirty: false,
    operationCount: 0,
  });
  panel = new LabPanel({
    root,
    labClient,
    launch: { publicRoom: "sandbox", map: "Default" },
    startPayload: {
      map: { name: "Default" },
      players: [{ id: 1, teamId: 1 }, { id: 2, teamId: 2 }],
    },
    match,
  });
  const buttonByText = (label) => findFakes(root, (el) => el.tagName === "BUTTON" && el.textContent === label)[0];
  const resolveLastLabResult = (options = {}) => {
    const envelope = sent.at(-1);
    net._emit("labResult", {
      t: "labResult",
      requestId: envelope.requestId,
      ok: options.ok !== false,
      op: envelope.op.op,
      error: options.error || "",
      outcome: options.outcome || null,
    });
  };

  assert(root.children.length === 1, "LabPanel mounts inside the app-owned root");
  assert(textWithin(root).includes("Reset"), "LabPanel exposes a reset affordance for its movable panel");
  assert(
    root.children[0].children.some((child) => child.className === "lab-panel-resize-handle"),
    "LabPanel exposes a visible resize handle",
  );
  assert(textWithin(root).includes("Operator"), "LabPanel renders role state");
  assert(buttonByText("Cancel tool").disabled, "LabPanel disables tool cancellation when no setup tool is armed");
  assert(panel.fields.has("lab-player"), "LabPanel exposes one shared player selector for lab setup tools");
  assert(!textWithin(root).includes("Advanced Spawn"), "LabPanel omits the advanced spawn form");
  assert(
    !panel.fields.has("spawn-owner") &&
      !panel.fields.has("advanced-spawn-owner") &&
      !panel.fields.has("resource-player") &&
      !panel.fields.has("research-player"),
    "LabPanel does not render per-tool player selectors for spawn or player-state controls",
  );
  assert(
    !textWithin(root).includes("Advanced Spawn") &&
      !panel.fields.has("advanced-spawn-kind") &&
      !panel.fields.has("advanced-spawn-completed") &&
      !panel.fields.has("spawn-completed") &&
      !panel.fields.has("research-completed"),
    "LabPanel does not expose advanced spawn or completion toggles",
  );
  const teamButton = root.children[0].children
    .flatMap((child) => child.children || [])
    .find((child) => child.textContent === "Team 2");
  teamButton.listeners.click();
  assert(sent.at(-1).op.vision.teamId === 2, "LabPanel vision controls send lab vision requests");
  panel.fields.get("lab-player").value = "2";
  panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  assert(armedTool?.kind === "spawnEntity", "LabPanel unit palette arms the spawn lab tool through Match");
  assert(armedTool?.keepArmedOnWorldClick === true, "LabPanel unit palette keeps the spawn tool armed across world clicks");
  assert(textWithin(root).includes("Armed: Spawn Rifleman"), "LabPanel shows the armed spawn tool state");
  assert(!buttonByText("Cancel tool").disabled, "LabPanel enables tool cancellation while a setup tool is armed");
  assert(
    armedTool.payload.owner === 2 &&
      armedTool.payload.kind === KIND.RIFLEMAN &&
      armedTool.payload.factionId === DEFAULT_FACTION_ID &&
      armedTool.payload.completed === true,
    "LabPanel unit palette captures owner, faction, and kind with completed spawn payloads",
  );
  armedCallbacks.onWorldClick({ tool: { ...armedTool }, x: 128.5, y: 160.25 });
  assert(match.clientIntent.activeLabTool?.id === armedTool.id, "LabPanel spawn tool stays armed after sending a spawn request");
  assert(
    sent.at(-1).op.op === "spawnEntity" &&
      sent.at(-1).op.owner === 2 &&
      sent.at(-1).op.kind === KIND.RIFLEMAN &&
      sent.at(-1).op.x === 128.5 &&
      sent.at(-1).op.y === 160.25 &&
      sent.at(-1).op.completed === true,
    "LabPanel spawn tool sends clicked world coordinates through LabClient with completed spawns",
  );
  net._emit("labResult", {
    t: "labResult",
    requestId: sent.at(-1).requestId,
    ok: false,
    op: "spawnEntity",
    error: "occupied placement",
  });
  assert(textWithin(root).includes("occupied placement"), "LabPanel surfaces rejected spawn results through the status path");
  assert(match.clientIntent.activeLabTool?.id === armedTool.id, "LabPanel spawn tool stays armed after rejected spawn results");
  assert(!buttonByText("Cancel tool").disabled, "LabPanel keeps cancellation available after rejected spawn results");
  panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  match.cancelLabTool("escape");
  assert(textWithin(root).includes("Spawn Rifleman cancelled."), "LabPanel surfaces keyboard cancellation through the status path");
  panel.fields.get("spawn-faction").value = "ekat";
  panel.fields.get("spawn-faction").listeners.change();
  assert(panel.spawnPalette.kind === KIND.EKAT, "LabPanel faction selection updates the unit palette deterministically");
  assert(buttonByText("Move to point").disabled, "LabPanel disables selected move without a selection");
  assert(buttonByText("Set owner").disabled, "LabPanel disables selected owner changes without a selection");
  assert(buttonByText("Delete").disabled, "LabPanel disables selected deletes without a selection");
  selectedEntities = [
    { id: 31, owner: 1, kind: KIND.RIFLEMAN },
    { id: 32, owner: 2, kind: KIND.RIFLEMAN },
  ];
  panel.render();
  assert(!buttonByText("Move to point").disabled, "LabPanel enables selected move for selected entities");
  buttonByText("Move to point").listeners.click();
  assert(
    armedTool?.kind === "moveSelected" && armedTool.payload.entityIds.join(",") === "31,32",
    "LabPanel move-selected tool captures the selected entity ids in the tool payload",
  );
  assert(textWithin(root).includes("Armed: Move 2 selected"), "LabPanel shows the armed selected-move tool state");
  match.cancelLabTool("rightClick");
  assert(textWithin(root).includes("Move 2 selected cancelled."), "LabPanel surfaces pointer cancellation through the status path");
  buttonByText("Move to point").listeners.click();
  const movePromise = armedCallbacks.onWorldClick({ tool: { ...armedTool }, x: 129.4, y: 160.6 });
  assert(
    sent.at(-1).op.op === "moveEntity" &&
      sent.at(-1).op.entityId === 31 &&
      sent.at(-1).op.x === 129.4 &&
      sent.at(-1).op.y === 160.6,
    "LabPanel selected move sends the clicked world coordinates for the first selected entity",
  );
  resolveLastLabResult({ outcome: { entityId: 31, x: 129.4, y: 160.6 } });
  await Promise.resolve();
  assert(
    sent.at(-1).op.op === "moveEntity" &&
      sent.at(-1).op.entityId === 32 &&
      sent.at(-1).op.x === 129.4 &&
      sent.at(-1).op.y === 160.6,
    "LabPanel selected move reuses the clicked world coordinates for each selected entity",
  );
  resolveLastLabResult({ ok: false, error: "entity 32 not found" });
  await movePromise;
  assert(
    textWithin(root).includes("Moved 1 entity; 1 rejected: #32: entity 32 not found."),
    "LabPanel summarizes partial selected-move rejections",
  );
  panel.fields.get("set-owner").value = "1";
  const setOwnerPromise = buttonByText("Set owner").listeners.click();
  assert(
    sent.at(-1).op.op === "setEntityOwner" &&
      sent.at(-1).op.entityId === 31 &&
      sent.at(-1).op.owner === 1,
    "LabPanel selected owner change sends the requested owner for the first selected entity",
  );
  resolveLastLabResult({ outcome: { entityId: 31, owner: 1 } });
  await Promise.resolve();
  assert(
    sent.at(-1).op.op === "setEntityOwner" &&
      sent.at(-1).op.entityId === 32 &&
      sent.at(-1).op.owner === 1,
    "LabPanel selected owner change sends all selected entity ids",
  );
  resolveLastLabResult({ outcome: { entityId: 32, owner: 1 } });
  await setOwnerPromise;
  assert(textWithin(root).includes("Updated owner for 2 entities."), "LabPanel summarizes accepted owner changes");
  const deletePromise = buttonByText("Delete").listeners.click();
  assert(sent.at(-1).op.op === "deleteEntity" && sent.at(-1).op.entityId === 31, "LabPanel selected delete sends the first selected entity id");
  resolveLastLabResult({ outcome: { entityId: 31 } });
  await Promise.resolve();
  assert(sent.at(-1).op.op === "deleteEntity" && sent.at(-1).op.entityId === 32, "LabPanel selected delete sends all selected entity ids");
  resolveLastLabResult({ outcome: { entityId: 32 } });
  await deletePromise;
  assert(textWithin(root).includes("Deleted 2 entities."), "LabPanel summarizes accepted deletes");
  panel.fields.get("lab-player").value = "1";
  panel.fields.get("resource-steel").value = "900";
  panel.fields.get("resource-oil").value = "300";
  buttonByText("Set resources").listeners.click();
  assert(
    sent.at(-1).op.op === "setPlayerResources" &&
      sent.at(-1).op.playerId === 1 &&
      sent.at(-1).op.steel === 900 &&
      sent.at(-1).op.oil === 300,
    "LabPanel resource fields normalize player state edits through the shared player selector",
  );
  resolveLastLabResult({ outcome: { playerId: 1, steel: 900, oil: 300 } });
  assert(
    panel.fields.get("lab-player").value === "1" &&
      panel.fields.get("resource-steel").value === "900" &&
      panel.fields.get("resource-oil").value === "300",
    "LabPanel preserves resource form values after set-resources results re-render the panel",
  );
  const giveAllPromise = buttonByText("Give All").listeners.click();
  assert(
    sent.at(-1).op.op === "setPlayerResources" &&
      sent.at(-1).op.playerId === 1 &&
      sent.at(-1).op.steel === 99999 &&
      sent.at(-1).op.oil === 99999,
    "LabPanel Give All starts by giving player one maximum lab resources",
  );
  resolveLastLabResult({ outcome: { playerId: 1, steel: 99999, oil: 99999 } });
  await Promise.resolve();
  assert(
    sent.at(-1).op.op === "setPlayerResources" &&
      sent.at(-1).op.playerId === 2 &&
      sent.at(-1).op.steel === 99999 &&
      sent.at(-1).op.oil === 99999,
    "LabPanel Give All sends maximum lab resources to every player",
  );
  resolveLastLabResult({ outcome: { playerId: 2, steel: 99999, oil: 99999 } });
  await giveAllPromise;
  assert(
    textWithin(root).includes("Gave 2 players 99999 steel and 99999 oil."),
    "LabPanel Give All summarizes the all-player resource grant",
  );
  panel.fields.get("lab-player").value = "2";
  panel.fields.get("research-upgrade").value = UPGRADE.TANK_UNLOCK;
  buttonByText("Set research").listeners.click();
  assert(
    sent.at(-1).op.op === "setCompletedResearch" &&
      sent.at(-1).op.playerId === 2 &&
      sent.at(-1).op.upgrade === UPGRADE.TANK_UNLOCK &&
      sent.at(-1).op.completed === true,
    "LabPanel research edits use the shared player selector and complete upgrades",
  );
  resolveLastLabResult({ outcome: { playerId: 2, upgrade: UPGRADE.TANK_UNLOCK, completed: true } });
  assert(
    panel.fields.get("lab-player").value === "2" &&
      panel.fields.get("resource-steel").value === "900" &&
      panel.fields.get("resource-oil").value === "300" &&
      panel.fields.get("research-upgrade").value === UPGRADE.TANK_UNLOCK,
    "LabPanel preserves resource and research form values after set-research results re-render the panel",
  );
  panel.fields.get("scenario-name").value = "saved setup";
  void labClient.exportScenario(panel.value("scenario-name"));
  assert(sent.at(-1).op.op === "exportScenario" && sent.at(-1).op.name === "saved setup", "LabPanel scenario name feeds export requests");
  panel.fields.get("scenario-json").value = JSON.stringify({
    schemaVersion: 1,
    kind: "labScenario",
    name: "saved setup",
    metadata: { exportedTick: 0, lab: { vision: labVision.fullWorld() } },
  });
  void panel.importScenario();
  assert(sent.at(-1).op.op === "importScenario" && sent.at(-1).op.scenario.name === "saved setup", "LabPanel imports pasted scenario JSON");
  panel.destroy();
  labClient.destroy();
  assert(cancelledToolReason === "panelDestroy", "LabPanel cancels an active lab tool on teardown");
  assert(root.children[0].removed === true, "LabPanel destroy removes its DOM root");
});

await withFakeDocument(async () => {
  const root = document.createElement("div");
  const el = document.createElement("aside");
  root.appendChild(el);
  const storage = fakeStorage();
  const windowListeners = new Map();
  const windowObj = {
    innerWidth: 1000,
    innerHeight: 800,
    localStorage: storage,
    addEventListener(type, handler) {
      windowListeners.set(type, handler);
    },
    removeEventListener(type, handler) {
      if (windowListeners.get(type) === handler) windowListeners.delete(type);
    },
  };
  const chrome = new LabPanelWindowChrome(el, {
    windowObj,
    storage,
    storageKey: "test.lab.panel.window",
  });
  const header = chrome.renderHeader({ kicker: "Lab", title: "sandbox" });
  const resizeHandle = chrome.renderResizeHandle();
  el.append(header, resizeHandle);

  const dragHandle = header.children[0];
  const resetButton = header.children[1];
  dragHandle.listeners.pointerdown({
    button: 0,
    pointerId: 7,
    clientX: 900,
    clientY: 90,
    preventDefault() {},
    stopPropagation() {},
  });
  windowListeners.get("pointermove")({
    pointerId: 7,
    clientX: 840,
    clientY: 126,
    preventDefault() {},
  });
  assert(el.style.left === "608px" && el.style.top === "94px", "LabPanelWindowChrome drags the panel by pointer delta");
  windowListeners.get("pointerup")({ pointerId: 7 });
  assert(storage.values.has("test.lab.panel.window"), "LabPanelWindowChrome persists moved panel geometry");

  resizeHandle.listeners.keydown({
    key: "ArrowRight",
    shiftKey: true,
    preventDefault() {},
  });
  assert(el.style.width === "392px", "LabPanelWindowChrome keyboard resize increases width by the large step");
  dragHandle.listeners.keydown({
    key: "ArrowLeft",
    preventDefault() {},
  });
  assert(el.style.left === "572px", "LabPanelWindowChrome keyboard move nudges the clamped panel");

  resetButton.listeners.click();
  assert(el.dataset.windowed === "false", "LabPanelWindowChrome reset returns to the stylesheet layout");
  assert(!storage.values.has("test.lab.panel.window"), "LabPanelWindowChrome reset clears stored geometry");
  chrome.destroy();
  assert(!windowListeners.has("resize"), "LabPanelWindowChrome removes global listeners on destroy");
});

// ---------------------------------------------------------------------------
// Command Budget
// ---------------------------------------------------------------------------
{
  function budgetState(entities) {
    const byId = new Map(entities.map((entity) => [entity.id, entity]));
    return {
      entityById(id) {
        return byId.get(id);
      },
      isOwnOwner(owner) {
        return owner === 1;
      },
    };
  }

  const tanks = Array.from({ length: 5 }, (_, index) => ({
    id: index + 1,
    owner: 1,
    kind: KIND.TANK,
    state: STATE.IDLE,
  }));
  const overBudget = commandWithinBudget(
    budgetState(tanks),
    cmd.move(tanks.map((tank) => tank.id), 100, 100),
  );
  assert(!overBudget.ok, "client command guard rejects five tanks without a Command Car");
  assert(overBudget.used === 40 && overBudget.cap === BASE_COMMAND_SUPPLY_CAP, "client reports base command budget usage");

  const commandCar = { id: 99, owner: 1, kind: KIND.COMMAND_CAR, state: STATE.IDLE };
  const legalWithCar = commandWithinBudget(
    budgetState(tanks.concat(commandCar)),
    cmd.attackMove(tanks.map((tank) => tank.id).concat(commandCar.id), 100, 100),
  );
  assert(legalWithCar.ok, "client command guard allows five tanks with one Command Car");
  assert(
    legalWithCar.used === 44 &&
      legalWithCar.cap === BASE_COMMAND_SUPPLY_CAP + COMMAND_CAR_SUPPLY_CAP_BONUS + STATS[KIND.COMMAND_CAR].supply,
    "client command guard offsets Command Car supply before adding bonus",
  );

  const legalInfantry = Array.from({ length: 24 }, (_, index) => ({
    id: index + 200,
    owner: 1,
    kind: KIND.RIFLEMAN,
    state: STATE.IDLE,
  }));
  assert(
    commandWithinBudget(
      budgetState(legalInfantry),
      cmd.stop(legalInfantry.map((entity) => entity.id)),
    ).ok,
    "client command guard allows 24 one-supply units",
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

  const disabledSent = [];
  const disabledPrediction = new PredictionController({
    enabled: false,
    sendCommand(command, clientSeq) {
      assert(Number.isInteger(clientSeq) && clientSeq > 0, "disabled PredictionController sends valid clientSeq");
      disabledSent.push({ command, clientSeq });
      return true;
    },
  });
  const disabledIssued = disabledPrediction.issueCommand(cmd.move([7], 120, 160));
  assert(disabledIssued.sent && !disabledIssued.predicted, "PredictionController disabled mode still sends commands");
  assert(disabledIssued.clientSeq === 1, "PredictionController disabled mode still emits protocol sequence ids");
  assert(disabledSent.length === 1 && disabledSent[0].clientSeq === 1, "disabled commands use sequenced protocol send shape");
  assert(disabledPrediction.pendingCommandCount === 0, "disabled commands are not tracked as prediction pending");
  assert(disabledPrediction.debugSummary().nextClientSeq === 2, "disabled commands consume sequence ids");

  const toggledSent = [];
  const toggledPrediction = new PredictionController({
    sendCommand(command, clientSeq) {
      toggledSent.push({ command, clientSeq });
      return true;
    },
  });
  toggledPrediction.issueCommand(cmd.stop([1]));
  toggledPrediction.reset({ enabled: false, preserveClientSeq: true });
  toggledPrediction.issueCommand(cmd.stop([2]));
  toggledPrediction.reset({ enabled: true, preserveClientSeq: true });
  toggledPrediction.issueCommand(cmd.stop([3]));
  assert(
    toggledSent.map((entry) => entry.clientSeq).join(",") === "1,2,3",
    "PredictionController preserves command sequence ids across prediction toggles",
  );
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
  assert(MINING_CC_RANGE_TILES === 9, "client mirrors the server mining City Centre range");
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
  assert(STATS[KIND.FACTORY].buildTicks === 749, "Vehicle Works build time mirrors server");
  assert(
    STATS[KIND.FACTORY].trains[0] === KIND.SCOUT_CAR,
    "Vehicle Works should put Scout Car in the leftmost train slot",
  );
  assert(
    STATS[KIND.FACTORY].trains.includes(KIND.TANK),
    "Vehicle Works should train Tanks after the unlock",
  );
  assert(
    STATS[KIND.FACTORY].trains[2] === KIND.COMMAND_CAR,
    "Vehicle Works should put Command Car in the top-right train slot",
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
  assert(KIND_CODE[KIND.COMMAND_CAR] === 18, "Command Car compact kind code should be reserved");
  assert(KIND_CODE[KIND.EKAT] === 19, "Ekat compact kind code should be reserved");
  assert(KIND_CODE[KIND.ZAMOK] === 20, "Zamok compact kind code should be reserved");
  assert(KIND_CODE[KIND.TANK_TRAP] === 21, "Tank Trap compact kind code should be reserved");
  assert(ABILITY_CODE[ABILITY.POINT_FIRE] === 4, "Point Fire compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.BREAKTHROUGH] === 5, "Breakthrough compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.EKAT_TELEPORT] === 6, "Ekat Teleport compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.EKAT_LINE_SHOT] === 7, "Ekat Line Shot compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.EKAT_MAGIC_ANCHOR] === 8, "Ekat Magic Anchor compact ability code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.POINT_FIRE] === 10, "Point Fire compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.BREAKTHROUGH] === 11, "Breakthrough compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.EKAT_TELEPORT] === 12, "Ekat Teleport compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.EKAT_LINE_SHOT] === 13, "Ekat Line Shot compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.EKAT_MAGIC_ANCHOR] === 14, "Ekat Magic Anchor compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.DECONSTRUCT] === 15, "Deconstruct compact order stage code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_TARGET] === 7, "Artillery target compact event code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_IMPACT] === 8, "Artillery impact compact event code should be reserved");
  assert(EVENT_CODE[EVENT.MORTAR_LAUNCH] === 9, "Mortar launch compact event code should be reserved");
  assert(EVENT_CODE[EVENT.OVERPENETRATION] === 10, "Overpenetration compact event code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_FIRING] === 11, "Artillery firing compact event code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.MORTAR_AUTOCAST] === 5, "Mortar Autocast compact upgrade code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.COMMAND_CAR_UNLOCK] === 6, "Command Car unlock compact upgrade code should be reserved");
  assert(
    STATS[KIND.COMMAND_CAR].cost.steel === 150 &&
      STATS[KIND.COMMAND_CAR].cost.oil === 75 &&
      STATS[KIND.COMMAND_CAR].supply === 4 &&
      STATS[KIND.COMMAND_CAR].sight === 10 &&
      STATS[KIND.COMMAND_CAR].size < STATS[KIND.SCOUT_CAR].size &&
      STATS[KIND.COMMAND_CAR].body.length < STATS[KIND.SCOUT_CAR].body.length &&
      STATS[KIND.COMMAND_CAR].body.width < STATS[KIND.SCOUT_CAR].body.width,
    "Command Car stats mirror the planned server values and use a smaller body than Scout Car",
  );
  assert(
    ABILITIES[ABILITY.BREAKTHROUGH].carriers.includes(KIND.COMMAND_CAR) &&
      ABILITIES[ABILITY.BREAKTHROUGH].targetMode === "self" &&
      ABILITIES[ABILITY.BREAKTHROUGH].radiusTiles === 9 &&
      ABILITIES[ABILITY.BREAKTHROUGH].durationTicks === 180 &&
      ABILITIES[ABILITY.BREAKTHROUGH].cooldownTicks === 750,
    "Breakthrough ability exposes Command Car carrier, self target, radius, duration, and cooldown",
  );
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
    ABILITIES[ABILITY.EKAT_TELEPORT].queued === true &&
      ABILITIES[ABILITY.EKAT_LINE_SHOT].queued === true &&
      ABILITIES[ABILITY.EKAT_MAGIC_ANCHOR].queued === true,
    "Ekat abilities expose queued command support",
  );
  assert(
    STATS[KIND.STEELWORKS].footW === 3 && STATS[KIND.STEELWORKS].footH === 3,
    "Gun Works should be a 3x3 building",
  );
  assert(
    STATS[KIND.STEELWORKS].cost.steel === 150 && STATS[KIND.STEELWORKS].cost.oil === 100,
    "Gun Works cost mirrors server",
  );
  assert(STATS[KIND.STEELWORKS].buildTicks === 599, "Gun Works build time mirrors server");
  assert(
    STATS[KIND.STEELWORKS].trains.includes(KIND.ANTI_TANK_GUN),
    "Gun Works should train Anti-Tank Guns after the unlock",
  );
  assert(
    !STATS[KIND.STEELWORKS].researches,
    "Gun Works should no longer expose advanced unlock research",
  );
  assert(
    !STATS[KIND.BARRACKS].trains.includes(KIND.ANTI_TANK_GUN),
    "Barracks should no longer train Anti-Tank Guns",
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
    STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.ANTI_TANK_GUN_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.ARTILLERY_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.TANK_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.COMMAND_CAR_UNLOCK) &&
      STATS[KIND.RESEARCH_COMPLEX].researches.includes(UPGRADE.MORTAR_AUTOCAST),
    "R&D Complex should expose Anti-Tank Gun, Artillery, Tank, Command Car, and Mortar Autocast research",
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
    STATS[KIND.ANTI_TANK_GUN].upgradeRequiresText === "Requires research in R&D Complex",
    "Anti-Tank Gun training should explain the R&D Complex research requirement",
  );
  assert(
    STATS[KIND.TANK].upgradeRequiresText === "Requires research in R&D Complex",
    "Tank training should explain the R&D Complex research requirement",
  );
  assert(
    STATS[KIND.TANK_TRAP].label === "Tank Trap" &&
      STATS[KIND.TANK_TRAP].footW === 1 &&
      STATS[KIND.TANK_TRAP].footH === 1 &&
      STATS[KIND.TANK_TRAP].sight === 0 &&
      STATS[KIND.TANK_TRAP].cost.steel === 15 &&
      STATS[KIND.TANK_TRAP].cost.oil === 0 &&
      STATS[KIND.TANK_TRAP].buildTicks === TICK_HZ * 10 &&
      STATS[KIND.TANK_TRAP].requires === KIND.TRAINING_CENTRE,
    "Tank Trap dormant metadata mirrors Phase 1 server rules",
  );
  assert(
    WORKER_BUILDABLE.includes(KIND.TANK_TRAP),
    "Tank Trap is exposed in the worker build menu in the placement phase",
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
    if (!hud.clientIntent) hud.clientIntent = new ClientIntent();
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

    renderedButtons.length = 0;
    const selectedCommandCar = {
      id: 601,
      owner: playerId,
      kind: KIND.COMMAND_CAR,
      abilities: [{
        ability: ABILITY.BREAKTHROUGH,
        cooldownLeft: 0,
      }],
    };
    const commandCarHud = Object.create(HUD.prototype);
    commandCarHud.state = {
      playerId,
      resources: { steel: 100, oil: 100 },
      map: { tileSize: 32 },
      commandTarget: null,
      selectedEntities: () => [selectedCommandCar],
      entitiesInterpolated: () => [selectedCommandCar],
      updateAbilityTargetPreview(preview) {
        this.abilityTargetPreview = preview;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    commandCarHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    commandCarHud.audio = null;
    commandCarHud._cardSig = null;
    renderCommandCard(commandCarHud);
    const breakthroughButton = renderedButtons.find((button) => button.innerHTML.includes("Breakthrough"));
    assert(breakthroughButton?.dataset.hotkey === "E", "Breakthrough should use the E command-card slot");
    breakthroughButton.click({ shiftKey: true });
    const breakthroughCommand = sent[sent.length - 1];
    assert(
      breakthroughCommand?.c === "useAbility" &&
        breakthroughCommand.ability === ABILITY.BREAKTHROUGH &&
        breakthroughCommand.units[0] === selectedCommandCar.id &&
        breakthroughCommand.queued === true &&
        !("x" in breakthroughCommand) &&
        !("y" in breakthroughCommand),
      "Clicking Breakthrough should issue a queued self-target ability command without coordinates",
    );

    const leftCommandCar = {
      ...selectedCommandCar,
      id: 602,
      x: 0,
      y: 0,
    };
    const centralCommandCar = {
      ...selectedCommandCar,
      id: 603,
      x: 9,
      y: 0,
    };
    const rightCommandCar = {
      ...selectedCommandCar,
      id: 604,
      x: 30,
      y: 0,
    };
    const coolingDownCommandCar = {
      ...selectedCommandCar,
      id: 605,
      x: 10,
      y: 0,
      abilities: [{
        ability: ABILITY.BREAKTHROUGH,
        cooldownLeft: 5,
      }],
    };
    commandCarHud.state.selectedEntities = () => [
      leftCommandCar,
      centralCommandCar,
      rightCommandCar,
      coolingDownCommandCar,
    ];
    commandCarHud.state.entitiesInterpolated = commandCarHud.state.selectedEntities;
    commandCarHud._cardSig = null;
    renderedButtons.length = 0;
    renderCommandCard(commandCarHud);
    const multiBreakthroughButton = renderedButtons.find((button) => button.innerHTML.includes("Breakthrough"));
    multiBreakthroughButton.dispatchEvent({ type: "mouseenter" });
    assert(
      commandCarHud.clientIntent.abilityTargetPreview?.areaOrigins.length === 1 &&
        commandCarHud.clientIntent.abilityTargetPreview.areaOrigins[0].id === centralCommandCar.id,
      "Breakthrough hover preview should show only the Command Car that would activate",
    );
    multiBreakthroughButton.click({});
    const multiBreakthroughCommand = sent[sent.length - 1];
    assert(
      multiBreakthroughCommand.units.length === 1 &&
        multiBreakthroughCommand.units[0] === centralCommandCar.id,
      "Breakthrough should issue from the most central ready Command Car only",
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
    const hotkeyCommand = sent[sent.length - 1];
    assert(
      hotkeyCommand?.c === "research" &&
        hotkeyCommand.building === 77 &&
        hotkeyCommand.upgrade === UPGRADE.METHAMPHETAMINES,
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
    const commandCarButton = renderedButtons.find((button) => button.innerHTML.includes("Command Car"));
    const tankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("TK+"));
    assert(scoutCarButton?.dataset.hotkey === "Q", "Scout Car training should keep the Q slot");
    assert(tankButton?.dataset.hotkey === "W", "Tank training should occupy the top-middle W slot");
    assert(commandCarButton?.dataset.hotkey === "E", "Command Car training should occupy the top-right E slot");
    assert(commandCarButton?.disabled, "Command Car training should be disabled before its R&D unlock");
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
    const antiTankGunButton = renderedButtons.find((button) => button.innerHTML.includes("Anti-Tank Gun"));
    const artilleryButton = renderedButtons.find((button) => button.innerHTML.includes("Artillery"));
    const antiTankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("ATG+"));
    const artilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    assert(mortarButton?.dataset.hotkey === "Q", "Mortar Team training should occupy the top-left Q slot");
    assert(antiTankGunButton?.dataset.hotkey === "W", "Anti-Tank Gun training should occupy the top-middle W slot");
    assert(artilleryButton?.dataset.hotkey === "E", "Artillery training should occupy the top-right E slot");
    assert(!antiTankResearchButton, "Anti-Tank Gun Crews research should move out of Gun Works");
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
    const rdAntiTankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("ATG+"));
    const rdArtilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    const rdTankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("TK+"));
    const rdCommandCarResearchButton = renderedButtons.find((button) => button.innerHTML.includes("CC+"));
    const rdMortarAutocastButton = renderedButtons.find((button) => button.innerHTML.includes("MT+"));
    assert(rdAntiTankResearchButton?.dataset.hotkey === "Q", "Anti-Tank Gun Crews research should appear in R&D Complex");
    assert(rdTankResearchButton?.dataset.hotkey === "E", "Tank Production research should appear in R&D Complex");
    assert(rdMortarAutocastButton?.dataset.hotkey === "A", "Mortar Autocast research should appear in R&D Complex");
    assert(rdCommandCarResearchButton?.dataset.hotkey === "S", "Command Car research should appear in R&D Complex");
    assert(rdCommandCarResearchButton?.disabled, "Command Car research should be disabled before Tank Production");
    assert(rdCommandCarResearchButton?.title === "Requires Tank Production", "Command Car research should name Tank prerequisite");
    assert(rdArtilleryResearchButton?.dataset.hotkey === "W", "Unlock Artillery research should appear in R&D Complex");
    assert(rdArtilleryResearchButton?.disabled, "Artillery research should be disabled before Anti-Tank Gun research");
    assert(rdArtilleryResearchButton?.title === "Requires Anti-Tank Gun Research", "Artillery research should name Anti-Tank Gun prerequisite");

    renderedButtons.length = 0;
    rdHud.state.upgrades = [UPGRADE.ANTI_TANK_GUN_UNLOCK];
    rdHud._cardSig = null;
    renderCommandCard(rdHud);
    const unlockedArtilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    assert(unlockedArtilleryResearchButton && !unlockedArtilleryResearchButton.disabled, "Artillery research should enable after Anti-Tank Gun research");

    renderedButtons.length = 0;
    rdHud.state.upgrades = [UPGRADE.TANK_UNLOCK];
    rdHud._cardSig = null;
    renderCommandCard(rdHud);
    const unlockedCommandCarResearchButton = renderedButtons.find((button) => button.innerHTML.includes("CC+"));
    assert(unlockedCommandCarResearchButton && !unlockedCommandCarResearchButton.disabled, "Command Car research should enable after Tank Production");

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

    shortResourceHud.clientIntent = new ClientIntent();
    shortResourceHud.clientIntent.openWorkerBuildMenu();
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

    assert(
      shortResourceHud._missingResourceSoundId(
        { steel: 50, oil: 0 },
        { steel: 50, oil: 0, supplyUsed: 10, supplyCap: 10 },
        1,
      ) === "notice_supply",
      "train unavailable feedback should play the supply voice line when resources are available",
    );

    renderedButtons.length = 0;
    sent.length = 0;
    const selectedAntiTankGun = { id: 88, owner: playerId, kind: KIND.ANTI_TANK_GUN, setupState: SETUP.DEPLOYED };
    const selectedArtillery = { id: 89, owner: playerId, kind: KIND.ARTILLERY, setupState: SETUP.PACKED };
    const antiTankGunHud = Object.create(HUD.prototype);
    antiTankGunHud.state = {
      playerId,
      resources: { steel: 0, oil: 0 },
      commandTarget: null,
      selectedEntities: () => [selectedAntiTankGun, selectedArtillery],
      entitiesInterpolated: () => [selectedAntiTankGun, selectedArtillery],
      beginCommandTarget(kind) {
        this.commandTarget = kind;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    antiTankGunHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    antiTankGunHud._cardSig = null;

    renderCommandCard(antiTankGunHud);
    const setupButton = renderedButtons.find((button) => button.innerHTML.includes("Set Up"));
    const tearDownButton = renderedButtons.find((button) => button.innerHTML.includes("Tear Down"));
    assert(setupButton?.dataset.hotkey, "anti-tank gun Set Up button should keep its command-card hotkey");
    assert(!tearDownButton, "anti-tank gun Tear Down should not occupy a command-card slot");

    const setupCommands = [];
    const setupInput = Object.create(Input.prototype);
    setupInput.state = {
      playerId,
      selectedEntities: () => [selectedAntiTankGun, selectedArtillery],
    };
    setupInput.clientIntent = new ClientIntent();
    setupInput.clientIntent.beginCommandTarget("setupAntiTankGuns");
    setupInput.clientIntent.addCommandFeedback = () => {};
    setupInput.commandIssuer = { issueCommand: (command) => setupCommands.push(command) };
    setupInput._worldAt = (x, y) => ({ x, y });
    setupInput._entityAtWorld = () => null;
    setupInput._selectedOwnUnitIds = () => [selectedAntiTankGun.id, selectedArtillery.id];
    setupInput._issueTargetedCommand({ x: 160, y: 192 }, { shiftKey: true });
    assert(
      setupCommands[0]?.c === "setupAntiTankGuns" &&
        setupCommands[0].units.includes(selectedAntiTankGun.id) &&
        setupCommands[0].units.includes(selectedArtillery.id) &&
        setupCommands[0].queued === true,
      "setupAntiTankGuns targeting includes selected artillery as setup-capable support weapons",
    );

    const movingAntiTankGun = {
      ...selectedAntiTankGun,
      x: 100,
      y: 120,
      orderPlan: [
        { kind: ORDER_STAGE.MOVE, x: 320, y: 192 },
        { kind: ORDER_STAGE.SETUP_ANTI_TANK_GUNS, x: 640, y: 192 },
      ],
    };
    const movingArtillery = {
      ...selectedArtillery,
      x: 140,
      y: 120,
      orderPlan: [
        { kind: ORDER_STAGE.ATTACK_MOVE, x: 352, y: 224 },
      ],
    };
    const stationaryAntiTankGun = {
      id: 90,
      owner: playerId,
      kind: KIND.ANTI_TANK_GUN,
      x: 180,
      y: 120,
    };
    const previewInput = Object.create(Input.prototype);
    previewInput.mouse = { x: 500, y: 300 };
    previewInput.state = {
      playerId,
      selectedEntities: () => [movingAntiTankGun, movingArtillery, stationaryAntiTankGun],
    };
    previewInput.clientIntent = new ClientIntent();
    previewInput.clientIntent.beginCommandTarget("setupAntiTankGuns");
    previewInput._worldAt = (x, y) => ({ x, y });
    previewInput._refreshAntiTankGunSetupPreview();
    const unqueuedPreviewGuns = previewInput.clientIntent.antiTankGunSetupPreview?.guns || [];
    assert(
      unqueuedPreviewGuns[0]?.x === 100 && unqueuedPreviewGuns[0]?.y === 120,
      "unqueued support setup preview keeps the current gun position",
    );

    previewInput._shiftKeyDown = true;
    previewInput._refreshAntiTankGunSetupPreview();
    const previewGuns = previewInput.clientIntent.antiTankGunSetupPreview?.guns || [];
    assert(
      previewGuns[0]?.x === 320 &&
        previewGuns[0]?.y === 192 &&
        movingAntiTankGun.x === 100 &&
        movingAntiTankGun.y === 120,
      "queued anti-tank gun setup preview uses the accepted movement endpoint without mutating the selected entity",
    );
    assert(
      previewGuns[1]?.x === 352 && previewGuns[1]?.y === 224,
      "artillery setup preview uses attack-move formation endpoints as projected origins",
    );
    assert(
      previewGuns[2]?.x === 180 && previewGuns[2]?.y === 120,
      "support setup preview falls back to current position when no movement plan is accepted",
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
      { id: 2, teamId: 7, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
      { id: 3, teamId: 7, name: "C", color: "#0000ff", startTileX: 3, startTileY: 3 },
    ],
  };
  const state = new GameState(start);
  assert(state instanceof GameState, "GameState constructor should return an instance");
  assert(state.playerId === 1, "GameState.playerId");
  assert(state.startInfo === start, "GameState.startInfo");
  assert(state.map.width === 4, "GameState.map");
  assert(state.map.resources.length === 2, "GameState keeps start payload resources");
  assert(state.resourceById.get(200).kind === KIND.STEEL, "GameState indexes resources by id");
  assert(state.resourceById.get(200).remaining === 1000, "steel defaults to full known amount");
  assert(state.resourceById.get(201).remaining === 3333, "oil defaults to full known amount");
  assert(Array.isArray(state.players), "GameState.players");
  assert(state.playerById(1)?.teamId === 1, "GameState defaults missing teamId to singleton FFA");
  assert(state.teamIdForPlayer(2) === 7, "GameState.teamIdForPlayer returns explicit team");
  assert(state.isOwnOwner(1), "GameState.isOwnOwner detects local owner");
  assert(state.isAllyOwner(2) === false, "GameState.isAllyOwner treats singleton FFA as non-allied");
  assert(state.isEnemyOwner(2), "GameState.isEnemyOwner detects another team");
  assert(state.isNeutralOwner(0), "GameState.isNeutralOwner detects neutral owner");
  const allyState = new GameState({ ...start, playerId: 2 });
  assert(allyState.isAllyOwner(3), "GameState.isAllyOwner detects shared team");
  assert(!allyState.isEnemyOwner(3), "GameState.isEnemyOwner excludes shared team");
  const localTeamState = new GameState({
    ...start,
    players: [
      { id: 1, teamId: 7, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 7, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
      { id: 3, teamId: 8, name: "C", color: "#0000ff", startTileX: 3, startTileY: 3 },
    ],
  });
  assert(localTeamState.isAllyOwner(2), "GameState.isAllyOwner classifies local-player allies from start.players");
  assert(localTeamState.isEnemyOwner(3), "GameState.isEnemyOwner classifies hostile teams from start.players");
  assert(!localTeamState.isEnemyOwner(2), "GameState.isEnemyOwner excludes local-player allies");
  assertHasMethod(state, "applySnapshot", "GameState");
  assertHasMethod(state, "entitiesInterpolated", "GameState");
  assertHasGetter(state, "prevRecvTime", "GameState");
  assertHasGetter(state, "currRecvTime", "GameState");
  assert(state.prevRecvTime === null, "prevRecvTime null before snapshots");
  assert(state.currRecvTime === null, "currRecvTime null before snapshots");
  assert(state.resources !== undefined, "GameState.resources");
  assert(Array.isArray(state.events), "GameState.events");
  assert(!("resourceMiningPreview" in state), "GameState no longer exposes resource preview shims");
  assert(!("antiTankGunSetupPreview" in state), "GameState no longer exposes support preview shims");
  assert(!("updateResourceMiningPreview" in state), "GameState no longer exposes preview update shims");
  assert(state.selection instanceof Set, "GameState.selection");
  assert(
    state.diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.NONE,
    "GameState defaults movement path diagnostics to none",
  );
  assert(state.debugPathOverlaysAvailable === false, "GameState hides waypoint diagnostics by default");
  assert(state.debugPathOverlaysEnabled === false, "GameState leaves waypoint diagnostics off by default");
  assertHasMethod(state, "setSelection", "GameState");
  assertHasMethod(state, "addToSelection", "GameState");
  assertHasMethod(state, "clearSelection", "GameState");
  assertHasMethod(state, "selectedEntities", "GameState");
  assertHasMethod(state, "entityById", "GameState");
  assert(!("commandCardMode" in state), "GameState no longer exposes command-card intent shims");
  assert(!("openWorkerBuildMenu" in state), "GameState no longer exposes command-card methods");
  assert(!("placement" in state), "GameState no longer exposes placement intent shims");
  assert(!("beginPlacement" in state), "GameState no longer exposes placement methods");

  const debugState = new GameState({
    ...start,
    diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.OWNER_ONLY },
    map: {
      ...start.map,
      resources: start.map.resources.map((resource) => ({ ...resource })),
    },
  });
  assert(debugState.debugPathOverlaysAvailable === true, "GameState exposes advertised waypoint diagnostics");
  assert(debugState.debugPathOverlaysEnabled === true, "GameState enables advertised waypoint diagnostics");
  assert(debugState.showAllDebugPathOverlays === false, "owner-only waypoint diagnostics stay selection scoped");

  const fullDiagnosticState = new GameState({
    ...start,
    diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.ALL },
    map: {
      ...start.map,
      resources: start.map.resources.map((resource) => ({ ...resource })),
    },
  });
  assert(fullDiagnosticState.showAllDebugPathOverlays === true, "full waypoint diagnostics may draw every projected path");

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
  assert(state.resourceById.get(200).remaining === 0, "visible resource death tombstones known resource");
  assert(state.entityById(200) === undefined, "depleted resources are not exposed as local entities");
  assert(state.entityById(201).remaining === 3333, "untouched resources keep their last-known amount");
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

  const overpenEventState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  overpenEventState.applySnapshot({
    tick: 12,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{ id: 22, owner: 2, kind: KIND.WORKER, x: 166, y: 108, hp: 30, maxHp: 40, state: STATE.IDLE }],
    events: [{ e: EVENT.OVERPENETRATION, to: 22 }],
  });
  assert(overpenEventState.liveMuzzleFlashes(performance.now()).length === 0, "overpenetration event does not draw a tracer");
  assert(overpenEventState.weaponRecoil(22, KIND.WORKER, performance.now()) === 0, "overpenetration event does not trigger weapon recoil");

  // Interpolation clamps alpha to [0,1]
  const entsNeg = state.entitiesInterpolated(-0.5);
  const entsOver = state.entitiesInterpolated(1.5);
  const entsMid = state.entitiesInterpolated(0.5);
  const midWorker = entsMid.find((e) => e.id === 1);
  assert(entsMid.length === 2 && midWorker, "entitiesInterpolated returns units and live known resources");
  assert(!entsMid.some((e) => e.id === 200), "entitiesInterpolated omits depleted resources");
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

  const budgetSelectionState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  const budgetRiflemen = Array.from({ length: 30 }, (_, index) => ({
    id: 300 + index,
    owner: 1,
    kind: KIND.RIFLEMAN,
    x: index * 4,
    y: 0,
    hp: 40,
    maxHp: 40,
    state: STATE.IDLE,
  }));
  const budgetTanks = Array.from({ length: 5 }, (_, index) => ({
    id: 400 + index,
    owner: 1,
    kind: KIND.TANK,
    x: index * 12,
    y: 20,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
  }));
  const budgetExtraTank = {
    id: 455,
    owner: 1,
    kind: KIND.TANK,
    x: 100,
    y: 20,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
  };
  const budgetCommandCar = {
    id: 450,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    x: 80,
    y: 20,
    hp: 80,
    maxHp: 80,
    state: STATE.IDLE,
  };
  budgetSelectionState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 80,
    entities: budgetRiflemen.concat(budgetTanks, budgetExtraTank, budgetCommandCar),
    events: [],
  });
  budgetSelectionState.setSelection(budgetRiflemen.map((entity) => entity.id));
  assert(
    budgetSelectionState.selection.size === BASE_COMMAND_SUPPLY_CAP,
    "selection budget admits 24 one-supply units by command supply",
  );
  assert(
    budgetSelectionState.selectionBudgetOverflow?.cap === BASE_COMMAND_SUPPLY_CAP,
    "selection overflow records budget state for the HUD",
  );
  budgetSelectionState.setSelection(budgetTanks.map((entity) => entity.id));
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "400,401,402",
    "selection budget admits three eight-supply tanks without a Command Car",
  );
  budgetSelectionState.setSelection(budgetTanks.map((entity) => entity.id).concat([budgetExtraTank.id, budgetCommandCar.id]));
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "450,400,401,402,403,404",
    "selection budget offsets Command Car supply before filling normal candidates",
  );
  budgetSelectionState.setSelection(budgetTanks.slice(0, 4).map((entity) => entity.id));
  budgetSelectionState.addToSelection([budgetRiflemen[0].id]);
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "400,401,402",
    "shift-add ignores overflow without replacing the existing selection",
  );
  budgetSelectionState.addToSelection([budgetCommandCar.id, budgetTanks[4].id]);
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "400,401,402,450,404",
    "shift-add can admit a Command Car bonus and then later candidates",
  );
  budgetSelectionState.setControlGroup(0, budgetRiflemen.map((entity) => entity.id));
  assert(
    budgetSelectionState.controlGroups[0].length === BASE_COMMAND_SUPPLY_CAP,
    "control-group save admits 24 one-supply units",
  );
  assert(
    budgetSelectionState.selectionBudgetOverflow?.cap === BASE_COMMAND_SUPPLY_CAP,
    "control-group save records overflow for ignored one-supply units",
  );
  budgetSelectionState.setControlGroup(1, budgetTanks.map((entity) => entity.id));
  assert(
    budgetSelectionState.controlGroups[1].join(",") === "400,401,402",
    "control-group save ignores over-budget Tanks",
  );
  budgetSelectionState.addToControlGroup(1, [budgetRiflemen[0].id]);
  assert(
    budgetSelectionState.controlGroups[1].join(",") === "400,401,402",
    "control-group add ignores overflow without trimming existing legal members",
  );
  budgetSelectionState.addToControlGroup(1, [budgetCommandCar.id, budgetTanks[4].id]);
  assert(
    budgetSelectionState.controlGroups[1].join(",") === "400,401,402,450,404",
    "control-group add can admit one Command Car bonus and then later candidates",
  );
  const secondBudgetCommandCar = {
    id: 451,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    x: 96,
    y: 20,
    hp: 80,
    maxHp: 80,
    state: STATE.IDLE,
  };
  budgetSelectionState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 80,
    entities: budgetRiflemen.concat(budgetTanks, budgetExtraTank, budgetCommandCar, secondBudgetCommandCar),
    events: [],
  });
  budgetSelectionState.setControlGroup(2, budgetTanks.map((entity) => entity.id).concat([budgetCommandCar.id, secondBudgetCommandCar.id]));
  assert(
    budgetSelectionState.controlGroups[2].join(",") === "450,451,400,401,402,403,404",
    "control-group save stacks multiple Command Car bonuses",
  );
  budgetSelectionState.controlGroups[3] = budgetTanks.map((entity) => entity.id).concat(budgetCommandCar.id);
  const recalledLateCar = budgetSelectionState.selectControlGroup(3);
  assert(
    recalledLateCar.join(",") === "450,400,401,402,403,404",
    "control-group recall pre-admits a Command Car stored late in old runtime order",
  );
  assert(
    budgetSelectionState.controlGroups[3].join(",") === "450,400,401,402,403,404",
    "control-group recall rewrites old over-budget runtime groups to legal admitted order",
  );
  budgetSelectionState.controlGroups[4] = budgetTanks.map((entity) => entity.id);
  const recalledOverBudgetTanks = budgetSelectionState.selectControlGroup(4);
  assert(
    recalledOverBudgetTanks.join(",") === "400,401,402",
    "control-group recall filters old over-budget Tank groups before selection",
  );
  assert(
    budgetSelectionState.selectionBudgetOverflow?.cap === BASE_COMMAND_SUPPLY_CAP,
    "control-group recall records overflow feedback for old over-budget groups",
  );
  const controlGroupCommands = [];
  const controlGroupToasts = [];
  const guardedControlGroupIssuer = {
    issueCommand(command) {
      const budget = commandWithinBudget(budgetSelectionState, command);
      if (!budget.ok) {
        controlGroupToasts.push(budget);
        return { sent: false, blocked: "commandBudget", budget };
      }
      controlGroupCommands.push(command);
      return { sent: true };
    },
  };
  guardedControlGroupIssuer.issueCommand(cmd.move(Array.from(budgetSelectionState.selection), 10, 20));
  guardedControlGroupIssuer.issueCommand(cmd.move(budgetTanks.map((entity) => entity.id), 10, 20));
  assert(controlGroupCommands.length === 1, "legal recalled control-group command is sent through the budget guard");
  assert(controlGroupToasts.length === 1, "over-budget command restored from stale group data is blocked before send");

  // Command-card submenu is local-only and is closed by mode-changing intent actions.
  const commandCardIntent = new ClientIntent();
  commandCardIntent.openWorkerBuildMenu();
  assert(commandCardIntent.commandCardMode === "workerBuild", "worker build submenu opens");
  assert(commandCardIntent.closeCommandCardMenu() === true, "closeCommandCardMenu reports an open submenu");
  assert(commandCardIntent.closeCommandCardMenu() === false, "closeCommandCardMenu reports when no submenu was open");
  commandCardIntent.openWorkerBuildMenu();
  commandCardIntent.beginCommandTarget("attack");
  assert(commandCardIntent.commandCardMode === null, "command targeting closes the worker build submenu");
  assert(commandCardIntent.commandTarget === "attack", "command targeting mirrors the composer target");
  const queuedIssue = commandCardIntent.issueCommandTarget({ shiftKey: true });
  assert(queuedIssue.keepArmed && commandCardIntent.commandTarget === "attack", "Shift-issued command remains armed");
  commandCardIntent.releaseCommandTargetShift();
  assert(commandCardIntent.commandTarget === null, "Shift release clears a Shift-preserved command target");
  commandCardIntent.openWorkerBuildMenu();
  commandCardIntent.beginPlacement(KIND.DEPOT);
  assert(commandCardIntent.commandCardMode === null, "build placement closes the worker build submenu");

  // Control groups are local-only, own controllable entities only, and budgeted like selection.
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
    cgState.controlGroups[0].join(",") === "100,101,112,113,102,103,104,105,106,107,108,109,110,111",
    "control groups store own units/buildings only in selection order within budget",
  );
  cgState.addToControlGroup(0, [110, 111, 112, 113]);
  assert(cgState.controlGroups[0].length === 14, "adding duplicates to a budgeted control group is stable");
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

  const teamSelectionState = new GameState({
    ...start,
    map: { ...start.map, width: 12, height: 12, resources: [] },
    players: [
      { id: 1, teamId: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 1, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
      { id: 3, teamId: 2, name: "C", color: "#0000ff", startTileX: 3, startTileY: 3 },
    ],
  });
  const ownWorker = { id: 201, owner: 1, kind: KIND.WORKER, x: 32, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
  const allyWorker = { id: 202, owner: 2, kind: KIND.WORKER, x: 64, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
  const enemyWorker = { id: 203, owner: 3, kind: KIND.WORKER, x: 96, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
  teamSelectionState.applySnapshot({
    tick: 0,
    steel: 100,
    oil: 100,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [ownWorker, allyWorker, enemyWorker],
    events: [],
  });
  const selectionInput = Object.create(Input.prototype);
  selectionInput.state = teamSelectionState;
  selectionInput.camera = { screenToWorld: (x, y) => ({ x, y }) };
  selectionInput.dom = { clientWidth: 400, clientHeight: 300 };
  selectionInput._worldAt = Input.prototype._worldAt;
  selectionInput._entityAtWorld = Input.prototype._entityAtWorld;
  selectionInput._worldPointHitsEntity = Input.prototype._worldPointHitsEntity;
  selectionInput._entityIntersectsRect = Input.prototype._entityIntersectsRect;
  selectionInput._closestIdsToPoint = Input.prototype._closestIdsToPoint;
  selectionInput._commitClickSelection = Input.prototype._commitClickSelection;
  selectionInput._commitBoxSelection = Input.prototype._commitBoxSelection;
  selectionInput._ownBuildingsOfKindInViewport = Input.prototype._ownBuildingsOfKindInViewport;
  selectionInput._closestOwnUnitKindInViewport = Input.prototype._closestOwnUnitKindInViewport;
  selectionInput._commitClickSelection({ x: allyWorker.x, y: allyWorker.y }, false, false);
  assert(
    Array.from(teamSelectionState.selection).join(",") === String(allyWorker.id),
    "single-click can select an allied entity for inspection",
  );
  selectionInput._commitBoxSelection({ x0: 0, y0: 0, x1: 120, y1: 64 }, false);
  assert(
    Array.from(teamSelectionState.selection).join(",") === String(ownWorker.id),
    "box selection skips allied and enemy units",
  );
  selectionInput._commitClickSelection({ x: allyWorker.x, y: allyWorker.y }, true, false);
  assert(
    Array.from(teamSelectionState.selection).join(",") === `${ownWorker.id},${allyWorker.id}`,
    "shift-click can add an allied inspection target to the current selection",
  );
  teamSelectionState.setControlGroup(2, teamSelectionState.selection);
  assert(
    teamSelectionState.controlGroups[2].join(",") === String(ownWorker.id),
    "mixed own/allied selections save only own entities into control groups",
  );
  const allyOnlyCard = buildCommandCardDescriptors(commandCardCtx({
    playerId: 1,
    selection: [allyWorker],
    entities: [ownWorker, allyWorker],
  }));
  assert(commandButtons(allyOnlyCard).length === 0, "allied-only inspection selection exposes no command buttons");
  const mixedCard = buildCommandCardDescriptors(commandCardCtx({
    playerId: 1,
    selection: [ownWorker, allyWorker],
    entities: [ownWorker, allyWorker],
  }));
  const holdIntent = buttonByAction(mixedCard, "holdPosition")?.intent;
  assert(
    holdIntent?.unitIds?.join(",") === String(ownWorker.id),
    "mixed own/allied command card emits commands only for own entity ids",
  );

  const budgetInputState = new GameState({
    ...start,
    map: { ...start.map, width: 12, height: 12, resources: [] },
  });
  const boxRiflemen = Array.from({ length: 30 }, (_, index) => ({
    id: 5100 + index,
    owner: 1,
    kind: KIND.RIFLEMAN,
    x: 8 + index * 4,
    y: 140,
    hp: 45,
    maxHp: 45,
    state: STATE.IDLE,
  }));
  const doubleClickRiflemen = Array.from({ length: 30 }, (_, index) => ({
    id: 5200 + index,
    owner: 1,
    kind: KIND.MACHINE_GUNNER,
    x: 8 + index * 4,
    y: 180,
    hp: 55,
    maxHp: 55,
    state: STATE.IDLE,
  }));
  const budgetInputTanks = Array.from({ length: 5 }, (_, index) => ({
    id: 5300 + index,
    owner: 1,
    kind: KIND.TANK,
    x: 8 + index * 16,
    y: 220,
    hp: 292,
    maxHp: 292,
    state: STATE.IDLE,
  }));
  const lateBoxCommandCar = {
    id: 5400,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    x: 96,
    y: 220,
    hp: 225,
    maxHp: 225,
    state: STATE.IDLE,
  };
  budgetInputState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 80,
    entities: boxRiflemen.concat(doubleClickRiflemen, budgetInputTanks, lateBoxCommandCar),
    events: [],
  });
  const budgetSelectionInput = Object.create(Input.prototype);
  budgetSelectionInput.state = budgetInputState;
  budgetSelectionInput.camera = selectionInput.camera;
  budgetSelectionInput.dom = selectionInput.dom;
  budgetSelectionInput._worldAt = Input.prototype._worldAt;
  budgetSelectionInput._entityAtWorld = Input.prototype._entityAtWorld;
  budgetSelectionInput._worldPointHitsEntity = Input.prototype._worldPointHitsEntity;
  budgetSelectionInput._entityIntersectsRect = Input.prototype._entityIntersectsRect;
  budgetSelectionInput._closestIdsToPoint = Input.prototype._closestIdsToPoint;
  budgetSelectionInput._commitClickSelection = Input.prototype._commitClickSelection;
  budgetSelectionInput._commitBoxSelection = Input.prototype._commitBoxSelection;
  budgetSelectionInput._ownBuildingsOfKindInViewport = Input.prototype._ownBuildingsOfKindInViewport;
  budgetSelectionInput._closestOwnUnitKindInViewport = Input.prototype._closestOwnUnitKindInViewport;
  budgetSelectionInput._commitBoxSelection({ x0: 0, y0: 124, x1: 140, y1: 156 }, false);
  assert(
    Array.from(budgetInputState.selection).length === BASE_COMMAND_SUPPLY_CAP,
    "drag selection admits the base budget of one-supply units",
  );
  budgetSelectionInput._commitClickSelection({ x: doubleClickRiflemen[0].x, y: doubleClickRiflemen[0].y }, false, true);
  assert(
    Array.from(budgetInputState.selection).length === BASE_COMMAND_SUPPLY_CAP / STATS[KIND.MACHINE_GUNNER].supply &&
      Array.from(budgetInputState.selection).every((id) => id >= 5200 && id < 5300),
    "double-click same-kind selection is filtered by command supply",
  );
  budgetSelectionInput._commitBoxSelection({ x0: 0, y0: 204, x1: 120, y1: 236 }, false);
  assert(
    Array.from(budgetInputState.selection).join(",") === "5400,5300,5301,5302,5303,5304",
    "drag selection pre-admits a late Command Car before budget-filling Tanks",
  );
  const alliedRightClickCommands = [];
  const rightClickInput = Object.create(Input.prototype);
  rightClickInput.state = teamSelectionState;
  rightClickInput.camera = selectionInput.camera;
  rightClickInput._worldAt = Input.prototype._worldAt;
  rightClickInput._entityAtWorld = Input.prototype._entityAtWorld;
  rightClickInput._worldPointHitsEntity = Input.prototype._worldPointHitsEntity;
  rightClickInput._resourceAtWorld = Input.prototype._resourceAtWorld;
  rightClickInput._selectedOwnUnitIds = Input.prototype._selectedOwnUnitIds;
  rightClickInput._selectedWorkerIds = Input.prototype._selectedWorkerIds;
  rightClickInput._selectedProducerBuildingIds = Input.prototype._selectedProducerBuildingIds;
  rightClickInput._issueCommand = (command) => alliedRightClickCommands.push(command);
  teamSelectionState.setSelection([ownWorker.id]);
  rightClickInput._onRightClick({ x: allyWorker.x, y: allyWorker.y });
  assert(
    alliedRightClickCommands.length === 1 &&
      alliedRightClickCommands[0].c === "move" &&
      alliedRightClickCommands[0].units.join(",") === String(ownWorker.id),
    "right-clicking an allied entity with own units selected sends move, not attack",
  );
  const minimapLike = Object.create(Minimap.prototype);
  minimapLike.state = teamSelectionState;
  assert(
    minimapLike._blipColor(allyWorker) === `#${COLORS.selectAlly.toString(16).padStart(6, "0")}`,
    "minimap blip color distinguishes allies from enemies",
  );

  // Placement is local-only
  const placementIntent = new ClientIntent();
  placementIntent.beginPlacement("barracks");
  assert(placementIntent.placement !== null, "placement started");
  placementIntent.updatePlacement(2, 3, true);
  assert(placementIntent.placement.tileX === 2, "updatePlacement sets tileX");
  assert(placementIntent.placement.tileY === 3, "updatePlacement sets tileY");
  assert(placementIntent.placement.valid === true, "updatePlacement sets valid");
  placementIntent.endPlacement();
  assert(placementIntent.placement === null, "endPlacement clears placement");

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
  assert(
    movementBodyClass(KIND.WORKER) === "infantryLike" &&
      movementBodyClass(KIND.RIFLEMAN) === "infantryLike" &&
      movementBodyClass(KIND.MORTAR_TEAM) === "vehicleBody" &&
      movementBodyClass(KIND.ANTI_TANK_GUN) === "vehicleBody" &&
      movementBodyClass(KIND.ARTILLERY) === "vehicleBody" &&
      movementBodyClass(KIND.SCOUT_CAR) === "vehicleBody" &&
      movementBodyClass(KIND.TANK) === "vehicleBody" &&
      movementBodyClass(KIND.COMMAND_CAR) === "vehicleBody",
    "client placement movement-body classes mirror server vehicle-body blockers",
  );
  assert(
    placementPolicyForBuilding(KIND.TANK_TRAP).unitOverlap === "infantryAllowed" &&
      placementPolicyForBuilding(KIND.DEPOT).unitOverlap === "none",
    "Tank Trap placement policy allows infantry overlap without changing ordinary buildings",
  );
  assert(
    footprintValidAgainstEntities(
      [other],
      new Set(),
      1,
      1,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === true,
    "Tank Trap advisory preview allows infantry bodies inside the footprint",
  );
  const tank = { id: 9, owner: 1, kind: KIND.TANK, x: 116, y: 64 };
  assert(
    footprintValidAgainstEntities([tank], new Set(), 1, 1, 2, 2, map) === false,
    "client preview should reject a tank body touching a footprint edge",
  );
  const trapTank = { id: 91, owner: 1, kind: KIND.TANK, x: 58, y: 48 };
  assert(
    footprintValidAgainstEntities(
      [trapTank],
      new Set(),
      1,
      1,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === false,
    "Tank Trap advisory preview still rejects vehicle-body overlap",
  );
  assert(STATS[KIND.TANK].body.length === 50.4, "tank client body length mirrors server");
  assert(STATS[KIND.TANK].body.width === 28.8, "tank client body width mirrors server");
  assert(STATS[KIND.ANTI_TANK_GUN].body.length === 42.0, "anti-tank gun client body length mirrors server");
  assert(STATS[KIND.ANTI_TANK_GUN].body.width === 24.0, "anti-tank gun client body width mirrors server");
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
  input.state.entitiesInterpolated = () => [other];
  assert(
    input._footprintValid(1, 1, 1, 1, map, KIND.TANK_TRAP) === true,
    "Tank Trap input preview allows non-builder infantry overlap",
  );
  input.state.entitiesInterpolated = () => [trapTank];
  assert(
    input._footprintValid(1, 1, 1, 1, map, KIND.TANK_TRAP) === false,
    "Tank Trap input preview rejects vehicle-body units",
  );
  const rockMap = { ...map, terrain: [...map.terrain] };
  rockMap.terrain[2 * rockMap.width + 2] = TERRAIN.ROCK;
  assert(
    footprintPlacementBlocker([], new Set(), 2, 2, 1, 1, rockMap, placementPolicyForBuilding(KIND.TANK_TRAP)) === "terrain",
    "Tank Trap placement blocker classifies impassable terrain",
  );
  assert(
    footprintPlacementBlocker(
      [{ id: 92, owner: 1, kind: KIND.BARRACKS, x: 80, y: 80 }],
      new Set(),
      2,
      2,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === "structure",
    "Tank Trap placement blocker classifies buildings separately from units",
  );
  assert(
    footprintPlacementBlocker(
      [trapTank],
      new Set(),
      1,
      1,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === "unit",
    "Tank Trap placement blocker classifies vehicle-body unit blockers",
  );
  delete input._selectedWorkerIds;

  const pairs = (tiles) => tiles.map((site) => [site.tileX, site.tileY]);
  const exactTankTrapSpacing = (tiles) => tiles.every((site, index) => {
    if (index === 0) return true;
    const prev = tiles[index - 1];
    const dx = Math.abs(site.tileX - prev.tileX);
    const dy = Math.abs(site.tileY - prev.tileY);
    return (dx === 1 && dy === 1) || (dx === 2 && dy === 0) || (dx === 0 && dy === 2);
  });
  assertDeepEqual(
    pairs(tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 5, tileY: 0 })),
    [[0, 0], [2, 0], [4, 0]],
    "Tank Trap horizontal line uses one-tile orthogonal gaps and omits off-cadence end",
  );
  assertDeepEqual(
    pairs(tankTrapLineTiles({ tileX: 1, tileY: 1 }, { tileX: 1, tileY: 5 })),
    [[1, 1], [1, 3], [1, 5]],
    "Tank Trap vertical line includes the end when it lands on cadence",
  );
  assertDeepEqual(
    pairs(tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 3, tileY: 3 })),
    [[0, 0], [1, 1], [2, 2], [3, 3]],
    "Tank Trap diagonal line keeps corner-touching sites",
  );
  const shallowLine = tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 5, tileY: 2 });
  const steepLine = tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 2, tileY: 5 });
  assert(exactTankTrapSpacing(shallowLine), "Tank Trap shallow line uses only allowed vehicle-blocking spacing");
  assert(exactTankTrapSpacing(steepLine), "Tank Trap steep line uses only allowed vehicle-blocking spacing");
  assertDeepEqual(
    pairs(shallowLine),
    [[0, 0], [1, 1], [3, 1], [4, 2]],
    "Tank Trap shallow bridge sites avoid knight and two-by-two gaps",
  );
  const lineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 4, tileY: 0 },
    isValid: (tileX) => tileX !== 2,
  });
  assertDeepEqual(
    lineSites.map((site) => [site.tileX, site.tileY, site.valid]),
    [[0, 0, true], [2, 0, false], [4, 0, false]],
    "Tank Trap line preview marks sites invalid when skipping would create an oversized gap",
  );
  assertDeepEqual(
    pairs(validTankTrapLineSites(lineSites)),
    [[0, 0]],
    "Tank Trap dispatch preserves required spacing after invalid preview sites",
  );
  const terrainSkippedLineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 4, tileY: 0 },
    isValid: (tileX) => ({ valid: tileX !== 2, blockedBy: tileX === 2 ? "terrain" : null }),
  });
  assertDeepEqual(
    terrainSkippedLineSites.map((site) => [site.tileX, site.tileY, site.valid, site.skipped]),
    [[0, 0, true, false], [2, 0, false, true], [4, 0, true, false]],
    "Tank Trap line preview skips impassable terrain and resumes on the other side",
  );
  assertDeepEqual(
    pairs(validTankTrapLineSites(terrainSkippedLineSites)),
    [[0, 0], [4, 0]],
    "Tank Trap dispatch omits skipped terrain sites without stopping the line",
  );
  const structureSkippedLineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 6, tileY: 0 },
    isValid: (tileX) => ({ valid: tileX !== 2, blockedBy: tileX === 2 ? "structure" : null }),
  });
  assertDeepEqual(
    pairs(validTankTrapLineSites(structureSkippedLineSites)),
    [[0, 0], [4, 0], [6, 0]],
    "Tank Trap dispatch resumes normal spacing after skipping a building",
  );
  const unitBlockedLineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 4, tileY: 0 },
    isValid: (tileX) => ({ valid: tileX !== 2, blockedBy: tileX === 2 ? "unit" : null }),
  });
  assertDeepEqual(
    unitBlockedLineSites.map((site) => [site.tileX, site.tileY, site.valid, site.skipped]),
    [[0, 0, true, false], [2, 0, false, false], [4, 0, false, false]],
    "Tank Trap line preview keeps unit-blocked gaps as line-stopping invalid sites",
  );
  const diagonalGapSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 2, tileY: 2 },
    isValid: (tileX, tileY) => !(tileX === 1 && tileY === 1),
  });
  assertDeepEqual(
    diagonalGapSites.map((site) => [site.tileX, site.tileY, site.valid]),
    [[0, 0, true], [1, 1, false], [2, 2, false]],
    "Tank Trap line preview forbids dispatching across a two-by-two diagonal gap",
  );
  const lineCommands = tankTrapBuildCommands([77, 88], [
    { tileX: 0, tileY: 0, valid: true },
    { tileX: 2, tileY: 0, valid: true },
    { tileX: 4, tileY: 0, valid: true },
    { tileX: 6, tileY: 0, valid: true },
  ]);
  assertDeepEqual(
    lineCommands.map((command) => [command.units, command.tileX, command.tileY, command.queued === true]),
    [
      [[77], 0, 0, false],
      [[88], 2, 0, false],
      [[77, 88], 4, 0, true],
      [[77, 88], 6, 0, true],
    ],
    "Tank Trap line dispatch assigns immediate single-worker builds then queued overflow builds",
  );
  const gapCommands = tankTrapBuildCommands([77, 88], [
    { tileX: 0, tileY: 0, valid: true },
    { tileX: 2, tileY: 0, valid: false },
    { tileX: 4, tileY: 0, valid: true },
  ]);
  assertDeepEqual(
    gapCommands.map((command) => [command.units, command.tileX, command.tileY, command.queued === true]),
    [[[77], 0, 0, false]],
    "Tank Trap line dispatch stops before valid-looking sites that would skip over a gap",
  );
  const lineDragInput = Object.create(Input.prototype);
  let dragRefreshes = 0;
  let dragConfirms = 0;
  lineDragInput.clientIntent = new ClientIntent();
  lineDragInput.clientIntent.beginPlacement(KIND.TANK_TRAP);
  lineDragInput.clientIntent.updatePlacement(3, 4, true, {
    lineSites: [{ tileX: 3, tileY: 4, valid: true }],
  });
  lineDragInput._refreshPlacement = () => {
    dragRefreshes += 1;
  };
  lineDragInput._confirmPlacement = () => {
    dragConfirms += 1;
  };
  lineDragInput._onLeftDown({ x: 10, y: 10 }, {});
  assert(
    lineDragInput._placementDrag?.tileX === 3 &&
      lineDragInput._placementDrag?.tileY === 4 &&
      lineDragInput._drag === undefined &&
      dragRefreshes === 1 &&
      dragConfirms === 0,
    "Tank Trap left-down starts placement drag instead of selection drag or immediate confirm",
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
  const clickableAntiTankGun = { id: 11, owner: 1, kind: KIND.ANTI_TANK_GUN, x: 0, y: 0, facing: 0 };
  assert(
    input._worldPointHitsEntity(clickableAntiTankGun, 22, 0, 32) === true,
    "anti-tank gun hit testing should reach the wheeled body axis",
  );
  assert(
    input._worldPointHitsEntity(clickableAntiTankGun, 0, 18, 32) === false,
    "anti-tank gun hit testing should not use the old circular radius",
  );

  const overlappingWorker = { id: 30, owner: 1, kind: KIND.WORKER, x: 100, y: 100 };
  const overlappingSteel = { id: 31, owner: 0, kind: KIND.STEEL, x: 104, y: 100, remaining: 1000 };
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

  const deconstructWorker = { id: 42, owner: 1, kind: KIND.WORKER, x: 150, y: 150 };
  const enemyTankTrap = { id: 43, owner: 2, kind: KIND.TANK_TRAP, x: 180, y: 180 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [deconstructWorker, enemyTankTrap],
    selectedEntities: () => [deconstructWorker],
    addCommandFeedback() {},
  };
  rightClickCommands.length = 0;
  input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "deconstruct" &&
      rightClickCommands[0].units.join(",") === "42" &&
      rightClickCommands[0].target === enemyTankTrap.id &&
      rightClickCommands[0].queued === true,
    "Shift right-click on a Tank Trap with workers selected should send queued deconstruct",
  );

  input.dom = { clientWidth: 800, clientHeight: 600 };
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  const deployedAntiTankGun = {
    id: 21,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
  };
  const otherDeployedAntiTankGun = {
    id: 22,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 120,
    y: 100,
    setupState: SETUP.DEPLOYED,
  };
  const packedAntiTankGun = {
    id: 23,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 110,
    y: 100,
    setupState: SETUP.PACKED,
  };
  input.state = {
    playerId: 1,
    entitiesInterpolated: () => [deployedAntiTankGun, otherDeployedAntiTankGun, packedAntiTankGun],
  };
  assert(
    input
      ._closestOwnUnitKindInViewport(
        KIND.ANTI_TANK_GUN,
        deployedAntiTankGun.x,
        deployedAntiTankGun.y,
        deployedAntiTankGun,
      )
      .join(",") === "21,22",
    "selecting set-up anti-tank guns should not include packed anti-tank guns",
  );
  assert(
    input
      ._closestOwnUnitKindInViewport(KIND.ANTI_TANK_GUN, packedAntiTankGun.x, packedAntiTankGun.y, packedAntiTankGun)
      .join(",") === "23",
    "selecting packed anti-tank guns should not include set-up anti-tank guns",
  );
  assert(
    input._closestOwnUnitKindInViewport(KIND.ANTI_TANK_GUN, deployedAntiTankGun.x, deployedAntiTankGun.y).join(",") ===
      "21,23,22",
    "kind-only Anti-Tank Gun selection helper calls should keep legacy all Anti-Tank Gun behavior",
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
    clearSelection() {
      selectionCleared += 1;
    },
  };
  menuCancelInput.clientIntent = new ClientIntent();
  menuCancelInput.clientIntent.closeCommandCardMenu = () => {
    menuClosed += 1;
    return true;
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
  const targetedIntent = new ClientIntent();
  targetedIntent.addCommandFeedback = (kind, x, y) => {
    feedback.push({ kind, x, y });
  };
  targetedIntent.beginCommandTarget("attack");
  targetedInput.state = {
    playerId: 1,
  };
  targetedInput.clientIntent = targetedIntent;
  targetedInput.renderer = { drawSelectionBox() {} };
  targetedInput.commandIssuer = { issueCommand: (command) => sentCommands.push(command) };
  targetedInput._worldAt = (x, y) => ({ x, y });
  targetedInput._entityAtWorld = () => ownBuilding;
  targetedInput._selectedOwnUnitIds = () => [7];
  targetedInput._commitClickSelection = (p) => selectionClicks.push(p);
  targetedInput._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  targetedInput._trackMouse = () => {};
  targetedInput._onLeftDown({ x: 200, y: 200 }, {});
  assert(targetedInput.clientIntent.commandTarget === null, "attack targeting clears after one click");
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

  const labToolInput = Object.create(Input.prototype);
  const labToolEvents = [];
  const labToolSelections = [];
  const selectionBoxes = [];
  let labToolConsumedCancel = null;
  labToolInput.clientIntent = new ClientIntent();
  labToolInput.clientIntent.beginLabTool({
    kind: "spawnEntity",
    payload: { xField: "spawn-x" },
    keepArmedOnWorldClick: true,
  });
  labToolInput.labToolController = {
    consumeWorldClick(event) {
      labToolEvents.push(event);
    },
    cancel(reason) {
      labToolConsumedCancel = reason;
      return labToolInput.clientIntent.cancelLabTool(reason);
    },
  };
  labToolInput.pointerLocked = false;
  labToolInput.cameraNavigation = null;
  labToolInput.renderer = { drawSelectionBox(box) { selectionBoxes.push(box); } };
  labToolInput._worldAt = (x, y) => ({ x: x + 100, y: y + 200 });
  labToolInput._commitClickSelection = (p) => labToolSelections.push(p);
  labToolInput._eventScreenPos = () => ({ x: 12, y: 24 });
  labToolInput._trackMouse = () => {};
  labToolInput._routeLockedPointerUp = () => false;
  labToolInput._finishTankTrapPlacementDrag = () => false;
  labToolInput._onLeftDown({ x: 12, y: 24 }, { shiftKey: true });
  assert(labToolEvents.length === 0, "active lab spawn tool waits for a completed click before placing");
  labToolInput._handleMouseUp({ button: 0, shiftKey: true });
  assert(labToolEvents.length === 1, "active lab spawn tool consumes the completed world click");
  assert(labToolEvents[0].x === 112 && labToolEvents[0].y === 224, "lab tool click callback receives exact world coordinates");
  assert(labToolEvents[0].tool.payload.xField === "spawn-x", "lab tool click callback receives current tool payload");
  assert(labToolInput.clientIntent.activeLabTool !== null, "persistent lab spawn tool stays armed after a world click");
  assert(labToolConsumedCancel === null, "persistent lab spawn tool does not cancel on world click");
  labToolInput._onLeftDown({ x: 20, y: 28 }, {});
  labToolInput._eventScreenPos = () => ({ x: 20, y: 28 });
  labToolInput._handleMouseUp({ button: 0, shiftKey: false });
  assert(labToolEvents.length === 2, "persistent lab spawn tool places again on the next completed click");
  assert(labToolInput._drag == null && labToolSelections.length === 0, "lab tool click does not fall through to selection drag");

  const labDragInput = Object.create(Input.prototype);
  const labDragEvents = [];
  const labDragSelections = [];
  const labDragBoxes = [];
  let labDragCancel = null;
  labDragInput.clientIntent = new ClientIntent();
  labDragInput.clientIntent.beginLabTool({ kind: "spawnEntity", keepArmedOnWorldClick: true });
  labDragInput.labToolController = {
    consumeWorldClick(event) {
      labDragEvents.push(event);
    },
    cancel(reason) {
      labDragCancel = reason;
      return labDragInput.clientIntent.cancelLabTool(reason);
    },
  };
  labDragInput.pointerLocked = false;
  labDragInput.cameraNavigation = null;
  labDragInput.renderer = { drawSelectionBox(box) { labDragBoxes.push(box); } };
  labDragInput._screenPos = () => ({ x: 40, y: 42 });
  labDragInput._eventScreenPos = () => ({ x: 40, y: 42 });
  labDragInput._trackMouse = () => {};
  labDragInput._routeLockedPointerMove = () => false;
  labDragInput._routeLockedPointerUp = () => false;
  labDragInput._finishTankTrapPlacementDrag = () => false;
  labDragInput._commitBoxSelection = (drag) => labDragSelections.push(drag);
  labDragInput._onLeftDown({ x: 12, y: 24 }, {});
  labDragInput._handleMouseMove({});
  labDragInput._handleMouseUp({ button: 0, shiftKey: false });
  assert(labDragEvents.length === 0, "dragging with a lab spawn tool does not place a unit");
  assert(labDragCancel === "boxSelect", "box selection cancels an active lab spawn tool");
  assert(labDragInput.clientIntent.activeLabTool === null, "box selection clears the active lab spawn tool");
  assert(labDragSelections.length === 1, "dragging with a lab spawn tool falls through to box selection");
  assert(labDragBoxes.some(Boolean), "dragging with a lab spawn tool draws the selection box");

  const labRightClickInput = Object.create(Input.prototype);
  let labRightClickCancel = null;
  labRightClickInput.clientIntent = new ClientIntent();
  labRightClickInput.clientIntent.beginLabTool({ kind: "fieldPoint" });
  labRightClickInput.labToolController = {
    cancel(reason) {
      labRightClickCancel = reason;
      return labRightClickInput.clientIntent.cancelLabTool(reason);
    },
  };
  labRightClickInput._selectedOwnUnitIds = () => {
    throw new Error("right-click lab tool cancellation must not issue normal commands");
  };
  labRightClickInput._onRightClick({ x: 5, y: 6 }, {});
  assert(labRightClickInput.clientIntent.activeLabTool === null, "right-click cancels an active lab tool");
  assert(labRightClickCancel === "rightClick", "right-click lab tool cancellation flows through the controller");

  targetedInput.clientIntent.endCommandTarget();
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

  targetedInput.clientIntent.beginCommandTarget("move");
  targetedInput._onLeftDown({ x: 260, y: 260 }, { shiftKey: true });
  let lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "move", "move targeting should issue a move command");
  assert(lastSent.queued === true, "Shift move targeting should queue movement");

  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput._entityAtWorld = () => null;
  targetedInput._onLeftDown({ x: 280, y: 280 }, { shiftKey: true });
  lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "attackMove", "attack targeting terrain should attack-move");
  assert(lastSent.queued === true, "Shift attack-move targeting should queue attack-move");

  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput._entityAtWorld = () => ({ id: 99, owner: 2, kind: KIND.RIFLEMAN, x: 300, y: 300 });
  targetedInput._onLeftDown({ x: 300, y: 300 }, { shiftKey: true });
  lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "attack", "attack targeting an enemy should issue attack");
  assert(
    lastSent.queued === true,
    "Shift enemy attack targeting should queue attack",
  );

  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput.clientIntent.holdCommandTarget("attack", "KeyA", true);
  targetedInput._entityAtWorld = () => null;
  targetedInput._onLeftDown({ x: 320, y: 320 }, { shiftKey: true });
  assert(
    targetedInput.clientIntent.commandTarget === "attack",
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
    targetedInput.clientIntent.commandTarget === "attack",
    "held A keeps attack targeting armed after an unqueued click",
  );

  targetedInput.clientIntent.endCommandTarget();
  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput.clientIntent.holdCommandTarget("attack", "KeyA");
  targetedInput._handleKeyUp({ code: "KeyA", preventDefault() {} });
  assert(targetedInput.clientIntent.commandTarget === null, "A keyup exits sticky attack targeting");

  const originalDocument = globalThis.document;
  const hotkeyTargetedInput = Object.create(Input.prototype);
  const hotkeyIssues = [];
  const quickCastSelectionClicks = [];
  const quickCastBoxSelections = [];
  hotkeyTargetedInput.mouse = { x: 420, y: 260 };
  hotkeyTargetedInput.pointerLocked = false;
  hotkeyTargetedInput.cameraNavigation = null;
  hotkeyTargetedInput._panDrag = null;
  hotkeyTargetedInput._drag = null;
  hotkeyTargetedInput._dragging = false;
  hotkeyTargetedInput._placementDrag = null;
  hotkeyTargetedInput.renderer = { drawSelectionBox() {} };
  hotkeyTargetedInput._handleControlGroupHotkey = () => false;
  hotkeyTargetedInput._quickCastCommandTarget = (ev) => {
    hotkeyIssues.push({ shiftKey: !!ev.shiftKey, mouse: hotkeyTargetedInput.mouse });
    return Input.prototype._quickCastCommandTarget.call(hotkeyTargetedInput, ev);
  };
  hotkeyTargetedInput._issueTargetedCommand = (p, ev) => {
    hotkeyIssues.push({ issuedAt: p, queued: !!ev.shiftKey });
  };
  hotkeyTargetedInput._eventScreenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  hotkeyTargetedInput._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  hotkeyTargetedInput._trackMouse = () => {};
  hotkeyTargetedInput._commitClickSelection = (p) => quickCastSelectionClicks.push(p);
  hotkeyTargetedInput._commitBoxSelection = (drag) => quickCastBoxSelections.push(drag);
  hotkeyTargetedInput.state = {};
  hotkeyTargetedInput.clientIntent = new ClientIntent();
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
              hotkeyTargetedInput.clientIntent.beginCommandTarget("attack", { now: 100 + hotkeyIssues.length * 100 });
            },
          }];
        },
      };
    },
  };
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyA"));
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === null && hotkeyIssues.length === 0,
    "unbound legacy A key should not arm attack when Attack is rebound",
  );
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY"));
  hotkeyTargetedInput._handleKeyUp({ code: "KeyY", shiftKey: false, preventDefault() {} });
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === "attack",
    "plain targeted-order hotkey tap should stay armed after keyup",
  );
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY"));
  assert(
    hotkeyIssues.some((entry) => entry.issuedAt === hotkeyTargetedInput.mouse && entry.queued === false),
    "second same targeted-order hotkey should quick-cast at the cursor",
  );
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === null,
    "unqueued quick-cast should consume the armed targeted order",
  );
  hotkeyTargetedInput._onLeftDown({ x: 422, y: 261 }, {});
  hotkeyTargetedInput._handleMouseUp({
    button: 0,
    clientX: 422,
    clientY: 261,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(
    quickCastSelectionClicks.length === 0,
    "near click after unqueued quick-cast should not become selection",
  );
  assert(
    hotkeyTargetedInput._postQuickCastSelectionGuard === null,
    "post quick-cast selection guard should be one-shot",
  );

  armPostQuickCastSelectionGuard(hotkeyTargetedInput, { x: 420, y: 260 });
  hotkeyTargetedInput._onLeftDown({ x: 420, y: 260 }, {});
  hotkeyTargetedInput._handleMouseMove({ clientX: 428, clientY: 260 });
  hotkeyTargetedInput._handleMouseUp({
    button: 0,
    clientX: 428,
    clientY: 260,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(
    quickCastBoxSelections.length === 1,
    "drag after unqueued quick-cast should still perform box selection",
  );

  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY", { shiftKey: true }));
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY", { shiftKey: true }));
  assert(
    hotkeyIssues.some((entry) => entry.issuedAt === hotkeyTargetedInput.mouse && entry.queued === true),
    "Shift double-tap targeted-order hotkey should quick-cast a queued order at the cursor",
  );
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === "attack",
    "Shift quick-cast should keep the targeted order armed until Shift is released",
  );
  hotkeyTargetedInput._handleKeyUp({ code: "KeyY", shiftKey: true, preventDefault() {} });
  hotkeyTargetedInput._handleKeyUp({ code: "ShiftLeft", preventDefault() {} });
  assert(hotkeyTargetedInput.clientIntent.commandTarget === null, "Shift release clears the queued hotkey target");
  globalThis.document = originalDocument;

  const placementKeyInput = Object.create(Input.prototype);
  let placementEnded = 0;
  let commandTargetShiftReleased = 0;
  let shiftKeyupPrevented = false;
  placementKeyInput.state = {};
  placementKeyInput.clientIntent = new ClientIntent();
  placementKeyInput.clientIntent.placement = { building: KIND.DEPOT, tileX: 2, tileY: 3, valid: true };
  placementKeyInput.clientIntent.releaseCommandTargetShift = () => {
    commandTargetShiftReleased += 1;
  };
  placementKeyInput.clientIntent.endPlacement = () => {
    placementEnded += 1;
    placementKeyInput.clientIntent.placement = null;
  };
  placementKeyInput._handleKeyUp({
    code: "ShiftRight",
    preventDefault() {
      shiftKeyupPrevented = true;
    },
  });
  assert(commandTargetShiftReleased === 1, "Shift release still clears command-target preservation");
  assert(placementEnded === 1 && placementKeyInput.clientIntent.placement === null, "Shift release clears build placement");
  assert(shiftKeyupPrevented === true, "Shift placement release prevents browser default");

  const placementBlurInput = Object.create(Input.prototype);
  let blurPlacementEnded = 0;
  placementBlurInput.pointerLocked = false;
  placementBlurInput.keys = { up: true, down: true, left: true, right: true };
  placementBlurInput.mouse = { x: 1, y: 2 };
  placementBlurInput._spacePan = true;
  placementBlurInput._panDrag = { x: 1, y: 2, button: 1 };
  placementBlurInput._drag = null;
  placementBlurInput.state = {};
  placementBlurInput.clientIntent = new ClientIntent();
  placementBlurInput.clientIntent.placement = { building: KIND.DEPOT, tileX: 2, tileY: 3, valid: true };
  placementBlurInput.clientIntent.endCommandTarget = () => {};
  placementBlurInput.clientIntent.endPlacement = () => {
    blurPlacementEnded += 1;
    placementBlurInput.clientIntent.placement = null;
  };
  placementBlurInput._handleBlur();
  assert(blurPlacementEnded === 1 && placementBlurInput.clientIntent.placement === null, "window blur clears build placement");

  const labBlurInput = Object.create(Input.prototype);
  let labBlurCancel = 0;
  labBlurInput.pointerLocked = false;
  labBlurInput.keys = { up: true, down: true, left: true, right: true };
  labBlurInput.mouse = { x: 1, y: 2 };
  labBlurInput._spacePan = true;
  labBlurInput._panDrag = { x: 1, y: 2, button: 1 };
  labBlurInput._drag = null;
  labBlurInput.state = {};
  labBlurInput.clientIntent = new ClientIntent();
  const labBlurTool = labBlurInput.clientIntent.beginLabTool({ kind: "spawnEntity", keepArmedOnWorldClick: true });
  labBlurInput.labToolController = {
    cancel() {
      labBlurCancel += 1;
      return labBlurInput.clientIntent.cancelLabTool("blur");
    },
  };
  labBlurInput._handleBlur();
  assert(labBlurInput.clientIntent.activeLabTool?.id === labBlurTool.id, "window blur leaves active lab spawn tools armed");
  assert(labBlurCancel === 0, "window blur does not route lab tool cancellation");

  const placementConfirmInput = Object.create(Input.prototype);
  const placementCommands = [];
  let confirmedPlacementEnded = 0;
  placementConfirmInput.commandIssuer = {
    command(command) {
      placementCommands.push(command);
    },
  };
  placementConfirmInput.state = {};
  placementConfirmInput.clientIntent = new ClientIntent();
  placementConfirmInput.clientIntent.placement = { building: KIND.DEPOT, tileX: 4, tileY: 5, valid: true };
  placementConfirmInput.clientIntent.endPlacement = () => {
    confirmedPlacementEnded += 1;
    placementConfirmInput.clientIntent.placement = null;
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

  const trapPlacementInput = Object.create(Input.prototype);
  const trapPlacementCommands = [];
  let trapPlacementEnded = 0;
  trapPlacementInput.commandIssuer = {
    command(command) {
      trapPlacementCommands.push(command);
    },
  };
  trapPlacementInput.state = {};
  trapPlacementInput.clientIntent = new ClientIntent();
  trapPlacementInput.clientIntent.placement = {
    building: KIND.TANK_TRAP,
    tileX: 0,
    tileY: 0,
    valid: true,
    lineSites: [
      { tileX: 0, tileY: 0, valid: true },
      { tileX: 2, tileY: 0, valid: false },
      { tileX: 4, tileY: 0, valid: true },
      { tileX: 6, tileY: 0, valid: true },
    ],
  };
  trapPlacementInput.clientIntent.endPlacement = () => {
    trapPlacementEnded += 1;
    trapPlacementInput.clientIntent.placement = null;
  };
  trapPlacementInput._selectedWorkerIds = () => [77, 88];
  trapPlacementInput._confirmPlacement({ shiftKey: true });
  assertDeepEqual(
    trapPlacementCommands.map((command) => [command.units, command.tileX, command.tileY, command.queued === true]),
    [
      [[77], 0, 0, false],
    ],
    "Tank Trap placement confirmation preserves spacing when preview sites contain a gap",
  );
  assert(trapPlacementEnded === 0, "Shift Tank Trap placement preserves placement mode without changing overflow queueing");

  const artilleryCommands = [];
  const artilleryFeedback = [];
  const selectedArtillery = {
    id: 44,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
    setupFacing: Math.PI,
  };
  const pointFireInput = Object.create(Input.prototype);
  pointFireInput.mouse = { x: 900, y: 100 };
  pointFireInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    selectedEntities: () => [selectedArtillery],
  };
  pointFireInput.clientIntent = new ClientIntent();
  pointFireInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  pointFireInput.clientIntent.addCommandFeedback = (kind, x, y, queued, radiusTiles) => {
    artilleryFeedback.push({ kind, x, y, queued, radiusTiles });
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
  assert(pointFireInput.clientIntent.abilityTargetPreview?.hoverInRange === false, "Point Fire preview rejects the minimum range dead zone");
  assert(pointFireInput.clientIntent.abilityTargetPreview?.hoverInsideMinRange === true, "Point Fire preview identifies minimum range invalidity");
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.minRangePx === ARTILLERY_MIN_RANGE_TILES * 32,
    "Point Fire preview exposes minimum range in pixels",
  );
  assert(
    ARTILLERY_MIN_RANGE_TILES === 15 && ARTILLERY_MAX_RANGE_TILES === 55,
    "Artillery point-fire range mirrors the 15-55 tile balance band",
  );
  const deployedArtillery = {
    ...selectedArtillery,
    setupState: SETUP.DEPLOYED,
    setupFacing: 0,
  };
  const artilleryConeGfx = new RecordingGraphics();
  _drawAntiTankGunSetupPreview.call(
    { _feedbackGfx: artilleryConeGfx, _map: { tileSize: 32 } },
    { playerId: 1, selectedEntities: () => [deployedArtillery] },
  );
  const artilleryConeArcs = artilleryConeGfx.calls.filter((call) => call[0] === "arc");
  assert(
    artilleryConeArcs.some((call) => call[3] === ARTILLERY_MAX_RANGE_TILES * 32),
    "Artillery field-of-fire cone preview uses the mirrored maximum range",
  );
  assert(
    artilleryConeArcs.some((call) => call[3] === ARTILLERY_MIN_RANGE_TILES * 32 && call[6] === true),
    "Artillery field-of-fire cone preview cuts out the mirrored minimum range",
  );
  pointFireInput.mouse = { x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 + 16, y: selectedArtillery.y };
  pointFireInput._refreshAbilityTargetPreview();
  assert(pointFireInput.clientIntent.abilityTargetPreview?.hoverInRange === true, "Point Fire preview accepts targets past minimum range");
  assert(pointFireInput.clientIntent.abilityTargetPreview?.hoverInsideMinRange === false, "Point Fire preview clears minimum range invalidity outside the dead zone");
  const targetingConeGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: targetingConeGfx, _map: { tileSize: 32 } },
    { abilityTargetPreview: pointFireInput.clientIntent.abilityTargetPreview },
  );
  const targetingConeArcs = targetingConeGfx.calls.filter((call) => call[0] === "arc");
  assert(
    targetingConeArcs.some((call) => call[3] === ARTILLERY_MAX_RANGE_TILES * 32),
    "Point Fire targeting cone uses the mirrored maximum range",
  );
  assert(
    targetingConeArcs.some((call) => call[3] === ARTILLERY_MIN_RANGE_TILES * 32 && call[6] === true),
    "Point Fire targeting cone cuts out the mirrored minimum range",
  );

  const previewGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: previewGfx },
    { abilityTargetPreview: { ...pointFireInput.clientIntent.abilityTargetPreview, carriers: [] } },
  );
  const validHorizontalStroke = previewGfx.calls.some(
    (call, i, calls) =>
      call[0] === "moveTo" &&
      call[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY &&
      calls[i + 1]?.[0] === "lineTo" &&
      calls[i + 1]?.[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY,
  );
  assert(validHorizontalStroke, "Point Fire valid cursor keeps the crosshair stroke");

  pointFireInput.mouse = { x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8, y: selectedArtillery.y };
  pointFireInput._refreshAbilityTargetPreview();
  const invalidGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: invalidGfx },
    { abilityTargetPreview: { ...pointFireInput.clientIntent.abilityTargetPreview, carriers: [] } },
  );
  const invalidDiagonalStroke = invalidGfx.calls.some(
    (call, i, calls) =>
      call[0] === "moveTo" &&
      call[2] < pointFireInput.clientIntent.abilityTargetPreview.mouseY &&
      calls[i + 1]?.[0] === "lineTo" &&
      calls[i + 1]?.[2] > pointFireInput.clientIntent.abilityTargetPreview.mouseY,
  );
  assert(invalidDiagonalStroke, "Point Fire invalid minimum-range cursor draws an X");

  const ekatEntity = { id: 88, owner: 1, kind: KIND.EKAT, x: 200, y: 220 };
  const ekatInput = Object.create(Input.prototype);
  ekatInput.mouse = { x: 360, y: 236 };
  ekatInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    abilityObjects: [
      {
        id: 901,
        owner: 1,
        ability: ABILITY.EKAT_LINE_SHOT,
        kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR,
        x: 260,
        y: 260,
        ownerState: { radius: 12 },
      },
      {
        id: 902,
        owner: 2,
        ability: ABILITY.EKAT_LINE_SHOT,
        kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR,
        x: 280,
        y: 280,
      },
    ],
    selectedEntities: () => [ekatEntity],
  };
  ekatInput.clientIntent = new ClientIntent();
  ekatInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.EKAT_LINE_SHOT });
  ekatInput._worldAt = (x, y) => ({ x, y });
  ekatInput._refreshAbilityTargetPreview();
  assert(ekatInput.clientIntent.abilityTargetPreview?.pathOrigins.length === 2, "Ekat line preview includes caster plus owned anchor origin");
  assert(
    ekatInput.clientIntent.abilityTargetPreview.pathOrigins.some((origin) => origin.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR),
    "Ekat line preview marks anchor origin kind",
  );
  const ekatPreviewGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: ekatPreviewGfx },
    { abilityTargetPreview: ekatInput.clientIntent.abilityTargetPreview },
  );
  assert(
    ekatPreviewGfx.calls.some(
      (call) => call[0] === "lineStyle" && call[2] === 0xc7d07a,
    ),
    "Ekat line preview draws Magic Anchor origins without crashing",
  );

  const returnInput = Object.create(Input.prototype);
  returnInput.mouse = { x: 420, y: 260 };
  returnInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    abilityObjects: [
      {
        id: 903,
        owner: 1,
        ability: ABILITY.EKAT_TELEPORT,
        kind: ABILITY_OBJECT_KIND.RETURN_MARKER,
        x: 180,
        y: 190,
        expiresIn: 70,
      },
    ],
    selectedEntities: () => [ekatEntity],
  };
  returnInput.clientIntent = new ClientIntent();
  returnInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.EKAT_TELEPORT });
  returnInput._worldAt = (x, y) => ({ x, y });
  returnInput._refreshAbilityTargetPreview();
  assert(returnInput.clientIntent.abilityTargetPreview?.returnMarkers[0]?.id === 903, "Ekat dash preview exposes owned return marker preview");

  const abilityObjectGfx = new RecordingGraphics();
  _drawAbilityObjects.call(
    { _abilityObjectGfx: abilityObjectGfx, _map: { tileSize: 32 } },
    {
      abilityObjects: [
        { id: 904, kind: ABILITY_OBJECT_KIND.RETURN_MARKER, x: 128, y: 144 },
        { id: 905, kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR, x: 160, y: 192, ownerState: { hp: 80, radius: 16 } },
      ],
    },
  );
  assert(
    abilityObjectGfx.calls.some((call) => call[0] === "drawCircle" && call[1] === 128 && call[2] === 144) &&
      abilityObjectGfx.calls.some((call) => call[0] === "beginFill"),
    "ability object renderer draws return marker and anchor placeholders",
  );
}

{
  const tankTrapEntity = {
    id: 720,
    owner: 1,
    kind: KIND.TANK_TRAP,
    x: 64,
    y: 96,
    buildProgress: 0.5,
  };
  const fakePools = new Map();
  let iconCalls = 0;
  let tintCalls = 0;
  const fakeRenderer = {
    _map: { tileSize: 32 },
    _slot(pool, id) {
      const key = `${pool}:${id}`;
      if (!fakePools.has(key)) fakePools.set(key, new RecordingGraphics());
      return fakePools.get(key);
    },
    _tintFor() {
      tintCalls += 1;
      return 0x4878c8;
    },
    _icon() {
      iconCalls += 1;
    },
    _queueLabel() {},
  };
  _drawBuilding.call(fakeRenderer, tankTrapEntity, new Map([[1, 0x4878c8]]), {});
  const trapGraphics = fakePools.get("buildings:720");
  const trapPolygons = trapGraphics.calls.filter((call) => call[0] === "drawPolygon");
  const trapColors = trapGraphics.calls
    .filter((call) => call[0] === "beginFill" || call[0] === "lineStyle")
    .map((call) => (call[0] === "beginFill" ? call[1] : call[2]));
  assert(trapPolygons.length >= 12, "Tank Trap renderer draws filled steel hedgehog beams");
  assert(!trapColors.includes(0x4878c8), "Tank Trap renderer avoids owner/team coloring");
  assert(tintCalls === 0, "Tank Trap renderer does not request owner tint");
  assert(iconCalls === 0, "Tank Trap renderer uses geometry instead of the generic building stencil");
  const firstBeam = trapGraphics.calls.find((call) => call[0] === "drawPolygon")?.[1];
  const firstBeamXs = firstBeam.filter((_, index) => index % 2 === 0);
  const firstBeamYs = firstBeam.filter((_, index) => index % 2 === 1);
  const firstBeamSpan = Math.max(
    Math.max(...firstBeamXs) - Math.min(...firstBeamXs),
    Math.max(...firstBeamYs) - Math.min(...firstBeamYs),
  );
  assert(firstBeamSpan > 32, "Tank Trap renderer scales beams larger than the 1x1 footprint");

  const secondTrapEntity = { ...tankTrapEntity, id: 721 };
  _drawBuilding.call(fakeRenderer, secondTrapEntity, new Map([[1, 0x4878c8]]), {});
  const secondTrapGraphics = fakePools.get("buildings:721");
  const secondBeam = secondTrapGraphics.calls.find((call) => call[0] === "drawPolygon")?.[1];
  assert(
    firstBeam?.some((value, index) => Math.abs(value - secondBeam[index]) > 0.1),
    "Tank Trap renderer varies rotation between trap instances",
  );
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
  assert(fog.revision === 0, "Fog starts with a stable cache revision");
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
  const revisionAfterReveal = fog.revision;
  assert(revisionAfterReveal > 0, "Fog revision increments when visibility changes");
  assert(fog.isVisible(2, 2) === true, "tile under entity should be visible");
  assert(fog.isExplored(2, 2) === true, "tile under entity should be explored");
  fog.update(
    [{ kind: "worker", x: 64, y: 64 }],
    32,
  );
  assert(fog.revision === revisionAfterReveal, "Fog revision stays stable for identical visibility");

  // After clearing visible, explored should persist
  fog.update([], 32);
  assert(fog.revision > revisionAfterReveal, "Fog revision increments when current visibility clears");
  assert(fog.isVisible(2, 2) === false, "tile should no longer be visible");
  assert(fog.isExplored(2, 2) === true, "tile should still be explored");

  const serverFog = new Fog(2, 1);
  serverFog.update([], 32, new Uint8Array([1, 0]));
  const serverRevision = serverFog.revision;
  serverFog.update([], 32, new Uint8Array([1, 0]));
  assert(serverFog.revision === serverRevision, "server fog revisions are stable for repeated grids");
  serverFog.update([], 32, new Uint8Array([0, 1]));
  assert(serverFog.revision > serverRevision, "server fog revisions change for new grids");

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
  assertApprox(mid.gain, 1 / 5, 0.001, "Audio spatial gain quadruples far-distance attenuation");

  const far = audio._computeSpatial(1300, 100);
  assert(far !== null, "Audio spatial max-distance edge should play");
  assertApprox(far.gain, 1 / 9, 0.001, "Audio spatial gain attenuates harder at maxDist");
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

// ---------------------------------------------------------------------------
// Observer analysis overlay
// ---------------------------------------------------------------------------
{
  const players = [
    { id: 1, name: "Red", color: "#cc1111" },
    { id: 2, name: "Blue", color: "#1133bb" },
  ];
  const calculatorRows = calculateViewportArmyValue({
    players,
    cameraBounds: { x: 0, y: 0, width: 100, height: 100 },
    stats: {
      [KIND.RIFLEMAN]: { size: 9, cost: { steel: 50, oil: 0 } },
      [KIND.TANK]: { size: 18, cost: { steel: 300, oil: 150 } },
      [KIND.BARRACKS]: { size: 24, cost: { steel: 150, oil: 0 } },
    },
    entities: [
      { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 },
      { id: 2, owner: 1, kind: KIND.TANK, x: 150, y: 20 },
      { id: 3, owner: 2, kind: KIND.TANK, x: 99, y: 50 },
      { id: 4, owner: 2, kind: KIND.BARRACKS, x: 20, y: 20 },
      { id: 5, owner: 1, kind: KIND.RIFLEMAN, x: 40, y: 40, shotReveal: true },
      { id: 6, owner: 1, kind: KIND.RIFLEMAN, x: 60, y: 40, visionOnly: true },
      { id: 7, owner: 1, kind: KIND.STEEL, x: 25, y: 25 },
      { id: 8, owner: 2, kind: KIND.MACHINE_GUNNER, x: 30, y: 30 },
    ],
  });
  const redValue = calculatorRows.find((row) => row.owner === 1);
  const blueValue = calculatorRows.find((row) => row.owner === 2);
  assert(redValue.steel === 100 && redValue.oil === 0, "army value counts visible units and visionOnly units");
  assert(blueValue.steel === 300 && blueValue.oil === 150, "army value groups costs by owner");
  assert(calculatorRows.length === 2, "army value keeps known player rows only for known owners");

  const emptyRows = calculateViewportArmyValue({
    players,
    cameraBounds: { x: 500, y: 500, width: 100, height: 100 },
    entities: [{ id: 9, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 }],
  });
  assert(
    emptyRows.every((row) => row.steel === 0 && row.oil === 0),
    "army value reports zero for players with no visible on-screen units",
  );

  const storage = fakeStorage();
  const prefs = createObserverAnalysisOverlayPreferences(storage);
  prefs.selectedTab = "units-lost";
  prefs.visible = false;
  prefs.collapsed = true;

  const restored = createObserverAnalysisOverlayPreferences(storage);
  assert(restored.selectedTab === "units-lost", "observer analysis selected tab persists");
  assert(restored.visible === false, "observer analysis visible state persists");
  assert(restored.collapsed === true, "observer analysis collapsed state persists");

  restored.selectedTab = "not-a-tab";
  assert(
    restored.selectedTab === OBSERVER_ANALYSIS_TABS[0].id,
    "observer analysis rejects unknown tab ids",
  );

  const legacyStorage = fakeStorage();
  legacyStorage.setItem("rts.replayAnalysisOverlay", JSON.stringify({
    selectedTab: "production",
    visible: false,
    collapsed: true,
  }));
  const migrated = createObserverAnalysisOverlayPreferences(legacyStorage);
  assert(migrated.selectedTab === "production", "observer analysis reads legacy replay preference key");
  migrated.visible = true;
  assert(
    legacyStorage.getItem("rts.observerAnalysisOverlay") !== null,
    "observer analysis writes the observer preference key after reading legacy preferences",
  );

  assert(
    shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { replay: {}, spectator: true, diagnostics: { observerAnalysis: true } },
      }),
    }),
    "observer analysis mounts when the start payload advertises it for replay viewers",
  );
  assert(
    shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { spectator: true, diagnostics: { observerAnalysis: true } },
      }),
    }),
    "observer analysis mounts when the start payload advertises it for live spectators",
  );
  assert(
    !shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { spectator: false, diagnostics: { observerAnalysis: false } },
      }),
    }),
    "observer analysis stays hidden without diagnostic metadata",
  );
  assert(
    !shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { replay: {}, spectator: true },
      }),
    }),
    "observer analysis does not mount from replay identity alone",
  );

  withFakeOverlayDocument(({ FakeElement }) => {
    const root = new FakeElement("section");
    restored.selectedTab = "army-value";
    restored.visible = true;
    restored.collapsed = false;
    const overlay = new ObserverAnalysisOverlay({
      root,
      preferences: restored,
      getPlayers: () => players,
      getCameraBounds: () => ({ x: 0, y: 0, width: 100, height: 100 }),
      getEntities: () => [{ id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 }],
    });
    assert(root.children.length === 1, "observer analysis overlay mounts generated DOM");
    const overlayRoot = root.children[0];
    assert(root.querySelector(".replay-army-value-row"), "observer analysis renders army value rows");
    assert(
      findFakes(root, (el) => el.classList.contains("replay-army-value-steel"))
        .some((cell) => cell.querySelector(".steel"))
        && findFakes(root, (el) => el.classList.contains("replay-army-value-oil"))
          .some((cell) => cell.querySelector(".oil")),
      "observer analysis army value uses shared steel and oil icons",
    );
    const analysisBody = root.querySelector("#replay-analysis-body");
    const stableArmyValueRenders = analysisBody.replaceChildrenCount;
    overlay.update({
      authoritativeEntities: [{ id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 }],
    });
    assert(
      analysisBody.replaceChildrenCount === stableArmyValueRenders,
      "observer analysis skips unchanged army-value body DOM replacement",
    );
    overlay.update({
      authoritativeEntities: [
        { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 },
        { id: 2, owner: 2, kind: KIND.TANK, x: 20, y: 20 },
      ],
    });
    assert(
      analysisBody.replaceChildrenCount === stableArmyValueRenders + 1,
      "observer analysis replaces army-value body when visible values change",
    );

    const unitsTab = root.querySelector(".replay-analysis-tab");
    assert(unitsTab, "observer analysis renders tab buttons");
    overlayRoot.listeners.click?.({ target: unitsTab, preventDefault() {}, stopPropagation() {} });
    assert(
      restored.selectedTab === unitsTab.dataset.tabId,
      "observer analysis tab clicks update shared preferences",
    );

    const hide = root.querySelector(".replay-analysis-hide");
    overlayRoot.listeners.click?.({ target: hide, preventDefault() {}, stopPropagation() {} });
    assert(restored.visible === false, "observer analysis hide action updates shared preferences");

    const show = root.querySelector(".replay-analysis-show");
    overlayRoot.listeners.click?.({ target: show, preventDefault() {}, stopPropagation() {} });
    assert(restored.visible === true, "observer analysis show action updates shared preferences");
    assert(restored.collapsed === false, "observer analysis show expands the panel");

    restored.selectedTab = "production";
    overlay.render();
    assert(
      textWithin(root).includes("Waiting for observer analysis"),
      "production tab shows a loading state before analysis arrives",
    );
    overlay.applyObserverAnalysis({ tick: 1, players: [{ id: 1, units: [], production: [] }, { id: 2, units: [], production: [] }] });
    assert(
      textWithin(root).includes("No active production"),
      "production tab handles empty production cleanly",
    );

    overlay.applyObserverAnalysis({
      tick: 12,
      players: [
        {
          id: 1,
          units: [],
          production: [
            {
              buildingId: 11,
              buildingKind: KIND.BARRACKS,
              itemKind: KIND.MACHINE_GUNNER,
              itemType: "unit",
              progress: 0.42,
              queueDepth: 2,
            },
          ],
        },
        {
          id: 2,
          units: [],
          production: [
            {
              buildingId: 21,
              buildingKind: KIND.RESEARCH_COMPLEX,
              itemKind: UPGRADE.TANK_UNLOCK,
              itemType: "upgrade",
              progress: 0.75,
              queueDepth: 1,
            },
          ],
        },
      ],
    });
    const productionText = textWithin(root);
    assert(productionText.includes("Red"), "production tab groups rows by first player");
    assert(productionText.includes("Blue"), "production tab groups rows by second player");
    assert(
      productionText.includes("Machine Gunner at Barracks") && productionText.includes("42") && productionText.includes("Q 2"),
      "production tab renders active unit production with progress and queue depth",
    );
    assert(
      productionText.includes("Tank Production at R&D Complex") && productionText.includes("75"),
      "production tab renders active research with mirrored upgrade labels",
    );

    restored.selectedTab = "units";
    overlay.render();
    overlay.applyObserverAnalysis({
      tick: 20,
      players: [
        {
          id: 1,
          units: [
            { kind: KIND.RIFLEMAN, count: 3, steelValue: 150, oilValue: 0 },
            { kind: KIND.TANK, count: 1, steelValue: 300, oilValue: 150 },
          ],
          production: [],
        },
        {
          id: 2,
          units: [{ kind: KIND.WORKER, count: 2, steelValue: 100, oilValue: 0 }],
          production: [],
        },
      ],
    });
    const unitText = textWithin(root);
    assert(unitText.includes("Total") && unitText.includes("4") && unitText.includes("450") && unitText.includes("150"),
      "units tab includes totals for the current player group");
    assert(unitText.includes("Rifleman") && unitText.includes("Tank"), "units tab renders per-kind unit rows");
    assert(unitText.includes("Blue") && unitText.includes("Engineer"), "units tab renders multiple players");
    assert(
      findFakes(root, (el) => el.classList.contains("replay-units-steel"))
        .some((cell) => cell.querySelector(".steel"))
        && findFakes(root, (el) => el.classList.contains("replay-units-oil"))
          .some((cell) => cell.querySelector(".oil")),
      "units tab uses shared steel and oil icons for resource values",
    );

    overlay.applyObserverAnalysis({
      tick: 5,
      players: [{ id: 1, units: [{ kind: KIND.WORKER, count: 1, steelValue: 50, oilValue: 0 }], production: [] }],
    });
    const replacedUnitText = textWithin(root);
    assert(replacedUnitText.includes("Engineer"), "units tab renders replacement analysis after seek");
    assert(!replacedUnitText.includes("Tank"), "units tab drops stale rows after seek replacement");

    restored.selectedTab = "units-lost";
    overlay.render();
    assert(
      textWithin(root).includes("No units lost"),
      "units lost tab handles analysis with no loss rows cleanly",
    );
    overlay.applyObserverAnalysis({
      tick: 30,
      players: [
        {
          id: 1,
          units: [],
          production: [],
          unitsLost: [
            { kind: KIND.RIFLEMAN, count: 2, steelValue: 100, oilValue: 0 },
            { kind: KIND.TANK, count: 1, steelValue: 300, oilValue: 150 },
          ],
          resourcesLost: { steel: 400, oil: 150 },
        },
        {
          id: 2,
          units: [],
          production: [],
          unitsLost: [{ kind: KIND.WORKER, count: 3, steelValue: 150, oilValue: 0 }],
          resourcesLost: { steel: 150, oil: 0 },
        },
      ],
    });
    const unitsLostText = textWithin(root);
    assert(
      unitsLostText.includes("Total lost") && unitsLostText.includes("3") && unitsLostText.includes("400") && unitsLostText.includes("150"),
      "units lost tab includes per-player totals with steel and oil value lost",
    );
    assert(
      unitsLostText.includes("Rifleman") && unitsLostText.includes("Tank") && unitsLostText.includes("Engineer"),
      "units lost tab renders per-kind loss rows for multiple players",
    );

    restored.selectedTab = "resources-lost";
    overlay.render();
    const resourcesLostText = textWithin(root);
    assert(
      resourcesLostText.includes("Dead unit value")
        && resourcesLostText.includes("Spent steel and oil value of units that died")
        && resourcesLostText.includes("Total")
        && resourcesLostText.includes("550")
        && resourcesLostText.includes("150"),
      "resources lost tab labels the narrow observer analysis definition and totals killed unit value",
    );
    assert(
      resourcesLostText.includes("Red") && resourcesLostText.includes("Blue"),
      "resources lost tab renders per-player killed unit value",
    );

    const tabButtons = root.querySelectorAll(".replay-analysis-tab");
    const firstTab = tabButtons[0];
    overlayRoot.listeners.keydown?.({
      target: firstTab,
      key: "End",
      preventDefault() {},
      stopPropagation() {},
    });
    assert(restored.selectedTab === "resources-lost", "observer analysis keyboard End selects the last tab");
    assert(tabButtons[tabButtons.length - 1].focused === true, "observer analysis keyboard navigation focuses the selected tab");

    overlay.destroy();
    assert(root.children.length === 0, "observer analysis overlay removes generated DOM on destroy");
  });
}

{
  const editorHtml = fs.readFileSync(new URL("../client/map-editor.html", import.meta.url), "utf8");
  assert(!editorHtml.includes('data-view="atlas"'), "map editor does not expose an Atlas tab");
  assert(!editorHtml.includes('MAP_ATLAS_URL'), "map editor does not request atlas diagnostics");
  assert(!editorHtml.includes("atlas-readout"), "map editor does not include atlas controls");
}

function textWithin(node) {
  if (!node) return "";
  let out = node.textContent || "";
  for (const child of node.children || []) out += ` ${textWithin(child)}`;
  return out;
}

console.log("✅ client_contracts.mjs: all contract assertions passed");
