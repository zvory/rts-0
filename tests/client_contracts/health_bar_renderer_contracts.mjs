import { KIND } from "../../client/src/protocol.js";
import { _drawSelectionAndHp, _hpBar } from "../../client/src/renderer/entities.js";
import { assert } from "./assertions.mjs";
import { installFakePixi } from "./pixi_fakes.mjs";

const restorePixi = installFakePixi();
try {
  const makeBar = () => {
    const bar = new PIXI.Container();
    bar.rtsBackground = new PIXI.Graphics();
    bar.rtsFill = new PIXI.Graphics();
    bar.rtsTicks = new PIXI.Graphics();
    return bar;
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
  assert(
    bar.rtsTicks.calls.filter((call) => call[0] === "drawRect").length === 2,
    "40 HP health bar rounds to three whole 15 HP segments",
  );
  assert(
    bar.rtsTicks.calls.filter((call) => call[0] === "drawRect").every((call) => call[3] === 0.4),
    "ordinary 15 HP dividers use thin hairline marks",
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
  const majorDividers = highHpDividers.filter((call) => call[3] === 1.2);
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
} finally {
  restorePixi();
}

console.log("health_bar_renderer_contracts: ok");
