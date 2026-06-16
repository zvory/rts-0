import assert from "node:assert/strict";

import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  commandCardActivationCandidates,
  duplicateCommandIdsForCard,
  factionCommandId,
} from "../client/src/hud_command_card.js";
import {
  EKAT_FACTION_ID,
  FIXTURE_FACTION_ID,
  WORKER_BUILDABLE,
  commandCardAbilitiesForFaction,
  workerBuildablesForFaction,
} from "../client/src/config.js";
import { ABILITY, KIND, UPGRADE } from "../client/src/protocol.js";

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
  assert.equal(ids[1], `research:${UPGRADE.ARTILLERY_UNLOCK}`);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.MORTAR_AUTOCAST}`);
  assert.deepEqual(slotCommandIds(rAndDCard()).slice(0, 4), [
    kriegsiaCommandId("research", UPGRADE.ANTI_TANK_GUN_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.ARTILLERY_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.TANK_UNLOCK),
    kriegsiaCommandId("research", UPGRADE.MORTAR_AUTOCAST),
  ]);
}

{
  const ids = slotIds(rAndDCard([UPGRADE.ANTI_TANK_GUN_UNLOCK]));
  assert.equal(ids[0], null);
  assert.equal(ids[1], `research:${UPGRADE.ARTILLERY_UNLOCK}`);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.MORTAR_AUTOCAST}`);
}

{
  const ids = slotIds(rAndDCard([UPGRADE.ANTI_TANK_GUN_UNLOCK, UPGRADE.ARTILLERY_UNLOCK]));
  assert.equal(ids[0], null);
  assert.equal(ids[1], null);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.MORTAR_AUTOCAST}`);
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
    "city-centre-train",
    "factory-train",
    "gun-works-train",
    "research-complex",
  ]);
  assert.deepEqual(WORKER_BUILDABLE, [
    KIND.CITY_CENTRE,
    KIND.DEPOT,
    KIND.BARRACKS,
    KIND.TRAINING_CENTRE,
    KIND.RESEARCH_COMPLEX,
    KIND.FACTORY,
    KIND.STEELWORKS,
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
