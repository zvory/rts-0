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

  const tanks = Array.from({ length: 6 }, (_, index) => ({
    id: index + 1,
    owner: 1,
    kind: KIND.TANK,
    state: STATE.IDLE,
  }));
  const overBudget = commandWithinBudget(
    budgetState(tanks),
    cmd.move(tanks.map((tank) => tank.id), 100, 100),
  );
  assert(!overBudget.ok, "client command guard rejects six tanks without a Command Car");
  assert(overBudget.used === 30 && overBudget.cap === BASE_COMMAND_SUPPLY_CAP, "client reports base command budget usage");

  const commandCar = { id: 99, owner: 1, kind: KIND.COMMAND_CAR, state: STATE.IDLE };
  const stillOverWithCar = commandWithinBudget(
    budgetState(tanks.concat(commandCar)),
    cmd.attackMove(tanks.map((tank) => tank.id).concat(commandCar.id), 100, 100),
  );
  assert(!stillOverWithCar.ok, "Command Car does not make six tanks commandable");
  assert(
    stillOverWithCar.used === 30 &&
      stillOverWithCar.cap === BASE_COMMAND_SUPPLY_CAP + COMMAND_CAR_SUPPLY_CAP_BONUS,
    "Command Car has zero command cost and adds no capacity",
  );

  const fiveTanksWithCar = commandWithinBudget(
    budgetState(tanks.slice(0, 5).concat(commandCar)),
    cmd.stop(tanks.slice(0, 5).map((tank) => tank.id).concat(commandCar.id)),
  );
  assert(fiveTanksWithCar.ok && fiveTanksWithCar.used === 25, "five Tanks and a Command Car fit the base budget");

  const legalInfantry = Array.from({ length: 25 }, (_, index) => ({
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
    "client command guard allows 25 one-supply units",
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
