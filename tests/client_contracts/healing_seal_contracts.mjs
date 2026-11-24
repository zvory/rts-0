import assert from "node:assert/strict";
import { ABILITIES } from "../../client/src/config.js";
import { ABILITY, KIND } from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { HudSelectionPanel } from "../../client/src/hud_selection_panel.js";
import { _drawBuilding } from "../../client/src/renderer/buildings.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";
import { withFakeHudDocument } from "./fakes.mjs";

const shop = { id: 7, owner: 1, kind: KIND.SHOP, x: 320, y: 320, hp: 200 };
const commands = [];
const input = Object.create(Input.prototype);
input.state = { playerId: 1, factionId: "cultivators", map: { tileSize: 32 }, selectedEntities: () => [shop] };
input.clientIntent = new ClientIntent();
input.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.HEALING_SEAL });
input._groundAtScreen = (x, y) => ({ x, y });
input._selectedOwnUnitIds = () => [];
input._resourceAtScreen = () => null;
input._addCommandFeedback = () => {};
input.commandInteraction = { issueCommand: (command) => commands.push(command) };
input._issueTargetedCommand({ x: 656, y: 336 });
assert.equal(commands.length, 1, "Shop can issue a world-point seal purchase without selecting units");
assert.equal(commands[0].ability, "healingSeal");
assert.deepEqual(commands[0].units, [7]);
assert.equal(commands[0].x, 656);
assert.equal(commands[0].y, 336);
assert.equal(ABILITIES[ABILITY.HEALING_SEAL].cost.oil, 150);

// Ground magic remains selectable, but must not swallow movement or attack-move clicks.
const seal = { id: 8, owner: 2, kind: KIND.HEALING_SEAL, x: 656, y: 336, hp: 400, maxHp: 400 };
const warrior = { id: 9, owner: 1, kind: KIND.WARRIOR };
const enemy = { ...warrior, id: 10, owner: 2, x: seal.x, y: seal.y };
let hitEntities = [seal];
input.clientIntent = new ClientIntent();
input.state = {
  playerId: 1, map: { tileSize: 32 }, selectedEntities: () => [warrior],
  isEnemyOwner: (owner) => owner === 2,
};
input._selectedOwnUnitIds = () => [warrior.id];
input._selectedGathererIds = () => [];
input._selectedWorkerIds = () => [];
input._selectionEntities = () => hitEntities;
input._entityAtScreen = (_point, _ownPreferred, eligible = () => true) => hitEntities.find(eligible) || null;
input._onRightClick({ x: seal.x, y: seal.y });
assert.equal(commands.at(-1).c, "move", "right-clicking enemy ground magic moves onto it");
input.clientIntent.beginCommandTarget("attack");
input._issueTargetedCommand({ x: seal.x, y: seal.y });
assert.equal(commands.at(-1).c, "attackMove", "attack-clicking a seal attacks toward the ground");
hitEntities = [seal, enemy];
input.clientIntent.endCommandTarget();
input._onRightClick({ x: seal.x, y: seal.y });
assert.equal(commands.at(-1).c, "attack");
assert.equal(commands.at(-1).target, enemy.id, "seal does not mask an attackable unit inside it");

withFakeHudDocument(({ FakeElement }) => {
  const panel = new HudSelectionPanel(new FakeElement("section"), { players: [{ id: 1, factionId: "cultivators" }] }, null, () => "");
  const node = panel._singleSelectionNode({ id: 8, owner: 1, kind: KIND.HEALING_SEAL, hp: 75, maxHp: 400 });
  assert(node.innerHTML.includes("75 / 400 healing left"));
  assert(node.innerHTML.includes('sel-hpfill good'), "capacity bar stays green at low capacity");
  assert(!node.innerHTML.includes("Armor:"), "capacity is not presented as armored combat health");
  panel.destroy();
});

function drawAt(now) {
  const graphics = new RecordingGraphics();
  const renderer = { _map: { tileSize: 32 }, visualNow: () => now,
    _slot: (layer, id) => {
      assert.equal(layer, "buildingShadows", "seal stays underneath unit bodies");
      assert.equal(id, 8);
      return graphics;
    },
  };
  _drawBuilding.call(renderer, { id: 8, kind: KIND.HEALING_SEAL, x: 656, y: 336 }, new Map(), {});
  return graphics.calls;
}
const low = drawAt(1500);
const high = drawAt(500);
assert.deepEqual(low.filter((call) => call[0] === "drawCircle"), high.filter((call) => call[0] === "drawCircle"),
  "pulse preserves the exact healing boundary");
assert.notDeepEqual(low, high, "glow pulses with the deterministic visual clock");
