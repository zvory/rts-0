// tests/client_contracts/frame_entity_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { buildFrameEntityViews } from "../../client/src/frame_entity_views.js";
import { MatchHealth } from "../../client/src/match_health.js";
import { KIND } from "../../client/src/protocol.js";
import { GameState } from "../../client/src/state.js";

// Frame entity views
// ---------------------------------------------------------------------------

{
  const calls = [];
  const selected = [{ id: 10, owner: 1, kind: KIND.WORKER, x: 80, y: 80 }];
  const state = {
    playerId: 1,
    spectator: false,
    entitiesInterpolated(alpha, options = {}) {
      calls.push({ alpha, includePrediction: options.includePrediction !== false });
      if (options.includePrediction === false) {
        return [
          { id: 1, owner: 1, kind: KIND.WORKER, x: 10, y: 12 },
          { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 30, y: 32 },
          { id: 3, owner: 0, kind: KIND.STEEL, x: 40, y: 40 },
          { id: 4, owner: 1, kind: KIND.RIFLEMAN, x: 50, y: 50, shotReveal: true },
          { id: 5, owner: 1, kind: KIND.RIFLEMAN, x: 60, y: 60, visionOnly: true },
        ];
      }
      return [{ id: 1, owner: 1, kind: KIND.WORKER, x: alpha * 100, y: alpha * 100 }];
    },
    selectedEntities() {
      return selected;
    },
  };

  const frameViews = buildFrameEntityViews(state, { alpha: 0.5 });
  assert(frameViews.interpolatedEntities[0].x === 50, "frame entity views keep alpha-interpolated entities");
  assert(frameViews.currentEntities[0].x === 100, "frame entity views keep latest predicted current entities");
  assert(frameViews.authoritativeEntities.some((entity) => entity.id === 2), "frame entity views keep authoritative entities");
  assert(frameViews.selectedEntities === selected, "frame entity views reuse the selected entity array");
  assert(
    frameViews.fogSourceEntities.length === 1 && frameViews.fogSourceEntities[0].id === 1,
    "frame entity views filter fog sources to own non-shot-reveal non-vision entries",
  );
  assert(frameViews.debug.entitiesInterpolatedCalls === 3, "frame entity views cap interpolation calls for a mixed-alpha frame");
  assert(frameViews.debug.entityVariantBuildCalls === 0, "generic states retain the legacy interpolation fallback");
  assert(frameViews.debug.entityTraversals === 0, "fallback diagnostics do not claim a production entity traversal");
  assert(frameViews.debug.selectedEntitiesCalls === 1, "frame entity views resolve selection once");
  assert(
    calls.map((call) => `${call.alpha}:${call.includePrediction}`).join("|") === "0.5:true|1:true|1:false",
    "frame entity views request predicted alpha, predicted current, and no-prediction current views",
  );

  calls.length = 0;
  const currentFrameViews = buildFrameEntityViews(state, { alpha: 1 });
  assert(
    currentFrameViews.currentEntities === currentFrameViews.interpolatedEntities,
    "frame entity views reuse alpha-1 predicted entities for current predicted consumers",
  );
  assert(currentFrameViews.debug.entitiesInterpolatedCalls === 2, "alpha-1 frame skips duplicate predicted current interpolation");

  state.spectator = true;
  const spectatorViews = buildFrameEntityViews(state, { alpha: 1 });
  assert(
    spectatorViews.fogSourceEntities.map((entity) => entity.id).join(",") === "1,2",
    "spectator fog sources include non-neutral visible entities from the authoritative union",
  );
}

