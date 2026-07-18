// tests/client_contracts/hud_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import {
  fakeClassList,
  withFakeDocument,
  withFakeHudDocument,
} from "./fakes.mjs";
import {
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  STATS,
  TICK_HZ,
} from "../../client/src/config.js";
import {
  HUD,
  activeIdleWorkers,
  formatGameTime,
  groupCooldownClocks,
  playerHasCompletedKind,
  selectionBudgetBlockShape,
  selectionBudgetGridModel,
} from "../../client/src/hud.js";
import { entrenchmentSelectionStatus } from "../../client/src/hud_selection_panel.js";
import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  duplicateCommandIdsForCard,
  factionCommandId,
} from "../../client/src/hud_command_card.js";
import {
  DEFAULT_FACTION_ID,
  ABILITY,
  KIND,
  LAB_ROLE,
  ORDER_STAGE,
  SETUP,
  STATE,
  UPGRADE,
  cmd,
} from "../../client/src/protocol.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { _handleKeyDown } from "../../client/src/input/camera_controls.js";

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
    currentEntities: entities,
    groupCooldownClocks,
    playerHasCompleteKind: (kind) => playerHasCompletedKind(entities, playerId, kind),
  };
}

{
  const workers = [
    { id: 1, owner: 1, kind: KIND.WORKER, state: STATE.IDLE },
    { id: 2, owner: 1, kind: KIND.WORKER, state: STATE.MOVE },
    { id: 3, owner: 1, kind: KIND.WORKER, state: STATE.GATHER },
    { id: 4, owner: 1, kind: KIND.WORKER, state: STATE.BUILD },
    { id: 5, owner: 2, kind: KIND.WORKER, state: STATE.IDLE },
    { id: 6, owner: 1, kind: KIND.RIFLEMAN, state: STATE.IDLE },
    { id: 7, owner: 1, kind: KIND.WORKER, state: STATE.IDLE, visionOnly: true },
  ];
  const state = { playerId: 1, isOwnOwner: (owner) => owner === 1 };
  assert(
    activeIdleWorkers(workers, state).map((worker) => worker.id).join(",") === "1",
    "HUD idle-worker query includes only live own workers with authoritative idle activity",
  );
}

