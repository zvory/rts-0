import { assert, assertDeepEqual } from "./assertions.mjs";
import { ABILITY, KIND } from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { groupCooldownClocks } from "../../client/src/hud.js";
import { selectedAbilityAffordances } from "../../client/src/hud_command_card.js";

const swordInput = Object.create(Input.prototype);
const swordSelection = [
  { id: 101, owner: 1, kind: KIND.WARRIOR, dao: "war", x: 200, y: 220 },
  { id: 102, owner: 1, kind: KIND.WARRIOR, dao: "magic", x: 500, y: 220 },
  { id: 103, owner: 1, kind: KIND.WARRIOR, dao: "lightness", x: 500, y: 220 },
  { id: 104, owner: 1, kind: KIND.WARRIOR, x: 500, y: 220 },
  { id: 105, owner: 2, kind: KIND.WARRIOR, dao: "war", x: 500, y: 220 },
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
const swordAffordance = selectedAbilityAffordances({
  playerId: 1, factionId: "cultivators", groupCooldownClocks,
}, swordSelection)
  .find((entry) => entry.definition.ability === ABILITY.THROW_SWORD);
assertDeepEqual(swordAffordance.carrierIds, [101],
  "Throw Sword command card shares targeting carrier eligibility");

console.log("Dao ability targeting contracts passed");
