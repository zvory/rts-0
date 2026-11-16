import { KIND } from "../../client/src/protocol.js";
import {
  _drawAboveFogHp,
  _drawSelectionAndHp,
  _hpBar,
  _hpBarSlot,
} from "../../client/src/renderer/entities.js";
import { buildingProgressStatus } from "../../client/src/renderer/entity_state.js";
import { assert } from "./assertions.mjs";
import { installFakePixi } from "./pixi_fakes.mjs";

const restorePixi = installFakePixi();
try {
  const constructionStatus = buildingProgressStatus({
    kind: KIND.STEEL_MINE,
    hp: 5,
    maxHp: 50,
    buildProgress: 0.5,
  });
  assert(
    constructionStatus?.fraction === 0.5,
    "construction status bars follow authoritative build progress instead of scaffold HP",
  );
  const makeBar = () => {
    return _hpBarSlot.call({
      _pools: { hpBars: new Map() },
      _seen: { hpBars: new Set() },
      layers: { hpBars: new PIXI.Container() },
    }, 1);
  };
  const bar = makeBar();
  _hpBar.call({ _map: { tileSize: 32 } }, bar, {
    kind: KIND.RIFLEMAN,
    owner: 1,
    x: 100,
    y: 120,
    hp: 20,
    maxHp: 40,
  }, null, 0x4878c8);
  assert(bar.rtsFill.tint === 0x4878c8, "health bar fill uses the owning player's color");
  assert(bar.rtsFill.scaleX === 0.5, "health bar fill retains authoritative HP fraction");
  assert(bar.children.length === 3 && !bar.rtsInsight,
    "ordinary health bars allocate no Disciple overlay graphics");
  assert(
    bar.rtsTicks.calls.filter((call) => call[0] === "drawRect").length === 2,
    "40 HP health bar rounds to three whole 15 HP segments",
  );
  assert(
    bar.rtsTicks.calls.filter((call) => call[0] === "drawRect").every((call) => call[3] === 0.75),
    "ordinary 15 HP dividers stay visible with three-quarter-pixel marks",
  );

  const highHpBar = makeBar();
  _hpBar.call({ _map: { tileSize: 32 } }, highHpBar, {
    kind: KIND.RIFLEMAN,
    owner: 1,
    x: 100,
    y: 120,
    hp: 250,
    maxHp: 250,
  }, null, 0x4878c8);
  const highHpDividers = highHpBar.rtsTicks.calls.filter((call) => call[0] === "drawRect");
  const majorDividers = highHpDividers.filter((call) => call[3] === 1.5);
  assert(
    highHpDividers.length === 16 && majorDividers.length === 2,
    "250 HP health bar thickens two existing dividers without adding marks",
  );
  assert(
    Math.abs((majorDividers[0][1] + majorDividers[0][3] / 2) - (-10 + 20 * 7 / 17)) < 0.001
      && Math.abs((majorDividers[1][1] + majorDividers[1][3] / 2) - (-10 + 20 * 14 / 17)) < 0.001,
    "every seventh health-cell divider is heavier",
  );

  let barsDrawn = 0;
  const renderer = {
    _hpBarSlot() {
      barsDrawn += 1;
      return bar;
    },
    _hpBar() {},
  };
  const fullHealthRifleman = {
    id: 7,
    kind: KIND.RIFLEMAN,
    owner: 1,
    x: 100,
    y: 120,
    hp: 40,
    maxHp: 40,
  };
  _drawSelectionAndHp.call(renderer, fullHealthRifleman, new Set(), {
    showHealthBarsAlwaysEnabled: false,
  });
  assert(barsDrawn === 0, "full-health unselected units hide HP bars by default");
  _drawSelectionAndHp.call(renderer, fullHealthRifleman, new Set(), {
    showHealthBarsAlwaysEnabled: true,
  });
  assert(barsDrawn === 1, "always-show preference reveals full-health unit HP bars");

  for (const kind of [KIND.STEEL, KIND.OIL]) {
    _drawSelectionAndHp.call(renderer, {
      id: kind === KIND.STEEL ? 8 : 9,
      kind,
      owner: 0,
      x: 100,
      y: 120,
      hp: 1,
      maxHp: 1,
    }, new Set(), { showHealthBarsAlwaysEnabled: true });
  }
  assert(barsDrawn === 1, "always-show preference excludes underlying steel and oil patch HP");

  for (const kind of [KIND.STEEL_MINE, KIND.PUMP_JACK]) {
    _drawSelectionAndHp.call(renderer, {
      id: kind === KIND.STEEL_MINE ? 10 : 11,
      kind,
      owner: 1,
      x: 100,
      y: 120,
      hp: 100,
      maxHp: 100,
    }, new Set(), { showHealthBarsAlwaysEnabled: true });
  }
  assert(barsDrawn === 3, "always-show preference keeps extractor building HP visible");

  _drawAboveFogHp.call(renderer, {
    id: 12,
    kind: KIND.STEEL,
    owner: 0,
    x: 100,
    y: 120,
    hp: 1,
    maxHp: 2,
    aboveFogReveal: true,
  }, { showHealthBarsAlwaysEnabled: true });
  assert(barsDrawn === 3, "above-fog rendering also excludes raw resource HP bars");

  _drawAboveFogHp.call(renderer, {
    id: 13,
    kind: KIND.STEEL_MINE,
    owner: 1,
    x: 100,
    y: 120,
    hp: 100,
    maxHp: 100,
    aboveFogReveal: true,
  }, { showHealthBarsAlwaysEnabled: true });
  assert(barsDrawn === 4, "above-fog rendering keeps extractor building HP visible");

  const disciple = {
    id: 14, kind: KIND.WARRIOR, owner: 1, x: 100, y: 120,
    hp: 135, maxHp: 135, insight: 30, aboveFogReveal: true,
  };
  _drawSelectionAndHp.call(renderer, disciple, new Set(), {});
  _drawAboveFogHp.call(renderer, disciple, {});
  assert(barsDrawn === 6, "full-health Disciples retain insight overlays both below and above fog");

  const discipleBar = makeBar();
  for (const dao of [null, "lightness", "war", "magic"]) {
    _hpBar.call({ _map: { tileSize: 32 } }, discipleBar, { ...disciple, dao });
    const rects = discipleBar.rtsInsight.calls.filter((call) => call[0] === "drawRect");
    assert(rects.at(-1)[3] === (rects.at(-2)[3] - 2) / 2,
      "30 insight fills half the Disciple insight bar");
    assert(discipleBar.rtsDao.rotation === (dao === "war" ? Math.PI / 2 : 0),
      "Dao icon rotation resets when the authoritative specialization changes");
    assert(discipleBar.rtsDao.scaleY === (dao === "war" ? 1.2 : 1),
      "only the War icon receives the sword length multiplier");
  }
  assert(discipleBar.children.length === 6 && discipleBar.children[0] === discipleBar.rtsRadiance,
    "Disciple overlays are allocated once with radiance behind the health bar");
  _hpBar.call({ _map: { tileSize: 32 } }, discipleBar, { ...disciple, kind: KIND.RIFLEMAN });
  assert(!discipleBar.rtsInsight.visible && !discipleBar.rtsDao.visible && !discipleBar.rtsRadiance.visible,
    "reused bars hide Disciple overlays when the entity kind changes");
  _hpBar.call({ _map: { tileSize: 32 } }, discipleBar, { ...disciple, dao: "magic" });
  assert(discipleBar.children.length === 6 && discipleBar.rtsInsight.visible && discipleBar.rtsDao.visible,
    "returning Disciples reuse and reveal their existing overlays");
} finally {
  restorePixi();
}

console.log("health_bar_renderer_contracts: ok");
