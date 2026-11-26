import assert from "node:assert/strict";
import { shopItemTooltipHtml } from "../client/src/hud_shop_tooltip.js";
import { createCommandButton, renderDescriptorCardDom, syncCooldownClockElement } from "../client/src/hud_command_dom.js";
import { fakeClassList, withFakeDocument, withFakeHudDocument } from "./client_contracts/fakes.mjs";
import { groupCooldownClocks } from "../client/src/hud.js";
import { COMMAND_CARD_SLOT_ORDER } from "../client/src/hud_command_layout.js";

import {
  gridHotkeyForSlot,
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  commandCardActivationCandidates,
  duplicateCommandIdsForCard,
  factionCommandId,
} from "../client/src/hud_command_card.js";
import { createLabControlPolicy } from "../client/src/lab_control_policy.js";
import {
  researchDefinition,
  EKAT_FACTION_ID,
  FIXTURE_FACTION_ID,
  WORKER_BUILDABLE,
  commandCardAbilitiesForFaction,
  factionCatalog,
  workerBuildablesForFaction,
} from "../client/src/config.js";
import { ABILITY, KIND, LAB_ROLE, SETUP, UPGRADE } from "../client/src/protocol.js";

const kriegsiaCommandId = (family, subject) => factionCommandId("kriegsia", family, subject);
const ekatCommandId = (family, subject) => factionCommandId(EKAT_FACTION_ID, family, subject);

const engineeringComplex = {
  id: 10,
  owner: 1,
  kind: KIND.ENGINEERING_COMPLEX,
};

function testGroupCooldownClocks(values, totalTicks) {
  return values
    .filter((value) => value > 0 && totalTicks > 0)
    .map((value) => ({
      count: 1,
      cooldownLeft: value,
      progress: 1 - value / totalTicks,
      rotationDeg: (1 - value / totalTicks) * 360,
    }));
}

