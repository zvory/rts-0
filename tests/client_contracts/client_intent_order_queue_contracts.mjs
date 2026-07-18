// tests/client_contracts/client_intent_order_queue_contracts.mjs

import { ClientIntent } from "../../client/src/client_intent.js";
import { KIND, cmd } from "../../client/src/protocol.js";
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

// Queued mortar setup is terminal, while the shared support-weapon setup stage remains
// non-terminal for directional Anti-Tank Guns and Artillery.
const mortarSetupIntent = new ClientIntent();
const mortar = { id: 77, kind: KIND.MORTAR_TEAM, x: 96, y: 96, orderPlan: [] };
mortarSetupIntent.recordPlannedCommand(
  cmd.move([mortar.id], 320, 288, true),
  [mortar],
  { sent: true, clientSeq: 80 },
);
mortarSetupIntent.recordPlannedCommand(
  cmd.setupAntiTankGuns([mortar.id], mortar.x, mortar.y, true),
  [mortar],
  { sent: true, clientSeq: 81 },
);
mortarSetupIntent.recordPlannedCommand(
  cmd.move([mortar.id], 448, 288, true),
  [mortar],
  { sent: true, clientSeq: 82 },
);
assertDeepEqual(
  mortarSetupIntent.plannedOrderPlanForEntity(mortar),
  [
    { kind: "move", x: 320, y: 288 },
    { kind: "setupAntiTankGuns", x: mortar.x, y: mortar.y },
  ],
  "queued mortar setup remains after preceding movement and stops later queued commands",
);

// Immediate mortar setup replaces the old plan but does not prevent a later queued command.
const immediateMortarSetupIntent = new ClientIntent();
immediateMortarSetupIntent.recordPlannedCommand(
  cmd.setupAntiTankGuns([mortar.id], mortar.x, mortar.y),
  [mortar],
  { sent: true, clientSeq: 90 },
);
immediateMortarSetupIntent.recordPlannedCommand(
  cmd.move([mortar.id], 512, 288, true),
  [mortar],
  { sent: true, clientSeq: 91 },
);
assertDeepEqual(
  immediateMortarSetupIntent.plannedOrderPlanForEntity(mortar),
  [
    { kind: "setupAntiTankGuns", x: mortar.x, y: mortar.y },
    { kind: "move", x: 512, y: 288 },
  ],
  "immediate mortar setup still allows a subsequent queued command",
);
