import fs from "node:fs";
import { PredictionController, PREDICTION_STATE } from "../client/src/prediction_controller.js";
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
  controller.recordSocketReceipt(4, { serverTick: 10 });
  assert(controller.debugSummary().pendingClientSeqs.join(",") === "4,5", "socket receipt is diagnostic only");
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
  controller.reset({ enabled: false, preserveClientSeq: true });
  controller.issueCommand({ c: "stop", units: [2] });
  controller.reset({ enabled: true, preserveClientSeq: true });
  controller.issueCommand({ c: "stop", units: [3] });
  assert(sentSeqs(sent) === "1,2,3", "prediction toggles preserve monotonic command sequence ids");
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
  state.setPredictedSnapshot({
    tick: 3,
    entities: [{ id: 10, owner: 1, kind: "worker", x: 52, y: 32, hp: 40, maxHp: 40, state: "move" }],
  });
  assert(state.entitiesInterpolated(1)[0].x === 52, "render reads predicted owned position");
  assert(
    state.entitiesInterpolated(1, { includePrediction: false })[0].x === 32,
    "authoritative reads can ignore prediction for fog",
  );
  assert(state.entityById(10).x === 52, "entityById exposes predicted owned position for local UX");
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
  controller.issueCommand({ c: "train", building: 20, unit: "rifleman" });
  let ui = controller.optimisticUiState();
  assert(ui.production.length === 1, "train optimism appears immediately");
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
  state.setOptimisticCommandState({
    production: [{ building: 30, unit: "worker", optimisticQueue: 1 }],
    rally: [{ building: 30, plan: [{ kind: "move", x: 220, y: 240 }] }],
  });
  const selected = state.selectedEntities()[0];
  assert(selected.optimisticProduction === true && selected.prodQueue === 1, "selected building exposes optimistic production");
  assert(selected.optimisticRally === true && selected.rallyPlan[0].x === 220, "selected building exposes optimistic rally plan");
  const rendered = state.entitiesInterpolated(1).find((e) => e.id === 30);
  assert(rendered.optimisticProduction === true, "rendered building exposes optimistic production");
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
    assert(source.includes("this._issueCommand"), `${file} routes ${label} through the guarded command issuer`);
    assert(!source.includes(".net.command("), `${file} does not send gameplay commands through Net`);
  }
  const matchSource = fs.readFileSync(new URL("../client/src/match.js", import.meta.url), "utf8");
  assert(matchSource.includes("new SimWasmPredictionAdapter"), "Match wires the WASM prediction adapter");
  assert(matchSource.includes("predictor: this.predictionAdapter"), "PredictionController receives the adapter");
  assert(matchSource.includes("advancePredictionVisual"), "Match advances predicted movement before render");
}

console.log("prediction_controller: ok");