function engineeringComplexCard(upgrades = [], prodUpgradeQueue = []) {
  return buildCommandCardDescriptors({
    playerId: 1,
    selection: [{ ...engineeringComplex, prodUpgradeQueue }],
    resources: { steel: 1000, oil: 1000 },
    upgrades,
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
}

function slotIds(card) {
  return card.slots.map((slot) => slot?.id || null);
}

function slotCommandIds(card) {
  return card.slots.map((slot) => slot?.commandId || null);
}

function buttonSlots(card) {
  return card.slots.filter(Boolean).map((slot) => ({
    commandId: slot.commandId,
    slotIndex: slot.slotIndex,
    hotkey: slot.hotkey,
  }));
}

{
  const ids = slotIds(engineeringComplexCard());
  assert.equal(ids[0], null);
  assert.equal(ids[1], null);
  assert.equal(ids[2], null);
  assert.equal(ids[3], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[4], `research:${UPGRADE.SMOKE_PLUS}`);
  assert.equal(ids[5], `research:${UPGRADE.SCOUT_PLANE_UNLOCK}`);
  assert.deepEqual(slotCommandIds(engineeringComplexCard()).slice(0, 6), [
    null,
    null,
    null,
    kriegsiaCommandId("research", UPGRADE.TANK_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.SMOKE_PLUS),
    kriegsiaCommandId("research", UPGRADE.SCOUT_PLANE_UNLOCK),
  ]);
  assert.equal(engineeringComplexCard().slots[1], null);
  assert.equal(engineeringComplexCard().slots[2], null);
}

{
  const card = engineeringComplexCard([], [UPGRADE.ARTILLERY_UNLOCK]);
  assert.equal(card.slots[1], null);
  assert.equal(card.slots[2], null);
}

{
  const ids = slotIds(engineeringComplexCard([UPGRADE.TANK_UNLOCK]));
  assert.equal(ids[0], null);
  assert.equal(ids[1], null);
  assert.equal(ids[2], null);
  assert.equal(ids[3], null);
  assert.equal(ids[4], `research:${UPGRADE.SMOKE_PLUS}`);
  assert.equal(ids[5], `research:${UPGRADE.SCOUT_PLANE_UNLOCK}`);
}

{
  const card = engineeringComplexCard([UPGRADE.ARTILLERY_UNLOCK]);
  const ids = slotIds(card);
  assert.equal(ids[0], null);
  assert.equal(ids[1], null);
  assert.equal(ids[2], null);
  assert.equal(ids[3], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(card.slots[2], null);
}

{
  const worker = { id: 20, owner: 1, kind: KIND.WORKER };
  const workerCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [worker],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.deepEqual(buttonSlots(workerCard), [
    { commandId: "unit.move", slotIndex: 0, hotkey: "Q" },
    { commandId: "unit.holdPosition", slotIndex: 1, hotkey: "W" },
    { commandId: "unit.attack", slotIndex: 3, hotkey: "A" },
    { commandId: "unit.stop", slotIndex: 4, hotkey: "S" },
    { commandId: "worker.buildMenu", slotIndex: 6, hotkey: "Z" },
    { commandId: "worker.advancedBuildMenu", slotIndex: 7, hotkey: "X" },
  ]);

  const buildCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [worker],
    commandCardMode: "workerBuild",
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.equal(buildCard.slots[0].commandId, kriegsiaCommandId("build", KIND.RESOURCE_DEPOT));
  assert.equal(buildCard.slots[0].slotIndex, 0);
  assert.equal(buildCard.slots[0].hotkey, "Q");
  assert.equal(buildCard.slots[1].commandId, kriegsiaCommandId("build", KIND.PUMP_JACK));
  assert.equal(buildCard.slots[1].hotkey, "W");
  assert.equal(buildCard.slots[2].commandId, kriegsiaCommandId("build", KIND.BARRACKS));
  assert.equal(buildCard.slots[2].hotkey, "E");
  assert.equal(buildCard.slots[7].commandId, kriegsiaCommandId("build", KIND.TANK_TRAP));
  assert.equal(buildCard.slots[7].label, "Tank Trap");
  assert.equal(buildCard.slots[6].commandId, kriegsiaCommandId("build", KIND.BARBED_WIRE));
  assert.equal(buildCard.slots[6].label, "Barbed Wire");
  assert.equal(buildCard.slots[7].hotkey, "X");
  assert.equal(
    buildCard.slots.some((slot) => slot?.commandId === kriegsiaCommandId("build", KIND.PUMP_JACK)),
    true,
    "Engineers can place Pump Jacks",
  );
  const advancedKinds = [KIND.ENGINEERING_COMPLEX, KIND.STEELWORKS, KIND.FACTORY];
  assert(!buildCard.slots.some((slot) => advancedKinds.includes(slot?.intent?.building)));
  const advancedCard = buildCommandCardDescriptors({
    playerId: 1, selection: [worker], commandCardMode: "workerAdvancedBuild",
    resources: { steel: 1000, oil: 1000 }, playerHasCompleteKind: () => true,
  });
  assert.equal(advancedCard.kind, "workerAdvancedBuild");
  assert.deepEqual(advancedCard.slots.slice(0, 3).map((slot) => slot.intent.building), advancedKinds);
  assert.deepEqual(advancedCard.slots.slice(0, 3).map((slot) => slot.hotkey), ["Q", "W", "E"]);
  assert(advancedCard.slots.slice(0, 3).every((slot) => slot.enabled));
  assert.equal(advancedCard.slots[8].commandId, "worker.return");
  assert.notEqual(advancedCard.signature, buildCard.signature);
  const lockedCard = buildCommandCardDescriptors({
    playerId: 1, selection: [worker], commandCardMode: "workerAdvancedBuild",
    resources: { steel: 0, oil: 0 }, playerHasCompleteKind: () => false,
  });
  assert(lockedCard.slots.slice(0, 3).every((slot) => !slot.enabled));
  assert.equal(buildCard.slots[8].commandId, "worker.return");
  assert.equal(buildCard.slots[8].hotkey, "C");
  assert.deepEqual(commandCardActivationCandidates(workerCard, "worker.buildMenu"), [{
    commandId: "worker.buildMenu",
    slotIndex: 6,
    hotkey: "Z",
    label: "Build",
    enabled: true,
  }], "worker build menu dispatch stays descriptor-driven");
  assert.deepEqual(buildCard.slots[0].intent, {
    type: "beginPlacement",
    building: KIND.RESOURCE_DEPOT,
  }, "worker build dispatch keeps placement intent in the descriptor");
  assert.deepEqual(buildCard.slots[8].intent, {
    type: "closeCommandCardMenu",
  }, "worker return dispatch keeps submenu-close intent in the descriptor");
}

{
  const labWorker = { id: 24, owner: 2, kind: KIND.WORKER };
  const labState = {
    selectedEntities() {
      return [labWorker];
    },
  };
  const labPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
  const labCard = buildCommandCardDescriptors({
    spectator: true,
    commandSurfaceEnabled: labPolicy.canUseCommandSurface(labState),
    playerId: 99,
    selection: [labWorker],
    state: labState,
    controlPolicy: labPolicy,
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.deepEqual(buttonSlots(labCard), [
    { commandId: "unit.move", slotIndex: 0, hotkey: "Q" },
    { commandId: "unit.holdPosition", slotIndex: 1, hotkey: "W" },
    { commandId: "unit.attack", slotIndex: 3, hotkey: "A" },
    { commandId: "unit.stop", slotIndex: 4, hotkey: "S" },
    { commandId: "worker.buildMenu", slotIndex: 6, hotkey: "Z" },
    { commandId: "worker.advancedBuildMenu", slotIndex: 7, hotkey: "X" },
  ], "lab operator command card treats the controlled selected owner as commandable");

  const viewerPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } });
  const viewerCard = buildCommandCardDescriptors({
    spectator: true,
    commandSurfaceEnabled: viewerPolicy.canUseCommandSurface(labState),
    selection: [labWorker],
    state: labState,
    controlPolicy: viewerPolicy,
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.equal(viewerCard.kind, "spectator", "read-only lab viewer command card stays hidden");

  const mixedSelection = [
    { id: 25, owner: 1, kind: KIND.RIFLEMAN },
    { id: 26, owner: 2, kind: KIND.RIFLEMAN },
  ];
  const mixedState = {
    selectedEntities() {
      return mixedSelection;
    },
  };
  const mixedCard = buildCommandCardDescriptors({
    spectator: true,
    commandSurfaceEnabled: labPolicy.canUseCommandSurface(mixedState),
    playerId: 99,
    selection: mixedSelection,
    state: mixedState,
    controlPolicy: labPolicy,
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.equal(buttonSlots(mixedCard).length, 0, "mixed-owner lab selections stay non-commandable");
}

{
  const resourceDepot = { id: 27, owner: 1, kind: KIND.RESOURCE_DEPOT, buildProgress: null };
  const resourceDepotCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [resourceDepot],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 20 },
    upgrades: [],
    playerHasCompleteKind: (kind) => kind === KIND.STEELWORKS,
    groupCooldownClocks: () => [],
  });
  const scoutPlaneTrainCommandId = kriegsiaCommandId("train", KIND.SCOUT_PLANE);
  for (const steel of [0, 150]) {
    const card = buildCommandCardDescriptors({
      playerId: 1,
      selection: [resourceDepot],
      resources: { steel, oil: 0, supplyUsed: 0, supplyCap: 20 },
      upgrades: [],
      playerHasCompleteKind: () => true,
      groupCooldownClocks: () => [],
    });
    const pumpJack = card.slots.find((slot) => slot?.id === `train:${KIND.PUMP_JACK}`);
    assert.equal(pumpJack.cost, null, "free depot Pump Jacks have no manual construction cost badge");
    assert.equal(pumpJack.unaffordable, false, "free depot Pump Jacks never require a Steel balance");
    assert.equal(pumpJack.enabled, false, "automatic production remains informational");
    assert.equal(pumpJack.intent, null);
  }
  assert.deepEqual(
    commandCardActivationCandidates(resourceDepotCard, scoutPlaneTrainCommandId),
    [],
    "Resource Depot no longer exposes Scout Plane production",
  );
  const researchedDepotCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [resourceDepot],
    resources: { steel: 38, oil: 56, supplyUsed: 0, supplyCap: 20 },
    upgrades: [UPGRADE.SCOUT_PLANE_UNLOCK],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.deepEqual(
    commandCardActivationCandidates(
      researchedDepotCard,
      kriegsiaCommandId("ability", ABILITY.SCOUT_PLANE),
    ),
    [],
    "researched Resource Depot cannot activate Scout Plane",
  );
  assert.deepEqual(
    resourceDepotCard.slots.slice(0, 3).map((slot) => slot?.commandId),
    [
      kriegsiaCommandId("train", KIND.WORKER),
      kriegsiaCommandId("train", KIND.STEEL_MINE),
      kriegsiaCommandId("train", KIND.PUMP_JACK),
    ],
    "Resource Depot exposes Engineer and both depot-local extractors",
  );
  assert.equal(resourceDepotCard.slots[1].enabled, false);
  assert.equal(resourceDepotCard.slots[1].contextIntent, null);
  assert.equal(resourceDepotCard.slots[1].autobuildIndicatorCount, 1);
  assert.match(resourceDepotCard.slots[1].title, /Automatically builds for free/);
}

{
  const unfinishedBarracks = {
    id: 28,
    owner: 1,
    kind: KIND.BARRACKS,
    buildProgress: 0.4,
  };
  const constructionCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [unfinishedBarracks],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 20 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const rifleman = constructionCard.slots.find((slot) => slot?.id === `train:${KIND.RIFLEMAN}`);
  assert.equal(constructionCard.kind, "construction");
  assert.equal(rifleman?.enabled, false, "unfinished producers cannot train immediately");
  assert.deepEqual(rifleman?.contextIntent, {
    type: "adjustProductionRepeat",
    buildingIds: [unfinishedBarracks.id],
    unit: KIND.RIFLEMAN,
  }, "unfinished producers expose their auto-build hotkey action");
  assert.equal(constructionCard.slots[8]?.commandId, "construction.cancel");
}

for (const [kind, upgrade] of [
  [KIND.TRAINING_CENTRE, UPGRADE.ENTRENCHMENT],
  [KIND.ENGINEERING_COMPLEX, UPGRADE.TANK_UNLOCK],
]) {
  const unfinishedResearchBuilding = {
    id: kind === KIND.TRAINING_CENTRE ? 29 : 30,
    owner: 1,
    kind,
    buildProgress: 0.4,
  };
  const constructionCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [unfinishedResearchBuilding],
    currentEntities: [unfinishedResearchBuilding],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 20 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const research = constructionCard.slots.find((slot) => slot?.id === `research:${upgrade}`);
  assert.equal(constructionCard.kind, "construction");
  assert.equal(research?.enabled, true, `${kind} accepts research while under construction`);
  assert.deepEqual(research?.intent, {
    type: "research",
    upgrade,
    buildingId: unfinishedResearchBuilding.id,
  });
  assert.equal(constructionCard.slots[8]?.commandId, "construction.cancel");
}

{
  const scoutCar = {
    id: 30,
    owner: 1,
    kind: KIND.SCOUT_CAR,
    abilities: [{
      ability: ABILITY.SMOKE,
      cooldownLeft: 0,
      remainingUses: 2,
      chargeRechargeLeft: 0,
    }],
  };
  const abilityCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [scoutCar],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: testGroupCooldownClocks,
  });
  const smokeCommandId = kriegsiaCommandId("ability", ABILITY.SMOKE);
  const smoke = abilityCard.slots.find((slot) => slot?.commandId === smokeCommandId);
  assert.equal(smoke.slotIndex, 5);
  assert.equal(smoke.hotkey, "D");
  assert.equal(smoke.countBadge, "2", "Smoke displays its remaining charge count");
  assert.deepEqual(commandCardActivationCandidates(abilityCard, smokeCommandId), [{
    commandId: smokeCommandId,
    slotIndex: 5,
    hotkey: "D",
    label: "Smoke",
    enabled: true,
  }]);

  const rechargingCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [{
      ...scoutCar,
      abilities: [{
        ability: ABILITY.SMOKE,
        cooldownLeft: 0,
        remainingUses: 0,
        chargeRechargeLeft: 225,
      }],
    }],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: testGroupCooldownClocks,
  });
  const rechargingSmoke = rechargingCard.slots.find((slot) => slot?.commandId === smokeCommandId);
  assert.equal(rechargingSmoke.enabled, false);
  assert.equal(rechargingSmoke.title, "Target a smoke grenade location");
  assert.equal(rechargingSmoke.countBadge, "0");
  assert.equal(rechargingSmoke.cooldownClocks.length, 1);
  assert.equal(rechargingSmoke.cooldownClocks[0].rotationDeg, 180);
  assert.equal(rechargingSmoke.cooldownSeconds, 8, "charge recharge rounds up instead of showing 0s early");
}

