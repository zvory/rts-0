import assert from "node:assert/strict";

import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  commandCardActivationCandidates,
  duplicateCommandIdsForCard,
  factionCommandId,
} from "../client/src/hud_command_card.js";
import { createLabControlPolicy } from "../client/src/lab_control_policy.js";
import {
  EKAT_FACTION_ID,
  FIXTURE_FACTION_ID,
  WORKER_BUILDABLE,
  commandCardAbilitiesForFaction,
  workerBuildablesForFaction,
} from "../client/src/config.js";
import { ABILITY, KIND, LAB_ROLE, SETUP, UPGRADE } from "../client/src/protocol.js";

const kriegsiaCommandId = (family, subject) => factionCommandId("kriegsia", family, subject);
const ekatCommandId = (family, subject) => factionCommandId(EKAT_FACTION_ID, family, subject);

const researchComplex = {
  id: 10,
  owner: 1,
  kind: KIND.RESEARCH_COMPLEX,
};

function rAndDCard(upgrades = []) {
  return buildCommandCardDescriptors({
    playerId: 1,
    selection: [researchComplex],
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
  const ids = slotIds(rAndDCard());
  assert.equal(ids[0], `research:${UPGRADE.ANTI_TANK_GUN_UNLOCK}`);
  assert.equal(ids[1], `research:${UPGRADE.BALLISTIC_TABLES}`);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.COMMAND_CAR_UNLOCK}`);
  assert.equal(ids[4], `research:${UPGRADE.MORTAR_AUTOCAST}`);
  assert.equal(ids[5], `research:${UPGRADE.SMOKE_PLUS}`);
  assert.deepEqual(slotCommandIds(rAndDCard()).slice(0, 6), [
    kriegsiaCommandId("research", UPGRADE.ANTI_TANK_GUN_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.BALLISTIC_TABLES),
    kriegsiaCommandId("research", UPGRADE.TANK_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.COMMAND_CAR_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.MORTAR_AUTOCAST),
    kriegsiaCommandId("research", UPGRADE.SMOKE_PLUS),
  ]);
  assert.equal(rAndDCard().slots[1].enabled, false);
  assert.equal(rAndDCard().slots[1].title, "Requires Heavy Guns");
  assert.equal(rAndDCard().slots[1].label, "Artillery Fire Control");
  assert.equal(rAndDCard().slots[1].icon, "AFC");
  assert(!ids.includes(`research:${UPGRADE.ARTILLERY_UNLOCK}`));
}

{
  const card = rAndDCard([UPGRADE.ANTI_TANK_GUN_UNLOCK]);
  const ids = slotIds(card);
  assert.equal(ids[0], `research:${UPGRADE.ARTILLERY_UNLOCK}`);
  assert.equal(ids[1], `research:${UPGRADE.BALLISTIC_TABLES}`);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.COMMAND_CAR_UNLOCK}`);
  assert.equal(ids[4], `research:${UPGRADE.MORTAR_AUTOCAST}`);
  assert.equal(ids[5], `research:${UPGRADE.SMOKE_PLUS}`);
  assert.equal(card.slots[0].label, "Heavy Guns");
  assert.equal(card.slots[0].enabled, true);
  assert.equal(card.slots[1].enabled, false);
  assert.equal(card.slots[1].title, "Requires Heavy Guns");
}

{
  const ids = slotIds(rAndDCard([UPGRADE.ANTI_TANK_GUN_UNLOCK, UPGRADE.TANK_UNLOCK]));
  assert.equal(ids[0], `research:${UPGRADE.ARTILLERY_UNLOCK}`);
  assert.equal(ids[1], `research:${UPGRADE.BALLISTIC_TABLES}`);
  assert.equal(ids[2], null);
  assert.equal(ids[3], `research:${UPGRADE.COMMAND_CAR_UNLOCK}`);
  assert.equal(ids[4], `research:${UPGRADE.MORTAR_AUTOCAST}`);
  assert.equal(ids[5], `research:${UPGRADE.SMOKE_PLUS}`);
}

{
  const card = rAndDCard([UPGRADE.ANTI_TANK_GUN_UNLOCK, UPGRADE.ARTILLERY_UNLOCK]);
  const ids = slotIds(card);
  assert.equal(ids[0], null);
  assert.equal(ids[1], `research:${UPGRADE.BALLISTIC_TABLES}`);
  assert.equal(card.slots[1].enabled, true);
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
  assert.equal(buildCard.slots[0].commandId, kriegsiaCommandId("build", KIND.CITY_CENTRE));
  assert.equal(buildCard.slots[0].slotIndex, 0);
  assert.equal(buildCard.slots[0].hotkey, "Q");
  assert.equal(buildCard.slots[7].commandId, kriegsiaCommandId("build", KIND.TANK_TRAP));
  assert.equal(buildCard.slots[7].slotIndex, 7);
  assert.equal(buildCard.slots[7].hotkey, "X");
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
    building: KIND.CITY_CENTRE,
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
  const cityCentre = { id: 27, owner: 1, kind: KIND.CITY_CENTRE, buildProgress: null };
  const cityCentreCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [cityCentre],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 20 },
    upgrades: [],
    playerHasCompleteKind: (kind) => kind === KIND.STEELWORKS,
    groupCooldownClocks: () => [],
  });
  const scoutPlaneCommandId = kriegsiaCommandId("train", KIND.SCOUT_PLANE);
  assert.deepEqual(
    commandCardActivationCandidates(cityCentreCard, scoutPlaneCommandId),
    [],
    "City Centre no longer exposes Scout Plane production",
  );
}

