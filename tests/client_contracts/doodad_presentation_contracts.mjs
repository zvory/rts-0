import assert from "node:assert/strict";

import {
  MAX_PRESENTED_DOODADS,
  PresentationFrameAssembler,
} from "../../client/src/presentation/frame.js";
import { createMapGenerationMessage } from "../../client/src/renderer/worker_messages.js";
import { createWorkerPresentationState } from "../../client/src/renderer/worker_rehydration.js";

const assembler = new PresentationFrameAssembler({
  map: {
    width: 4,
    height: 3,
    tileSize: 32,
    terrain: new Uint8Array(12),
    resources: [],
    doodads: [
      { id: 1, typeId: "tree.oak", x: 32, y: 48 },
      { id: 2, typeId: "wildflower.cluster", x: 40, y: 52, color: "#AABBCC" },
      { id: 2, typeId: "tree.pine", x: 64, y: 64 },
      { id: 3, typeId: "legacy.forest", x: 20, y: 20 },
      { id: 4, typeId: "tree.birch", x: 72, y: 20 },
      { id: 6, typeId: "tree.aspen", x: 96, y: 20 },
      { id: 5, typeId: "tree.spruce", x: 80, y: 72 },
    ],
  },
});

assert.equal(MAX_PRESENTED_DOODADS, 4096, "static-map presentation enforces the authored doodad cap");
assert.deepEqual(assembler.staticMap.doodads, [
  { id: 1, typeId: "tree.oak", x: 32, y: 48 },
  { id: 2, typeId: "wildflower.cluster", x: 40, y: 52, color: "#aabbcc" },
  { id: 5, typeId: "tree.spruce", x: 80, y: 72 },
], "static-map presentation detaches valid doodads and drops duplicates, removed species, and legacy forest data");
assert(Object.isFrozen(assembler.staticMap.doodads) && Object.isFrozen(assembler.staticMap.doodads[0]),
  "static doodads are immutable presentation records");

const packet = createMapGenerationMessage(assembler.staticMap);
assert.equal(packet.message.payload.map.doodads.length, 3, "map-generation carries doodads once with static terrain");
assert.notEqual(packet.message.payload.map.doodads, assembler.staticMap.doodads, "worker transfer owns a detached doodad array");

const workerState = createWorkerPresentationState();
workerState.reset(1);
const rehydrated = workerState.map(packet.message);
assert.deepEqual(rehydrated.doodads, assembler.staticMap.doodads, "worker rehydration preserves the static doodad DTO");
assert(Object.isFrozen(rehydrated.doodads) && Object.isFrozen(rehydrated.doodads[0]),
  "worker-owned doodad staging cannot be mutated between frames");

const empty = new PresentationFrameAssembler({
  map: { width: 1, height: 1, tileSize: 32, terrain: [0], resources: [] },
});
assert.deepEqual(empty.staticMap.doodads, [], "maps without doodads retain zero-doodad compatibility");
assert.deepEqual(createMapGenerationMessage(empty.staticMap).message.payload.map.doodads, [],
  "zero-doodad maps retain the same one-shot map-generation path");

console.log("✅ doodad_presentation_contracts.mjs: static map normalization and worker lifetime passed");
