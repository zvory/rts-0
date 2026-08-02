import fs from "node:fs";
import {
  COMMAND_PREDICTION_POLICIES,
  PredictionController,
  PREDICTION_STATE,
} from "../client/src/prediction_controller.js";
import {
  predictionCompatibility,
  predictionRuntimeCompatibility,
} from "../client/src/prediction_compatibility.js";
import { DEFAULT_FACTION_ID, PREDICTION_PROTOCOL_VERSION } from "../client/src/protocol.js";
import { createRigRenderContext } from "../client/src/renderer/rigs/animation.js";
import { GameState } from "../client/src/state.js";
import { SimWasmPredictionAdapter } from "../client/src/sim_wasm_adapter.js";
import { CaptureRenderClock } from "../client/src/visual_clock.js";
import {
  finishPredictionRuntimeInit,
  recoverPredictionRuntimeAfterBudget,
} from "../client/src/prediction_runtime_startup.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

{
  const snapshot = { tick: 9, entities: [{ id: 1, prodProgress: 0.1 }] };
  let reconciles = 0;
  let frames = 0;
  let destroyed = 0;
  const adapter = { destroy() { destroyed += 1; } };
  const match = {
    predictionInitToken: 2,
    predictionAdapter: adapter,
    progressPredictionEligible: true,
    latestPredictionSnapshot: snapshot,
    prediction: {
      enabled: true,
      reconcilePredictor(value) {
        reconciles += 1;
        assert(value === snapshot, "delayed WASM init imports the latest authoritative snapshot");
      },
    },
    predictionRuntimeEnabled: () => true,
    applyPredictionFrame: () => { frames += 1; },
    publishPredictionDebug() {},
    logPredictionStatus() {},
    mountSettings() {},
  };
  finishPredictionRuntimeInit(match, { token: 1, adapter, ready: true, remountSettings: false });
  finishPredictionRuntimeInit(match, { token: 2, adapter, ready: true, remountSettings: false });
  assert(destroyed === 0, "a stale callback never destroys the still-current loading adapter");
  assert(reconciles === 1, "WASM readiness catches up without waiting for another packet");
  assert(frames === 1, "WASM readiness publishes the caught-up prediction frame immediately");
}

{
  let disableReason = null;
  let cleared = 0;
  let frames = 0;
  const match = {
    predictionInitToken: 1,
    progressPredictionEligible: true,
    latestPredictionSnapshot: { tick: 4, entities: [] },
    prediction: {
      enabled: false,
      recordDisableReason(reason) { disableReason = reason; },
    },
    predictionAdapter: null,
    predictionRuntimeEnabled: () => true,
    state: { clearPredictionFrame() { cleared += 1; } },
    applyPredictionFrame() { frames += 1; },
    publishPredictionDebug() {},
    logPredictionStatus() {},
    mountSettings() {},
  };
  const adapter = { reconcile() { throw new Error("bad baseline"); }, destroy() {} };
  match.predictionAdapter = adapter;
  finishPredictionRuntimeInit(match, { token: 1, adapter, ready: true, remountSettings: false });
  assert(disableReason === "progress-reconcile-failed" && cleared === 1,
    "progress-only startup baseline failures fall back to authoritative display");
  assert(frames === 0, "failed startup reconciliation never publishes a stale progress frame");
}

{
  let initialized = 0;
  let resetAdapters = 0;
  const match = {
    prediction: {
      enabled: true,
      recordReplayBudgetExceeded() {},
      reset() {},
    },
    resetPredictionAdapter() { resetAdapters += 1; },
    initPredictionAdapter() { initialized += 1; },
    applyPredictionDisplayOverlay() {},
    publishPredictionDebug() {},
    logPredictionStatus() {},
  };
  assert(recoverPredictionRuntimeAfterBudget(match, {
    budgetExceededCount: 1,
    lastReplayBudgetExceeded: true,
    lastTickMs: 8,
    lastReplayTicks: 3,
  }) === true, "replay-budget recovery handles an exceeded frame");
  assert(resetAdapters === 1 && initialized === 1,
    "replay-budget recovery immediately initializes the replacement shared runtime");
  assert(recoverPredictionRuntimeAfterBudget(match, {
    budgetExceededCount: 1,
    lastReplayBudgetExceeded: true,
    lastTickMs: 9,
    lastReplayTicks: 3,
  }) === false, "an over-budget replacement terminates recovery instead of restarting recursively");
  assert(resetAdapters === 1 && initialized === 1, "replay-budget recovery is bounded to one restart");
  recoverPredictionRuntimeAfterBudget(match, {
    budgetExceededCount: 1,
    lastReplayBudgetExceeded: false,
  });
  assert(recoverPredictionRuntimeAfterBudget(match, {
    budgetExceededCount: 2,
    lastReplayBudgetExceeded: true,
    lastTickMs: 8,
    lastReplayTicks: 2,
  }) === true, "a healthy measured window rearms future replay-budget recovery");
}