{
  const barracks = { id: 28, owner: 1, kind: KIND.BARRACKS, buildProgress: null };
  const panzerfaustCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [barracks],
    resources: { steel: 1000, oil: 1000, supplyUsed: 0, supplyCap: 20 },
    upgrades: [],
    playerHasCompleteKind: (kind) => kind === KIND.TRAINING_CENTRE,
    groupCooldownClocks: () => [],
  });
  const panzerfaustCommandId = kriegsiaCommandId("train", KIND.PANZERFAUST);
  assert.deepEqual(buttonSlots(panzerfaustCard).slice(0, 3), [
    { commandId: kriegsiaCommandId("train", KIND.RIFLEMAN), slotIndex: 0, hotkey: "Q" },
    { commandId: kriegsiaCommandId("train", KIND.MACHINE_GUNNER), slotIndex: 1, hotkey: "W" },
    { commandId: panzerfaustCommandId, slotIndex: 2, hotkey: "E" },
  ]);
  assert.deepEqual(commandCardActivationCandidates(panzerfaustCard, panzerfaustCommandId), [{
    commandId: panzerfaustCommandId,
    slotIndex: 2,
    hotkey: "E",
    label: "Panzerfaust",
    enabled: true,
  }]);
}

{
  const scoutCar = {
    id: 30,
    owner: 1,
    kind: KIND.SCOUT_CAR,
    abilities: [{ ability: ABILITY.SMOKE, cooldownLeft: 0, remainingUses: 1 }],
  };
  const abilityCard = buildCommandCardDescriptors({
    playerId: 1,
    selection: [scoutCar],
    resources: { steel: 1000, oil: 1000 },
    upgrades: [],
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const smokeCommandId = kriegsiaCommandId("ability", ABILITY.SMOKE);
  const smoke = abilityCard.slots.find((slot) => slot?.commandId === smokeCommandId);
  assert.equal(smoke.slotIndex, 5);
  assert.equal(smoke.hotkey, "D");
  assert.deepEqual(commandCardActivationCandidates(abilityCard, smokeCommandId), [{
    commandId: smokeCommandId,
    slotIndex: 5,
    hotkey: "D",
    label: "Smoke",
    enabled: true,
  }]);
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
    commandTarget: { kind: "ability", ability: ABILITY.BLANKET_FIRE },
    playerHasCompleteKind: () => true,
    groupCooldownClocks: () => [],
  });
  const pointFireCommandId = kriegsiaCommandId("ability", ABILITY.POINT_FIRE);
  const blanketFireCommandId = kriegsiaCommandId("ability", ABILITY.BLANKET_FIRE);
  const pointFire = artilleryCard.slots.find((slot) => slot?.commandId === pointFireCommandId);
  const blanketFire = artilleryCard.slots.find((slot) => slot?.commandId === blanketFireCommandId);
  assert.equal(pointFire.slotIndex, 7);
  assert.equal(pointFire.hotkey, "X");
  assert.equal(pointFire.label, "Point Fire");
  assert.equal(blanketFire.slotIndex, 8);
  assert.equal(blanketFire.hotkey, "C");
  assert.equal(blanketFire.label, "Blanket Fire");
  assert.equal(blanketFire.intent.ability, ABILITY.BLANKET_FIRE);
  assert(!pointFire.cls.includes("active"), "Point Fire does not share Blanket Fire active state");
  assert(blanketFire.cls.includes("active"), "Blanket Fire has its own active targeting state");
  assert.deepEqual(duplicateCommandIdsForCard(artilleryCard), [], "artillery fire buttons keep distinct command ids");
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
  assert.equal(commandCarCard.slots[8].cost.steel, 50);
  assert.equal(commandCarCard.slots[8].cost.oil, 50);
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
    abilities: [{ ability: ABILITY.SMOKE, cooldownLeft: 0, remainingUses: 1 }],
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
    "mixed-army-support",
    "command-car",
    "city-centre-train",
    "barracks-train",
    "factory-train",
    "gun-works-train",
    "research-complex",
    "research-complex-medium-guns",
  ]);
  assert(
    catalog.some((entry) =>
      entry.id === "research-complex-medium-guns" &&
        entry.card.slots.some((slot) =>
          slot?.commandId === kriegsiaCommandId("research", UPGRADE.ARTILLERY_UNLOCK)
        )
    ),
    "command-card context catalog samples Heavy Guns after Medium Guns so direct hotkeys can discover it",
  );
  assert.deepEqual(WORKER_BUILDABLE, [
    KIND.CITY_CENTRE,
    KIND.DEPOT,
    KIND.BARRACKS,
    KIND.TRAINING_CENTRE,
    KIND.RESEARCH_COMPLEX,
    KIND.FACTORY,
    KIND.STEELWORKS,
    KIND.TANK_TRAP,
  ]);
  for (const entry of catalog) {
    assert.equal(duplicateCommandIdsForCard(entry.card).length, 0, `${entry.id} has duplicate command ids`);
    for (const slot of entry.card.slots) {
      if (!slot) continue;
      assert.equal(typeof slot.commandId, "string", `${entry.id} command has identity`);
      assert.equal(slot.hotkey, ["Q", "W", "E", "A", "S", "D", "Z", "X", "C"][slot.slotIndex]);
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