{
  const clockSamples = [];
  const state = Object.create(GameState.prototype);
  state._cur = { entities: [
    { id: 1, owner: 1, kind: KIND.WORKER, x: 30, y: 50, facing: -3, weaponFacing: 3, nested: { value: 1 } },
    { id: 2, owner: 2, kind: KIND.RIFLEMAN, x: 90, y: 120 },
  ] };
  state._prevById = new Map([[
    1,
    { id: 1, owner: 1, kind: KIND.WORKER, x: 10, y: 20, facing: 3, weaponFacing: -3 },
  ]]);
  state.visualNow = () => {
    const value = 100 + clockSamples.length;
    clockSamples.push(value);
    return value;
  };
  state._applyPredictedEntity = (entity, now) => ({ ...entity, predictionClock: now });
  state._applyDisplayEntity = (entity, now) => ({ ...entity, displayClock: now });
  state.selectedEntities = () => [];

  const legacyInterpolated = state.entitiesInterpolated(0.5);
  const legacyCurrent = state.entitiesInterpolated(1);
  const legacyAuthoritative = state.entitiesInterpolated(1, { includePrediction: false });
  clockSamples.length = 0;
  const batch = state.entityVariants(0.5);
  assert(
    JSON.stringify(batch.interpolatedEntities) === JSON.stringify(legacyInterpolated)
      && JSON.stringify(batch.currentEntities) === JSON.stringify(legacyCurrent)
      && JSON.stringify(batch.authoritativeEntities) === JSON.stringify(legacyAuthoritative),
    "batched entity variants match the legacy three-call output for interpolation, wraparound, and missing prior data",
  );
  assert(clockSamples.join(",") === "100,101,102", "batched variants preserve legacy visual-clock sampling order");
  assert(batch.entityTraversals === 1, "batched variants report one source traversal");
  assert(
    batch.interpolatedEntities[0] !== batch.currentEntities[0]
      && batch.currentEntities[0] !== batch.authoritativeEntities[0],
    "batched variants retain independent records across distinct views",
  );

  const frameViews = buildFrameEntityViews(state, { alpha: 0.5 });
  assert(frameViews.debug.entityVariantBuildCalls === 1, "production frame views invoke one entity variant build");
  assert(frameViews.debug.entityTraversals === 1, "production frame views traverse current entities once");
  assert(frameViews.debug.entitiesInterpolatedCalls === 0, "production frame views bypass legacy interpolation calls");
}

// ---------------------------------------------------------------------------
// Match health
// ---------------------------------------------------------------------------

