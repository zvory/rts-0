import { assert } from "./assertions.mjs";
import { createWorkerPresentationState } from "../../client/src/renderer/worker_rehydration.js";

const state = createWorkerPresentationState();
state.reset(1);
const staticMap = state.map(request("mapGeneration", {
  map: {
    version: 1,
    revision: 1,
    width: 1,
    height: 1,
    tileSize: 32,
    terrain: grid(1, [0]),
    elevation: grid(1, [3]),
    sun: { azimuthDegrees: 315, elevationDegrees: 12, warmth: 75 },
  },
}));
assert(staticMap.elevation.values[0] === 3 && staticMap.sun.warmth === 75,
  "worker rehydration retains elevation and sun in the static map");
state.revisions(request("revisionedGrids", {
  revisions: { visible: grid(1, [1]), explored: grid(1, [1]) },
}));
state.retainDecals(request("durableDecals", { revision: 1, decals: [decal(1)] }));
const first = state.frame(request("frame", { frame: frame(1, 1) }));
state.retainDecals(request("durableDecals", { revision: 2, decals: [decal(2)] }));
state.decalsPresented(first.groundDecalRevision);
const second = state.frame(request("frame", { frame: frame(2, 2) }));
assert(second.layers.persistentGroundMark.some((record) => record.id === 2),
  "acknowledging an older frame retains a newer independently delivered decal revision");
assert(!second.layers.persistentGroundMark.some((record) => record.id === 1),
  "acknowledging an older frame retires only the durable decals that frame included");
state.resetDecals(request("resetGroundDecals", { decalEpoch: 1 }));
assert(!state.retainDecals(request("durableDecals", { decalEpoch: 0, revision: 3, decals: [decal(3)] })),
  "a decal-only reset rejects durable updates queued for the previous viewpoint epoch");
assert(state.retainDecals(request("durableDecals", { decalEpoch: 1, revision: 1, decals: [decal(4)] })),
  "a decal-only reset accepts a lower authoritative revision for the replacement viewpoint");

function request(type, payload) {
  return { generation: 1, type, payload };
}

function grid(revision, values) {
  return { version: 1, revision, width: 1, height: 1, values: Uint8Array.from(values).buffer };
}

function decal(id) {
  return { type: "groundDecal", id };
}

function frame(frameId, revision) {
  return {
    frameId,
    staticMapRevision: 1,
    visible: { revision: 1 },
    explored: { revision: 1 },
    groundDecalRevision: revision,
    layers: { persistentGroundMark: [] },
  };
}