// Countdowns describe the next actual timer, not ready units, clock angles, or cluster averages.
{
  const unit = (id, cooldownLeft, extra = {}) => ({
    id, owner: 1, kind: KIND.WAR_ADEPT, dao: "war",
    abilities: [{ ability: ABILITY.THROW_SWORD, cooldownLeft }], ...extra,
  });
  const cardFor = (selection) => buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators", selection, resources: {}, upgrades: [],
    groupCooldownClocks, playerHasCompleteKind: () => true,
  });
  const sword = (card) => card.slots.find((slot) => slot?.ability === ABILITY.THROW_SWORD);
  const mixed = sword(cardFor([
    unit(1, 0), unit(2, 900), unit(3, 600, { daoMaster: true }),
    unit(4, 1, { kind: KIND.WARRIOR }), unit(5, 1, { owner: 2 }),
  ]));
  assert.equal(mixed.cooldownSeconds, 20, "ignore ready units and non-carriers/other owners");
  assert.equal(mixed.countBadge, "1");
  assert.equal(mixed.enabled, true, "the countdown must not disable a partly ready group");
  assert.equal(sword(cardFor([unit(1, 0)])).cooldownSeconds, 0);

  const before = cardFor([unit(1, 31), unit(2, 33)]);
  const after = cardFor([unit(1, 30), unit(2, 32)]);
  assert.equal(sword(after).cooldownClocks[0].cooldownLeft, 31, "nearby timers share an averaged arm");
  assert.equal(sword(after).cooldownSeconds, 1, "use the raw minimum despite the averaged arm crossing 1s");
  assert.equal(before.signature, after.signature, "seconds alone must not rebuild the command card");
  assert.equal(sword(before).cooldownSeconds, 2);

  withFakeDocument(() => {
    const button = createCommandButton({ ...sword(after), onClick() {} });
    assert(button.innerHTML.includes('aria-label="Next cooldown ends in 1 second">1s</span>'));
    const ready = createCommandButton({ ...sword(cardFor([unit(1, 0)])), onClick() {} });
    assert(!ready.innerHTML.includes("cmd-cd-seconds"), "ready buttons have no countdown");
  });
  let label;
  const countdown = { textContent: "2s", classList: fakeClassList(), setAttribute(_name, value) { label = value; } };
  const button = { querySelector: () => countdown, querySelectorAll: () => [] };
  syncCooldownClockElement(button, [], 1);
  assert.equal(countdown.textContent, "1s", "update existing countdown without replacing the button");
  assert.equal(label, "Next cooldown ends in 1 second");
  syncCooldownClockElement(button, [], 100);
  assert(countdown.classList.contains("long"));
  syncCooldownClockElement(button, [], 99);
  assert(!countdown.classList.contains("long"));
}

{
  const artillery = {
    id: 32,
    owner: 1,
    kind: KIND.ARTILLERY,
    setupState: SETUP.DEPLOYED,
    abilities: [
      { ability: ABILITY.POINT_FIRE, cooldownLeft: 0, remainingUses: null },
      { ability: ABILITY.BLANKET_FIRE, cooldownLeft: 0, remainingUses: null },
    ],
  };
  const artilleryCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [artillery],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    commandTarget: { kind: "ability", ability: ABILITY.POINT_FIRE },
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const pointFireCommandId = kriegsiaCommandId("ability", ABILITY.POINT_FIRE);
  const pointFire = artilleryCard.slots.find((slot) => slot?.commandId === pointFireCommandId);
  assert.equal(pointFire.slotIndex, 7);
  assert.equal(pointFire.hotkey, "X");
  assert.equal(pointFire.label, "Fire");
  assert.equal(pointFire.intent.ability, ABILITY.POINT_FIRE);
  assert(pointFire.cls.includes("active"), "unified Artillery Fire reflects active targeting state");
  assert(!artilleryCard.slots.some((slot) => slot?.ability === ABILITY.BLANKET_FIRE));
  assert.deepEqual(duplicateCommandIdsForCard(artilleryCard), [], "unified Artillery Fire keeps a unique command id");
}

{
  const commandCar = {
    id: 31,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    abilities: [{ ability: ABILITY.BREAKTHROUGH, cooldownLeft: 0, remainingUses: null }],
  };
  const commandCarCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [commandCar],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.deepEqual(buttonSlots(commandCarCard), [
    { commandId: "unit.move", slotIndex: 0, hotkey: "Q" },
    { commandId: "unit.holdPosition", slotIndex: 1, hotkey: "W" },
    { commandId: kriegsiaCommandId("ability", ABILITY.BREAKTHROUGH), slotIndex: 2, hotkey: "E" },
    { commandId: "unit.attack", slotIndex: 3, hotkey: "A" },
    { commandId: "unit.stop", slotIndex: 4, hotkey: "S" },
    { commandId: kriegsiaCommandId("ability", ABILITY.SCOUT_PLANE), slotIndex: 8, hotkey: "C" },
  ]);
  assert.equal(commandCarCard.slots[8].cost.steel, 0);
  assert.equal(commandCarCard.slots[8].cost.oil, 0);
}