{
  const net = { latency: null, latencyUpdatedAt: 0 };
  let badgePayload = null;
  const health = new MatchHealth({
    net,
    statusBadge: { setMatchMetrics(metrics) { badgePayload = metrics; } },
    snapshotMs: 33,
  });

  net.latency = 179;
  net.latencyUpdatedAt = 1;
  health.refreshLatency();
  assert(health.metrics().latencyMs === 179, "MatchHealth records latest latency sample");
  assert(!health.metrics().issues.latency.active, "latency below threshold stays inactive");
  assert(health.metrics().issues.latency.count === 0, "latency below threshold does not count as bad RTT");

  net.latency = 180;
  net.latencyUpdatedAt = 2;
  health.refreshLatency();
  assert(health.metrics().issues.latency.active, "latency at threshold marks active issue");
  assert(health.metrics().issues.latency.count === 1, "latency issue count increments on bad sample");
  assert(health.reportStats.badRttSamples === 1, "bad RTT samples feed net report stats");
  health.refreshLatency();
  assert(health.metrics().issues.latency.count === 1, "unchanged latency timestamp does not double-count");

  health.noteSnapshotArrival(100, false);
  health.noteSnapshotArrival(133, false);
  assert(health.metrics().jitterMs === 0, "on-cadence snapshots report zero jitter");
  health.noteSnapshotArrival(187, false);
  assert(health.metrics().jitterMs === 21, "snapshot jitter records max delta in window");
  assert(health.metrics().issues.jitter.active, "snapshot jitter threshold marks active issue");
  assert(health.metrics().issues.jitter.count === 1, "snapshot jitter issue count increments");
  assert(health.reportStats.jitterSamples === 1, "jitter samples feed net report stats");

  health.noteFrameGap(16, 1000);
  health.noteFrameGap(16, 1016);
  assert(health.metrics().fps === 62.5, "MatchHealth records live FPS from the latest frame gap");
  assert(health.metrics().fpsOneMinute === 62.5, "MatchHealth records rolling one-minute FPS");
  health.noteFrameGap(32, 62017);
  assert(health.metrics().fps === 31.25, "live FPS follows the latest frame");
  assert(health.metrics().fpsOneMinute === 31.25, "one-minute FPS prunes stale frame samples");

  let now = 187;
  for (let i = 0; i < 8; i += 1) {
    now += 34;
    health.noteSnapshotArrival(now, false);
  }
  assert(health.metrics().jitterMs === 1, "snapshot jitter window drops old outlier samples");
  assert(!health.metrics().issues.jitter.active, "jitter active state follows the latest visible delta");
  const jitterBeforeHidden = health.metrics().jitterMs;
  health.noteSnapshotArrival(now + 500, true);
  assert(health.metrics().jitterMs === jitterBeforeHidden, "hidden document snapshots do not update jitter");

  const cadenceHealth = new MatchHealth({ net, statusBadge: null, snapshotMs: 33 });
  cadenceHealth.noteSnapshotArrival(0, false, 10);
  cadenceHealth.noteSnapshotArrival(1, false, 10);
  cadenceHealth.noteSnapshotArrival(2, false, 13);
  cadenceHealth.noteSnapshotArrival(3, false, 12);
  assert(cadenceHealth.reportStats.duplicateSnapshotCount === 1, "duplicate snapshot ticks feed report stats");
  assert(cadenceHealth.reportStats.skippedSnapshotCount === 1, "skipped snapshot ticks feed report stats");
  assert(cadenceHealth.reportStats.staleSnapshotCount === 1, "stale snapshot ticks feed report stats");
  assert(cadenceHealth.reportStats.snapshotTickGapMax === 3, "snapshot tick gap max is reported");
  assert(cadenceHealth.reportStats.snapshotBurstCount === 1, "multiple snapshots before a frame count as one burst");
  assert(cadenceHealth.reportStats.snapshotBurstMax === 4, "snapshot burst max records per-frame receive pressure");
  cadenceHealth.noteFrameGap(16, 20);
  cadenceHealth.noteSnapshotArrival(21, false, 14);
  assert(cadenceHealth.reportStats.snapshotBurstMax === 4, "frame boundaries reset current burst without clearing report max");

  const commandHealth = new MatchHealth({ net, statusBadge: null, snapshotMs: 33 });
  commandHealth.noteCommandIssued(1000);
  commandHealth.noteCommandIssued(1100);
  commandHealth.noteCommandIssued(1200);
  commandHealth.noteCommandIssued(1301);
  assert(commandHealth.reportStats.commandBurstMax === 3, "command burst max uses a bounded short window");
  commandHealth.noteFrameSummary({ at: 1320, frameGapMs: 48, worstPhase: "match.input", worstPhaseMs: 12 });
  assert(commandHealth.reportStats.commandBurstFrameGapMaxMs === 48, "command burst frames track frame gap max");
  assert(commandHealth.reportStats.commandBurstWorstFramePhase === "match.input", "command burst frames track worst phase");
  commandHealth.noteSnapshotArrival(1400, false, 10);
  commandHealth.noteFrameSummary({ at: 1490, frameGapMs: 16, worstPhase: "match.renderer", worstPhaseMs: 4 }, {
    predictedSnapshotPresent: true,
  });
  assert(commandHealth.reportStats.snapshotLateFrameCount === 1, "late snapshot frames are counted");
  assert(
    commandHealth.reportStats.predictedSnapshotLateFrameCount === 1,
    "predicted snapshot coverage during late snapshot frames is counted",
  );
  commandHealth.resetReportStats(1500);
  assert(commandHealth.reportStats.commandBurstMax === 0, "command burst stats reset with the net report window");

  health.applyServerNetStatus({
    tickMs: 44,
    serverLagMs: 120,
    slowTick: true,
    slowTickCount: 3,
    headOfLine: true,
    headOfLineCount: 4,
  });
  assert(health.metrics().serverTickMs === 44, "server tick timing propagates to metrics");
  assert(health.metrics().serverLagMs === 120, "server lag timing propagates to metrics");
  assert(health.metrics().issues.slowTick.active, "slow tick status propagates to issues");
  assert(health.metrics().issues.slowTick.count === 3, "slow tick count propagates to issues");
  assert(health.metrics().issues.headOfLine.active, "head-of-line status propagates to issues");
  assert(health.metrics().issues.headOfLine.count === 4, "head-of-line count propagates to issues");

  health.publish();
  assert(badgePayload !== null, "MatchHealth publishes status badge payload");
  assert(
    Object.keys(badgePayload).join(",") === "latencyMs,serverTickMs,serverLagMs,jitterMs,fps,fpsOneMinute,issues",
    "status badge payload shape stays unchanged",
  );
  assert(
    Object.keys(badgePayload.issues).join(",") === "latency,slowTick,headOfLine,jitter",
    "status badge issue payload shape stays unchanged",
  );
}