{
  const listeners = new Map();
  const button = {
    disabled: true,
    title: "",
    dataset: {},
    addEventListener(type, handler) { listeners.set(type, handler); },
    removeEventListener(type, handler) {
      if (listeners.get(type) === handler) listeners.delete(type);
    },
    setAttribute(name, value) { this[name] = value; },
  };
  const label = { textContent: "Idle workers (T):" };
  const count = { textContent: "0" };
  const ownIdle = [
    { id: 11, owner: 1, kind: KIND.WORKER, state: STATE.IDLE },
    { id: 12, owner: 1, kind: KIND.WORKER, state: STATE.IDLE },
  ];
  const entities = ownIdle.concat([
    { id: 13, owner: 1, kind: KIND.WORKER, state: STATE.GATHER },
    { id: 14, owner: 2, kind: KIND.WORKER, state: STATE.IDLE },
  ]);
  const predictedEntities = entities.map((entity) => (
    entity.id === 11 ? { ...entity, state: STATE.MOVE, predicted: true } : entity
  ));
  let selected = [];
  let menuClosed = 0;
  let plannedSelection = [];
  const state = {
    playerId: 1,
    spectator: false,
    selection: new Set(),
    isOwnOwner: (owner) => owner === 1,
    entitiesInterpolated: (_alpha, options = {}) => (
      options.includePrediction === false ? entities : predictedEntities
    ),
    entityById: (id) => entities.find((entity) => entity.id === id) || null,
    setSelection(ids) {
      selected = Array.from(ids);
      this.selection = new Set(selected);
    },
  };
  const intent = {
    closeCommandCardMenu() { menuClosed += 1; },
    clearPlannedOrdersOutsideSelection(ids) { plannedSelection = Array.from(ids); },
  };
  const root = {
    querySelector(selector) {
      if (selector === "#idle-workers") return button;
      if (selector === ".idle-workers-label") return label;
      if (selector === "#idle-workers-count") return count;
      return null;
    },
  };
  let idleHotkey = "T";
  const hotkeys = { hotkeyForCommand: () => idleHotkey };
  const hud = new HUD(root, state, {}, null, hotkeys, intent);
  assert(
    count.textContent === "2" && button.disabled === true && button.dataset.selectable === "true",
    "HUD shows the active idle-worker count while keeping its pointer control disabled",
  );
  assert(!listeners.has("click"), "HUD idle-worker status does not install a click action");
  assert(label.textContent === "Idle workers (T):", "HUD shows the resolved idle-worker hotkey in its text");
  assert(
    predictedEntities[0].state === STATE.MOVE,
    "HUD idle-worker count ignores prediction-overlaid activity until authority confirms it",
  );
  assert(button["aria-label"] === "Press T to select 2 idle workers", "HUD exposes the idle-worker hotkey accessibly");

  hud.controlPolicy = { canUseCommandSurface: () => false };
  hud._renderIdleWorkers();
  assert(
    count.textContent === "2" && button.disabled === true &&
      button["aria-label"] === "2 idle workers; selection unavailable",
    "read-only HUDs preserve the idle-worker count without announcing that no workers exist",
  );
  hud.controlPolicy = null;
  hud._renderIdleWorkers();
  idleHotkey = "K";
  hud._renderIdleWorkers();
  let prevented = false;
  _handleKeyDown.call({
    hotkeyProfiles: hotkeys,
    globalHotkeyActions: [{ commandId: "hud.selectIdleWorkers", activate: () => hud.selectIdleWorkers() }],
  }, {
    code: "KeyK",
    key: "k",
    repeat: false,
    target: null,
    preventDefault() { prevented = true; },
  });
  assert(label.textContent === "Idle workers (K):", "HUD refreshes the hint after a hotkey profile change");
  assert(prevented && selected.join(",") === "11,12", "the configured hotkey selects every active idle worker");
  assert(menuClosed === 1 && plannedSelection.join(",") === "11,12", "idle-worker selection reconciles local HUD intent");

  let modifiedActivations = 0;
  for (const modifier of ["altKey", "ctrlKey", "metaKey"]) {
    let modifiedPrevented = false;
    _handleKeyDown.call({
      hotkeyProfiles: hotkeys,
      globalHotkeyActions: [{
        commandId: "hud.selectIdleWorkers",
        activate: () => { modifiedActivations += 1; },
      }],
      _activateCommandHotkey: () => false,
      _handleControlGroupHotkey: () => false,
    }, {
      code: "KeyK",
      key: "k",
      repeat: false,
      target: null,
      [modifier]: true,
      preventDefault() { modifiedPrevented = true; },
    });
    assert(!modifiedPrevented, `idle-worker hotkey leaves ${modifier} browser chords available`);
  }
  assert(modifiedActivations === 0, "idle-worker hotkey ignores browser and OS modifier chords");

  ownIdle[0].state = STATE.MOVE;
  ownIdle[1].state = STATE.BUILD;
  hud._renderIdleWorkers();
  assert(count.textContent === "0" && button.disabled === true, "HUD keeps the idle-worker status disabled when none are idle");
  hud.destroy();
  assert(!listeners.has("click"), "HUD teardown leaves no idle-worker click listener");
  assert(button.dataset.selectable === "false", "HUD teardown clears the idle-worker selectable styling state");
  assert(button["aria-label"] === "No idle workers", "HUD teardown restores accurate idle-worker text");
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

// ---------------------------------------------------------------------------
// HUD game timer
// ---------------------------------------------------------------------------
{
  const issued = [];
  const hud = {
    commandInteraction: { issueCommand: (command) => issued.push(command) },
    _intent() {
      return { endCommandTarget() {} };
    },
  };
  HUD.prototype._dispatchCommandIntent.call(
    hud,
    { type: "holdPosition", unitIds: [7] },
    { shiftKey: true },
  );
  assert(
    issued.length === 1 && issued[0].c === "holdPosition" && issued[0].queued === true,
    "Shift-held Hold Position appends a queued hold command",
  );
}

{
  const issued = [];
  const hud = {
    commandInteraction: { issueCommand: (command) => issued.push(command) },
  };
  HUD.prototype._dispatchCommandIntent.call(
    hud,
    { type: "cancelConstruction", buildingId: 91 },
  );
  assert(
    issued.length === 1 && issued[0].c === "cancel" && issued[0].building === 91 &&
      issued[0].construction === true,
    "construction cancellation should issue the authoritative building cancel command",
  );
}

{
  const issued = [];
  const hud = {
    commandInteraction: { issueCommand: (command) => issued.push(command) },
  };
  const intent = {
    type: "adjustProductionRepeat",
    buildingIds: [20, 21, 22],
    unit: KIND.RIFLEMAN,
  };
  HUD.prototype._dispatchCommandIntent.call(hud, intent, { shiftKey: false });
  HUD.prototype._dispatchCommandIntent.call(hud, intent, { shiftKey: true });
  assert(
    issued[0]?.c === "adjustProductionRepeat" && issued[0]?.delta === 1 &&
      issued[1]?.c === "adjustProductionRepeat" && issued[1]?.delta === -1,
    "Alt production-repeat actions add one allocation while Shift removes one",
  );
}

{
  assert(formatGameTime(null) === "00:00", "HUD game timer treats missing ticks as match start");
  assert(formatGameTime(-90) === "00:00", "HUD game timer clamps negative ticks");
  assert(formatGameTime(TICK_HZ - 1) === "00:00", "HUD game timer floors partial seconds");
  assert(formatGameTime(TICK_HZ) === "00:01", "HUD game timer formats one elapsed second");
  assert(formatGameTime(TICK_HZ * 60) === "01:00", "HUD game timer formats minutes");
  assert(formatGameTime(TICK_HZ * 3661) === "1:01:01", "HUD game timer formats hours");

  let text = "";
  const timerEl = {
    title: "",
    textWrites: 0,
    get textContent() {
      return text;
    },
    set textContent(value) {
      this.textWrites += 1;
      text = String(value);
    },
  };
  const state = { tick: 0 };
  const hud = new HUD({
    querySelector(selector) {
      return selector === "#game-timer" ? timerEl : null;
    },
  }, state, {}, null);
  assert(timerEl.textContent === "00:00", "HUD initializes the minimap game timer");
  const initialWrites = timerEl.textWrites;
  hud._renderGameTimer();
  assert(timerEl.textWrites === initialWrites, "HUD game timer skips duplicate DOM writes");
  state.tick = TICK_HZ * 125;
  hud._renderGameTimer();
  assert(timerEl.textContent === "02:05", "HUD game timer updates from authoritative state tick");
  assert(timerEl.title === "Game time 02:05", "HUD game timer title mirrors the visible time");
  state.tick = TICK_HZ * 65;
  hud._renderGameTimer();
  assert(timerEl.textContent === "01:05", "HUD game timer can move backward for replay seeks");
  hud.destroy();
  assert(timerEl.textContent === "00:00", "HUD destroy resets the persistent minimap game timer DOM");
  assert(timerEl.title === "Game time 00:00", "HUD destroy resets the minimap game timer title");
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

  {
    const ownResearchState = {
      playerId: 1,
      upgrades: [UPGRADE.ENTRENCHMENT],
      isOwnOwner(owner) {
        return owner === 1;
      },
    };
    const ownStatus = entrenchmentSelectionStatus(
      { id: 2300, owner: 1, kind: KIND.RIFLEMAN },
      ownResearchState,
    );
    const occupiedStatus = entrenchmentSelectionStatus(
      { id: 2301, owner: 2, kind: KIND.MACHINE_GUNNER, occupiedTrenchId: 81 },
      { playerId: 1, upgrades: [] },
    );
    const unsupportedStatus = entrenchmentSelectionStatus(
      { id: 2302, owner: 1, kind: KIND.TANK },
      ownResearchState,
    );
    const workerStatus = entrenchmentSelectionStatus(
      { id: 2304, owner: 1, kind: KIND.WORKER },
      ownResearchState,
    );
    assert(ownStatus?.value === "Hold still 3s to dig", "HUD status explains researched dig-in availability");
    assert(
      occupiedStatus?.value.includes("+1 range") &&
        occupiedStatus.value.includes("-50% direct") &&
        occupiedStatus.value.includes("-25% blast"),
      "HUD status summarizes occupied-trench combat benefits",
    );
    assert(unsupportedStatus === null, "HUD status omits excluded units");
    assert(workerStatus === null, "HUD status omits Engineers from entrenchment");
  }

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const selected = { id: 2303, owner: 1, kind: KIND.RIFLEMAN, hp: 40, maxHp: 40 };
    const state = {
      playerId: 1,
      upgrades: [],
      selectionBudgetOverflow: null,
      selectedEntities() {
        return [selected];
      },
      isOwnOwner(owner) {
        return owner === 1;
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    assert(panel.children[0].innerHTML.includes("Can use existing trenches"), "HUD shows neutral trench reuse before research");
    const reuseNode = panel.children[0];

    state.upgrades = [UPGRADE.ENTRENCHMENT];
    hud._renderSelectedPanel();
    assert(panel.children[0] !== reuseNode, "HUD selected detail refreshes when Entrenchment research completes");
    assert(panel.children[0].innerHTML.includes("Hold still 3s to dig"), "HUD shows dig-in text after research");

    selected.occupiedTrenchId = 82;
    hud._renderSelectedPanel();
    assert(panel.children[0].innerHTML.includes("Occupied: +1 range"), "HUD shows occupied trench benefits from server state");
  });

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const selected = {
      id: 2306,
      owner: 1,
      kind: KIND.BARRACKS,
      hp: 500,
      maxHp: 500,
      prodQueue: 1,
      prodKind: KIND.RIFLEMAN,
      prodProgress: 0,
      prodWaiting: true,
    };
    const state = {
      playerId: 1,
      selectionBudgetOverflow: null,
      selectedEntities() {
        return [selected];
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    assert(panel.children[0].innerHTML.includes("waiting for resources / supply"), "HUD labels unpaid production");
    const waitingNode = panel.children[0];

    selected.prodWaiting = false;
    hud._renderSelectedPanel();
    assert(panel.children[0] !== waitingNode, "HUD selected detail refreshes when production pays before rounded progress changes");
    assert(!panel.children[0].innerHTML.includes("waiting for resources / supply"), "HUD removes the waiting label after payment");
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

  const p2CityCentre = { id: 701, owner: 2, kind: KIND.CITY_CENTRE, buildProgress: null };
  const p2TrainingCentre = { id: 702, owner: 2, kind: KIND.TRAINING_CENTRE, buildProgress: null };
  const p2Barracks = { id: 703, owner: 2, kind: KIND.BARRACKS, buildProgress: null };
  const p2Steelworks = { id: 704, owner: 2, kind: KIND.STEELWORKS, buildProgress: null };
  const p2Entities = [p2CityCentre, p2TrainingCentre, p2Barracks, p2Steelworks];
  let p2Selection = [p2Barracks];
  const p2LabHudState = {
    playerId: 1,
    spectator: true,
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 50 },
    playerResources: [
      { id: 1, steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 50 },
      { id: 2, steel: 25, oil: 0, supplyUsed: 4, supplyCap: 50 },
    ],
    upgrades: [],
    playerUpgrades: new Map([[2, [UPGRADE.ANTI_TANK_GUN_UNLOCK]]]),
    players: [
      { id: 1, teamId: 1, factionId: DEFAULT_FACTION_ID },
      { id: 2, teamId: 2, factionId: DEFAULT_FACTION_ID },
    ],
    selectedEntities() {
      return p2Selection;
    },
    entitiesInterpolated() {
      return p2Entities;
    },
  };
  const p2Hud = new HUD({ querySelector: () => null }, p2LabHudState, {}, null, null, null, labPolicy);
  const p2BarracksCard = buildCommandCardDescriptors(
    p2Hud._commandDescriptorContext({ selectedEntities: p2Selection, currentEntities: p2Entities }),
  );
  assert(
    buttonByLabel(p2BarracksCard, "Rifleman").unaffordable,
    "lab command card affordability uses the selected owner's resources instead of the viewer resources",
  );
  p2Selection = [p2Steelworks];
  p2LabHudState.playerResources[1] = { id: 2, steel: 1000, oil: 1000, supplyUsed: 4, supplyCap: 50 };
  const p2SteelworksCard = buildCommandCardDescriptors(
    p2Hud._commandDescriptorContext({ selectedEntities: p2Selection, currentEntities: p2Entities }),
  );
  assert(
    buttonByLabel(p2SteelworksCard, "Anti-Tank Gun").enabled,
    "lab command card tech checks use selected-owner upgrades when per-owner upgrade data is present",
  );

  const p1CommandCar = { id: 705, owner: 1, kind: KIND.COMMAND_CAR };
  const p1LabHudState = {
    playerId: 1,
    spectator: true,
    resources: { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 },
    playerResources: [
      { id: 1, steel: 99999, oil: 99999, supplyUsed: 6, supplyCap: 50 },
      { id: 2, steel: 99999, oil: 99999, supplyUsed: 6, supplyCap: 50 },
    ],
    players: p2LabHudState.players,
    selectedEntities() {
      return [p1CommandCar];
    },
    entitiesInterpolated() {
      return [p1CommandCar];
    },
  };
  const p1Hud = new HUD({ querySelector: () => null }, p1LabHudState, {}, null, null, null, labPolicy);
  const p1CommandCarCard = buildCommandCardDescriptors(
    p1Hud._commandDescriptorContext({ selectedEntities: [p1CommandCar], currentEntities: [p1CommandCar] }),
  );
  assert(
    buttonByLabel(p1CommandCarCard, "Scout Plane").enabled,
    "blue-player Scout Plane affordability uses its authoritative lab resource row",
  );
  p1LabHudState.playerResources = [p1LabHudState.playerResources[1]];
  const missingP1ResourceCard = buildCommandCardDescriptors(
    p1Hud._commandDescriptorContext({ selectedEntities: [p1CommandCar], currentEntities: [p1CommandCar] }),
  );
  assert(
    !buttonByLabel(missingP1ResourceCard, "Scout Plane").enabled,
    "blue-player affordability never borrows the orange player's resource row",
  );

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
  assert(buildCard.slots[0].enabled, "unaffordable build buttons enter placement and wait at the site");
  assert(buildCard.slots[1] == null, "worker build menu keeps the former Supply Depot W slot empty");
  assert(buildCard.slots[3].label === "Training Centre", "worker build menu should include Training Centre");
  assert(!buildCard.slots[3].enabled, "locked build buttons should be disabled");
  assert(buildCard.slots[3].title === "Requires Barracks", "locked build tooltip should explain requirement");
  assert(buildCard.slots[7].label === "Tank Trap", "worker build menu should include Tank Trap");
  assert(!buildCard.slots[7].enabled, "Tank Trap should require a completed Training Centre");
  assert(buildCard.slots[7].title === "Requires Training Centre", "Tank Trap tooltip should explain its requirement");
  assert(buildCard.slots[8].intent.type === "closeCommandCardMenu", "worker return button should close submenu");
  assert(buildCard.slots[8].commandId === "worker.return", "worker return should expose stable command identity");

  const unfinishedDepot = {
    id: 17,
    owner: 1,
    kind: KIND.DEPOT,
    buildProgress: 0.45,
  };
  const constructionCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [unfinishedDepot],
    entities: [unfinishedDepot],
  }));
  assert(constructionCard.kind === "construction", "unfinished buildings should use the construction card");
  assert(commandButtons(constructionCard).length === 1, "construction card should expose only cancellation");
  assert(constructionCard.slots[8].label === "Cancel", "construction cancel should occupy the bottom-right slot");
  assert(constructionCard.slots[8].hotkey === "C", "construction cancel should use the bottom-right C hotkey");
  assert(constructionCard.slots[8].commandId === "construction.cancel", "construction cancel should have a stable command identity");
  assert(
    constructionCard.slots[8].intent.type === "cancelConstruction" &&
      constructionCard.slots[8].intent.buildingId === unfinishedDepot.id,
    "construction cancel should target the selected scaffold",
  );
  assert(
    constructionCard.slots[8].title.includes("full refund"),
    "construction cancel should explain its refund behavior",
  );

  const scoutPlane = { id: 18, owner: 1, kind: KIND.SCOUT_PLANE };
  const scoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [scoutPlane],
    entities: [scoutPlane],
  }));
  assert(!scoutPlaneCard.slots.some(Boolean), "Scout Plane-only selections are not commandable");
  assert(!buttonByAction(scoutPlaneCard, "move"), "Scout Plane command card does not expose move");
  assert(!buttonByAction(scoutPlaneCard, "ability"), "Scout Plane command card does not expose abilities");

  const workerScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [worker, scoutPlane],
    entities: [worker, cityCentre, scoutPlane],
  }));
  assert(
    buttonByAction(workerScoutPlaneCard, "move")?.enabled &&
      buttonByAction(workerScoutPlaneCard, "holdPosition")?.intent.unitIds.join(",") === "10" &&
      buttonByAction(workerScoutPlaneCard, "attack")?.intent.target === "attack" &&
      buttonByAction(workerScoutPlaneCard, "stop")?.intent.unitIds.join(",") === "10" &&
      buttonByAction(workerScoutPlaneCard, "openWorkerBuildMenu")?.enabled,
    "mixed Worker plus Scout Plane selection ignores the plane and preserves normal worker commands",
  );
  assert(!buttonByAction(workerScoutPlaneCard, "dismissScoutPlane"), "mixed Worker plus Scout Plane selection does not expose dismiss");

  const scoutPlaneMixedArtillery = {
    id: 19,
    owner: 1,
    kind: KIND.ARTILLERY,
    setupState: SETUP.DEPLOYED,
    abilities: [
      { ability: ABILITY.POINT_FIRE, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.BLANKET_FIRE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  const artilleryScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [scoutPlaneMixedArtillery, scoutPlane],
    entities: [scoutPlaneMixedArtillery, scoutPlane],
  }));
  assert(
    buttonByAction(artilleryScoutPlaneCard, "setupAntiTankGuns")?.slotIndex === 6 &&
      buttonByLabel(artilleryScoutPlaneCard, "Point Fire")?.slotIndex === 7 &&
      buttonByLabel(artilleryScoutPlaneCard, "Blanket Fire")?.slotIndex === 8,
    "mixed Artillery plus Scout Plane selection ignores the plane and preserves support-weapon controls",
  );
  assert(!buttonByAction(artilleryScoutPlaneCard, "dismissScoutPlane"), "mixed support-weapon plus Scout Plane selection does not expose dismiss");

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
    resources: { steel: 60, oil: 10 },
  }));
  assert(trainCard.kind === "train", "production building should use train descriptor card");
  assert(trainCard.slots[0].label === "Rifleman", "Barracks first train slot should be Rifleman");
  assert(trainCard.slots[0].commandId === defaultFactionCommandId("train", KIND.RIFLEMAN), "train button should expose stable train identity");
  assert(trainCard.slots[0].slotIndex === 0, "train button should expose rendered slot index");
  assert(trainCard.slots[0].repeatable, "train hotkeys should remain repeatable");
  assert(trainCard.slots[0].intent.type === "train", "train button should carry train intent");
  assert(
    trainCard.slots[0].contextIntent.type === "adjustProductionRepeat" &&
      trainCard.slots[0].contextIntent.buildingIds.join(",") === "20,21" &&
      trainCard.slots[0].contextIntent.unit === KIND.RIFLEMAN &&
      trainCard.slots[0].contextHotkeyModifiers.join(",") === "alt,ctrl,shift",
    "train buttons should adjust repeat production across selected compatible producers",
  );
  assert(
    trainCard.slots[0].countBadge === "0/2" && trainCard.slots[0].autobuildIndicatorCount === 0,
    "train buttons should show the selected producer allocation count",
  );
  assert(trainCard.slots[8].label === "Cancel", "production cancel should stay in C slot");
  assert(trainCard.slots[8].hotkey === "C", "cancel hotkey should stay C");
  assert(trainCard.slots[8].repeatable, "cancel hotkey should remain repeatable");
  assert(trainCard.slots[8].commandId === `production.cancel.${KIND.BARRACKS}`, "cancel button should expose stable production cancel identity");
  assert(trainCard.slots[1].label === "Machine Gunner", "Barracks second train slot should be Machine Gunner");
  assert(!trainCard.slots[1].enabled, "requirement-gated train button should be disabled");
  assert(
    trainCard.slots[1].title.startsWith("Requires Training Centre") &&
      trainCard.slots[1].title.includes("Shift+hotkey removes one"),
    "train locked tooltip should name its requirement and allocation controls",
  );
  assert(trainCard.slots[2] == null, "Barracks should no longer expose a standalone Panzerfaust unit");

  producingBarracks.prodRepeatKinds = [KIND.RIFLEMAN, KIND.MACHINE_GUNNER];
  const repeatingTrainCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [barracks, producingBarracks],
    entities: [cityCentre, barracks, producingBarracks],
    resources: { steel: 60, oil: 0 },
  }));
  assert(
    repeatingTrainCard.slots[0].cls.includes("autocast-enabled") &&
      repeatingTrainCard.slots[0].countBadge === "1/2" &&
      repeatingTrainCard.slots[0].autobuildIndicatorCount === 1,
    "authoritative repeat production should show one swirl and its partial allocation count",
  );
  delete producingBarracks.prodRepeatKinds;

  const supplyReservedTrainCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [cityCentre],
    entities: [cityCentre],
    resources: { steel: 100, oil: 0, supplyUsed: 9, supplyCap: 10 },
    optimisticProduction: [{ building: cityCentre.id, unit: KIND.WORKER, optimisticQueue: 1 }],
  }));
  assert(supplyReservedTrainCard.slots[0].enabled, "pending train optimism should still permit another manual queue entry");
  assert(supplyReservedTrainCard.slots[0].unaffordable, "supply-blocked train button should stay clickable for feedback");
  assert(supplyReservedTrainCard.slots[0].title.startsWith("Queue now; production waits for supply"), "supply-blocked train tooltip should explain waiting");
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
  assert(steelReservedTrainCard.slots[0].enabled, "pending train optimism should still permit another manual queue entry");
  assert(steelReservedTrainCard.slots[0].title.startsWith("Queue now; production waits for resources"), "resource-blocked train tooltip should explain waiting");

  const scoutPlaneCityCentre = { id: 70, owner: 1, kind: KIND.CITY_CENTRE, buildProgress: null };
  const cityCentreScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [scoutPlaneCityCentre],
    entities: [scoutPlaneCityCentre],
    resources: { steel: 50, oil: 75, supplyUsed: 0, supplyCap: 10 },
  }));
  assert(!cityCentreScoutPlaneCard.slots.some((slot) => slot?.label === "Scout Plane"), "City Centre no longer exposes Scout Plane training");

  const commandCar = {
    id: 74,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    abilities: [
      { ability: ABILITY.BREAKTHROUGH, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.SCOUT_PLANE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  const commandCarScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [commandCar],
    entities: [scoutPlaneCityCentre, commandCar],
    resources: { steel: 50, oil: 75, supplyUsed: 0, supplyCap: 10 },
  }));
  const scoutPlaneAbility = buttonByLabel(commandCarScoutPlaneCard, "Scout Plane");
  assert(scoutPlaneAbility.slotIndex === 8, "Command Car Scout Plane ability should use the C slot");
  assert(scoutPlaneAbility.commandId === defaultFactionCommandId("ability", ABILITY.SCOUT_PLANE), "Scout Plane ability should expose stable ability identity");
  assert(scoutPlaneAbility.intent.type === "ability", "Scout Plane button should arm an ability target");
  assert(scoutPlaneAbility.intent.targetMode === "worldPoint", "Scout Plane ability should target a world point");
  assert(scoutPlaneAbility.intent.readyIds.join(",") === "74", "Scout Plane ability should use the selected Command Car");
  assert(scoutPlaneAbility.cost.steel === 50 && scoutPlaneAbility.cost.oil === 75, "Scout Plane ability should show 50/75 cost");
  assert(scoutPlaneAbility.enabled, "Scout Plane ability should enable with sufficient resources");

  const noCityCentreScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [commandCar],
    entities: [commandCar],
    resources: { steel: 50, oil: 75, supplyUsed: 0, supplyCap: 10 },
    playerHasCompleteKind: (kind) => kind === KIND.FACTORY,
  }));
  const noCityCentreScoutPlane = buttonByLabel(noCityCentreScoutPlaneCard, "Scout Plane");
  assert(noCityCentreScoutPlane.enabled, "Scout Plane ability should not require a completed City Centre");

  const oilBlockedScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [commandCar],
    entities: [scoutPlaneCityCentre, commandCar],
    resources: { steel: 50, oil: 74, supplyUsed: 0, supplyCap: 10 },
  }));
  const oilBlockedScoutPlane = buttonByLabel(oilBlockedScoutPlaneCard, "Scout Plane");
  assert(!oilBlockedScoutPlane.enabled, "Scout Plane ability should disable when oil is short");
  assert(oilBlockedScoutPlane.unaffordable, "resource-blocked Scout Plane ability should stay clickable for feedback");
  assert(oilBlockedScoutPlane.title === "Not enough resources", "Scout Plane resource-blocked tooltip should name resources");

  const activeScoutPlane = {
    id: 75,
    owner: 1,
    kind: KIND.SCOUT_PLANE,
    x: 320,
    y: 448,
    scoutPlane: { sourceCommandCar: commandCar.id },
  };
  const activeScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [commandCar],
    entities: [scoutPlaneCityCentre, commandCar, activeScoutPlane],
    resources: { steel: 50, oil: 75, supplyUsed: 0, supplyCap: 10 },
  }));
  const activeBlockedScoutPlane = buttonByLabel(activeScoutPlaneCard, "Scout Plane");
  assert(!activeBlockedScoutPlane.enabled, "Scout Plane ability should disable while that Command Car's plane is active");
  assert(activeBlockedScoutPlane.title === "Scout Plane already active", "active Scout Plane block should explain the per-car limit");

  const secondCommandCar = {
    ...commandCar,
    id: 76,
  };
  const mixedScoutPlaneCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [commandCar, secondCommandCar],
    entities: [scoutPlaneCityCentre, commandCar, secondCommandCar, activeScoutPlane],
    resources: { steel: 50, oil: 75, supplyUsed: 0, supplyCap: 10 },
  }));
  const mixedScoutPlaneAbility = buttonByLabel(mixedScoutPlaneCard, "Scout Plane");
  assert(mixedScoutPlaneAbility.enabled, "another selected Command Car can launch while the first car's plane is active");
  assert(mixedScoutPlaneAbility.intent.readyIds.join(",") === "76", "Scout Plane targeting excludes only the source car with an active plane");

  const scoutCar = {
    id: 30,
    owner: 1,
    kind: KIND.SCOUT_CAR,
    abilities: [{ ability: ABILITY.SMOKE, cooldownLeft: 0, remainingUses: 2 }],
  };
  const smokeResearchComplex = {
    id: 29,
    owner: 1,
    kind: KIND.RESEARCH_COMPLEX,
  };
  const lockedAbilityCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [scoutCar],
    entities: [{ ...smokeResearchComplex, buildProgress: 0.5 }, scoutCar],
  }));
  const lockedSmoke = buttonByAction(lockedAbilityCard, "ability");
  assert(!lockedSmoke.enabled, "Smoke should be disabled until the player completes an R&D Complex");
  assert(lockedSmoke.title === "Requires R&D Complex", "locked Smoke should explain its R&D requirement");
  assert(
    lockedSmoke.tooltipHtml.includes("Requires R&D Complex"),
    "locked Smoke hover content should explain its R&D requirement",
  );
  const abilityCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [scoutCar],
    entities: [smokeResearchComplex, scoutCar],
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
    abilities: [
      { ability: ABILITY.POINT_FIRE, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.BLANKET_FIRE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  const pointFireCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [artillery],
    entities: [{ id: 40, owner: 1, kind: KIND.STEELWORKS }, artillery],
    resources: { steel: 0, oil: 0 },
  }));
  const pointFire = buttonByLabel(pointFireCard, "Point Fire");
  assert(pointFire.unaffordable, "unaffordable ability should stay clickable");
  assert(pointFire.onUnavailableIntent.type === "playNotEnough", "unaffordable ability should play resource notice");
  const blanketFire = buttonByLabel(pointFireCard, "Blanket Fire");
  assert(blanketFire.unaffordable, "Blanket Fire shares artillery ammunition affordability");
  assert(blanketFire.intent.targetMode === "worldPoint", "Blanket Fire always arms a world-point target");

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
  const packedBlanketFire = buttonByLabel(packedPointFireCard, "Blanket Fire");
  assert(packedPointFire.enabled, "packed artillery Point Fire should be enabled for auto-setup");
  assert(packedBlanketFire.enabled, "packed artillery Blanket Fire should be enabled for auto-setup");
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
  assert(antiTankGun.title.startsWith("Requires research in R&D Complex"), "upgrade-gated unit tooltip should name R&D research");
  assert(!buttonByLabel(upgradeCard, "Medium Guns"), "Gun Works should not expose R&D research");
  assert(!buttonByLabel(upgradeCard, "Heavy Guns"), "Gun Works should not expose R&D research");

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
  const mediumGuns = buttonByLabel(researchCard, "Medium Guns");
  assert(mediumGuns && mediumGuns.enabled, "available affordable upgrade should be enabled");
  assert(mediumGuns.commandId === defaultFactionCommandId("research", UPGRADE.ANTI_TANK_GUN_UNLOCK), "research button should expose stable research identity");
  assert(mediumGuns.intent.type === "research", "upgrade button should carry research intent");
  assert(!buttonByLabel(researchCard, "Heavy Guns"), "R&D should hide Heavy Guns until Medium Guns is researched");
  assert(!buttonByLabel(researchCard, "Unlock Artillery"), "R&D should not expose a separate Artillery unlock");

  const heavyGunsCard = buildCommandCardDescriptors(commandCardCtx({
    selection: [researchComplex],
    entities: [
      { id: 51, owner: 1, kind: KIND.CITY_CENTRE },
      { id: 52, owner: 1, kind: KIND.TRAINING_CENTRE },
      researchComplex,
    ],
    resources: { steel: 500, oil: 500 },
    upgrades: [UPGRADE.ANTI_TANK_GUN_UNLOCK],
  }));
  const heavyGuns = buttonByLabel(heavyGunsCard, "Heavy Guns");
  assert(heavyGuns && heavyGuns.enabled, "Heavy Guns should replace Medium Guns after Medium is researched");
  assert(heavyGuns.slotIndex === mediumGuns.slotIndex, "Heavy Guns should reuse the Medium Guns button slot");
  assert(heavyGuns.commandId === defaultFactionCommandId("research", UPGRADE.ARTILLERY_UNLOCK), "Heavy Guns should use the artillery unlock identity");

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
    const twoButton = HUD.prototype._cmdButton({
      icon: "RF",
      label: "Rifleman",
      enabled: true,
      autobuildIndicatorCount: 2,
      onClick() {},
    });
    assert(
      (twoButton.innerHTML.match(/cmd-autobuild-swirls/g) || []).length === 1 &&
        twoButton.innerHTML.includes("--autobuild-segment:180.000deg") &&
        twoButton.innerHTML.includes("--autobuild-peak:52.000deg") &&
        twoButton.innerHTML.includes("--autobuild-fade:104.000deg"),
      "two auto-build allocations share one bounded layer with indicators 180 degrees apart",
    );
    const threeButton = HUD.prototype._cmdButton({
      icon: "RF",
      label: "Rifleman",
      enabled: true,
      autobuildIndicatorCount: 3,
      onClick() {},
    });
    assert(
      (threeButton.innerHTML.match(/cmd-autobuild-swirls/g) || []).length === 1 &&
        threeButton.innerHTML.includes("--autobuild-segment:120.000deg") &&
        threeButton.innerHTML.includes("--autobuild-peak:52.000deg") &&
        threeButton.innerHTML.includes("--autobuild-fade:104.000deg"),
      "three auto-build allocations share one bounded layer with indicators 120 degrees apart",
    );
    const manyButton = HUD.prototype._cmdButton({
      icon: "RF",
      label: "Rifleman",
      enabled: true,
      autobuildIndicatorCount: 10,
      onClick() {},
    });
    assert(
      (manyButton.innerHTML.match(/cmd-autobuild-swirls/g) || []).length === 1 &&
        manyButton.innerHTML.includes("--autobuild-segment:36.000deg") &&
        manyButton.innerHTML.includes("--autobuild-peak:16.200deg") &&
        manyButton.innerHTML.includes("--autobuild-fade:32.400deg"),
      "large allocation counts keep one animation layer and shrink trails to remain distinct",
    );
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

  withFakeDocument(() => {
    const button = HUD.prototype._cmdButton({
      icon: "SMK",
      label: "Smoke",
      enabled: false,
      title: "Requires R&D Complex",
      tooltipHtml:
        `<span class="cmd-tooltip-title">Smoke</span>` +
        `<span class="cmd-tooltip-desc">Requires R&D Complex</span>`,
      onClick() {},
    });
    assert(button.disabled, "locked Smoke command should remain disabled");
    assert(
      button.innerHTML.includes("Requires R&D Complex"),
      "locked Smoke button should render its requirement in visible hover content",
    );
  });

  withFakeDocument(() => {
    let primaryClicks = 0;
    let repeatToggles = 0;
    const button = HUD.prototype._cmdButton({
      icon: "TK",
      label: "Tank",
      hotkey: "W",
      enabled: false,
      title: "Requires Heavy Guns",
      onAltClick: () => { repeatToggles += 1; },
      onClick: () => { primaryClicks += 1; },
    });
    assert(!button.disabled, "locked commands with a secondary action should accept pointer input");
    assert(
      button.className.includes("primary-disabled"),
      "secondary-only commands should retain disabled primary-action styling",
    );
    button.listeners.click({ altKey: false, preventDefault() {} });
    assert(primaryClicks === 0 && repeatToggles === 0, "ordinary clicks should not invoke a locked primary action");
    button.listeners.click({ altKey: true, preventDefault() {} });
    assert(primaryClicks === 0 && repeatToggles === 1, "Alt-click should invoke a locked command's secondary action");
  });
}