{
  const priorFetch = globalThis.fetch;
  let releaseModule = null;
  let importCount = 0;
  let predictorCount = 0;
  const moduleGate = new Promise((resolve) => { releaseModule = resolve; });
  globalThis.fetch = async () => ({
    ok: true,
    headers: { get: () => "text/javascript" },
  });
  try {
    const adapter = new SimWasmPredictionAdapter({
      startInfo: { tick: 0, map: { width: 1, height: 1, tileSize: 32, terrain: [], resources: [] }, players: [] },
      playerId: 1,
      importModule: async () => {
        importCount += 1;
        return {
          default: () => moduleGate,
          WasmPredictor: {
            fromStartJson() {
              predictorCount += 1;
              return {};
            },
          },
        };
      },
    });
    const first = adapter.init();
    const second = adapter.init();
    assert(first === second, "concurrent WASM initialization callers share one in-flight promise");
    releaseModule();
    assert(await first && await second, "all shared WASM initialization callers observe readiness");
    assert(importCount === 1 && predictorCount === 1, "shared WASM initialization imports and constructs once");
  } finally {
    globalThis.fetch = priorFetch;
  }
}

{
  const fractions = [];
  const captureClock = new CaptureRenderClock(1000);
  const adapter = new SimWasmPredictionAdapter({ visualNow: () => captureClock.now() });
  adapter.ready = true;
  adapter.displayValid = true;
  adapter.lastAdvanceAt = captureClock.now();
  adapter.predictor = {
    advanceTicks() {},
    renderPredictionFrameJson(fraction) {
      fractions.push(fraction);
      return JSON.stringify({ tick: 1, entities: [], progress: [] });
    },
  };
  adapter.renderPredictionFrame();
  captureClock.advanceTo(1016.6666667);
  adapter.renderPredictionFrame();
  assert(fractions[0] === 0 && Math.abs(fractions[1] - 0.5) < 0.0001,
    "fixed capture samples WASM progress from deterministic visual time");
}

function sentSeqs(sent) {
  return sent.map((entry) => entry.clientSeq).join(",");
}

{
  let fullSnapshotCalled = false;
  let visualNow = 16.6666667;
  let renderedFraction = null;
  let advanceTicksCalls = 0;
  const adapter = new SimWasmPredictionAdapter({ visualNow: () => visualNow });
  adapter.ready = true;
  adapter.displayValid = true;
  adapter.lastAdvanceAt = 0;
  adapter.predictor = {
    renderPredictionFrameJson: (fraction) => {
      renderedFraction = fraction;
      return JSON.stringify({
        tick: 7,
        entities: [{ id: 1, x: 4, y: 5 }],
        progress: [{ id: 2, kind: "production", identity: "unit:worker", fraction: 0.3 }],
      });
    },
    enqueueCommandJson() {},
    advanceTicks() { advanceTicksCalls += 1; },
    renderSnapshotJson: () => {
      fullSnapshotCalled = true;
      throw new Error("legacy full snapshot render must not be used");
    },
  };
  const frame = adapter.renderPredictionFrame();
  assert(frame.tick === 7 && frame.entities[0].x === 4, "WASM adapter consumes the sparse prediction-frame API");
  assert(frame.progress[0].fraction === 0.3, "WASM adapter preserves the separate sparse progress lane");
  assert(Math.abs(renderedFraction - 0.5) < 0.0001, "WASM render receives a render-clock fractional tick");
  assert(fullSnapshotCalled === false, "WASM adapter never requests a synthetic full snapshot");
  adapter.enqueueCommand(1, { c: "move", units: [1], x: 8, y: 9 });
  assert(advanceTicksCalls === 0, "command enqueue does not advance the shared display tick");
  adapter.pauseVisualClock(visualNow);
  const frozen = adapter.renderPredictionFrame(5000);
  assert(frozen === adapter.frozenFrame, "paused WASM display returns the frozen prediction frame");
  visualNow = 100;
  adapter.resumeVisualClock(visualNow);
  adapter.renderPredictionFrame(116.6666667);
  assert(Math.abs(renderedFraction - 0.5) < 0.0001, "resumed visual clock excludes paused wall time");

  adapter.module = {
    WasmPredictor: {
      baselineFromSnapshotJson() { throw new Error("bad baseline"); },
    },
  };
  let reconcileFailed = false;
  try {
    adapter.reconcile({ tick: 8, entities: [] }, []);
  } catch {
    reconcileFailed = true;
  }
  assert(reconcileFailed && adapter.renderPredictionFrame() === null,
    "failed reconcile invalidates stale pose and progress output for authoritative fallback");
}

