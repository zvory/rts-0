import fs from "node:fs";
import {
  COMMAND_PREDICTION_POLICIES,
  PredictionController,
  PREDICTION_STATE,
} from "../client/src/prediction_controller.js";
import { predictionCompatibility } from "../client/src/prediction_compatibility.js";
import { DEFAULT_FACTION_ID, PREDICTION_PROTOCOL_VERSION } from "../client/src/protocol.js";
import { GameState } from "../client/src/state.js";

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function sentSeqs(sent) {
  return sent.map((entry) => entry.clientSeq).join(",");
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
    predictedSnapshot: {
      tick: 3,
      entities: [{ id: 10, owner: 1, kind: "worker", x: 52, y: 32, hp: 40, maxHp: 40, state: "move" }],
    },
  });
  assert(state.entitiesInterpolated(1)[0].x === 52, "render reads predicted owned position");
  assert(
    state.entitiesInterpolated(1, { includePrediction: false })[0].x === 32,
    "authoritative reads can ignore prediction for fog",
  );
  assert(state.entityById(10).x === 52, "entityById exposes predicted owned position for local UX");
  state.applyPredictionDisplayOverlay({ optimisticCommands: { production: [], rally: [] } });
  assert(state.entitiesInterpolated(1)[0].x === 52, "optimistic overlay updates do not clear predicted movement");
  assert(state.localFactionId === DEFAULT_FACTION_ID, "GameState exposes normalized local faction identity");
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
    predictedSnapshot: {
      tick: 2,
      entities: [
        { id: 10, owner: 1, kind: "worker", x: 48, y: 32, hp: 40, maxHp: 40, state: "move" },
        { id: 11, owner: 2, kind: "worker", x: 128, y: 32, hp: 40, maxHp: 40, state: "move" },
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
