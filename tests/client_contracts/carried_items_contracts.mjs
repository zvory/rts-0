import assert from "node:assert/strict";
import { ABILITY, KIND } from "../../client/src/protocol.js";
import { ABILITIES, CARRIED_ITEMS, carriedItemMask, carriedItemsForEntity } from "../../client/src/config.js";
import { drawCarriedItemRosette, itemRosettePositions } from "../../client/src/renderer/carried_items.js";
import { _staticSlot } from "../../client/src/renderer/entities.js";
import { _sweep, LAYERS } from "../../client/src/renderer/layers.js";
import { HudSelectionPanel } from "../../client/src/hud_selection_panel.js";
import { carriedItemDetails } from "../../client/src/hud_carried_items.js";
import { installFakePixi } from "./pixi_fakes.mjs";
import { withFakeHudDocument } from "./fakes.mjs";

const equipped = { id: 5, owner: 1, kind: KIND.WAR_ADEPT, dao: "war", daoMaster: true,
  x: 320, y: 400, hp: 100, maxHp: 300, escapeTreasure: true,
  abilities: CARRIED_ITEMS.slice(0, 4).map(({ ability }) => ({ ability, remainingUses: 1 })) };
assert.equal(carriedItemsForEntity(equipped).length, 5);
assert.equal(carriedItemMask({ ...equipped, kind: KIND.SHOP }), 0, "Shop abilities are not inventory");
assert.equal(carriedItemMask({ id: 9, kind: KIND.WAR_ADEPT, itemRadianceTier: 6 }), 0,
  "public radiance never reconstructs private inventory");
assert.equal(carriedItemMask({ kind: KIND.WAR_ADEPT, abilities: [{ ability: ABILITY.HASTE, remainingUses: 0 }] }), 0);
for (let count = 0; count <= 9; count++) {
  const positions = itemRosettePositions(count);
  assert.equal(positions.length, count);
  assert(positions.flat().every(Number.isFinite));
  if (count > 1) assert(Math.min(...positions.map(([x]) => x)) <= 0 && Math.max(...positions.map(([x]) => x)) >= 0);
}
assert.deepEqual(itemRosettePositions(5)[4], [0, 0], "Escape is the center of a full rosette");
assert.deepEqual(itemRosettePositions(9, 4)[4], [0, 0], "larger rosettes retain a designated Escape center");

const restore = installFakePixi();
try {
  const renderer = { _pools: { carriedItems: new Map() }, _seen: { carriedItems: new Set() },
    _unseen: new Map(), layers: { carriedItems: new PIXI.Container() }, _staticSlot };
  drawCarriedItemRosette(renderer, equipped);
  const graphic = renderer._pools.carriedItems.get(equipped.id);
  assert.equal(graphic.x, equipped.x); assert.equal(graphic.y, equipped.y);
  assert.equal(graphic.calls.filter((call) => call[0] === "drawPolygon").length, 6, "five silhouettes plus Escape return arrow");
  assert.equal(graphic.calls.filter((call) => call[0] === "lineStyle" && call[1] === 0.63).length, 5);
  const drawCalls = graphic.calls.length;
  drawCarriedItemRosette(renderer, { ...equipped, x: 330, y: 410 });
  assert.equal(graphic.calls.length, drawCalls, "unchanged inventory reuses cached geometry");
  assert.equal(graphic.x, 330); assert.equal(graphic.y, 410, "movement updates the exact world center every frame");
  renderer._seen.carriedItems.clear();
  drawCarriedItemRosette(renderer, { id: 5, kind: KIND.WAR_ADEPT, itemRadianceTier: 6 });
  _sweep.call(renderer);
  assert.equal(graphic.visible, false, "loss of authorized inventory clears old icons in the same frame");
  for (let i = 0; i < 1000 && renderer._pools.carriedItems.size; i++) _sweep.call(renderer);
  assert.equal(renderer._pools.carriedItems.size, 0); assert(graphic.destroyed, "pooled item graphics are destroyed after eviction");
  assert(LAYERS.indexOf("carriedItems") > LAYERS.indexOf("hpBars") && LAYERS.indexOf("carriedItems") < LAYERS.indexOf("fog"));
  assert(LAYERS.indexOf("aboveFogCarriedItems") > LAYERS.indexOf("aboveFogHpBars"));
} finally { restore(); }

withFakeHudDocument(({ FakeElement }) => {
  const players = [{ id: 1, factionId: "cultivators" }];
  const empty = { id: 6, owner: 1, kind: KIND.WIND_ADEPT, dao: "wind" };
  const selection = [equipped, empty];
  const panel = new FakeElement("section");
  const ui = new HudSelectionPanel(panel, { players, selectedEntities: () => selection }, null, () => "");
  ui.render();
  let summary = panel.querySelector(".sel-item-summary");
  const value = CARRIED_ITEMS.reduce((sum, { ability }) => sum + ABILITIES[ability].cost.oil, 0);
  assert(summary.textContent.includes(`War Master · 5 carried items · ${value} oil value`));
  let portraits = panel.querySelectorAll("[data-selection-entity-id]");
  assert.equal(portraits[0].querySelector(".sel-item-count").textContent, "5");
  assert(portraits[0].className.includes("has-carried-escape"));
  assert(portraits[0].getAttribute("aria-describedby"));
  panel.listeners.focusin({ target: portraits[1] });
  assert(summary.textContent.includes("Wind Adept · 0 carried items"), "keyboard focus inspects the actual portrait type");
  panel.listeners.pointerover({ target: portraits[0] });
  assert(summary.textContent.includes("Escape Treasure"), "hover exposes exact loadout without active commands");
  const original = equipped.abilities;
  equipped.abilities = []; equipped.escapeTreasure = false;
  ui.render();
  assert.equal(panel.querySelector(".sel-item-summary"), null, "inventory loss invalidates the selection cache");
  equipped.abilities = original; equipped.escapeTreasure = true;
  ui.render(); assert(panel.querySelector(".sel-item-summary"));
  ui.destroy();
  assert.equal(panel.listeners.pointerover, undefined); assert.equal(panel.listeners.focusin, undefined);
  assert.equal(panel.listeners.click, undefined); assert.equal(panel.listeners.contextmenu, undefined);
});
assert(carriedItemDetails({ id: 1, kind: KIND.WAR_ADEPT, itemRadianceTier: 6 }).includes("0 carried items"));
console.log("carried_items_contracts: ok");
