import { assert } from "./assertions.mjs";
import { withFakeHudDocument } from "./fakes.mjs";
import { HUD } from "../../client/src/hud.js";
import { DEFAULT_FACTION_ID, KIND, UPGRADE } from "../../client/src/protocol.js";

// Observer cards use projected owner data and retain no command dispatch path.
{
  let selected = [{ id: 900, owner: 2, kind: KIND.SCOUT_CAR }];
  const state = { spectator: true, playerId: 99, localFactionId: "ekat",
    players: [{ id: 1, factionId: "ekat" }, { id: 2, factionId: DEFAULT_FACTION_ID }],
    playerResources: [{ id: 2, steel: 123, oil: 45, upgrades: [UPGRADE.SMOKE_PLUS] }],
    upgrades: [UPGRADE.TANK_UNLOCK], resources: { steel: 9999 },
    selectedEntities: () => selected, entitiesInterpolated: () => selected };
  const hud = new HUD({ querySelector: () => null }, state, {}, null);
  const ctx = hud._commandDescriptorContext();
  assert(ctx.playerId === 2 && ctx.factionId === DEFAULT_FACTION_ID, "observer card uses selected owner's faction");
  assert(ctx.resources.steel === 123 && ctx.upgrades.includes(UPGRADE.SMOKE_PLUS), "observer card uses selected owner's projected research and resources");
  selected = [{ id: 901, owner: 1, kind: KIND.WORKER }];
  const other = hud._commandDescriptorContext();
  assert(other.playerId === 1 && other.factionId === "ekat" && other.upgrades.length === 0, "switching owner never borrows another player's research");
  hud._intent = () => { throw new Error("observer must not arm command targeting"); };
  hud._dispatchCommandIntent({ type: "beginCommandTarget", target: { kind: "attack" } });
}
withFakeHudDocument(({ FakeElement }) => {
  const panel = new FakeElement("section");
  const entity = { id: 910, owner: 2, kind: KIND.SCOUT_CAR, hp: 100, maxHp: 100 };
  const row = { id: 2, upgrades: [] };
  const state = { spectator: true, playerResources: [row], selectedEntities: () => [entity] };
  const hud = new HUD({ querySelector: (selector) => selector === "#selected-panel" ? panel : null }, state, {}, null);
  hud._renderSelectedPanel();
  const before = panel.children[0];
  assert(before.children.some((child) => child.textContent === "Player research: None"), "observer selection shows current player research");
  row.upgrades = [UPGRADE.SMOKE_PLUS];
  hud._renderSelectedPanel();
  assert(panel.children[0] !== before, "research completion invalidates selected detail cache");
  assert(panel.children[0].children.some((child) => child.textContent.includes("Smoke")), "completed research appears without reselection");
});
