// tests/client_contracts/client_intent_order_queue_contracts.mjs

import { ClientIntent } from "../../client/src/client_intent.js";
import { cmd } from "../../client/src/protocol.js";
import { assertDeepEqual } from "./assertions.mjs";

// Shift-held Hold Position is a terminal local order stage after queued movement.
const queuedHoldIntent = new ClientIntent();
const queuedHoldUnit = { id: 76, x: 64, y: 96, orderPlan: [] };
queuedHoldIntent.recordPlannedCommand(
  cmd.move([queuedHoldUnit.id], 320, 288, false),
  [queuedHoldUnit],
  { sent: true, clientSeq: 70 },
);
queuedHoldIntent.recordPlannedCommand(
  cmd.holdPosition([queuedHoldUnit.id], true),
  [queuedHoldUnit],
  { sent: true, clientSeq: 71 },
);
queuedHoldIntent.recordPlannedCommand(
  cmd.move([queuedHoldUnit.id], 448, 288, true),
  [queuedHoldUnit],
  { sent: true, clientSeq: 72 },
);
assertDeepEqual(
  queuedHoldIntent.plannedOrderPlanForEntity(queuedHoldUnit),
  [
    { kind: "move", x: 320, y: 288 },
    { kind: "holdPosition" },
  ],
  "Shift-held Hold Position remains after queued movement and stops later queued commands",
);
queuedHoldIntent.recordPlannedCommand(
  cmd.holdPosition([queuedHoldUnit.id]),
  [queuedHoldUnit],
  { sent: true, clientSeq: 73 },
);
assertDeepEqual(
  queuedHoldIntent.plannedOrderPlanForEntity(queuedHoldUnit),
  [],
  "an immediate Hold Position still replaces a locally queued plan",
);
