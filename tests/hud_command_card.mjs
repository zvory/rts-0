import assert from "node:assert/strict";

import { buildCommandCardDescriptors } from "../client/src/hud_command_card.js";
import { KIND, UPGRADE } from "../client/src/protocol.js";

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

{
  const ids = slotIds(rAndDCard());
  assert.equal(ids[0], `research:${UPGRADE.AT_GUN_UNLOCK}`);
  assert.equal(ids[1], `research:${UPGRADE.ARTILLERY_UNLOCK}`);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.MORTAR_AUTOCAST}`);
}

{
  const ids = slotIds(rAndDCard([UPGRADE.AT_GUN_UNLOCK]));
  assert.equal(ids[0], null);
  assert.equal(ids[1], `research:${UPGRADE.ARTILLERY_UNLOCK}`);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.MORTAR_AUTOCAST}`);
}

{
  const ids = slotIds(rAndDCard([UPGRADE.AT_GUN_UNLOCK, UPGRADE.ARTILLERY_UNLOCK]));
  assert.equal(ids[0], null);
  assert.equal(ids[1], null);
  assert.equal(ids[2], `research:${UPGRADE.TANK_UNLOCK}`);
  assert.equal(ids[3], `research:${UPGRADE.MORTAR_AUTOCAST}`);
}