{
  const mortar = {
    id: 90,
    owner: 1,
    kind: KIND.MORTAR_TEAM,
    hp: 75,
    setupState: SETUP.PACKED,
    abilities: [{ ability: ABILITY.MORTAR_FIRE, cooldownLeft: 0 }],
  };
  const mortarCard = buildCommandCardDescriptors({
    playerId: 1,
    factionId: "kriegsia",
    selection: [mortar],
    entities: [mortar],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.equal(
    buttonSlots(mortarCard).find((slot) => slot.commandId === "unit.setupSupportWeapon"),
    undefined,
    "mortar has no setup command",
  );
}

{
  const abilitiesByKind = {
    [KIND.SCOUT_CAR]: [ABILITY.SMOKE],
    [KIND.MORTAR_TEAM]: [ABILITY.MORTAR_FIRE],
    [KIND.ARTILLERY]: [ABILITY.POINT_FIRE, ABILITY.BLANKET_FIRE],
    [KIND.COMMAND_CAR]: [ABILITY.BREAKTHROUGH, ABILITY.SCOUT_PLANE],
  };
  const everyKriegsiaUnit = factionCatalog("kriegsia").units.map((kind, index) => ({
    id: 100 + index,
    owner: 1,
    kind,
    hp: 100,
    x: index * 32,
    y: 32,
    ...(kind === KIND.ANTI_TANK_GUN || kind === KIND.ARTILLERY
      ? { setupState: SETUP.DEPLOYED }
      : {}),
    abilities: (abilitiesByKind[kind] || []).map((ability) => ({
      ability,
      cooldownLeft: 0,
      remainingUses: ability === ABILITY.SMOKE ? 2 : null,
    })),
  }));
  const combinedCard = buildCommandCardDescriptors({
    playerId: 1,
    factionId: "kriegsia",
    selection: everyKriegsiaUnit,
    entities: everyKriegsiaUnit,
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });

  assert.deepEqual(buttonSlots(combinedCard), [
    { commandId: "unit.move", slotIndex: 0, hotkey: "Q" },
    { commandId: "unit.holdPosition", slotIndex: 1, hotkey: "W" },
    { commandId: kriegsiaCommandId("ability", ABILITY.BREAKTHROUGH), slotIndex: 2, hotkey: "E" },
    { commandId: "unit.attack", slotIndex: 3, hotkey: "A" },
    { commandId: "unit.stop", slotIndex: 4, hotkey: "S" },
    { commandId: kriegsiaCommandId("ability", ABILITY.SMOKE), slotIndex: 5, hotkey: "D" },
    { commandId: "unit.setupSupportWeapon", slotIndex: 6, hotkey: "Z" },
    { commandId: kriegsiaCommandId("ability", ABILITY.MORTAR_FIRE), slotIndex: 7, hotkey: "X" },
    { commandId: kriegsiaCommandId("ability", ABILITY.SCOUT_PLANE), slotIndex: 8, hotkey: "C" },
  ], "mixed ability collisions keep their preferred slots and favor non-artillery abilities");
  assert(!combinedCard.slots.some((slot) =>
    slot?.ability === ABILITY.POINT_FIRE || slot?.ability === ABILITY.BLANKET_FIRE
  ), "unified Artillery Fire yields its colliding slot in a full Kriegsia selection");
}

{
  const fixtureWorker = { id: 40, owner: 1, kind: KIND.WORKER };
  const fixtureBuildCard = buildCommandCardDescriptors({
    playerId: 1,
    factionId: FIXTURE_FACTION_ID,
    selection: [fixtureWorker],
    commandCardMode: "workerBuild",
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.deepEqual(workerBuildablesForFaction(FIXTURE_FACTION_ID), [], "fixture faction has an alternate empty build menu");
  assert.deepEqual(commandCardAbilitiesForFaction(FIXTURE_FACTION_ID), [], "fixture faction does not inherit Kriegsia ability buttons");
  assert.equal(fixtureBuildCard.kind, "workerBuild");
  assert.deepEqual(fixtureBuildCard.slots.slice(0, 8), new Array(8).fill(null));
  assert.equal(fixtureBuildCard.slots[8].commandId, "worker.return");

  const fixtureDepot = { id: 41, owner: 1, kind: KIND.DEPOT, buildProgress: null };
  const fixtureDepotCard = buildCommandCardDescriptors({
    playerId: 1,
    factionId: FIXTURE_FACTION_ID,
    selection: [fixtureDepot],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert.equal(fixtureDepotCard.kind, "empty", "fixture Depot does not inherit Kriegsia production");

  const fixtureScout = {
    id: 42,
    owner: 1,
    kind: KIND.SCOUT_CAR,
    abilities: [{ ability: ABILITY.SMOKE, cooldownLeft: 0, remainingUses: 2 }],
  };
  const fixtureScoutCard = buildCommandCardDescriptors({
    playerId: 1,
    factionId: FIXTURE_FACTION_ID,
    selection: [fixtureScout],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  assert(!fixtureScoutCard.slots.some((slot) => slot?.action === "ability"), "fixture Scout Car does not inherit Smoke");
}

{
  const ekat = {
    id: 41,
    owner: 1,
    kind: KIND.EKAT,
    abilities: [{
      ability: ABILITY.EKAT_TELEPORT,
      cooldownLeft: 120,
      activeObjectId: 77,
      availableTick: 45,
      expiresIn: 90,
    }],
  };
  const card = buildCommandCardDescriptors({
    playerId: 1,
    factionId: EKAT_FACTION_ID,
    selection: [ekat],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const consume = card.slots.find((slot) =>
    slot?.commandId === ekatCommandId("ability", ABILITY.EKAT_CONSUME_GOLEM),
  );
  assert.equal(consume.enabled, true, "non-queueable Consume remains usable immediately");
  assert.deepEqual(consume.intent.readyIds, [41]);
  const dash = card.slots.find((slot) =>
    slot?.commandId === ekatCommandId("ability", ABILITY.EKAT_TELEPORT),
  );
  assert.deepEqual(dash.intent, {
    type: "ability",
    ability: ABILITY.EKAT_TELEPORT,
    targetMode: "recast",
    readyIds: [41],
    targetObjectId: 77,
  });
  assert.equal(dash.enabled, true);
}

{
  const ekat = {
    id: 42,
    owner: 1,
    kind: KIND.EKAT,
    abilities: [{
      ability: ABILITY.EKAT_MAGIC_ANCHOR,
      cooldownLeft: 0,
      lockoutUntilTick: 1900,
    }],
  };
  const card = buildCommandCardDescriptors({
    playerId: 1,
    factionId: EKAT_FACTION_ID,
    selection: [ekat],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const anchor = card.slots.find((slot) =>
    slot?.commandId === ekatCommandId("ability", ABILITY.EKAT_MAGIC_ANCHOR),
  );
  assert.equal(anchor.enabled, false);
  assert.equal(anchor.title, "On cooldown");
}

{
  const catalog = buildCommandCardContextCatalog();
  assert.deepEqual(catalog.map((entry) => entry.id), [
    "empty",
    "worker-main",
    "worker-build",
    "worker-advanced-build",
    "mixed-army-support",
    "command-car",
    "tank",
    "artillery",
    "resource-depot-train",
    "barracks-train",
    "factory-train",
    "gun-works-train",
    "training-centre",
    "engineering-complex",
    "engineering-complex-artillery",
    "ekat-unit",
    "ekat-zamok-train",
    "cultivators-disciple-base",
    "cultivators-disciple-wind",
    "cultivators-disciple-war",
    "cultivators-disciple-magic",
    "cultivators-double-damage",
    "cultivators-war-master",
    "cultivators-shop",
    "cultivators-buy-menu",
    "cultivators-worker",
    "cultivators-build",
    "cultivators-producer-resource_depot",
    "cultivators-producer-portal",
    "cultivators-producer-three_dao_pavilion",
    "cultivators-producer-tournament_grounds",
    "cultivators-producer-proven_grounds",
  ]);
  assert(
    catalog.some((entry) =>
      entry.id === "engineering-complex-artillery" &&
        !entry.card.slots.some((slot) =>
          slot?.commandId === kriegsiaCommandId("research", UPGRADE.BALLISTIC_TABLES)
        )
    ),
    "command-card context catalog hides Fire Control even after Artillery is researched",
  );
  assert(
    catalog.some((entry) =>
      entry.id === "ekat-unit" &&
        entry.card.slots.some((slot) =>
          slot?.commandId === ekatCommandId("ability", ABILITY.EKAT_LINE_SHOT)
        )
    ),
    "command-card context catalog samples Ekat abilities so direct hotkeys can discover them",
  );
  assert.deepEqual(WORKER_BUILDABLE, [
    KIND.RESOURCE_DEPOT,
    KIND.PUMP_JACK,
    KIND.BARRACKS,
    KIND.TRAINING_CENTRE,
    KIND.ENGINEERING_COMPLEX,
    KIND.FACTORY,
    KIND.STEELWORKS,
    KIND.TANK_TRAP,
    KIND.BARBED_WIRE,
  ]);
  for (const entry of catalog) {
    assert.equal(duplicateCommandIdsForCard(entry.card).length, 0, `${entry.id} has duplicate command ids`);
    for (const slot of entry.card.slots) {
      if (!slot) continue;
      assert.equal(typeof slot.commandId, "string", `${entry.id} command has identity`);
      assert.equal(slot.hotkey, slot.passive ? "" : gridHotkeyForSlot(slot.slotIndex));
      if (slot.passive) {
        assert.equal(slot.enabled, false, "passive indicators stay disabled");
        assert.equal(slot.hotkeyCode, "", "passive indicators have no hotkey");
        assert.equal(slot.intent, null, "passive indicators have no cast action");
        assert.equal(slot.contextIntent, null, "passive indicators have no toggle action");
      }
    }
  }

  assert.deepEqual(duplicateCommandIdsForCard({
    slots: [
      { commandId: "unit.move", slotIndex: 0 },
      null,
      { commandId: "unit.move", slotIndex: 2 },
    ],
  }), [{
    commandId: "unit.move",
    firstSlotIndex: 0,
    duplicateSlotIndex: 2,
  }]);
}

{
  const card = buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators",
    selection: [{ id: 1, owner: 1, kind: KIND.WORKER }],
    resources: { steel: 1000, oil: 1000 }, commandCardMode: "workerBuild",
  });
  const builds = card.slots.filter((slot) => slot?.action === "build");
  assert.deepEqual(
    builds.map((slot) => slot.intent.building),
    [KIND.RESOURCE_DEPOT, KIND.PUMP_JACK, KIND.PORTAL, KIND.THREE_DAO_PAVILION, KIND.TOURNAMENT_GROUNDS, KIND.SHOP],
  );
  assert.equal(builds[0].label, "Nexus");
  assert.equal(builds[0].commandId, "cultivators.build." + KIND.RESOURCE_DEPOT);
  assert.equal(builds[1].label, "Pump Jack");
  assert.equal(builds[1].commandId, "cultivators.build." + KIND.PUMP_JACK);
  assert.equal(builds[1].enabled, true);
  assert.deepEqual(builds[1].cost, { steel: 150, oil: 0 });
  assert.equal(builds[2].label, "Portal");
  assert.equal(builds[2].commandId, "cultivators.build." + KIND.PORTAL);
  assert.equal(builds[3].label, "Three Dao Pavilion");
  assert.equal(builds[3].commandId, "cultivators.build." + KIND.THREE_DAO_PAVILION);
  assert.equal(builds[3].hotkey, "A");
  assert.equal(builds[4].label, "Tournament Grounds");
  assert.equal(builds[4].commandId, "cultivators.build." + KIND.TOURNAMENT_GROUNDS);
}

{
  const { statsForFaction } = await import("../client/src/config.js");
  const { selectionBudgetGridModel } = await import("../client/src/hud_selection_panel.js");
  assert.equal(statsForFaction(KIND.RESOURCE_DEPOT, "cultivators").label, "Nexus");
  assert.equal(statsForFaction(KIND.WORKER, "cultivators").label, "Serf");
  assert.equal(statsForFaction(KIND.WORKER, "kriegsia").label, "Engineer");
  const training = buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators",
    selection: [{ id: 2, owner: 1, kind: KIND.RESOURCE_DEPOT }],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 100 },
  });
  assert.equal(training.slots.find((slot) => slot?.action === "train").label, "Serf");
  assert.equal(statsForFaction(KIND.RESOURCE_DEPOT, "kriegsia").label, "Resource Depot");
  const model = selectionBudgetGridModel([
    { id: 10, owner: 1, kind: KIND.RESOURCE_DEPOT },
    { id: 11, owner: 2, kind: KIND.RESOURCE_DEPOT },
  ], null, [{ id: 1, factionId: "cultivators" }, { id: 2, factionId: "kriegsia" }]);
  assert.deepEqual(model.pages.flatMap((page) => page.blocks).map((block) => block.label),
    ["Nexus", "Resource Depot"]);
}

for (const researched of [false, true]) {
  for (const hasVehicleWorks of [false, true]) {
    const card = buildCommandCardDescriptors({
      playerId: 1,
      selection: [{ id: 71, owner: 1, kind: KIND.STEELWORKS }],
      resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 100 },
      upgrades: researched ? [UPGRADE.ROCKETS] : [],
      playerHasCompleteKind: (kind) => kind === KIND.STEELWORKS ||
        (kind === KIND.FACTORY && hasVehicleWorks),
      groupCooldownClocks: () => [],
    });
    const rocketTruck = card.slots.find((slot) => slot?.id === `train:${KIND.ROCKET_LAUNCHER}`);
    assert(rocketTruck, "Gun Works should show Rocket Truck training");
    assert.equal(rocketTruck.enabled, researched,
      "Rocket Truck training requires Rockets research regardless of Vehicle Works ownership");
  }
}

{
  const tank = { id: 90, owner: 1, kind: KIND.TANK };
  const ctx = { playerId: 1, selection: [tank], entities: [tank], upgrades: [] };
  const point = buildCommandCardDescriptors(ctx).slots[7];
  assert.equal(point.commandId, "unit.pointTanks");
  assert.equal(point.hotkey, "X");
  assert.equal(point.enabled, true);
  assert.deepEqual(point.intent, { type: "beginCommandTarget", target: "pointTanks" });
  assert.equal(buildCommandCardDescriptors({ ...ctx, playerId: 2 }).slots.some((s) => s?.commandId === "unit.pointTanks"), false);
}

{
  const card = engineeringComplexCard([UPGRADE.TANK_UNLOCK]);
  assert.equal(card.slots[7]?.id, `research:${UPGRADE.TANK_PLUS}`);
  assert.equal(card.slots[7]?.label, "Tank Plus");
  assert.equal(card.slots[7]?.enabled, true);
}

// TEMPORARY HACK regression: historical Engineering Complex ownership keeps only AT Guns ready.
{
  const gunWorks = { id: 950, owner: 1, kind: KIND.STEELWORKS };
  const context = {
    selection: [gunWorks], playerId: 1,
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 100 },
    playerHasCompleteKind: (kind) => kind === KIND.STEELWORKS,
    upgrades: [],
  };
  const atSlot = (card) => card.slots.find((slot) => slot?.id === `train:${KIND.ANTI_TANK_GUN}`);
  assert.equal(atSlot(buildCommandCardDescriptors(context)).enabled, false);
  const unlocked = buildCommandCardDescriptors({ ...context, upgrades: [UPGRADE.ANTI_TANK_GUN_UNLOCK] });
  assert.equal(atSlot(unlocked).enabled, true);
  assert.equal(atSlot(unlocked).title.includes("Requires Engineering Complex"), false);
  const withoutWorks = buildCommandCardDescriptors({ ...context,
    upgrades: [UPGRADE.ANTI_TANK_GUN_UNLOCK], playerHasCompleteKind: () => false });
  assert.equal(atSlot(withoutWorks).enabled, false);
}

// Insight-gated Disciple evolution occupies the bottom row and swaps to Dao abilities.
{
  const context = {
    playerId: 1, factionId: "cultivators",
    resources: { steel: 1000, oil: 1000 },
    upgrades: [UPGRADE.DAO_WIND, UPGRADE.DAO_OF_HEAVINESS, UPGRADE.DAO_OF_MAGIC],
    playerHasCompleteKind: () => true, groupCooldownClocks: () => [],
  };
  const unit = { id: 50, owner: 1, kind: KIND.WARRIOR, insight: 59 };
  const cardFor = (entity, extra = {}) => buildCommandCardDescriptors({ ...context, ...extra, selection: [entity] });
  let card = cardFor(unit);
  assert.deepEqual(card.slots.slice(6, 9).map((s) => s.ability), [ABILITY.EVOLVE_WIND, ABILITY.EVOLVE_WAR, ABILITY.EVOLVE_MAGIC]);
  assert(card.slots.slice(6, 9).every((s) => s.enabled && !s.cls.includes("dao-ready")));
  card = cardFor({ ...unit, insight: 60 });
  assert(card.slots.slice(6, 9).every((s) => s.enabled && s.cls.includes("dao-ready")));
  assert.deepEqual(card.slots.slice(6, 9).map((s) => s.hotkey), ["Z", "X", "C"]);
  assert(cardFor({ ...unit, insight: 60 }, { upgrades: [] }).slots.slice(6, 9).every((s) => s.enabled));
  const light = cardFor({ ...unit, kind: KIND.WIND_ADEPT, insight: 60, dao: "wind" });
  assert(light.slots.some((s) => s?.ability === ABILITY.WIND_DASH));
  for (const remainingUses of [0, 1, 2]) {
    const master = cardFor({ ...unit, kind: KIND.WIND_ADEPT, daoMaster: true,
      abilities: [{ ability: ABILITY.WIND_DASH, cooldownLeft: 0, remainingUses }] });
    const dash = master.slots.find((slot) => slot?.ability === ABILITY.WIND_DASH);
    assert.equal(dash.countBadge, `${remainingUses}`);
    assert.equal(dash.enabled, remainingUses > 0);
    assert.equal(dash.intent.targetMode, "worldPoint");
  }
  assert(!light.slots.some((s) => s?.ability === ABILITY.EKAT_TELEPORT));
  assert(!light.slots.some((s) => s?.ability === ABILITY.EKAT_MAGIC_ANCHOR || s?.ability === ABILITY.VECTOR_FIELD || s?.ability?.startsWith("evolve")));
  const magic = cardFor({ ...unit, kind: KIND.MAGIC_ADEPT, insight: 60, dao: "magic" });
  assert(magic.slots.some((s) => s?.ability === ABILITY.VECTOR_FIELD && s.label === "Vector Field"));
  assert(!magic.slots.some((s) => s?.ability === ABILITY.EKAT_MAGIC_ANCHOR));
  assert(magic.slots.some((s) => s?.ability === ABILITY.DISCIPLE_SHIELD && s.enabled));
  assert(!magic.slots.some((s) => s?.ability === ABILITY.WIND_DASH));
}

{
  const { PRESENTATION_ENTITY_FIELDS } = await import("../client/src/presentation/entity_snapshot.js");
  assert(PRESENTATION_ENTITY_FIELDS.includes("insight"));
  assert(PRESENTATION_ENTITY_FIELDS.includes("dao"));
}

// Dao choices remain actionable before research and cover every eligible selected unit.
{
  const selection = [
    { id: 1, owner: 1, kind: KIND.WARRIOR, insight: 0, chosenDao: "war" },
    { id: 2, owner: 1, kind: KIND.WARRIOR, insight: 60, chosenDao: "magic" },
    { id: 3, owner: 1, kind: KIND.WAR_ADEPT, insight: 0, dao: "war", daoMaster: true },
    { id: 4, owner: 2, kind: KIND.WARRIOR, insight: 0 },
    { id: 5, owner: 1, kind: KIND.WORKER },
  ];
  const context = { playerId: 1, factionId: "cultivators", selection, upgrades: [],
    resources: { steel: 0, oil: 0 }, playerHasCompleteKind: () => false,
    groupCooldownClocks: () => [] };
  const card = buildCommandCardDescriptors(context);
  for (const ability of [ABILITY.EVOLVE_WIND, ABILITY.EVOLVE_WAR, ABILITY.EVOLVE_MAGIC]) {
    const slot = card.slots.find((slot) => slot?.ability === ability);
    assert(slot.enabled);
    assert.deepEqual(slot.intent.readyIds, [1, 2]);
    assert(slot.title.includes("60 insight"));
    assert(slot.cls.includes("dao-ready"));
    if (ability !== ABILITY.EVOLVE_WIND) {
      assert(slot.cls.includes("active"));
      assert.equal(slot.countBadge, "1/2");
    }
  }
  const sword = card.slots.find((slot) => slot?.ability === ABILITY.THROW_SWORD);
  assert(sword.enabled);
  const charge = card.slots.find((slot) => slot?.ability === ABILITY.CHARGE);
  assert(charge?.passive && !charge.enabled, "mixed Novice/War selection keeps Charge when a slot is available");
  assert.equal(sword.hotkey, "E");
  assert.deepEqual(sword.intent.readyIds, [3]);
  const changed = buildCommandCardDescriptors({ ...context,
    selection: selection.map((e) => e.id === 1 ? { ...e, chosenDao: "wind" } : e) });
  assert.notEqual(changed.signature, card.signature);
}

// Charge must coexist with Adept and Master abilities in mixed selections.
{
  const selection = [KIND.WARRIOR, KIND.WIND_ADEPT, KIND.WAR_ADEPT, KIND.MAGIC_ADEPT]
    .map((kind, index) => ({ id: index + 1, owner: 1, kind, insight: 60, daoMaster: kind === KIND.WAR_ADEPT }));
  selection[0].escapeTreasure = {};
  const card = buildCommandCardDescriptors({ playerId: 1, factionId: "cultivators", selection,
    resources: { steel: 0, oil: 0 }, upgrades: [], groupCooldownClocks: () => [] });
  for (const ability of [ABILITY.WIND_DASH, ABILITY.THROW_SWORD, ABILITY.VECTOR_FIELD]) {
    const slot = card.slots.find((slot) => slot?.ability === ability);
    assert(slot?.enabled, `mixed selection preserves active ${ability}`);
    assert.equal(slot.hotkey, commandCardAbilitiesForFaction("cultivators")
      .find((definition) => definition.ability === ability).hotkey);
  }
  const charge = card.slots.find((slot) => slot?.ability === ABILITY.CHARGE);
  assert.equal(charge, undefined, "a full mixed card gives active abilities priority over passive Charge");
  assert.equal(card.slots[9]?.intent.type, "openShopBuyMenu");
}

// Dao order is chosen by the player; completed and paid active researches reserve tiers.
{
  const dao = UPGRADE.DAO_OF_MAGIC;
  const pavilion = { id: 900, owner: 1, kind: KIND.THREE_DAO_PAVILION,
    prodUpgrade: UPGRADE.DAO_WIND };
  for (const [upgrades, entities, cost, ticks] of [
    [[], [], 50, 600],
    [[UPGRADE.DAO_OF_HEAVINESS], [], 100, 1200],
    [[UPGRADE.DAO_OF_HEAVINESS], [pavilion], 200, 2400],
    [[], [{ ...pavilion, prodWaiting: true }], 50, 600],
    [[], [{ ...pavilion, owner: 2 }], 50, 600],
  ]) {
    const def = researchDefinition(dao, upgrades, entities, 1);
    assert.deepEqual(def.cost, { steel: cost, oil: cost * 2 });
    assert.equal(def.researchTicks, ticks);
  }

  const selected = { id: 901, owner: 1, kind: KIND.THREE_DAO_PAVILION };
  const context = {
    playerId: 1,
    factionId: "cultivators",
    selection: [selected],
    resources: { steel: 75, oil: 150 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  };
  const before = buildCommandCardDescriptors({ ...context, currentEntities: [selected] });
  const after = buildCommandCardDescriptors({ ...context, currentEntities: [selected, pavilion] });
  const slot = (card) => card.slots.find((entry) => entry?.id === `research:${dao}`);
  assert.equal(slot(before).unaffordable, false);
  assert.equal(slot(after).unaffordable, true);
  assert.deepEqual(slot(after).cost, { steel: 100, oil: 200 });
  assert.equal(slot(after).researchDefinition.researchTicks, 1200);
  assert.notEqual(before.signature, after.signature);
}

// Paid training queues plans even while poor; research and queue capacity are the gates.
{
  const building = { id: 42, owner: 1, kind: KIND.PROVEN_GROUNDS,
    daoTrainingQueue: [UPGRADE.DAO_OF_MAGIC, UPGRADE.DAO_OF_HEAVINESS], prodKind: KIND.WARRIOR };
  const ctx = { playerId: 1, factionId: "cultivators", selection: [building], entities: [building],
    resources: { steel: 0, oil: 0 }, upgrades: [UPGRADE.DAO_OF_MAGIC], groupCooldownClocks: () => [] };
  const card = buildCommandCardDescriptors(ctx);
  assert.equal(card.slots[0].enabled, false);
  assert.equal(card.slots[1].enabled, false);
  assert.equal(card.slots[2].enabled, true);
  assert.equal(card.slots[2].countBadge, "1");
  assert.equal(card.slots[2].hotkey, "E");
  assert.deepEqual(card.slots[2].cost, { steel: 0, oil: 100 });
  assert.match(card.slots[2].title, /costs 100 oil on entry/);
  assert.deepEqual(card.slots[2].intent, { type: "research", upgrade: UPGRADE.DAO_OF_MAGIC, buildingId: 42 });
  assert.equal(card.slots[8].enabled, true);
  building.daoTrainingQueue = [UPGRADE.DAO_OF_MAGIC];
  assert.equal(buildCommandCardDescriptors(ctx).slots[8], null, "active trainee cannot be canceled");
  building.daoTrainingQueue = Array(64).fill(UPGRADE.DAO_OF_MAGIC);
  assert.equal(buildCommandCardDescriptors(ctx).slots[2].enabled, false);
}

// The Shop exposes its construction cost and occupies an ordinary build slot.
{
  const card = buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators",
    selection: [{ id: 1, owner: 1, kind: KIND.WORKER }],
    resources: { steel: 200, oil: 200 }, commandCardMode: "workerBuild",
  });
  const shop = card.slots.find((slot) => slot?.intent?.building === KIND.SHOP);
  assert.equal(shop.label, "Shop");
  assert.equal(shop.hotkey, "Z");
  assert.equal(shop.commandId, "cultivators.build.shop");
  const idle = buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators",
    selection: [{ id: 2, owner: 1, kind: KIND.SHOP }],
    groupCooldownClocks: () => [],
    resources: { steel: 1000, oil: 1000 },
  });
  const items = idle.slots.filter(Boolean);
  assert.equal(items.length, 8);
  assert.equal(idle.slots[2].ability, ABILITY.BREAKTHROUGH_TREASURE);
  assert.equal(idle.slots[2].hotkey, "E");
  assert.deepEqual(items.map((item) => item.ability), [ABILITY.ESCAPE_TREASURE, ABILITY.DOUBLE_XP, ABILITY.BREAKTHROUGH_TREASURE, ABILITY.REGEN, ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE, ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE]);
  assert.deepEqual(items.map((item) => item.intent.targetMode), Array(8).fill("friendlyUnit"));
  assert.deepEqual(items.map((item) => item.cost.oil), [100, 50, 100, 50, 100, 100, 100, 150]);
  assert(!items.some((item) => item.ability === ABILITY.HEALING_SEAL));
  assert.deepEqual(idle.rowLabels, ["Util", "Defense", "Offense"]);
}

// Portal Adepts require their Dao, 150 steel, 45 oil and three supply.
{
  const portal = { id: 42, owner: 1, kind: KIND.PORTAL };
  const ctx = { playerId: 1, factionId: "cultivators", selection: [portal], entities: [portal],
    resources: { steel: 150, oil: 45, supplyUsed: 0, supplyCap: 20 }, upgrades: [] };
  const locked = buildCommandCardDescriptors(ctx);
  const choices = [
    [KIND.WIND_ADEPT, UPGRADE.DAO_WIND],
    [KIND.WAR_ADEPT, UPGRADE.DAO_OF_HEAVINESS],
    [KIND.MAGIC_ADEPT, UPGRADE.DAO_OF_MAGIC],
  ];
  for (const [kind, upgrade] of choices) {
    const slotFor = (card) => card.slots.find((slot) => slot?.intent?.unit === kind);
    assert.equal(slotFor(locked).enabled, false);
    assert.match(slotFor(locked).title, /Requires/);
    const unlocked = buildCommandCardDescriptors({ ...ctx, upgrades: [upgrade] });
    assert.equal(slotFor(unlocked).enabled, true);
    assert.equal(slotFor(unlocked).unaffordable, false);
    assert.deepEqual(slotFor(unlocked).cost, { steel: 150, oil: 45 });
    for (const [other] of choices.filter(([other]) => other !== kind)) {
      assert.equal(unlocked.slots.find((slot) => slot?.intent?.unit === other).enabled, false);
    }
    const noSteel = buildCommandCardDescriptors({ ...ctx, upgrades: [upgrade], resources: { ...ctx.resources, steel: 149 } });
    assert.equal(slotFor(noSteel).unaffordable, true);
    const noSupply = buildCommandCardDescriptors({ ...ctx, upgrades: [upgrade], resources: { ...ctx.resources, supplyUsed: 18 } });
    assert.equal(slotFor(noSupply).unaffordable, true);
    const poor = buildCommandCardDescriptors({ ...ctx, upgrades: [upgrade], resources: { ...ctx.resources, oil: 44 } });
    assert.equal(slotFor(poor).unaffordable, true);
  }
}

// The display order keeps carried item categories in the fourth column.
assert.deepEqual(COMMAND_CARD_SLOT_ORDER.map(gridHotkeyForSlot),
  ["Q", "W", "E", "R", "A", "S", "D", "F", "Z", "X", "C", "V"]);
for (const context of buildCommandCardContextCatalog()) {
  for (const slot of context.card.slots.filter(Boolean)) {
    if (slot.slotIndex === 9) {
      assert.equal(slot.intent.type, "openShopBuyMenu");
      continue;
    }
    if (slot.slotIndex === 10) {
      assert([ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE].includes(slot.ability));
      if (slot.commandId === "cultivators.ability.vitalityTreasure") assert.equal(slot.hotkey, "F");
      else assert(slot.passive && !slot.hotkey);
      continue;
    }
    if (slot.slotIndex === 11) {
      assert.equal(slot.intent.type, "closeCommandCardMenu");
      assert.equal(slot.hotkey, "V");
      continue;
    }
    assert(slot.slotIndex < 9, `${context.id}: other fourth-column slots remain reserved`);
    const displayIndex = COMMAND_CARD_SLOT_ORDER.indexOf(slot.slotIndex);
    assert.equal(Math.floor(displayIndex / 4), Math.floor(slot.slotIndex / 3));
    assert.equal(displayIndex % 4, slot.slotIndex % 3);
  }
}

withFakeHudDocument(() => {
  const card = document.createElement("div");
  const slots = Array.from({ length: 9 }, (_, slotIndex) => ({ slotIndex }));
  renderDescriptorCardDom(card, { slots }, (slot) => {
    const button = document.createElement("button");
    button.dataset.slotIndex = slot.slotIndex;
    return button;
  });
  const cells = card.children[0].children;
  assert(cells.length === 12, "command card renders twelve cells");
  assert([3, 7, 11].every((i) => cells[i].className === "cmd-empty"),
    "new right column stays empty");
  assert(cells[4].dataset.slotIndex === 3 && cells[10].dataset.slotIndex === 8,
    "A and C commands retain their original row and column");
});

// Carried items never occupy the command card or bind RFV, including mixed factions.
for (const factionId of ["cultivators", "kriegsia", "ekat"]) {
  const card = buildCommandCardDescriptors({ playerId: 1, factionId,
    selection: [{ id: 1, owner: 1, kind: KIND.WARRIOR, escapeTreasure: true,
      abilities: [ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE, ABILITY.ARMOR_TREASURE,
        ABILITY.VITALITY_TREASURE].map((ability) => ({ ability, remainingUses: 1 })) }],
    resources: {}, groupCooldownClocks: () => [] });
  assert(!card.slots.some((slot) => slot?.commandId?.startsWith("cultivators.item.") && slot.action !== "openShopBuyMenu"));
  for (const index of [10, 11]) assert.equal(card.slots[index] ?? null, null);
}

// Shop goods occupy the same rows as the carried movement/defense/attack slots.
{
  const card = buildCommandCardDescriptors({
    playerId: 1, factionId: "cultivators",
    selection: [{ id: 1, owner: 1, kind: KIND.SHOP }],
    resources: { steel: 0, oil: 1000 }, groupCooldownClocks: () => [],
  });
  const rows = COMMAND_CARD_SLOT_ORDER.map((i) => card.slots[i]?.ability || null);
  assert.deepEqual(rows.slice(0, 4), [ABILITY.ESCAPE_TREASURE, ABILITY.DOUBLE_XP, ABILITY.BREAKTHROUGH_TREASURE, null]);
  assert.deepEqual(rows.slice(4, 8), [ABILITY.REGEN, ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE, null]);
  assert.deepEqual(rows.slice(8), [ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE, null, null]);
  for (const ability of [ABILITY.BREAKTHROUGH_TREASURE, ABILITY.DOUBLE_XP, ABILITY.HASTE, ABILITY.DOUBLE_DAMAGE, ABILITY.ARMOR_TREASURE, ABILITY.VITALITY_TREASURE]) {
    const carried = buildCommandCardDescriptors({
      playerId: 1, factionId: "cultivators",
      selection: [{ id: 2, owner: 1, kind: KIND.WARRIOR,
        abilities: [{ ability, remainingUses: 1, cooldownLeft: 0 }] }],
      resources: {}, groupCooldownClocks: () => [],
    });
    const slot = carried.slots.find((entry) => entry?.ability === ability);
    assert.equal(slot, undefined);
  }
}

// R opens a persistent buy menu without changing selected units or arming a world target.
{
  const unit = { id: 10, owner: 1, kind: KIND.WAR_ADEPT, hp: 176 };
  const shop = { id: 20, owner: 1, kind: KIND.SHOP, hp: 200 };
  const ctx = { playerId: 1, factionId: "cultivators", selection: [unit], entities: [unit, shop],
    resources: { oil: 1000, steel: 0 }, groupCooldownClocks: () => [] };
  const main = buildCommandCardDescriptors(ctx);
  assert.equal(main.slots[9].hotkey, "R");
  assert.equal(main.slots[9].intent.type, "openShopBuyMenu");
  const menu = buildCommandCardDescriptors({ ...ctx, commandCardMode: "shopBuy" });
  assert.deepEqual(menu.rowLabels, ["Util", "Defense", "Offense"]);
  const goods = menu.slots.filter((s) => s?.action === "ability");
  assert.equal(goods.length, 8);
  assert(goods.every((s) => s.intent.type === "buyShopItem" && s.enabled));
  assert.deepEqual(goods.map((s) => s.icon), ["ESC", "2X", "BT", "HP", "ARM", "+HP", "HST", "DD"]);
  assert.equal(menu.slots[11].intent.type, "closeCommandCardMenu");
  assert.equal(menu.slots[11].hotkey, "V");
  assert.equal(menu.slots[1].ability, ABILITY.DOUBLE_XP);
  assert.equal(menu.slots[1].label, "2X XP");
  assert.equal(menu.slots[1].cost.oil, 50);
  assert.equal(menu.slots[4].ability, ABILITY.ARMOR_TREASURE);
  assert.equal(menu.slots[4].hotkey, "S");
  assert.equal(menu.slots[4].label, "+50% armor");
  assert.equal(menu.slots[3].label, "+3 HP per second");
  assert.equal(menu.slots[5].ability, ABILITY.VITALITY_TREASURE);
  assert.equal(menu.slots[5].hotkey, "D");
  const unavailable = buildCommandCardDescriptors({ ...ctx, entities: [unit], commandCardMode: "shopBuy" });
  assert(unavailable.slots.filter((s) => s?.action === "ability").every((s) => !s.enabled && !s.unaffordable));
  const serf = buildCommandCardDescriptors({ ...ctx, selection: [{ ...unit, kind: KIND.WORKER }] });
  assert(!serf.slots.some((s) => s?.intent?.type === "openShopBuyMenu"));
  withFakeHudDocument(() => {
    const element = document.createElement("div");
    renderDescriptorCardDom(element, menu, createCommandButton);
    assert.equal(element.children[0].children.length, 12, "edge tabs preserve twelve grid cells");
    assert.deepEqual(Array.from(element.children[0].children).filter((e) => e.className === "shop-row-start")
      .map((e) => e.children[1].textContent), ["Util", "Defense", "Offense"]);
  });
}


// Shop and Buy reuse command tooltips; short copy never parses the verbose title.
{
  const shop = { id: 20, owner: 1, kind: KIND.SHOP, hp: 200 };
  const card = buildCommandCardDescriptors({ playerId: 1, factionId: "cultivators",
    selection: [shop], entities: [shop], resources: { steel: 1000, oil: 1000 },
    groupCooldownClocks: () => [] });
  const items = card.slots.filter((s) => s?.action === "ability");
  assert.equal(items.length, 8);
  for (const item of items) {
    assert.match(item.tooltipHtml, /cmd-tooltip-title/);
    assert.match(item.tooltipHtml, /cmd-shop-effect/);
    assert.match(item.tooltipHtml, new RegExp(`${item.cost.oil} oil`));
    assert(!item.tooltipHtml.includes("at any distance"));
    withFakeHudDocument(() => {
      const button = createCommandButton(item);
      assert.equal(button.title, "", "rich tooltip suppresses delayed native popup");
      assert.equal(button.getAttribute("aria-label"), item.title);
      assert.match(button.innerHTML, /role="tooltip"/);
    });
  }
  const regen = items.find((s) => s.ability === ABILITY.REGEN);
  assert.match(regen.tooltipHtml, /HP Regen/);
  assert.match(regen.tooltipHtml, /\+3 HP per second/);
  assert.match(regen.tooltipHtml, /Permanent/);
  assert.match(shopItemTooltipHtml(ABILITY.REGEN, { oil: 73 }, "Requires <Shop>"),
    /73 oil.*Requires &lt;Shop&gt;/);
  assert.equal(shopItemTooltipHtml("not-an-item", {}), "");
  withFakeHudDocument(() => {
    const button = createCommandButton({ label: "Move", title: "Move units", enabled: true });
    assert.equal(button.title, "Move units", "plain buttons keep existing native help");
  });
}
