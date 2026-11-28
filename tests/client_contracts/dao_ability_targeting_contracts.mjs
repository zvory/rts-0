import { assert, assertDeepEqual } from "./assertions.mjs";
import { ABILITY, KIND, UPGRADE } from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { Minimap } from "../../client/src/minimap.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { groupCooldownClocks } from "../../client/src/hud.js";
import { buildCommandCardDescriptors, selectedAbilityAffordances } from "../../client/src/hud_command_card.js";
import { HotkeyProfileService, buildHotkeyCommandCatalog } from "../../client/src/hotkey_profiles.js";
import { createCommandButton } from "../../client/src/hud_command_dom.js";
import { _drawAbilityTargetPreview } from "../../client/src/renderer/feedback.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";
import { withFakeDocument } from "./fakes.mjs";

const swordInput = Object.create(Input.prototype);
const swordSelection = [
  { id: 101, owner: 1, kind: KIND.WAR_ADEPT, dao: "war", daoMaster: true, x: 200, y: 220 },
  { id: 102, owner: 1, kind: KIND.MAGIC_ADEPT, dao: "magic", x: 500, y: 220 },
  { id: 103, owner: 1, kind: KIND.WIND_ADEPT, dao: "wind", x: 500, y: 220 },
  { id: 107, owner: 1, kind: KIND.WAR_ADEPT, dao: "war", x: 500, y: 220 },
  { id: 104, owner: 1, kind: KIND.WARRIOR, x: 500, y: 220 },
  { id: 105, owner: 2, kind: KIND.WAR_ADEPT, dao: "war", daoMaster: true, x: 500, y: 220 },
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
  [101, 107], "Throw Sword previews originate from owned War Adepts and Masters",
);
assert(swordInput.clientIntent.abilityTargetPreview.hoverInRange,
  "A nearby War Adept marks the Throw Sword cursor in range");
const swordCommands = [];
swordInput.commandInteraction = { issueCommand: (command) => swordCommands.push(command) };
swordInput._selectedOwnUnitIds = () => swordSelection.filter((e) => e.owner === 1).map((e) => e.id);
swordInput._resourceAtScreen = () => null;
swordInput._addCommandFeedback = () => {};
swordInput._issueTargetedCommand(swordInput.mouse);
assertDeepEqual(swordCommands[0].units, [101, 107],
  "Throw Sword commands include the same eligible carriers as the preview");
const swordMinimap = Object.create(Minimap.prototype);
swordMinimap.state = swordInput.state;
swordMinimap._intent = () => swordInput.clientIntent;
swordMinimap.commandInteraction = swordInput.commandInteraction;
swordMinimap._addCommandFeedback = () => {};
swordMinimap._issueOrder(500, 220, true);
assertDeepEqual(swordCommands[1].units, [101, 107],
  "Minimap Throw Sword commands exclude other Dao paths and enemy Disciples");
assert(swordCommands[1].queued, "Minimap Throw Sword retains queued targeting");
const swordAffordance = selectedAbilityAffordances({
  playerId: 1, factionId: "cultivators", groupCooldownClocks,
}, swordSelection)
  .find((entry) => entry.definition.ability === ABILITY.THROW_SWORD);
assertDeepEqual(swordAffordance.carrierIds, [101, 107],
  "Throw Sword command card shares targeting carrier eligibility");

for (const daoMaster of [false, true]) {
  const card = buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators", groupCooldownClocks,
    selection: [{ id: 107, owner: 1, kind: KIND.WAR_ADEPT, daoMaster }],
    resources: { steel: 0, oil: 0 }, upgrades: [],
  });
  assert(card.slots.some((slot) => slot?.ability === ABILITY.THROW_SWORD),
    "Throw Sword appears for War Adepts and Masters");
}

for (const [ability, kind, adeptTicks, masterTicks] of [
  [ABILITY.THROW_SWORD, KIND.WAR_ADEPT, 1200, 600],
  [ABILITY.VECTOR_FIELD, KIND.MAGIC_ADEPT, 900, 600],
]) {
  for (const [daoMaster, total] of [[false, adeptTicks], [true, masterTicks]]) {
    const affordance = selectedAbilityAffordances({
      playerId: 1, factionId: "cultivators", groupCooldownClocks,
      resources: { steel: 0, oil: 0 }, upgrades: [],
    }, [{ id: 107, owner: 1, kind, daoMaster,
      abilities: [{ ability, cooldownLeft: total / 2 }] }])
      .find((entry) => entry.definition.ability === ability);
    assertDeepEqual(affordance.cooldownClocks, groupCooldownClocks([total / 2], total),
      `${ability} clock uses its rank's full cooldown`);
    assertDeepEqual(affordance.readyIds, [], `${ability} is unavailable during cooldown`);
  }
}