{
  let now = 0;
  const adapter = new SimWasmPredictionAdapter({ now: () => now, replayBudgetMs: 4 });
  adapter.ready = true;
  adapter.module = {
    WasmPredictor: {
      baselineFromSnapshotJson() { return "{}"; },
    },
  };
  adapter.predictor = {
    importBaselineJson() { now += 5; },
    diagnosticsJson() { return JSON.stringify({ correctionMagnitude: 0 }); },
    renderPredictionFrameJson() { return JSON.stringify({ tick: 1, entities: [], progress: [] }); },
  };
  adapter.reconcile({ tick: 1 }, []);
  assert(adapter.diagnostics().lastReplayBudgetExceeded === true,
    "adapter diagnostics distinguish a latest over-budget replay from the cumulative count");
  now = 10;
  adapter.predictor.importBaselineJson = () => { now += 1; };
  adapter.reconcile({ tick: 2 }, []);
  const diagnostics = adapter.diagnostics();
  assert(diagnostics.budgetExceededCount === 1 && diagnostics.lastReplayBudgetExceeded === false,
    "a healthy replay clears only the latest-result flag and preserves cumulative telemetry");
}

{
  const sent = [];
  const controller = new PredictionController({
    now: () => 1000,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  for (const id of [1, 2, 3]) controller.issueCommand({ c: "stop", units: [id] });
  assert(sentSeqs(sent) === "1,2,3", "commands 1,2,3 are sequenced");
  controller.applyAuthoritativeSnapshot({ tick: 30, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "2,3", "ack 1 drops only command 1");
}

{
  const sent = [];
  let now = 100;
  const controller = new PredictionController({
    now: () => now,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  controller.issueCommand({ c: "stop", units: [1] });
  now = 180;
  controller.applyAuthoritativeSnapshot({ tick: 30, netStatus: { lastSimConsumedClientSeq: 1 } });
  const summary = controller.debugSummary();
  assert(summary.ackLatencyMs === 80, "ack latency records issue-to-sim-consumption duration");
  assert(summary.maxAckLatencyMs === 80, "max ack latency tracks observed latency");
}

{
  const sent = [];
  const controller = new PredictionController({
    now: () => 2000,
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  for (const id of [1, 2, 3, 4, 5]) controller.issueCommand({ c: "stop", units: [id] });
  controller.applyAuthoritativeSnapshot({ tick: 10, netStatus: { lastSimConsumedClientSeq: 3 } });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "4,5", "ack 3 leaves 4 and 5 pending");
  let report = controller.consumeCommandReportStats();
  assert(report.commandsIssued === 5, "command report counts issued commands");
  assert(report.commandSocketSendAccepted === 5, "command report counts browser-accepted sends");
  assert(report.commandSimAcknowledged === 3, "command report counts sim acknowledgements");
  assert(report.commandIssueToSimAckMaxMs === 0, "same-clock sim ack latency is tracked");
  controller.recordSocketReceipt(4, { serverTick: 10 });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "4,5", "socket receipt is diagnostic only");
  report = controller.consumeCommandReportStats();
  assert(report.commandServerReceived === 1, "command report counts server receipts");
}

{
  let now = 100;
  const controller = new PredictionController({
    now: () => now,
    sendCommand: () => true,
  });
  controller.issueCommand({ c: "stop", units: [1] });
  now = 160;
  controller.recordSocketReceipt(1, { serverTick: 4, accepted: true });
  now = 220;
  controller.applyAuthoritativeSnapshot({ tick: 5, netStatus: { lastSimConsumedClientSeq: 1 } });
  now = 226;
  controller.recordAckSnapshotApplied(1, 220);
  const report = controller.consumeCommandReportStats();
  assert(report.commandIssueToServerReceiptLatestMs === 60, "issue-to-receipt latest is tracked");
  assert(report.commandServerReceiptToSimAckLatestMs === 60, "receipt-to-sim-ack latest is tracked");
  assert(report.commandIssueToSimAckLatestMs === 120, "issue-to-sim-ack latest is tracked");
  assert(report.commandAckSnapshotReceivedToAppliedLatestMs === 6, "ack snapshot apply timing is tracked");
  assert(report.commandSimAcknowledged === 1, "sim ack count is reported");
}

{
  let now = 0;
  const controller = new PredictionController({
    enabled: false,
    now: () => now,
    sendCommand: () => true,
  });
  controller.issueCommand({ c: "stop", units: [1] });
  now = 40;
  controller.recordSocketReceipt(1, { accepted: false, reason: "notPlayer", serverTick: 0 });
  const report = controller.consumeCommandReportStats();
  assert(report.commandsIssued === 1, "disabled command diagnostics still count issued commands");
  assert(report.commandRejected === 1, "disabled command diagnostics count receipt rejections");
  assert(controller.debugSummary().pendingCommandCount === 0, "disabled prediction pending remains empty");
}

{
  const controller = new PredictionController({ sendCommand: () => true });
  controller.issueCommand({ c: "stop", units: [1] });
  controller.issueCommand({ c: "stop", units: [2] });
  controller.applyAuthoritativeSnapshot({ tick: 5, netStatus: { lastSimConsumedClientSeq: 0 } });
  controller.applyAuthoritativeSnapshot({ tick: 5, netStatus: { lastSimConsumedClientSeq: 0 } });
  controller.applyAuthoritativeSnapshot({ tick: 8, netStatus: { lastSimConsumedClientSeq: 1 } });
  controller.applyAuthoritativeSnapshot({ tick: 7, netStatus: { lastSimConsumedClientSeq: 2 } });
  const summary = controller.debugSummary();
  assert(summary.duplicateSnapshotCount === 1, "duplicate snapshots are counted");
  assert(summary.skippedSnapshotCount === 1, "skipped ticks are counted");
  assert(summary.staleSnapshotCount === 1, "out-of-date snapshots are ignored");
  assert(summary.pendingClientSeqs.join(",") === "2", "stale snapshot did not apply ack 2");
}

{
  let now = 0;
  const controller = new PredictionController({
    now: () => now,
    commandTimeoutMs: 10,
    sendCommand: () => true,
  });
  controller.issueCommand({ c: "stop", units: [1] });
  controller.recordCommandRejection(1, "bad command");
  assert(controller.pendingCommandCount === 1, "rejection does not imply sim consumption");
  now = 20;
  assert(controller.expireTimedOutCommands() === 1, "pending command timeout is reported");
  controller.applyAuthoritativeSnapshot({ tick: 2, netStatus: { lastSimConsumedClientSeq: 1 } });
  assert(controller.pendingCommandCount === 0, "ack clears rejected/timed-out command");
}

{
  const sent = [];
  const controller = new PredictionController({
    enabled: false,
    sendCommand(command, clientSeq) {
      assert(Number.isInteger(clientSeq) && clientSeq > 0, "disabled sends still carry a valid clientSeq");
      sent.push({ command, clientSeq });
      return true;
    },
  });
  assert(controller.debugSummary().mode === PREDICTION_STATE.DISABLED, "disabled mode is exposed");
  const result = controller.issueCommand({ c: "stop", units: [1] });
  assert(result.sent === true && result.predicted === false, "disabled controller still sends gameplay commands");
  assert(result.clientSeq === 1, "disabled controller attaches protocol sequence ids");
  assert(sent.length === 1 && sent[0].clientSeq === 1, "disabled sends use sequenced protocol commands");
  assert(controller.debugSummary().pendingCommandCount === 0, "disabled controller does not track prediction pending commands");
  assert(controller.debugSummary().nextClientSeq === 2, "disabled controller advances sequence ids");
}

{
  const sent = [];
  const controller = new PredictionController({
    sendCommand(command, clientSeq) {
      sent.push({ command, clientSeq });
      return true;
    },
  });
  controller.issueCommand({ c: "stop", units: [1] });
  controller.reset({ enabled: false, preserveClientSeq: true, reason: "user-disabled" });
  assert(controller.debugSummary().disableReasons["user-disabled"] === 1, "disable reasons are counted");
  controller.issueCommand({ c: "stop", units: [2] });
  controller.reset({ enabled: true, preserveClientSeq: true });
  controller.issueCommand({ c: "stop", units: [3] });
  assert(sentSeqs(sent) === "1,2,3", "prediction toggles preserve monotonic command sequence ids");
}

{
  const controller = new PredictionController({ sendCommand: () => true });
  controller.recordReplayBudgetExceeded({ elapsedMs: 9.4, replayTicks: 11 });
  const report = controller.consumeCommandReportStats();
  assert(report.predictionReplayBudgetExceededCount === 1, "replay budget exceeds are counted in the report window");
  assert(report.predictionReplayMaxMs === 9.4, "replay budget report preserves max replay milliseconds");
  assert(report.predictionReplayMaxTicks === 11, "replay budget report preserves max replay ticks");
  assert(controller.debugSummary().disableReasons["replay-budget-exceeded"] === 1, "replay budget resets use a stable reason");
  assert(
    controller.consumeCommandReportStats().predictionReplayBudgetExceededCount === 0,
    "replay budget report counters reset after consumption",
  );
}

{
  const calls = [];
  const fakePredictor = {
    enqueueCommand(clientSeq, command) {
      calls.push(["enqueue", clientSeq, command.c]);
      return command.c === "move";
    },
    reconcile(snapshot, pending) {
      calls.push(["reconcile", snapshot.tick, pending.map((entry) => entry.clientSeq).join(",")]);
      return { correctionDistance: 4, snapCorrection: false };
    },
  };
  const controller = new PredictionController({
    predictor: fakePredictor,
    sendCommand: () => true,
  });
  const issued = controller.issueCommand({ c: "move", units: [1], x: 120, y: 100 });
  assert(issued.predicted === true, "predictable movement command is enqueued locally");
  controller.applyAuthoritativeSnapshot({ tick: 12, netStatus: { lastSimConsumedClientSeq: 0 } });
  const summary = controller.debugSummary();
  assert(summary.mode === PREDICTION_STATE.RESYNCING, "correction enters resync mode");
  assert(summary.maxCorrectionDistance === 4, "correction distance is tracked");
  assert(
    calls.some((call) => call[0] === "reconcile" && call[2] === "1"),
    "unacknowledged commands are replayed after authoritative snapshot",
  );
}

{
  const fakePredictor = {
    enqueueCommand() {
      return true;
    },
    reconcile(snapshot, pending) {
      return {
        correctionDistance: pending.length === 0 ? 0 : 1,
        snapCorrection: false,
      };
    },
  };
  const controller = new PredictionController({
    predictor: fakePredictor,
    sendCommand: () => true,
  });
  controller.issueCommand({ c: "move", units: [1], x: 120, y: 100 });
  controller.applyAuthoritativeSnapshot({ tick: 1, netStatus: { lastSimConsumedClientSeq: 1 } });
  const summary = controller.debugSummary();
  assert(summary.pendingCommandCount === 0, "acknowledged command is dropped before replay");
  assert(summary.mode === PREDICTION_STATE.TRACKING, "no correction returns to tracking mode");
}

{
  const calls = [];
  const controller = new PredictionController({
    predictor: {
      enqueueCommand(clientSeq, command) {
        calls.push([clientSeq, command.c]);
        return true;
      },
    },
    sendCommand: () => true,
  });
  const issued = controller.issueCommand({ c: "move", units: [1], x: 120, y: 100 }, {
    predictMovement: false,
  });
  assert(issued.sent === true && issued.predicted === false, "paused movement prediction still sends commands without a local tick");
  assert(calls.length === 0, "paused movement prediction skips the WASM predictor enqueue");
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "1", "paused movement prediction still tracks the pending command");
}

{
  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [{ id: 10, owner: 1, kind: "worker", x: 32, y: 32, hp: 40, maxHp: 40, state: "idle" }],
    events: [],
  });
  state.applyPredictionDisplayOverlay({
    predictionFrame: {
      tick: 3,
      entities: [{ id: 10, x: 52, y: 32, facing: 1.5, motion: "move" }],
    },
  });
  assert(state.entitiesInterpolated(1)[0].x === 52, "render reads predicted owned position");
  assert(state.entitiesInterpolated(1)[0].state === "move", "explicit predicted motion drives immediate movement presentation");
  assert(
    state.entitiesInterpolated(1, { includePrediction: false })[0].x === 32,
    "authoritative reads can ignore prediction for fog",
  );
  assert(state.entityById(10).x === 52, "entityById exposes predicted owned position for local UX");
  assert(state.entityById(10).facing === 1.5, "entityById composes predicted body facing onto authority");
  state.applyPredictionDisplayOverlay({ optimisticCommands: { production: [], rally: [] } });
  assert(state.entitiesInterpolated(1)[0].x === 52, "optimistic overlay updates do not clear predicted movement");
  assert(state.localFactionId === DEFAULT_FACTION_ID, "GameState exposes normalized local faction identity");
}

{
  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: [
      { id: 1, teamId: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 2, name: "B", color: "#0f0", startTileX: 2, startTileY: 2 },
    ],
  });
  const authoritative = [
    {
      id: 20, owner: 1, kind: "worker", x: 32, y: 32, facing: 0.25,
      hp: 31, maxHp: 40, state: "gather", weaponFacing: 2.4, targetId: 90,
      latchedNode: 90, setupState: "deployed", abilities: ["future_ability"],
      futureSentinel: { mustSurvive: true },
    },
    {
      id: 21, owner: 1, kind: "worker", x: 64, y: 32, facing: 0.5,
      hp: 38, maxHp: 40, state: "build", buildProgress: 0.4, targetId: 91,
      futureSentinel: "construction-preserved",
    },
    {
      id: 23, owner: 1, kind: "barracks", x: 64, y: 64, hp: 200, maxHp: 400,
      state: "construct", buildProgress: 0.25, buildActive: true,
    },
    { id: 22, owner: 2, kind: "worker", x: 96, y: 32, hp: 40, maxHp: 40, state: "idle" },
  ];
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 2,
    supplyCap: 10,
    entities: authoritative,
    events: [],
  });

  const applyBusyFrame = (offset) => state.applyPredictionDisplayOverlay({
    predictionFrame: {
      tick: 1 + offset,
      entities: [
        {
          id: 20, x: 32 + offset, y: 33, facing: 1,
          owner: 999, kind: "tank", hp: 0, state: "idle", weaponFacing: 0,
          latchedNode: null, abilities: [], futureSentinel: "clobbered",
        },
        {
          id: 21, x: 64 + offset, y: 34,
          state: "idle", buildProgress: 0, targetId: null, futureSentinel: "clobbered",
        },
        { id: 22, x: 140, y: 32, motion: "move" },
        { id: 999, x: 200, y: 200, motion: "move" },
      ],
      progress: [
        { id: 23, kind: "construction", identity: "build:barracks", fraction: 0.25 + offset * 0.01 },
      ],
    },
  });

  for (const offset of [1, 2, 3, 4]) {
    applyBusyFrame(offset);
    const gather = state.entityById(20);
    const build = state.entityById(21);
    assert(gather.x === 32 + offset && gather.facing === 1, "sparse prediction keeps owned pose responsive");
    assert(gather.state === "gather" && gather.latchedNode === 90, "missing motion preserves gathering activity across prediction frames");
    assert(createRigRenderContext(gather).busy === true, "gathering worker keeps the yellow busy indicator across prediction frames");
    assert(gather.hp === 31 && gather.weaponFacing === 2.4 && gather.targetId === 90, "prediction cannot overwrite authoritative combat and health fields");
    assert(gather.abilities[0] === "future_ability" && gather.futureSentinel.mustSurvive, "prediction preserves optional and future authoritative fields");
    assert(build.state === "build" && build.buildProgress === 0.4 && build.targetId === 91, "missing motion preserves construction activity across prediction frames");
    assert(createRigRenderContext(build).busy === true, "constructing worker keeps the yellow busy indicator across prediction frames");
    assert(build.futureSentinel === "construction-preserved", "construction projection preserves unknown future fields");
    const scaffold = state.entityById(23);
    assert(scaffold.buildProgress === 0.25 + offset * 0.01 && scaffold.buildProgressPredicted === true,
      "construction progress advances through the sparse lane while worker activity remains authoritative");
    assert(state.entityById(22).x === 96 && state.entityById(22).predicted !== true, "prediction patches cannot alter another player's entity");
    assert(state.entityById(999) === undefined, "prediction patches cannot create entities absent from authority");
    const variants = state.entityVariants(1);
    assert(variants.interpolatedEntities.find((e) => e.id === 20)?.state === "gather", "frame variants use the same authoritative-first compositor");
  }
}

