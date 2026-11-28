// Selection-panel grid, paging, and DOM interaction contracts.

import { assert } from "./assertions.mjs";
import { withFakeHudDocument } from "./fakes.mjs";
import {
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  STATS,
} from "../../client/src/config.js";
import {
  HUD,
  selectionBudgetBlockShape,
  selectionBudgetGridModel,
} from "../../client/src/hud.js";
import { KIND } from "../../client/src/protocol.js";
import { HudSelectionPanel } from "../../client/src/hud_selection_panel.js";
import { buildControlGroupSummaries } from "../../client/src/hud_control_groups.js";

withFakeHudDocument(({ FakeElement }) => {
  const players = [{ id: 1, factionId: "cultivators" }];
  for (const [dao, label] of [[null, "Sword Disciple"], ["wind", "Wind Adept"],
    ["war", "War Adept"], ["magic", "Magic Adept"]]) {
    const entity = { id: 1, owner: 1, kind: ({ wind: KIND.WIND_ADEPT, war: KIND.WAR_ADEPT, magic: KIND.MAGIC_ADEPT })[dao] || KIND.WARRIOR, dao };
    const state = {
      players,
      controlGroups: [[1]],
      controlGroupEntities: () => [entity],
    };
    const selectionPanel = new HudSelectionPanel(new FakeElement("section"), state, null, () => "");
    const node = selectionPanel._singleSelectionNode(entity);
    assert(node.innerHTML.includes(`<div class="sel-name">${label}</div>`),
      `single selection names ${label}`);
    selectionPanel._selectionEntityNode(node, entity);
    assert(node.getAttribute("aria-label") === `Selected ${label}`,
      `selection accessibility names ${label}`);
    assert(selectionBudgetGridModel([entity], null, players).blocks[0].label === label,
      `multi-selection names ${label}`);
    assert(buildControlGroupSummaries(state)[0].label === label,
      `control-group summary names ${label}`);
    selectionPanel.destroy();
  }
});

withFakeHudDocument(({ FakeElement }) => {
  const panel = new FakeElement("section");
  const selected = [
    { id: 1, owner: 1, kind: KIND.WARRIOR },
    { id: 2, owner: 1, kind: KIND.WARRIOR },
  ];
  const portraits = [];
  const selectionPanel = new HudSelectionPanel(panel, {
    players: [], selectedEntities: () => selected,
  }, null, (kind, { dao }) => {
    portraits.push(dao);
    return `<span>${dao || "sword"}</span>`;
  });
  selectionPanel.render();
  selectionPanel.render();
  assert(portraits.length === 2, "unchanged multi-selection reuses its portraits");
  selected[0].dao = "wind";
  selected[0].kind = KIND.WIND_ADEPT;
  selectionPanel.render();
  assert(portraits.length === 4 && portraits[2] === "wind",
    "evolution refreshes the selected group's portraits without reselection");
  selected[1].dao = "magic";
  selected[1].kind = KIND.MAGIC_ADEPT;
  selectionPanel.render();
  assert(portraits.length === 6 && portraits[5] === "magic",
    "each selected Disciple's Dao participates in portrait cache invalidation");
  selectionPanel.destroy();
});

// Rank is part of the displayed type: a minority Master must not name an Adept group.
for (const masterFirst of [true, false]) {
  const adept = { owner: 1, kind: KIND.WAR_ADEPT, dao: "war" };
  const master = { ...adept, daoMaster: true };
  for (const [entities, label] of [
    [masterFirst ? [master, adept, adept] : [adept, master, adept], "War Adept"],
    [masterFirst ? [master, adept, master] : [adept, master, master], "War Master"],
  ]) {
    const summary = buildControlGroupSummaries({
      players: [{ id: 1, factionId: "cultivators" }],
      controlGroups: [[1, 2, 3]], controlGroupEntities: () => entities,
    })[0];
    assert(summary.label === label, "control-group label reflects the dominant rank");
  }
}

