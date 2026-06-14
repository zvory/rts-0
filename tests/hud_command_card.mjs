import assert from "node:assert/strict";

import {
  buildCommandCardContextCatalog,
  buildCommandCardDescriptors,
  commandCardActivationCandidates,
  duplicateCommandIdsForCard,
  factionCommandId,
} from "../client/src/hud_command_card.js";
import { WORKER_BUILDABLE } from "../client/src/config.js";
import { ABILITY, KIND, UPGRADE } from "../client/src/protocol.js";

const kriegsiaCommandId = (family, subject) => factionCommandId("kriegsia", family, subject);

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