{
  const compatibility = predictionCompatibility({
    playerId: 1,
    spectator: false,
    predictionVersion: PREDICTION_PROTOCOL_VERSION,
    predictionBuildId: "same-build",
    players: [
      { id: 1, factionId: "phase2_empty_fixture" },
      { id: 2, factionId: DEFAULT_FACTION_ID },
    ],
  }, { clientBuildId: "same-build" });
  assert(compatibility.ok === false, "unsupported local faction disables prediction");
  assert(
    compatibility.reason === "unsupported-local-faction",
    "unsupported local faction uses stable diagnostic reason",
  );
  const runtime = predictionRuntimeCompatibility({
    playerId: 1,
    spectator: false,
    predictionVersion: PREDICTION_PROTOCOL_VERSION,
    predictionBuildId: "same-build",
    players: [{ id: 1, factionId: "phase2_empty_fixture" }],
  }, { clientBuildId: "same-build" });
  assert(runtime.ok === true, "unsupported pose faction remains eligible for owner-safe display progress");
}

{
  const compatibility = predictionCompatibility({
    playerId: 1,
    spectator: false,
    predictionVersion: PREDICTION_PROTOCOL_VERSION,
    predictionBuildId: "same-build",
    players: [
      { id: 1, factionId: DEFAULT_FACTION_ID },
      { id: 2, factionId: "phase2_empty_fixture" },
    ],
  }, { clientBuildId: "same-build" });
  assert(compatibility.ok === true, "unsupported remote faction alone does not disable local prediction");
}