// ---------------------------------------------------------------------------
// HUD selection budget grid
// ---------------------------------------------------------------------------
{
  const riflemen = Array.from({ length: 25 }, (_, index) => ({
    id: 1000 + index,
    owner: 1,
    kind: KIND.RIFLEMAN,
  }));
  const tanks = Array.from({ length: 5 }, (_, index) => ({
    id: 1100 + index,
    owner: 1,
    kind: KIND.TANK,
  }));
  const commandCar = { id: 1200, owner: 1, kind: KIND.COMMAND_CAR };
  const artillery = { id: 1300, owner: 1, kind: KIND.ARTILLERY };

  const infantryModel = selectionBudgetGridModel(riflemen);
  assert(infantryModel.used === 25 && infantryModel.cap === BASE_COMMAND_SUPPLY_CAP, "HUD budget grid reports 25/25 infantry supply");
  assert(infantryModel.cols === 13 && infantryModel.rows === 4,
    "HUD base budget grid uses four rows of thirteen large cells");
  assert(infantryModel.blocks.every((block) => block.weight === 1 && block.cols === 1 && block.rows === 1 && block.placed),
    "HUD infantry blocks occupy one fixed cell each");

  const tankModel = selectionBudgetGridModel(tanks);
  assert(tankModel.used === 25 && tankModel.cap === BASE_COMMAND_SUPPLY_CAP, "HUD budget grid reports five Tanks as 25/25");
  assert(tankModel.blocks.every((block) => block.weight === 5 && block.cols === 3 && block.rows === 2 && block.placed),
    "HUD Tank blocks occupy a two-row by three-column shape");

  const commandCarModel = selectionBudgetGridModel(tanks.concat(commandCar));
  assert(commandCarModel.used === 25 &&
    commandCarModel.cap === BASE_COMMAND_SUPPLY_CAP + COMMAND_CAR_SUPPLY_CAP_BONUS,
    "HUD budget grid shows zero command cost and no cap increase for Command Car");
  assert(commandCarModel.cols === 13 && commandCarModel.rows === 4 && commandCarModel.pages.length === 1,
    "HUD four-row grid holds a five Tanks and a free Command Car without shrinking");
  assert(commandCarModel.blocks.every((block) => block.placed),
    "HUD paginated budget grid places every block without fallback overlap");

  const diversityPagedSelection = [
    ...Array.from({ length: 6 }, (_, index) => ({
      id: 1400 + index,
      owner: 1,
      kind: KIND.TANK,
    })),
    { id: 1410, owner: 1, kind: KIND.COMMAND_CAR },
    { id: 1411, owner: 1, kind: KIND.COMMAND_CAR },
    { id: 1412, owner: 1, kind: KIND.ARTILLERY },
    { id: 1413, owner: 1, kind: KIND.MORTAR_TEAM },
    { id: 1414, owner: 1, kind: KIND.RIFLEMAN },
  ];
  const diversityModel = selectionBudgetGridModel(diversityPagedSelection);
  const selectedKinds = new Set(diversityPagedSelection.map((entity) => entity.kind));
  const firstPageKinds = new Set(diversityModel.pages[0].blocks.map((block) => block.kind));
  assert(diversityModel.pages.length === 2 &&
    [...selectedKinds].every((kind) => firstPageKinds.has(kind)),
    "HUD first page reserves one representative of every selected unit kind before duplicates");

  const manyUniqueKinds = Array.from({ length: 60 }, (_, index) => ({
    id: 1500 + index,
    owner: 1,
    kind: `test_unique_kind_${index}`,
  }));
  const manyUniqueModel = selectionBudgetGridModel(manyUniqueKinds);
  assert(manyUniqueModel.pages.length === 2 && manyUniqueModel.pages[1].blocks.length === 8,
    "HUD packs representatives that overflow the first page together before placing duplicates");

  const mixedSelection = [
    ...Array.from({ length: 6 }, (_, index) => ({
      id: 1250 + index,
      owner: 1,
      kind: KIND.RIFLEMAN,
    })),
    { id: 1260, owner: 1, kind: KIND.MORTAR_TEAM },
    { id: 1261, owner: 1, kind: KIND.MORTAR_TEAM },
    { id: 1262, owner: 1, kind: KIND.ARTILLERY },
    { id: 1263, owner: 1, kind: KIND.TANK },
  ];
  const mixedModel = selectionBudgetGridModel(mixedSelection);
  const occupiedCells = new Set();
  let hasOverlap = false;
  for (const block of mixedModel.blocks) {
    for (let row = block.row; row < block.row + block.rows; row++) {
      for (let col = block.col; col < block.col + block.cols; col++) {
        const key = `${block.page}:${row}:${col}`;
        if (occupiedCells.has(key)) hasOverlap = true;
        occupiedCells.add(key);
      }
    }
  }
  assert(mixedModel.pages.length === 1 && mixedModel.blocks.every((block) => block.placed) && !hasOverlap,
    "HUD packs late large units ahead of infantry without overlapping the mixed 21-supply selection");

  const artilleryShape = selectionBudgetBlockShape(STATS[KIND.ARTILLERY].supply);
  assert(artilleryShape.cols === 2 && artilleryShape.rows === 2 && artilleryShape.reservedCells == null,
    "HUD four-supply artillery uses a deterministic two-by-two shape");
  const artilleryModel = selectionBudgetGridModel([artillery]);
  assert(artilleryModel.blocks[0].reservedCells === 0, "HUD four-supply artillery has no reserved visual cell");

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const selectedUnit = {
      id: 999,
      owner: 1,
      kind: KIND.RIFLEMAN,
      hp: 45,
      maxHp: 45,
      unitsKilled: 7,
    };
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const state = {
      selectedEntities() {
        return [selectedUnit];
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    const detail = panel.children[0];
    assert(
      detail?.innerHTML.includes("sel-unit-kills") &&
        detail.innerHTML.includes(">Kills: 7</div>") &&
        !detail.innerHTML.includes("<strong>7</strong>"),
      "HUD shows the authoritative kill total inline for exactly one selected unit",
    );
  });

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    let kind = KIND.TANK;
    const hud = new HUD({ querySelector: () => panel }, {
      selectedEntities: () => [{ id: 999, owner: 1, kind, hp: 100, maxHp: 100, unitsKilled: 0 }],
    }, {}, null);
    for (const [selectedKind, armored] of [
      [KIND.TANK, true], [KIND.ARTILLERY, false], [KIND.RIFLEMAN, false],
      [KIND.SCOUT_CAR, false], [KIND.COMMAND_CAR, false],
    ]) {
      kind = selectedKind;
      hud._renderSelectedPanel();
      const html = panel.children[0].innerHTML;
      assert(html.includes(`>${armored ? "Armored" : "Unarmored"}</abbr>`),
        `${kind} selection reports its armor classification`);
      assert(html.includes(armored ? "75% less HP damage" : "no armor damage reduction") &&
        html.includes('title="') && (!armored || html.includes("Armor-piercing attacks")),
      `${kind} armor hover explains damage protection and penetration`);
      assert(html.indexOf("sel-armor") > html.indexOf("sel-unit-kills"),
        "armor appears beneath the single-unit kill count");
    }
    for (kind of [KIND.STEEL, KIND.BARRACKS]) {
      hud._renderSelectedPanel();
      assert(!panel.children[0].innerHTML.includes("sel-armor"),
        "resource nodes and buildings omit the unit armor row");
    }
  });

  withFakeHudDocument(({ FakeElement }) => {
    const panel = new FakeElement("section");
    const selections = [
      { id: 2001, owner: 0, kind: KIND.STEEL, x: 96, y: 128, hp: 1, maxHp: 1, remaining: 412 },
      { id: 2002, owner: 0, kind: KIND.OIL, x: 160, y: 128, hp: 1, maxHp: 1, remaining: 701 },
      { id: 2003, owner: 1, kind: KIND.STEEL_MINE, x: 96, y: 128, hp: 73, maxHp: 100 },
      { id: 2004, owner: 1, kind: KIND.PUMP_JACK, x: 160, y: 128, hp: 84, maxHp: 100 },
    ];
    let selectionIndex = 0;
    const state = {
      map: { resources: selections.slice(0, 2) },
      selectedEntities() {
        return [selections[selectionIndex]];
      },
    };
    const root = {
      querySelector(selector) {
        return selector === "#selected-panel" ? panel : null;
      },
    };
    const hud = new HUD(root, state, {}, null);

    hud._renderSelectedPanel();
    let detail = panel.children[0]?.innerHTML || "";
    assert(
      detail.includes("Steel Remaining:</span><strong>412</strong>") &&
        !detail.includes("sel-hpbar") && !detail.includes("sel-hptext"),
      "HUD shows remaining Steel but no HP for a selected raw Steel patch",
    );

    selectionIndex = 1;
    hud._renderSelectedPanel();
    detail = panel.children[0]?.innerHTML || "";
    assert(
      detail.includes("Oil Remaining:</span><strong>701</strong>") &&
        !detail.includes("sel-hpbar") && !detail.includes("sel-hptext"),
      "HUD shows remaining Oil but no HP for a selected raw Oil patch",
    );

    selectionIndex = 2;
    hud._renderSelectedPanel();
    detail = panel.children[0]?.innerHTML || "";
    assert(
      detail.includes("Steel Remaining:</span><strong>412</strong>") &&
        detail.includes("sel-hpbar") && detail.includes("73 / 100"),
      "HUD shows both HP and underlying Steel remaining for a selected Steel Mine",
    );

    selectionIndex = 3;
    hud._renderSelectedPanel();
    detail = panel.children[0]?.innerHTML || "";
    assert(
      detail.includes("Oil Remaining:</span><strong>701</strong>") &&
        detail.includes("sel-hpbar") && detail.includes("84 / 100"),
      "HUD shows both HP and underlying Oil remaining for a selected Pump Jack",
    );
  });

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
    assert(grid && grid.style.values.get("--sel-budget-cols") === "13", "HUD renders grid columns into selected panel DOM");
    assert(blocks.length === 5 && blocks.every((block) => block.className.includes("weight-5")),
      "HUD renders five Tank budget blocks into selected panel DOM");
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
    let selected = diversityPagedSelection;
    const state = {
      selectionBudgetOverflow: null,
      selectedEntities() {
        return selected;
      },
    };
    const hud = new HUD(root, state, {}, null);
    hud._renderSelectedPanel();
    const tabs = panel.querySelectorAll(".sel-page-tab");
    assert(tabs.length === 2 && tabs[0].className.includes("active"),
      "HUD renders one clickable tab per fixed-size selection page");
    const renderedFirstPageKinds = new Set(
      panel.querySelectorAll(".sel-budget-block")
        .map((block) => block.getAttribute("data-selection-kind")),
    );
    assert([...selectedKinds].every((kind) => renderedFirstPageKinds.has(String(kind))),
      "HUD first rendered page visibly includes every selected unit kind");

    panel.listeners.click({
      target: tabs[1],
      preventDefault() {},
    });
    const nextTabs = panel.querySelectorAll(".sel-page-tab");
    assert(nextTabs[1].className.includes("active") &&
      panel.querySelectorAll(".sel-budget-block").length > 0,
      "HUD selection tabs switch pages without changing the authoritative selection");

    selected = diversityPagedSelection.map((entity, index) => index === selected.length - 1
      ? { ...entity, id: 1499, kind: KIND.WORKER }
      : entity);
    hud._renderSelectedPanel();
    const resetTabs = panel.querySelectorAll(".sel-page-tab");
    assert(resetTabs[0].className.includes("active"),
      "HUD returns to the diversity-first page when the selected entity set changes");
  });
}

withFakeHudDocument(({ FakeElement }) => {
  const panel = new FakeElement("section");
  const text = (node) => node.innerHTML + [...node.children].map(text).join("");
  const disciple = { id: 1, owner: 1, kind: KIND.WARRIOR, insight: 60, hp: 115, maxHp: 115 };
  const selectionPanel = new HudSelectionPanel(panel, {
    players: [], selectedEntities: () => [disciple],
  }, null, () => "");
  selectionPanel.render();
  assert(text(panel).includes("Insight: 60 / 60"), "base Disciple details show the 60 insight cap");
  disciple.dao = "magic";
  disciple.kind = KIND.MAGIC_ADEPT;
  disciple.insight = 0;
  selectionPanel.render();
  assert(text(panel).includes("Insight: 0 / 200"), "evolution refreshes insight and the Adept cap");
  disciple.insight = 200;
  selectionPanel.render();
  assert(text(panel).includes("Insight: 200 / 200"), "evolved details display the full cap");
  selectionPanel.destroy();
});
