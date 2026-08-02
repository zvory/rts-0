import path from "node:path";

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
      id: "fixed-roster-hellhole-stream",
      description: "Client-only playback of Player 1's 2v2 Hellhole projection at the authored 30 Hz cadence.",
      kind: "snapshotStream",
      url: "/?snapshotStream=fixed-roster-hellhole",
      setup: {
        snapshotStreamId: "fixed-roster-hellhole",
        snapshotStreamFrameCount: 900,
        snapshotStreamPlayerId: 1,
        snapshotStreamSpectator: false,
        snapshotStreamTeamIds: Object.freeze([1, 2, 1, 2]),
        snapshotStreamVisibilityTileCount: 126 * 126,
        waitForMinEntities: 382,
        resetPerfAfterSetup: true,
      },
    },
    {
      id: "fixed-roster-hellhole-integrated",
      description: "Opt-in live Lab view with the authoritative Hellhole server and Pixi client in tandem.",
      kind: "labScenario",
      defaultEnabled: false,
      url: "/lab?room=client-perf-hellhole&map=1v1&scenario=fixed-roster-hellhole",
      setup: {
        liveLabScenario: {
          scenarioId: "fixed-roster-hellhole",
          mapWidth: 126,
          mapHeight: 126,
          projectedEntityCount: 468,
          // The 900-tick full-world harness reaches 461 between deaths and next-tick respawns.
          minimumProjectedEntityCount: 461,
        },
        waitForMinEntities: 461,
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
