// tests/client_contracts/smoke_plus_preview_contracts.mjs
// Smoke Plus targeting preview assertions.

import { assert } from "./assertions.mjs";
import {
  ABILITIES,
  SMOKE_CLOUD_RADIUS_TILES,
} from "../../client/src/config.js";
import {
  ABILITY,
  KIND,
  UPGRADE,
} from "../../client/src/protocol.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";

{
  const smokeInput = Object.create(Input.prototype);
  smokeInput.mouse = { x: 164, y: 100 };
  smokeInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    upgrades: [],
    controlPolicy: {
      kind: "lab",
      isCommandOwner(owner) {
        return Number(owner) === 2;
      },
      commandUpgrades() {
        return [UPGRADE.SMOKE_PLUS];
      },
    },
    selectedEntities: () => [{ id: 77, owner: 2, kind: KIND.SCOUT_CAR, x: 100, y: 100 }],
  };
  smokeInput.clientIntent = new ClientIntent();
  smokeInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.SMOKE });
  smokeInput._groundAtScreen = (x, y) => ({ x, y });
  smokeInput._refreshAbilityTargetPreview();
  const upgradedRadiusTiles = ABILITIES[ABILITY.SMOKE].upgradedRadiusTiles;
  assert(
    smokeInput.clientIntent.abilityTargetPreview?.radiusPx === upgradedRadiusTiles * 32 &&
      upgradedRadiusTiles === SMOKE_CLOUD_RADIUS_TILES * 1.5,
    "Smoke Plus targeting preview uses the command owner's upgraded cloud radius",
  );
}