{
  const controller = new PredictionController({
    now: () => 1000,
    sendCommand: () => true,
    uiConfirmationSnapshots: 4,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 1,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodQueue: 0 }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  const issued = controller.issueCommand({ c: "train", building: 20, unit: "rifleman" });
  assert(issued.sent === true && issued.predicted === false, "train commands remain network handoff only");
  assert(issued.clientSeq === 1, "optimistic UI entries are keyed by the sequenced command handoff");
  let ui = controller.optimisticUiState();
  const overlay = controller.predictionDisplayOverlay();
  assert(ui.production.length === 1, "train optimism appears immediately");
  assert(overlay.optimisticCommands.production.length === 1, "controller exposes optimism through prediction display overlay");
  assert(ui.production[0].optimisticQueue === 1, "train optimism exposes predicted queue depth");
  controller.applyAuthoritativeSnapshot({
    tick: 3,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodKind: "rifleman", prodQueue: 1 }],
    netStatus: { lastSimConsumedClientSeq: 1 },
  });
  ui = controller.optimisticUiState();
  assert(ui.production.length === 0, "authoritative production queue confirms train optimism");
  assert(controller.debugSummary().uiConfirmedCount === 1, "train confirmation is counted");
}

{
  const controller = new PredictionController({
    now: () => 1500,
    sendCommand: () => true,
    uiConfirmationSnapshots: 4,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 1,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodKind: "rifleman", prodQueue: 1 }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  controller.issueCommand({ c: "train", building: 20, unit: "machine_gunner" });
  controller.applyAuthoritativeSnapshot({
    tick: 2,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodKind: "rifleman", prodQueue: 2 }],
    netStatus: { lastSimConsumedClientSeq: 1 },
  });
  assert(controller.optimisticUiState().production.length === 1, "different authoritative prodKind does not confirm train optimism");
}