const mixedSword = selectedAbilityAffordances({
  playerId: 1, factionId: "cultivators", groupCooldownClocks,
  resources: { steel: 0, oil: 0 }, upgrades: [],
}, [false, true].map((daoMaster, index) => ({
  id: 200 + index, owner: 1, kind: KIND.WAR_ADEPT, daoMaster,
  abilities: [{ ability: ABILITY.THROW_SWORD, cooldownLeft: 300 }],
}))).find((entry) => entry.definition.ability === ABILITY.THROW_SWORD);
assertDeepEqual(mixedSword.cooldownClocks, [
  ...groupCooldownClocks([300], 1200), ...groupCooldownClocks([300], 600),
], "Mixed ranks keep separate cooldown clock durations");

const promotedSword = selectedAbilityAffordances({
  playerId: 1, factionId: "cultivators", groupCooldownClocks,
}, [900, 899].map((cooldownLeft, index) => ({
  id: 210 + index, owner: 1, kind: KIND.WAR_ADEPT, daoMaster: true,
  abilities: [{ ability: ABILITY.THROW_SWORD, cooldownLeft }],
}))).find((entry) => entry.definition.ability === ABILITY.THROW_SWORD);
assertDeepEqual(promotedSword.cooldownClocks, groupCooldownClocks([900, 899], 600),
  "Promoted Adept cooldowns share a clock while above the Master duration");

