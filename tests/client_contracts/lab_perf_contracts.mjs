import { assert } from "./assertions.mjs";
import {
  applyLabHellholeSetup,
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
  teamByPlayer: { 1: 1, 2: 2 },
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

{
  const previousWindow = globalThis.window;
  const entities = new Map([
    [1, { owner: 1, kind: "city_centre" }],
    [2, { owner: 2, kind: "city_centre" }],
  ]);
  let entityId = 3;
  for (const playerId of descriptor.playerIds) {
    for (const [kind, count] of Object.entries(descriptor.countsByOwner[playerId])) {
      for (let index = 0; index < count; index += 1) {
        entities.set(entityId, { owner: playerId, kind });
        entityId += 1;
      }
    }
  }
  globalThis.window = {
    __rts: {
      labLaunch: { scenario: descriptor.scenarioId, map: descriptor.map },
      match: {
        labMetadata: { vision: { mode: "all" }, godModePlayers: [1, 2] },
        predictionStartInfo: {
          players: [
            { id: 1, teamId: 1 },
            { id: 2, teamId: 2 },
          ],
        },
        state: {
          _curById: entities,
          playerResources: [
            { id: 1, supplyUsed: 300 },
            { id: 2, supplyUsed: 300 },
          ],
        },
        net: {
          snapshotReportStats: {
            snapshotCodec: "messagepack-compact",
            snapshotFrameKind: "binary",
            messageCount: 2,
          },
          on() {},
        },
      },
    },
    __rtsPerf: { summary: () => ({ frameCount: 10 }) },
  };
  const result = { actions: [] };
  try {
    await applyLabHellholeSetup({
      waitForFunction: async () => {},
      evaluate: async (fn, argument) => fn(argument),
    }, { labHellhole: descriptor }, result);
  } finally {
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
  }
  assert(!result.error, `Hellhole setup reads the selected launch and authoritative state: ${result.error || "ok"}`);
  assert(result.labHellhole?.facts?.map === descriptor.map,
    "Hellhole setup reads the map from the normalized Lab launch config");
  assert(result.labHellhole?.facts?.teamByPlayer?.[2] === 2,
    "Hellhole setup reads opposing teams from the server start payload");
}

for (const [label, patch] of [
  ["scenario", { scenarioId: "lategame" }],
  ["map", { map: "No Terrain" }],
  ["supply", { supplyByOwner: { 1: 299, 2: 300 } }],
  ["teams", { teamByPlayer: { 1: 1, 2: 1 } }],
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