{
  const controller = new PredictionController({
    now: () => 2000,
    sendCommand: () => true,
    uiConfirmationSnapshots: 2,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 1,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodQueue: 0 }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  controller.issueCommand({ c: "train", building: 20, unit: "rifleman" });
  controller.applyAuthoritativeSnapshot({
    tick: 2,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodQueue: 0 }],
    netStatus: { lastSimConsumedClientSeq: 1 },
  });
  assert(controller.optimisticUiState().production.length === 1, "unconfirmed train optimism survives first snapshot");
  controller.applyAuthoritativeSnapshot({
    tick: 3,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodQueue: 0 }],
    netStatus: { lastSimConsumedClientSeq: 1 },
  });
  assert(controller.optimisticUiState().production.length === 0, "unconfirmed train optimism expires");
  assert(controller.debugSummary().uiExpiredCount === 1, "train expiration is counted");
}

{
  const controller = new PredictionController({
    now: () => 2250,
    sendCommand: () => true,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 1,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodQueue: 0 }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  controller.issueCommand({ c: "train", building: 20, unit: "rifleman" });
  assert(controller.optimisticUiState().production.length === 1, "train optimism is present before rejection");
  controller.recordCommandRejection(1, "Not enough steel");
  assert(controller.optimisticUiState().production.length === 0, "rejection clears matching train optimism");
  assert(controller.debugSummary().uiRejectedCount === 1, "rejected optimism is counted");
}

{
  const controller = new PredictionController({
    now: () => 2500,
    sendCommand: () => true,
    uiConfirmationSnapshots: 4,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 1,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodQueue: 0 }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  controller.issueCommand({ c: "train", building: 20, unit: "rifleman" });
  controller.issueCommand({ c: "train", building: 20, unit: "rifleman" });
  let ui = controller.optimisticUiState();
  assert(ui.production.map((entry) => entry.optimisticQueue).join(",") === "1,2", "repeated train optimism stacks queue depths");
  controller.applyAuthoritativeSnapshot({
    tick: 2,
    entities: [{ id: 20, owner: 1, kind: "barracks", prodKind: "rifleman", prodQueue: 1 }],
    netStatus: { lastSimConsumedClientSeq: 2 },
  });
  ui = controller.optimisticUiState();
  assert(ui.production.length === 1 && ui.production[0].optimisticQueue === 2, "partial train confirmation leaves later queue optimism pending");
}

{
  const controller = new PredictionController({
    now: () => 3000,
    sendCommand: () => true,
    uiConfirmationSnapshots: 4,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 10,
    entities: [{
      id: 30,
      owner: 1,
      kind: "city_centre",
      rallyPlan: [{ kind: "move", x: 100, y: 100 }],
    }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  controller.issueCommand({ c: "setRally", building: 30, x: 160, y: 180, kind: "attackMove", queued: true });
  let ui = controller.optimisticUiState();
  assert(ui.rally.length === 1, "rally optimism appears immediately");
  assert(ui.rally[0].plan.length === 2 && ui.rally[0].plan[1].kind === "attackMove", "queued rally optimism appends to known plan");
  controller.applyAuthoritativeSnapshot({
    tick: 14,
    entities: [{
      id: 30,
      owner: 1,
      kind: "city_centre",
      rallyPlan: [
        { kind: "move", x: 100, y: 100 },
        { kind: "attackMove", x: 160, y: 180 },
      ],
    }],
    netStatus: { lastSimConsumedClientSeq: 1 },
  });
  assert(controller.optimisticUiState().rally.length === 0, "coalesced snapshot can confirm rally optimism");
}

{
  const controller = new PredictionController({
    now: () => 3500,
    sendCommand: () => true,
  });
  controller.applyAuthoritativeSnapshot({
    tick: 1,
    entities: [{ id: 10, owner: 1, kind: "worker" }],
    netStatus: { lastSimConsumedClientSeq: 0 },
  });
  controller.issueCommand({ c: "build", units: [10], building: "depot", tileX: 1, tileY: 1 });
  assert(controller.optimisticUiState().production.length === 0, "build commands remain authoritative-only");
  assert(COMMAND_PREDICTION_POLICIES.build.uiOptimism === false, "build policy documents no UI optimism");
  assert(COMMAND_PREDICTION_POLICIES.research.uiOptimism === false, "research policy documents no UI optimism");
  assert(COMMAND_PREDICTION_POLICIES.useAbility.uiOptimism === false, "ability policy documents no UI optimism");
}

{
  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: [{ id: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 }],
  });
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [{ id: 30, owner: 1, kind: "city_centre", x: 64, y: 64, hp: 500, maxHp: 500 }],
    events: [],
  });
  state.setSelection([30]);
  state.applyPredictionDisplayOverlay({
    optimisticCommands: {
      production: [{ building: 30, unit: "worker", optimisticQueue: 1 }],
      rally: [{ building: 30, plan: [{ kind: "move", x: 220, y: 240 }] }],
    },
  });
  assert(state.optimisticProduction.length === 1, "state keeps full optimistic production list for reservations");
  const selected = state.selectedEntities()[0];
  assert(selected.optimisticProduction === true && selected.prodQueue === 1, "selected building exposes optimistic production");
  assert(selected.optimisticRally === true && selected.rallyPlan[0].x === 220, "selected building exposes optimistic rally plan");
  const rendered = state.entitiesInterpolated(1).find((e) => e.id === 30);
  assert(rendered.optimisticProduction === true, "rendered building exposes optimistic production");
}

{
  const state = new GameState({
    playerId: 1,
    spectator: false,
    map: { width: 8, height: 8, tileSize: 32, terrain: new Array(64).fill(0), resources: [] },
    players: [
      { id: 1, teamId: 1, name: "A", color: "#f00", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 1, name: "B", color: "#0f0", startTileX: 2, startTileY: 2 },
    ],
  });
  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 32, y: 32, hp: 40, maxHp: 40, state: "idle" },
      { id: 11, owner: 2, kind: "worker", x: 96, y: 32, hp: 40, maxHp: 40, state: "idle" },
    ],
    events: [],
  });
  state.applyPredictionDisplayOverlay({
    predictionFrame: {
      tick: 2,
      entities: [
        { id: 10, x: 48, y: 32, motion: "move" },
        { id: 11, x: 128, y: 32, motion: "move" },
      ],
    },
  });
  const rendered = state.entitiesInterpolated(1);
  assert(rendered.find((e) => e.id === 10)?.predicted === true, "prediction applies to own units");
  const ally = rendered.find((e) => e.id === 11);
  assert(ally && ally.predicted !== true && ally.x === 96, "prediction remains own-unit-only for allied units");
}