for (const insight of [0, 60]) {
  const disciple = { id: 106, owner: 1, kind: KIND.WARRIOR, insight, chosenDao: "war" };
  for (const upgrades of [[], [UPGRADE.DAO_OF_HEAVINESS], [
    UPGRADE.DAO_WIND, UPGRADE.DAO_OF_HEAVINESS, UPGRADE.DAO_OF_MAGIC,
  ]]) {
    const card = buildCommandCardDescriptors({
      playerId: 1, factionId: "cultivators", selection: [disciple],
      resources: { steel: 0, oil: 0 }, upgrades, groupCooldownClocks,
      playerHasCompleteKind: () => true,
    });
    for (const [ability] of [
      [ABILITY.EVOLVE_WIND, UPGRADE.DAO_WIND],
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

const windEntity = { id: 89, owner: 1, kind: KIND.WIND_ADEPT, dao: "wind", x: 200, y: 1000 };
dashInput.state.selectedEntities = () => [ekatEntity, windEntity];
dashInput.state.abilityObjects = [];
dashInput._refreshAbilityTargetPreview();
const mixedDashPreview = dashInput.clientIntent.abilityTargetPreview;
assertDeepEqual(mixedDashPreview.rangeOrigins.map((origin) => origin.rangePx), [160],
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
assert(!mixedDashGfx.calls.some((call) => call[0] === "moveTo" && call[1] === 520 && call[2] === 1000),
  "Kat Dash does not target Wind units");

dashInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.WIND_DASH });
dashInput._refreshAbilityTargetPreview();
assertDeepEqual(dashInput.clientIntent.abilityTargetPreview.rangeOrigins.map((origin) => origin.rangePx), [384],
  "Wind Blink targets only Wind units with twelve-tile range");

console.log("Dao ability targeting contracts passed");

// Shop targeting must work with a building-only selection and preserve unit identity.
{
  const input = Object.create(Input.prototype);
  const shop = { id: 201, owner: 1, kind: KIND.SHOP, hp: 200, x: 100, y: 100 };
  let target = { id: 202, owner: 2, kind: KIND.RIFLEMAN, hp: 40, x: 200, y: 100 };
  const sent = [];
  input.state = { playerId: 1, map: { tileSize: 32 }, selectedEntities: () => [shop], isAllyOwner: (owner) => owner === 2 };
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.REGEN });
  input._selectedOwnUnitIds = () => [];
  input._selectedProducerBuildingIds = () => [];
  input._resourceAtScreen = () => null;
  input._groundAtScreen = () => ({ x: 200, y: 100 });
  input._entityAtScreen = () => target;
  input._addCommandFeedback = () => {};
  input.commandInteraction = { issueCommand: (command) => sent.push(command) };
  assert(input._issueTargetedCommand({ x: 200, y: 100 }), "Shop can target a friendly unit");
  assertDeepEqual(sent[0], { c: "useAbility", ability: ABILITY.REGEN, units: [201], targetId: 202 },
    "Shop purchase names exactly one source Shop and the clicked unit");
  for (const invalid of [null, { ...target, kind: KIND.BARRACKS }, { ...target, owner: 3 },
    { ...target, hp: 0 }, { ...target, visionOnly: true }]) {
    target = invalid;
    assert(!input._issueTargetedCommand({ x: 200, y: 100 }), "Invalid shop target keeps targeting armed");
  }
  assert(sent.length === 1, "Invalid shop targets issue no purchase commands");
  target = { id: 202, owner: 2, kind: KIND.RIFLEMAN, hp: 40, x: 2100, y: 100 };
  assert(input._issueTargetedCommand({ x: 200, y: 100 }), "Regen accepts distant allies");
  assert(sent.length === 2, "Distant potion purchase is issued");
  for (const ability of [ABILITY.ESCAPE_TREASURE, ABILITY.ARMOR_TREASURE,
    ABILITY.VITALITY_TREASURE, ABILITY.DOUBLE_XP, ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE]) {
    input.clientIntent.beginCommandTarget({ kind: "ability", ability });
    const before = sent.length;
    assert(input._issueTargetedCommand({ x: 200, y: 100 }), "Every carried Shop good accepts distant allies");
    assert(sent.length === before + 1, "Distant carried-item purchase is issued");
    assertDeepEqual(sent.at(-1), { c: "useAbility", ability, units: [201], targetId: 202 },
      "Unlimited-range targeting preserves ability, source Shop and recipient identity");
  }

  target = { ...target, owner: 1, x: 200 };
  const passiveItems = [ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE,
    ABILITY.DOUBLE_XP, ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE];
  for (const equipped of passiveItems) {
    target.abilities = [{ ability: equipped, remainingUses: 1 }];
    for (const ability of passiveItems) {
      input.clientIntent.beginCommandTarget({ kind: "ability", ability });
      const before = sent.length;
      const accepted = input._issueTargetedCommand({ x: 200, y: 100 });
      assert(accepted === (ability !== equipped),
        "Shop targeting rejects only the identical equipped passive item");
      assert(sent.length === before + (ability !== equipped ? 1 : 0),
        "Distinct item purchases reach the server, including within the old categories");
      if (ability !== equipped) {
        assertDeepEqual(sent.at(-1), { c: "useAbility", ability, units: [201], targetId: 202 },
          "Coexisting item purchase preserves ability, Shop and target identity");
      }
    }
  }
}

// Passive Charge remains observable through its real cooldown without accepting commands.
for (const [kind, daoMaster] of [[KIND.WARRIOR, false], [KIND.WAR_ADEPT, false], [KIND.WAR_ADEPT, true]]) {
  for (const cooldownLeft of [0, 450, 225, 1]) {
    const novice = { id: 910, owner: 1, kind, daoMaster, insight: 0,
      abilities: [{ ability: ABILITY.CHARGE, cooldownLeft, autocastEnabled: true }] };
    const ctx = { playerId: 1, factionId: "cultivators", selection: [novice],
      resources: { steel: 0, oil: 0 }, upgrades: [], groupCooldownClocks };
    const charge = buildCommandCardDescriptors(ctx).slots.find((slot) => slot?.ability === ABILITY.CHARGE);
    assert(charge && charge.label === "Charge" && !charge.enabled, "Charge is visibly disabled");
    assert(charge.cls.includes("autocast-enabled"), "Charge always shows the automatic swirl");
    assert(!charge.intent && !charge.contextIntent && !charge.onUnavailableIntent,
      "Charge has no cast, toggle, or unavailable-click action");
    assert(!charge.hotkey && !charge.hotkeyCode, "Charge has no misleading hotkey");
    const catalog = buildHotkeyCommandCatalog([buildCommandCardDescriptors(ctx)]);
    assert(!catalog.commands.some((entry) => entry.commandId === charge.commandId),
      "Charge cannot be assigned a custom hotkey");
    const profiles = new HotkeyProfileService({ storage: null, catalog });
    for (const mode of ["grid", "direct"]) {
      assert(!profiles.resolveSlot(charge, { mode }).hotkeyCode, "Hotkey profiles preserve passive indicators");
    }
    assertDeepEqual(charge.cooldownClocks, groupCooldownClocks([cooldownLeft], 450),
      "Charge clocks follow authoritative cooldown ticks");
    const affordance = selectedAbilityAffordances(ctx, [novice]).find((a) => a.definition.ability === ABILITY.CHARGE);
    assertDeepEqual(affordance.readyIds, [], "Charge is never manually ready, including off cooldown");
    withFakeDocument(() => {
      const button = createCommandButton(charge);
      assert(button.disabled, "Charge uses a native disabled button");
      assert(!button.dataset.contextAction, "Charge cannot be toggled by right click");
    });
    for (const kind of [KIND.WIND_ADEPT, KIND.MAGIC_ADEPT]) {
      assert(!buildCommandCardDescriptors({ ...ctx, selection: [{ ...novice, kind }] }).slots
        .some((slot) => slot?.ability === ABILITY.CHARGE), "Wind and Magic evolution remove Charge");
    }
  }
}

for (const [kind, ability, research] of [
  [KIND.WARRIOR, ABILITY.CHARGE, UPGRADE.TECHNIQUE_CHARGE],
  [KIND.WAR_ADEPT, ABILITY.CHARGE, UPGRADE.TECHNIQUE_CHARGE],
  [KIND.MAGIC_ADEPT, ABILITY.DISCIPLE_SHIELD, UPGRADE.TECHNIQUE_SHIELD],
  [KIND.WAR_ADEPT, ABILITY.THROW_SWORD, UPGRADE.TECHNIQUE_THROW_SWORD],
]) {
  for (const daoMaster of [false, true]) {
    const ctx = { playerId: 1, factionId: "cultivators", groupCooldownClocks,
      resources: { steel: 1000, oil: 1000 }, upgrades: [],
      selection: [{ id: 1, owner: 1, kind, daoMaster }] };
    const locked = selectedAbilityAffordances(ctx, ctx.selection).find((a) => a.definition.ability === ability);
    assert(!locked.unlocked, `${ability} starts research locked at both ranks`);
    const lockedButton = buildCommandCardDescriptors(ctx).slots.find((a) => a?.ability === ability);
    assert(lockedButton && !lockedButton.enabled && lockedButton.title.includes("Requires"),
      `${ability} visibly explains its research lock`);
    const unlocked = selectedAbilityAffordances({ ...ctx, upgrades: [research] }, ctx.selection)
      .find((a) => a.definition.ability === ability);
    assert(unlocked.unlocked, `${ability} unlocks with its own research`);
  }
}
const libraryCard = buildCommandCardDescriptors({ playerId: 1, factionId: "cultivators",
  selection: [{ id: 2, owner: 1, kind: KIND.TECHNIQUE_LIBRARY }],
  resources: { steel: 1000, oil: 1000 }, upgrades: [] });
assertDeepEqual(libraryCard.slots.filter((s) => s?.action === "research").map((s) => s.intent.upgrade),
  [UPGRADE.TECHNIQUE_CHARGE, UPGRADE.TECHNIQUE_SHIELD, UPGRADE.TECHNIQUE_THROW_SWORD],
  "Technique Library offers three independent researches");

for (const daoMaster of [false, true]) {
  const ctx = { playerId: 1, factionId: "cultivators", groupCooldownClocks,
    resources: { steel: 0, oil: 0 }, upgrades: [],
    selection: [{ id: 1, owner: 1, kind: KIND.WIND_ADEPT, daoMaster }] };
  const blink = selectedAbilityAffordances(ctx, ctx.selection)
    .find((a) => a.definition.ability === ABILITY.WIND_DASH);
  assert(blink.unlocked, "Blink starts unlocked at both Wind ranks");
  assert(buildCommandCardDescriptors(ctx).slots.some((s) => s?.ability === ABILITY.WIND_DASH && s.enabled),
    "Blink is usable without research");
}
