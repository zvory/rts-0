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

// Stale setup commands never create local Mortar Team order stages.
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
    { kind: "move", x: 448, y: 288 },
  ],
  "queued mortar setup is ignored without interrupting later queued commands",
);

// An immediate stale setup command is ignored as well.
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
    { kind: "move", x: 512, y: 288 },
  ],
  "immediate mortar setup does not create a local setup stage",
);

// Ordinary orders on an active constructor replace only the follow-up preview.
const builderIntent = new ClientIntent();
const constructionStage = { kind: "build", x: 120, y: 100 };
const builder = { id: 91, kind: KIND.WORKER, state: "build", targetId: 92,
  orderPlan: [constructionStage, { kind: "move", x: 300, y: 100 }] };
builderIntent.recordPlannedCommand(cmd.move([91], 400, 100), [builder], { sent: true, clientSeq: 101 });
builderIntent.recordPlannedCommand(cmd.move([91], 500, 100, true), [builder], { sent: true, clientSeq: 102 });
assertDeepEqual(builderIntent.plannedOrderPlanForEntity(builder), [constructionStage,
  { kind: "move", x: 400, y: 100 }, { kind: "move", x: 500, y: 100 }],
"constructor preview keeps the scaffold and appends Shift follow-ups");
builderIntent.recordPlannedCommand(cmd.move([91], 600, 100), [builder], { sent: true, clientSeq: 103 });
assertDeepEqual(builderIntent.plannedOrderPlanForEntity(builder), [constructionStage,
  { kind: "move", x: 600, y: 100 }], "latest ordinary click replaces every future marker");
builderIntent.reconcilePlannedOrders([{ ...builder, orderPlan: [constructionStage,
  { kind: "move", x: 600, y: 100 }] }], { acknowledgedClientSeq: 103 });
assertDeepEqual(builderIntent.plannedOrderPlanForEntity({ ...builder, orderPlan: [constructionStage,
  { kind: "move", x: 600, y: 100 }] }), [constructionStage, { kind: "move", x: 600, y: 100 }],
"authoritative handoff confirms the preview without duplication");
builderIntent.recordPlannedCommand(cmd.move([91], 700, 100), [{ ...builder, targetId: null }], { sent: true, clientSeq: 104 });
assertDeepEqual(builderIntent.plannedOrderPlanForEntity(builder), [{ kind: "move", x: 700, y: 100 }],
"workers travelling to a site retain immediate replacement previews");