{
  const files = [
    ["client/src/input/commands.js", "viewport right-click and hotkeys"],
    ["client/src/input/placement.js", "build placement"],
    ["client/src/minimap.js", "minimap right-click and rally"],
    ["client/src/hud.js", "HUD stop/train/research/cancel/ability"],
  ];
  for (const [file, label] of files) {
    const source = fs.readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
    assert(source.includes("this.commandInteraction.issueCommand"), `${file} routes ${label} through the shared command interaction`);
    assert(!source.includes(".net.command("), `${file} does not send gameplay commands through Net`);
  }
  const interactionSource = fs.readFileSync(new URL("../client/src/command_interaction.js", import.meta.url), "utf8");
  assert(
    interactionSource.includes("function issueGameplayCommand") &&
      interactionSource.includes("sender.issueCommand(command, options)"),
    "CommandInteraction owns the guarded command issuer",
  );
  const matchSource = fs.readFileSync(new URL("../client/src/match.js", import.meta.url), "utf8");
  assert(matchSource.includes("new SimWasmPredictionAdapter"), "Match wires the WASM prediction adapter");
  assert(matchSource.includes("predictor: this.predictionAdapter"), "PredictionController receives the adapter");
  assert(matchSource.includes("advancePredictionVisual"), "Match advances predicted movement before render");
  assert(matchSource.includes("applyPredictionDisplayOverlay"), "Match routes prediction display through the overlay seam");
}

console.log("prediction_controller: ok");
