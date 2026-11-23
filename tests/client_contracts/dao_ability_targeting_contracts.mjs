import { assert, assertDeepEqual } from "./assertions.mjs";
import { ABILITY, KIND, UPGRADE } from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { Minimap } from "../../client/src/minimap.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { groupCooldownClocks } from "../../client/src/hud.js";
import { buildCommandCardDescriptors, selectedAbilityAffordances } from "../../client/src/hud_command_card.js";
import { createCommandButton } from "../../client/src/hud_command_dom.js";
import { _drawAbilityTargetPreview } from "../../client/src/renderer/feedback.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";
import { withFakeDocument } from "./fakes.mjs";

const swordInput = Object.create(Input.prototype);
const swordSelection = [
  { id: 101, owner: 1, kind: KIND.WAR_ADEPT, dao: "war", x: 200, y: 220 },
  { id: 102, owner: 1, kind: KIND.MAGIC_ADEPT, dao: "magic", x: 500, y: 220 },
  { id: 103, owner: 1, kind: KIND.LIGHTNESS_ADEPT, dao: "lightness", x: 500, y: 220 },
  { id: 104, owner: 1, kind: KIND.WARRIOR, x: 500, y: 220 },
  { id: 105, owner: 2, kind: KIND.WAR_ADEPT, dao: "war", x: 500, y: 220 },
];
swordInput.mouse = { x: 500, y: 220 };
swordInput.state = {
  playerId: 1, factionId: "cultivators", map: { tileSize: 32 },
  selectedEntities: () => swordSelection,
};
swordInput.clientIntent = new ClientIntent();
swordInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.THROW_SWORD });
swordInput._groundAtScreen = (x, y) => ({ x, y });
swordInput._refreshAbilityTargetPreview();
assertDeepEqual(
  swordInput.clientIntent.abilityTargetPreview.pathOrigins.map((origin) => origin.id),
  [101], "Throw Sword previews originate only from owned War Disciples",
);
assert(!swordInput.clientIntent.abilityTargetPreview.hoverInRange,
  "Ineligible nearby Disciples do not mark the Throw Sword cursor in range");
const swordCommands = [];
swordInput.commandInteraction = { issueCommand: (command) => swordCommands.push(command) };
swordInput._selectedOwnUnitIds = () => swordSelection.filter((e) => e.owner === 1).map((e) => e.id);
swordInput._resourceAtScreen = () => null;
swordInput._addCommandFeedback = () => {};
swordInput._issueTargetedCommand(swordInput.mouse);
assertDeepEqual(swordCommands[0].units, [101],
  "Throw Sword commands include the same eligible carriers as the preview");
const swordMinimap = Object.create(Minimap.prototype);
swordMinimap.state = swordInput.state;
swordMinimap._intent = () => swordInput.clientIntent;
swordMinimap.commandInteraction = swordInput.commandInteraction;
swordMinimap._addCommandFeedback = () => {};
swordMinimap._issueOrder(500, 220, true);
assertDeepEqual(swordCommands[1].units, [101],
  "Minimap Throw Sword commands exclude other Dao paths and enemy Disciples");
assert(swordCommands[1].queued, "Minimap Throw Sword retains queued targeting");
const swordAffordance = selectedAbilityAffordances({
  playerId: 1, factionId: "cultivators", groupCooldownClocks,
}, swordSelection)
  .find((entry) => entry.definition.ability === ABILITY.THROW_SWORD);
assertDeepEqual(swordAffordance.carrierIds, [101],
  "Throw Sword command card shares targeting carrier eligibility");

for (const insight of [0, 60]) {
  const disciple = { id: 106, owner: 1, kind: KIND.WARRIOR, insight, chosenDao: "war" };
  for (const upgrades of [[], [UPGRADE.DAO_OF_HEAVINESS], [
    UPGRADE.DAO_OF_LIGHTNESS, UPGRADE.DAO_OF_HEAVINESS, UPGRADE.DAO_OF_MAGIC,
  ]]) {
    const card = buildCommandCardDescriptors({
      playerId: 1, factionId: "cultivators", selection: [disciple],
      resources: { steel: 0, oil: 0 }, upgrades, groupCooldownClocks,
      playerHasCompleteKind: () => true,
    });
    for (const [ability] of [
      [ABILITY.EVOLVE_LIGHTNESS, UPGRADE.DAO_OF_LIGHTNESS],
      [ABILITY.EVOLVE_WAR, UPGRADE.DAO_OF_HEAVINESS],
      [ABILITY.EVOLVE_MAGIC, UPGRADE.DAO_OF_MAGIC],
    ]) {
      const slot = card.slots.find((entry) => entry?.ability === ability);
      assert(slot.enabled, "Dao choices remain queueable before research and insight are ready");
      assert(!slot.cls.includes("dao-awaiting-research"),
        "Combat Dao choices never require research");
      assert(slot.cls.split(" ").includes("dao-ready") === (insight >= 60),
        "Dao radiance requires sufficient insight alone");
      assertDeepEqual(slot.intent.readyIds, [disciple.id], "Pending Dao choice retains its selected carrier");
      if (ability === ABILITY.EVOLVE_WAR) {
        assert(slot.countBadge === "1/1", "Pending chosen path retains its selection badge");
      }
      withFakeDocument(() => {
        let clicked = false;
        const button = createCommandButton({ ...slot, onClick: () => { clicked = true; } });
        assert(!button.disabled, "Grayed Dao choices remain interactive");
        button.click();
        assert(clicked, "Clicking a pending Dao choice still dispatches the choice");
      });
    }
  }
}

const dashInput = Object.create(Input.prototype);
const ekatEntity = { id: 88, owner: 1, kind: KIND.EKAT, x: 200, y: 220 };
dashInput.mouse = { x: 420, y: 260 };
dashInput.state = { playerId: 1, map: { tileSize: 32 } };
dashInput.clientIntent = new ClientIntent();
dashInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.EKAT_TELEPORT });
dashInput._groundAtScreen = (x, y) => ({ x, y });

const lightnessEntity = { id: 89, owner: 1, kind: KIND.LIGHTNESS_ADEPT, dao: "lightness", x: 200, y: 1000 };
dashInput.state.selectedEntities = () => [ekatEntity, lightnessEntity];
dashInput.state.abilityObjects = [];
dashInput._refreshAbilityTargetPreview();
const mixedDashPreview = dashInput.clientIntent.abilityTargetPreview;
assertDeepEqual(mixedDashPreview.rangeOrigins.map((origin) => origin.rangePx), [160, 320],
  "Mixed Dash selection preserves each carrier's authoritative range");
const mixedDashGfx = new RecordingGraphics();
_drawAbilityTargetPreview.call(
  { _feedbackGfx: mixedDashGfx },
  { abilityTargetPreview: mixedDashPreview },
);
assert(mixedDashGfx.calls.some((call) => call[0] === "moveTo" && call[1] === 360 && call[2] === 220),
  "Ekat Dash ring starts five tiles from its origin");
assert(!mixedDashGfx.calls.some((call) => call[0] === "moveTo" && call[1] === 520 && call[2] === 220),
  "Mixed selection does not expand Ekat's Dash ring to ten tiles");
assert(mixedDashGfx.calls.some((call) => call[0] === "moveTo" && call[1] === 520 && call[2] === 1000),
  "Lightness Dash ring starts ten tiles from its origin");

console.log("Dao ability targeting contracts passed");
