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
} from "../../client/src/config.js";
import {
  HUD,
  groupCooldownClocks,
  playerHasCompletedKind,
  selectionBudgetBlockShape,
  selectionBudgetGridModel,
} from "../../client/src/hud.js";
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
