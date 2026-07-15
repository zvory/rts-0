import { assert } from "./assertions.mjs";
import {
  validateLabHellholeFacts,
  validateLabHellholeSample,
} from "../../scripts/client-perf/lab_hellhole_setup.mjs";
import {
  buildClientPerfWorkloads,
  SUPPLY_300_LAB_HELLHOLE,
} from "../../scripts/client-perf/workloads.mjs";
import { labHellholeSampleErrors } from "../../scripts/client-perf/workload_setup.mjs";

const descriptor = SUPPLY_300_LAB_HELLHOLE;
const validFacts = {
  scenarioId: descriptor.scenarioId,
  map: descriptor.map,
  labMode: true,
  visionMode: "all",
  playerIds: [1, 2],
  godModePlayers: [1, 2],
  supplyByOwner: { 1: 300, 2: 300 },
  countsByOwner: descriptor.countsByOwner,
  entityCount: descriptor.projectedEntityCount,
  snapshotCodec: "messagepack-compact",
  snapshotFrameKind: "binary",
  snapshotMessageCount: 2,
};

{
  const workload = buildClientPerfWorkloads({}).find((entry) => entry.id === "supply-300-lab-hellhole");
  assert(workload?.kind === "labScenario", "Hellhole workload launches through the live Lab route");
  assert(workload.url.includes("scenario=supply-300-hellhole"), "Hellhole workload pins the scenario id");
  assert(validateLabHellholeFacts(validFacts, descriptor).length === 0,
    "Hellhole descriptor accepts the exact live Lab contract");
}

for (const [label, patch] of [
  ["scenario", { scenarioId: "lategame" }],
  ["map", { map: "No Terrain" }],
  ["supply", { supplyByOwner: { 1: 299, 2: 300 } }],
  ["unit counts", { countsByOwner: { ...descriptor.countsByOwner, 1: { ...descriptor.countsByOwner[1], tank: 8 } } }],
  ["god mode", { godModePlayers: [1] }],
  ["codec", { snapshotCodec: "json", snapshotFrameKind: "text" }],
]) {
  assert(validateLabHellholeFacts({ ...validFacts, ...patch }, descriptor).length > 0,
    `Hellhole setup rejects wrong ${label}`);
}

{
  const sample = {
    setupResult: { initialRenderedFrames: 20 },
    monitor: {
      snapshotCount: 10,
      combatSnapshotCount: 4,
      attackEventCount: 12,
      minEntityCount: descriptor.projectedEntityCount,
      maxEntityCount: descriptor.projectedEntityCount,
      lastSnapshotTick: 600,
      lastCombatTick: 590,
    },
    finalFrameCount: 40,
  };
  const errors = validateLabHellholeSample(sample, descriptor);
  assert(errors.length === 0, "Hellhole sampling accepts stable, rendered, continuing combat");
  assert(labHellholeSampleErrors(
    { labHellhole: descriptor },
    { labHellhole: sample.setupResult },
    { labHellholeMonitor: sample.monitor, perf: { summary: { frameCount: sample.finalFrameCount } } },
  ).length === 0, "Hellhole workload setup accepts the valid live sample");
}

{
  const errors = validateLabHellholeSample({
    setupResult: { initialRenderedFrames: 20 },
    monitor: {
      snapshotCount: 1,
      combatSnapshotCount: 0,
      attackEventCount: 0,
      minEntityCount: descriptor.projectedEntityCount - 1,
      maxEntityCount: descriptor.projectedEntityCount,
      lastSnapshotTick: 600,
      lastCombatTick: 0,
    },
    finalFrameCount: 0,
  }, descriptor);
  assert(errors.length >= 4, "Hellhole sampling rejects quiet, unstable, non-rendering rooms");
}
