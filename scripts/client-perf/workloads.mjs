import path from "node:path";

const ACTIVE_UNIT_ORDER = Object.freeze([
  "worker",
  "rifleman",
  "machine_gunner",
  "panzerfaust",
  "anti_tank_gun",
  "mortar_team",
  "artillery",
  "scout_car",
  "tank",
  "command_car",
]);

export const SUPPLY_ACTIVE_WORKLOADS = Object.freeze({
  200: Object.freeze({
    scenarioId: "supply_stress_active",
    scenarioSeed: 0x5a000300,
    targetSupply: 200,
    playerId: 1,
    spectator: false,
    predictionRequired: true,
    supplyCap: 50,
    projectedEntityCount: 135,
    countsByOwner: Object.freeze({
      1: Object.freeze(Object.fromEntries(ACTIVE_UNIT_ORDER.map((kind, index) => [kind, [7, 7, 7, 7, 7, 7, 6, 7, 6, 6][index]]))),
      2: Object.freeze(Object.fromEntries(ACTIVE_UNIT_ORDER.map((kind, index) => [kind, [7, 7, 7, 7, 7, 7, 6, 7, 6, 6][index]]))),
    }),
  }),
  300: Object.freeze({
    scenarioId: "supply_stress_active",
    scenarioSeed: 0x5a000300,
    targetSupply: 300,
    playerId: 1,
    spectator: false,
    predictionRequired: true,
    supplyCap: 50,
    projectedEntityCount: 201,
    countsByOwner: Object.freeze({
      1: Object.freeze(Object.fromEntries(ACTIVE_UNIT_ORDER.map((kind, index) => [kind, [12, 10, 10, 10, 10, 10, 10, 10, 9, 9][index]]))),
      2: Object.freeze(Object.fromEntries(ACTIVE_UNIT_ORDER.map((kind, index) => [kind, [12, 10, 10, 10, 10, 10, 10, 10, 9, 9][index]]))),
    }),
  }),
});

export function buildClientPerfWorkloads(env = process.env) {
  const incidentReplaySource = env.RTS_CLIENT_PERF_INCIDENT_REPLAY
    ? path.resolve(env.RTS_CLIENT_PERF_INCIDENT_REPLAY)
    : null;

  return Object.freeze([
    {
      id: "vehicle-wall-stress",
      description: "No-fog dev scenario with 15 tanks moving through a wall chokepoint.",
      kind: "devScenario",
      url: "/dev/scenarios?id=scout_car_wall_chokepoint&unit=tank&count=15",
    },
    {
      id: "selected-unit-hud-stress",
      description: "No-fog dev scenario with four selected tanks to exercise HUD and selection overlays.",
      kind: "devScenario",
      url: "/dev/scenarios?id=scout_car_snaking_corridor&unit=tank&count=4",
      setup: {
        selectFirstEntities: 4,
        minSelectedCount: 1,
      },
    },
    {
      id: "supply-200-active",
      description: "Active predicted player on the exact server-authoritative 200-supply stress setup.",
      kind: "activeDevScenario",
      url: "/dev/scenarios?id=supply_stress_active&unit=worker&count=200",
      setup: {
        activeSupplyStress: SUPPLY_ACTIVE_WORKLOADS[200],
        resetPerfAfterSetup: true,
      },
    },
    {
      id: "supply-300-active",
      description: "Active predicted player on the exact server-authoritative 300-supply stress setup.",
      kind: "activeDevScenario",
      url: "/dev/scenarios?id=supply_stress_active&unit=worker&count=300",
      setup: {
        activeSupplyStress: SUPPLY_ACTIVE_WORKLOADS[300],
        resetPerfAfterSetup: true,
      },
    },
    {
      id: "supply-300-hellhole-stream",
      description: "Client-only playback of Player 1's 2v2 Hellhole projection at the authored 30 Hz cadence.",
      kind: "snapshotStream",
      url: "/?snapshotStream=supply-300-hellhole",
      setup: {
        snapshotStreamId: "supply-300-hellhole",
        snapshotStreamFrameCount: 900,
        snapshotStreamPlayerId: 1,
        snapshotStreamSpectator: false,
        snapshotStreamTeamIds: Object.freeze([1, 2, 1, 2]),
        snapshotStreamVisibilityTileCount: 126 * 126,
        waitForMinEntities: 288,
        resetPerfAfterSetup: true,
      },
    },
    {
      id: "supply-300-hellhole-integrated",
      description: "Opt-in live Lab view with the authoritative Hellhole server and Pixi client in tandem.",
      kind: "labScenario",
      defaultEnabled: false,
      url: "/lab?room=client-perf-hellhole&map=1v1&scenario=supply-300-hellhole",
      setup: {
        liveLabScenario: {
          scenarioId: "supply-300-hellhole",
          mapWidth: 126,
          mapHeight: 126,
          projectedEntityCount: 380,
        },
        waitForMinEntities: 380,
        resetPerfAfterSetup: true,
      },
    },
    ...(incidentReplaySource ? [{
      id: "incident-120-commander-endgame",
      description: "Paused Commander-perspective replay at the 244-entity late-game render incident.",
      kind: "replayArtifact",
      source: incidentReplaySource,
      replayName: "incident-120-commander-endgame",
      url: "/?replayArtifact=incident-120-commander-endgame",
      setup: {
        visionSelectionPlayerId: 8,
        setRoomTimeSpeed: 8,
        waitRoomTimeTo: 29643,
        roomTimeWaitTimeoutMs: 90000,
        setRoomTimeSpeedAfterWait: 0,
        waitForMinEntities: 240,
        entityWaitTimeoutMs: 30000,
        resetPerfAfterSetup: true,
      },
    }] : []),
  ]);
}

export function defaultClientPerfWorkloads(workloads = buildClientPerfWorkloads()) {
  return Object.freeze(workloads.filter((workload) => workload.defaultEnabled !== false));
}
