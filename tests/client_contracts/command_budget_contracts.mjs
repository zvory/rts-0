// tests/client_contracts/command_budget_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import {
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  STATS,
} from "../../client/src/config.js";
import { admitSelectionIds, commandWithinBudget } from "../../client/src/command_budget.js";
import {
  KIND,
  STATE,
  cmd,
} from "../../client/src/protocol.js";

// Command Budget
// ---------------------------------------------------------------------------
{
  function budgetState(entities) {
    const byId = new Map(entities.map((entity) => [entity.id, entity]));
    return {
      entityById(id) {
        return byId.get(id);
      },
      isOwnOwner(owner) {
        return owner === 1;
      },
    };
  }

  const tanks = Array.from({ length: 5 }, (_, index) => ({
    id: index + 1,
    owner: 1,
    kind: KIND.TANK,
    state: STATE.IDLE,
  }));
  const overBudget = commandWithinBudget(
    budgetState(tanks),
    cmd.move(tanks.map((tank) => tank.id), 100, 100),
  );
  assert(!overBudget.ok, "client command guard rejects five tanks without a Command Car");
  assert(overBudget.used === 40 && overBudget.cap === BASE_COMMAND_SUPPLY_CAP, "client reports base command budget usage");

  const commandCar = { id: 99, owner: 1, kind: KIND.COMMAND_CAR, state: STATE.IDLE };
  const legalWithCar = commandWithinBudget(
    budgetState(tanks.concat(commandCar)),
    cmd.attackMove(tanks.map((tank) => tank.id).concat(commandCar.id), 100, 100),
  );
  assert(legalWithCar.ok, "client command guard allows five tanks with one Command Car");
  assert(
    legalWithCar.used === 44 &&
      legalWithCar.cap === BASE_COMMAND_SUPPLY_CAP + COMMAND_CAR_SUPPLY_CAP_BONUS + STATS[KIND.COMMAND_CAR].supply,
    "client command guard offsets Command Car supply before adding bonus",
  );

  const legalInfantry = Array.from({ length: 24 }, (_, index) => ({
    id: index + 200,
    owner: 1,
    kind: KIND.RIFLEMAN,
    state: STATE.IDLE,
  }));
  assert(
    commandWithinBudget(
      budgetState(legalInfantry),
      cmd.stop(legalInfantry.map((entity) => entity.id)),
    ).ok,
    "client command guard allows 24 one-supply units",
  );

  const labState = budgetState(tanks);
  const controlPolicy = { kind: "lab" };
  assert(
    admitSelectionIds(labState, tanks.map((tank) => tank.id), { controlPolicy }).ids.length === tanks.length,
    "lab selection is not changed by the command-limit option",
  );
  assert(
    !commandWithinBudget(labState, cmd.move(tanks.map((tank) => tank.id), 100, 100), { ownerId: 1 }).ok,
    "lab command guard can still apply command supply when limits are enabled",
  );
  assert(
    commandWithinBudget(labState, cmd.move(tanks.map((tank) => tank.id), 100, 100), {
      ownerId: 1,
      ignoreCommandLimits: true,
    }).ok,
    "lab command guard can ignore command supply when command limits are disabled",
  );
}

// ---------------------------------------------------------------------------
