import { assert } from "./assertions.mjs";
import { ClientIntent } from "../../client/src/client_intent.js";
import { CommandInteraction } from "../../client/src/command_interaction.js";
import { createControlPolicyProjection } from "../../client/src/control_policy_projection.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { LAB_ROLE } from "../../client/src/protocol.js";

const mutableLabPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
const readOnlyPolicy = createControlPolicyProjection(mutableLabPolicy);
assert(Object.isFrozen(readOnlyPolicy), "control-policy projection is frozen");
assert(readOnlyPolicy.canUseCommandSurface({ spectator: true }), "projection preserves Lab operator access");
assert(
  !("setIgnoreCommandLimits" in readOnlyPolicy) &&
    !("ignoreCommandLimitsEnabled" in readOnlyPolicy) &&
    !("issueCommand" in readOnlyPolicy) &&
    !("labClient" in readOnlyPolicy),
  "projection excludes mutable Lab settings, command authority, and transport",
);

const selected = [{ id: 41, owner: 1, orderPlan: [] }];
const queuedMove = { c: "move", units: [41], x: 96, y: 128, queued: true };

for (const name of ["Input", "HUD", "Minimap"]) {
  const issued = [];
  const clientIntent = new ClientIntent({ now: () => 1000 });
  const interaction = new CommandInteraction({
    commandIssuer: {
      issueCommand(command, options) {
        issued.push({ command, options });
        return { sent: true, predicted: true, clientSeq: 9 };
      },
    },
    clientIntent,
    selectedEntities: () => selected,
  });
  const options = name === "HUD" ? { predictMovement: false } : undefined;
  interaction.issueCommand(queuedMove, options);

  assert(issued.length === 1, `${name} issues the shared command once`);
  assert(issued[0].command === queuedMove, `${name} preserves the command object`);
  if (name === "HUD") {
    assert(issued[0].options === options, "HUD preserves command interaction options");
  }
  const planned = clientIntent.plannedOrderPlanForEntity(selected[0]);
  assert(planned.length === 1 && planned[0].kind === "move", `${name} records the queued planned command once`);
}

for (const name of ["Input", "HUD", "Minimap"]) {
  let issued = 0;
  const clientIntent = new ClientIntent({ now: () => 1000 });
  const interaction = new CommandInteraction({
    commandIssuer: {
      issueCommand() {
        issued += 1;
        return Promise.resolve({ sent: true, predicted: false, playerId: 2 });
      },
    },
    clientIntent,
    selectedEntities: () => selected,
  });
  const result = interaction.issueCommand(queuedMove);

  assert(result instanceof Promise && issued === 1, `${name} issues asynchronous Lab commands once`);
  assert(
    clientIntent.plannedOrderPlanForEntity(selected[0]).length === 0,
    `${name} records no optimistic planned order for Lab issue-as`,
  );
}

console.log("✅ command_interaction_contracts.mjs: shared command interaction passed");
