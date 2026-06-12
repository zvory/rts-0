#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { ArtifactWriter } from "./artifacts.mjs";
import { serializableScenario } from "./dsl.mjs";
import { compareOwnedOrderPlan, compareOwnedPosition, ownEntityByKind, summarizePlan } from "./diffs.mjs";
import { WasmLocalLane } from "./lanes/local_lane.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SCENARIO_DIR = path.join(HERE, "scenarios");
const DEFAULT_SCENARIOS = ["remote_client_basic_move", "queued_order_visibility", "dev_scenario_step_tick"];
const SCENARIO_GROUPS = Object.freeze({
  "phase-0.5": DEFAULT_SCENARIOS,
  "phase-2.5": [
    "client_seq_monotonic_all_paths",
    "ack_drops_consumed_pending_commands",
    "ack_three_leaves_four_five_pending",
    "socket_receipt_not_reconciliation_ack",
    "duplicate_and_skipped_snapshots_are_diagnostic",
    "stale_snapshot_ignored",
    "rejection_notice_does_not_imply_ack",
  ],
  "phase-3.5": [
    "local_lane_initializes_from_start",
    "local_lane_noop_ticks",
    "local_lane_simple_move",
    "local_lane_queued_move",
    "owner_safe_baseline_no_hidden_enemy_leak",
    "unsupported_command_is_explicit",
  ],
  "phase-4.5": [
    "move_predicts_before_authoritative_echo",
    "move_converges_after_ack_5_ticks",
    "move_converges_after_ack_10_ticks",
    "move_converges_after_ack_20_ticks",
    "coalesced_snapshots_replay_pending",
    "dropped_snapshot_does_not_stick_pending",
    "queued_move_order_stages_survive_replay",
    "stop_corrects_predicted_motion",
    "hidden_blocker_correction_no_leak",
    "prediction_disabled_authoritative_only",
    "spectator_replay_no_prediction",
  ],
  "phase-5": [
    "train_optimistic_queue_confirms_after_ack",
    "train_optimism_rejection_clears_by_seq",
    "rally_optimistic_marker_confirms_after_ack",
  ],
  "phase-6": [
    "combat_command_authoritative_only",
    "spectator_replay_no_prediction",
  ],
});

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const scenarioFiles = await scenarioFilesFor(args);
  if (args.list) {
    for (const file of scenarioFiles) console.log(path.basename(file, ".mjs"));
    return;
  }
  let failed = 0;
  for (const file of scenarioFiles) {
    const scenario = (await import(pathToFileURL(file).href)).default;
    const result = await runScenario(scenario, args);
    console.log(`${result.status === "passed" ? "PASS" : "FAIL"} ${scenario.name} ${result.artifactDir}`);
    if (result.status !== "passed" && result.failure) {
      console.error(result.failure.stack || result.failure.message || JSON.stringify(result.failure));
    }
    if (result.status !== "passed") failed += 1;
  }
  if (failed > 0 && !args.allowFailure) process.exit(1);
}

export async function runScenario(scenario, args = {}) {
  const artifacts = new ArtifactWriter(scenario.name, { root: args.artifactRoot });
  artifacts.writeScenario(serializableScenario(scenario));
  const command = reproductionCommand(scenario.name);
  const local = new WasmLocalLane({ scenario, artifacts });
  const lanes = { local };
  const roomPrefix = `tri-state-${scenario.name}-${Date.now()}-${Math.floor(Math.random() * 10000)}`;
  let status = "passed";
  let failure = null;
  try {
    if (scenario.setup.kind === "liveRoom") {
      const { RemoteLane } = await import("./lanes/remote_lane.mjs");
      const { ClientLane } = await import("./lanes/client_lane.mjs");
      lanes.remote = new RemoteLane({ scenario, room: `${roomPrefix}-remote`, artifacts, url: args.wsUrl });
      lanes.client = new ClientLane({ scenario, room: `${roomPrefix}-client`, artifacts, url: args.baseUrl, chrome: args.chrome });
      await lanes.remote.start();
      await lanes.client.start();
      const startInfo = await lanes.client.startPayload();
      await local.start({
        startInfo,
        playerId: startInfo?.playerId ?? lanes.remote.playerId,
        baselineSnapshot: await lanes.client.currentSnapshot(),
      });
    } else if (scenario.setup.kind === "devScenario") {
      const { ClientLane } = await import("./lanes/client_lane.mjs");
      lanes.client = new ClientLane({ scenario, room: `${roomPrefix}-client`, artifacts, url: args.baseUrl, chrome: args.chrome });
      await lanes.client.start();
      const startInfo = await lanes.client.startPayload();
      await local.start({
        startInfo,
        playerId: startInfo?.playerId,
        baselineSnapshot: await lanes.client.currentSnapshot(),
      });
    } else if (scenario.setup.kind !== "artifactOnly") {
      throw new Error(`unsupported setup kind: ${scenario.setup.kind}`);
    } else {
      await local.start();
    }

    const context = { scenario, artifacts, lanes, captures: new Map(), tickMarks: [] };
    for (let index = 0; index < scenario.steps.length; index += 1) {
      const step = scenario.steps[index];
      artifacts.timeline({ event: "step.begin", index, step });
      await executeStep(context, step);
      await local.applyStep(step);
      artifacts.timeline({ event: "step.end", index, op: step.op });
    }
  } catch (err) {
    status = "failed";
    failure = { message: err.message, stack: err.stack };
    artifacts.timeline({ event: "failure", failure });
  } finally {
    await Promise.allSettled(Object.values(lanes).map((lane) => lane?.close?.()));
    artifacts.writeSummary({
      status,
      failure,
      command,
      notes: [
        scenario.setup.kind === "liveRoom"
          ? "Live-room scenarios compare a direct WebSocket authoritative room with a real browser room driven by the same DSL commands."
          : "This scenario does not use both live lanes.",
        "The local lane uses generated rts-sim-wasm assets when present and records an explicit disabled reason when they are missing.",
      ],
    });
  }
  return { status, artifactDir: artifacts.dir, failure };
}

async function executeStep(context, step) {
  const { scenario, lanes, artifacts } = context;
  switch (step.op) {
    case "selectOwn":
      await lanes.remote?.selectOwn(step.kind, step.index);
      await lanes.client?.selectOwn(step.kind, step.index);
      await lanes.local?.selectOwn(step.kind, step.index);
      break;
    case "issue": {
      const remoteResult = lanes.remote ? await lanes.remote.issue(step.command, step.args) : null;
      const clientResult = lanes.client ? await lanes.client.issue(step.command, step.args) : null;
      if (lanes.remote) artifacts.timeline({ event: "remote.issue", result: remoteResult });
      if (lanes.client) artifacts.timeline({ event: "client.issue", result: clientResult });
      if (lanes.local) artifacts.timeline({ event: "local.issue", result: await lanes.local.issue(step.command, step.args, remoteResult || clientResult || {}) });
      break;
    }
    case "issueBurst":
      for (let i = 0; i < step.commands.length; i += 1) {
        const entry = step.commands[i];
        if (entry.select) {
          await lanes.remote?.selectOwn(entry.select.kind, entry.select.index || 0);
          await lanes.client?.selectOwn(entry.select.kind, entry.select.index || 0);
          await lanes.local?.selectOwn(entry.select.kind, entry.select.index || 0);
        }
        const remoteResult = lanes.remote ? await lanes.remote.issue(entry.command, entry.args || {}) : null;
        const clientResult = lanes.client ? await lanes.client.issue(entry.command, entry.args || {}) : null;
        if (lanes.remote) artifacts.timeline({ event: "remote.issue", burstIndex: i, result: remoteResult });
        if (lanes.client) artifacts.timeline({ event: "client.issue", burstIndex: i, result: clientResult });
        if (lanes.local) artifacts.timeline({ event: "local.issue", burstIndex: i, result: await lanes.local.issue(entry.command, entry.args || {}, remoteResult || clientResult || {}) });
      }
      break;
    case "waitForSnapshot":
      await Promise.all([
        lanes.remote?.waitForSnapshot(step),
        lanes.client?.waitForSnapshot(step),
      ].filter(Boolean));
      break;
    case "waitMs":
      await new Promise((resolve) => setTimeout(resolve, Math.max(0, Number(step.ms) || 0)));
      break;
    case "waitForAck":
      await Promise.all([
        lanes.remote?.waitForAck(step.clientSeq, step),
        lanes.client?.waitForAck(step.clientSeq, step),
      ].filter(Boolean));
      break;
    case "capture": {
      const [remote, client, local] = await Promise.all([
        lanes.remote ? Promise.resolve(lanes.remote.capture(step.label)) : Promise.resolve(null),
        lanes.client ? lanes.client.capture(step.label) : Promise.resolve(null),
        lanes.local.capture(step.label),
      ]);
      const capture = { remote, client, local };
      context.captures.set(step.label, capture);
      context.lastCapture = capture;
      break;
    }
    case "importLocalBaseline": {
      const source = step.source === "remote" ? lanes.remote?.lastSnapshot : await lanes.client?.currentSnapshot?.();
      artifacts.timeline({ event: "local.baseline", result: lanes.local.importBaseline(source, step.label || step.source || "scenario") });
      break;
    }
    case "advanceLocalTicks":
      artifacts.timeline({ event: "local.advance", result: lanes.local.advanceTicks(step.ticks, "scenario") });
      break;
    case "assertRemoteClientOwnedPosition": {
      requireLiveLanes(lanes, step.op);
      const frame = context.lastCapture || {};
      const diff = compareOwnedPosition({
        remote: frame.remote || lanes.remote.summary(),
        client: frame.client || await lanes.client.summary(),
        kind: step.unit,
        index: step.index || 0,
        tolerancePx: step.tolerancePx,
      });
      artifacts.diff({ assertion: step.op, diff });
      if (!diff.ok) throw new Error(`owned ${step.unit}[${step.index || 0}] position diverged: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientAuthoritativeOwnedStable": {
      requireClientLane(lanes, step.op);
      const diff = compareSummaryOwnedStable({
        before: context.captures.get(step.before)?.client,
        after: context.captures.get(step.after)?.client || context.lastCapture?.client || await lanes.client.summary(),
        kind: step.unit,
        index: step.index || 0,
        tolerancePx: step.tolerancePx,
      });
      artifacts.diff({ assertion: step.op, diff, network: scenario.network });
      if (!diff.ok) throw new Error(`client authoritative ${step.unit} was not stable: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientRenderedOwnedAdvanced": {
      requireClientLane(lanes, step.op);
      const diff = compareSummaryOwnedAdvanced({
        before: renderedSummary(context.captures.get(step.before)?.client),
        after: renderedSummary(context.captures.get(step.after)?.client || context.lastCapture?.client || await lanes.client.summary()),
        kind: step.unit,
        index: step.index || 0,
        minDistancePx: step.minDistancePx,
      });
      artifacts.diff({ assertion: step.op, diff, network: scenario.network });
      if (!diff.ok) throw new Error(`client rendered ${step.unit} did not advance: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientRenderedOwnedStable": {
      requireClientLane(lanes, step.op);
      const diff = compareSummaryOwnedStable({
        before: renderedSummary(context.captures.get(step.before)?.client),
        after: renderedSummary(context.captures.get(step.after)?.client || context.lastCapture?.client || await lanes.client.summary()),
        kind: step.unit,
        index: step.index || 0,
        tolerancePx: step.tolerancePx,
      });
      artifacts.diff({ assertion: step.op, diff, network: scenario.network });
      if (!diff.ok) throw new Error(`client rendered ${step.unit} was not stable: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientRenderedConverged": {
      requireClientLane(lanes, step.op);
      const summary = context.lastCapture?.client || await lanes.client.summary();
      const diff = compareOwnedPosition({
        remote: summary,
        client: renderedSummary(summary),
        kind: step.unit,
        index: step.index || 0,
        tolerancePx: step.tolerancePx,
      });
      artifacts.diff({ assertion: step.op, diff, network: scenario.network });
      if (!diff.ok) throw new Error(`client rendered ${step.unit} did not converge: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertOrderPlansMatch": {
      requireLiveLanes(lanes, step.op);
      const frame = context.lastCapture || {};
      const diff = compareOwnedOrderPlan({
        remote: frame.remote || lanes.remote.summary(),
        client: frame.client || await lanes.client.summary(),
        kind: step.unit,
        index: step.index || 0,
      });
      artifacts.diff({ assertion: step.op, diff });
      if (!diff.ok) throw new Error(`owned ${step.unit}[${step.index || 0}] order plan diverged: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalReady": {
      const summary = lanes.local.summary();
      const diff = { assertion: step.op, ok: !!summary.ready && summary.playerId != null, summary };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`local lane is not ready: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalDisabledReason": {
      const summary = lanes.local.summary();
      const reasons = summary.disabledReasons || [];
      const diff = { assertion: step.op, ok: reasons.includes(step.reason), expected: step.reason, reasons, summary };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`local lane missing disabled reason ${step.reason}: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalUnsupportedField": {
      const summary = lanes.local.summary();
      const fields = summary.unsupportedFields || [];
      const diff = { assertion: step.op, ok: fields.includes(step.field), expected: step.field, fields };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`local lane missing unsupported field ${step.field}: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalRenderOwnedOnly": {
      const frame = [...(lanes.local.frames || [])].reverse().find((entry) => entry.renderSnapshot);
      const entities = frame?.renderSnapshot?.entities || [];
      const playerId = lanes.local.playerId;
      const diff = {
        assertion: step.op,
        ok: frame && playerId != null && entities.every((entity) => entity.owner === playerId),
        playerId,
        entities,
      };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`local render snapshot included non-owned entities: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalOwnedStable": {
      const diff = compareLocalStable({
        before: context.captures.get(step.before)?.local,
        after: context.captures.get(step.after)?.local,
        kind: step.unit,
        index: step.index || 0,
        tolerancePx: step.tolerancePx ?? 0.01,
      });
      artifacts.diff({ assertion: step.op, diff });
      if (!diff.ok) throw new Error(`local owned ${step.unit} was not stable: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalOwnedAdvanced": {
      const diff = compareLocalAdvanced({
        before: context.captures.get(step.before)?.local,
        after: context.lastCapture?.local || lanes.local.summary(),
        kind: step.unit,
        index: step.index || 0,
        minDistancePx: step.minDistancePx ?? 1,
      });
      artifacts.diff({ assertion: step.op, diff });
      if (!diff.ok) throw new Error(`local owned ${step.unit} did not advance: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalOrderPlan": {
      const diff = compareLocalOrderPlan({
        summary: context.lastCapture?.local || lanes.local.summary(),
        kind: step.unit,
        index: step.index || 0,
        expected: step.expected || [],
      });
      artifacts.diff({ assertion: step.op, diff });
      if (!diff.ok) throw new Error(`local order plan mismatch: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalPendingClientSeqs": {
      const summary = lanes.local.summary();
      const diff = {
        assertion: step.op,
        ok: JSON.stringify(summary.pendingClientSeqs || []) === JSON.stringify(step.seqs || []),
        actual: summary.pendingClientSeqs || [],
        expected: step.seqs || [],
      };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`local pending clientSeqs mismatch: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalCorrectionAtMost": {
      const summary = lanes.local.summary();
      const actual = Number(summary.correctionMagnitude) || 0;
      const diff = { assertion: step.op, ok: actual <= step.maxPx, actual, expected: `<=${step.maxPx}`, network: scenario.network };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`local correction too large: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientCorrectionBudget": {
      requireClientLane(lanes, step.op);
      const debug = await lanes.client.predictionDebug();
      const controller = debug?.controller || {};
      const diff = {
        assertion: step.op,
        ok: (controller.maxCorrectionDistance || 0) <= step.maxPx
          && (controller.snapCorrectionCount || 0) <= step.maxSnapCorrections
          && (step.maxCorrectionCount == null || (controller.correctionCount || 0) <= step.maxCorrectionCount),
        maxCorrectionDistance: round(controller.maxCorrectionDistance || 0),
        correctionCount: controller.correctionCount || 0,
        snapCorrectionCount: controller.snapCorrectionCount || 0,
        pendingCommandCount: controller.pendingCommandCount || 0,
        latestAckSeq: controller.latestAckSeq || 0,
        networkProfile: scenario.network?.name || scenario.network?.mode || "direct",
        expected: {
          maxPx: step.maxPx,
          maxSnapCorrections: step.maxSnapCorrections,
          maxCorrectionCount: step.maxCorrectionCount ?? null,
        },
      };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`client correction budget exceeded: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertLocalBaselineOwnerSafe": {
      const baselineFrame = [...(lanes.local.frames || [])].reverse().find((frame) => frame.event === "baseline" && frame.imported);
      const diff = assertOwnerSafeBaseline(baselineFrame?.baselineSummary);
      artifacts.diff({ assertion: step.op, diff });
      if (!diff.ok) throw new Error(`owner-safe baseline leaked hidden state: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientSeqsStrictlyIncreasing": {
      const commands = lanes.client?.issuedCommands || [];
      const seqs = commands.map((entry) => entry.clientSeq);
      const ok = seqs.every((seq, index) => index === 0 || seq > seqs[index - 1]);
      const expectedCount = step.count ?? null;
      const diff = { assertion: step.op, ok: ok && (expectedCount == null || seqs.length === expectedCount), seqs, expectedCount };
      artifacts.diff(diff);
      if (!diff.ok) throw new Error(`clientSeqs are not strictly increasing: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientPrediction": {
      requireClientLane(lanes, step.op);
      const debug = await lanes.client.predictionDebug();
      const controller = debug?.controller || {};
      const diff = comparePredictionSummary(controller, step);
      artifacts.diff({ assertion: step.op, diff, controller });
      if (!diff.ok) throw new Error(`client prediction assertion failed: ${JSON.stringify(diff)}`);
      break;
    }
    case "assertClientOptimisticUi": {
      requireClientLane(lanes, step.op);
      const state = await lanes.client.optimisticCommandState();
      const diff = compareOptimisticUiState(state, step);
      artifacts.diff({ assertion: step.op, diff, state });
      if (!diff.ok) throw new Error(`client optimistic UI assertion failed: ${JSON.stringify(diff)}`);
      break;
    }
    case "waitForClientPredictionReady": {
      requireClientLane(lanes, step.op);
      const debug = await lanes.client.waitForPredictionReady(step);
      artifacts.timeline({ event: "client.predictionReady", debug });
      break;
    }
    case "advanceClientPredictionVisual": {
      requireClientLane(lanes, step.op);
      artifacts.timeline({ event: "client.advancePredictionVisual", result: await lanes.client.advancePredictionVisual() });
      break;
    }
    case "injectClientSnapshot":
      requireClientLane(lanes, step.op);
      artifacts.timeline({ event: "client.injectSnapshot", result: await lanes.client.injectSnapshot(step.kind, step) });
      break;
    case "setClientSnapshotDelivery":
      requireClientLane(lanes, step.op);
      artifacts.timeline({ event: "client.snapshotDelivery", result: await lanes.client.setSnapshotDelivery(step.enabled) });
      break;
    case "recordSocketReceipt":
      requireClientLane(lanes, step.op);
      artifacts.timeline({ event: "client.socketReceipt", result: await lanes.client.recordSocketReceipt(step.clientSeq, step.detail || {}) });
      break;
    case "recordCommandRejection":
      requireClientLane(lanes, step.op);
      artifacts.timeline({ event: "client.commandRejection", result: await lanes.client.recordCommandRejection(step.clientSeq, step.reason) });
      break;
    case "expireClientCommands":
      requireClientLane(lanes, step.op);
      artifacts.timeline({ event: "client.expireCommands", result: await lanes.client.expireCommands(step.elapsedMs) });
      break;
    case "setReplaySpeed":
      await lanes.client?.setReplaySpeed(step.speed);
      break;
    case "stepDevTick": {
      const before = await lanes.client.currentTick();
      await lanes.client.stepDevTick();
      await lanes.client.waitForSnapshot({ minTickDelta: 1, timeoutMs: 8000 });
      const after = await lanes.client.currentTick();
      context.tickMarks.push({ before, after });
      artifacts.diff({ assertion: "stepDevTick", before, after, delta: after - before });
      break;
    }
    case "assertTickAdvanced": {
      const mark = context.tickMarks.at(-1);
      if (!mark) throw new Error("assertTickAdvanced requires a preceding stepDevTick");
      const delta = mark.after - mark.before;
      artifacts.diff({ assertion: step.op, expected: step.delta, actual: delta, mark });
      if (delta !== step.delta) throw new Error(`expected tick delta ${step.delta}, got ${delta}`);
      break;
    }
    case "forceFailure":
      throw new Error(step.message || "forced failure");
    default:
      throw new Error(`unsupported scenario op: ${step.op}`);
  }
}

function requireLiveLanes(lanes, op) {
  if (!lanes.remote || !lanes.client) throw new Error(`${op} requires remote and client lanes`);
}

function requireClientLane(lanes, op) {
  if (!lanes.client) throw new Error(`${op} requires a client lane`);
}

function compareLocalStable({ before, after, kind, index = 0, tolerancePx = 0.01 }) {
  const a = ownEntityByKind(before, kind, index);
  const b = ownEntityByKind(after, kind, index);
  if (!a || !b) return { ok: false, reason: `missing owned ${kind}[${index}]`, before: a, after: b };
  const distance = Math.hypot((a.x || 0) - (b.x || 0), (a.y || 0) - (b.y || 0));
  return {
    ok: distance <= tolerancePx,
    distance: round(distance),
    tolerancePx,
    before: { id: a.id, x: a.x, y: a.y, tick: before?.tick },
    after: { id: b.id, x: b.x, y: b.y, tick: after?.tick },
  };
}

function compareSummaryOwnedStable({ before, after, kind, index = 0, tolerancePx = 0.01 }) {
  const a = ownEntityByKind(before, kind, index);
  const b = ownEntityByKind(after, kind, index);
  if (!a || !b) return { ok: false, reason: `missing owned ${kind}[${index}]`, before: a, after: b };
  const distance = Math.hypot((a.x || 0) - (b.x || 0), (a.y || 0) - (b.y || 0));
  return {
    ok: distance <= tolerancePx,
    distance: round(distance),
    tolerancePx,
    before: { id: a.id, x: a.x, y: a.y, tick: before?.tick },
    after: { id: b.id, x: b.x, y: b.y, tick: after?.tick },
  };
}

function compareSummaryOwnedAdvanced({ before, after, kind, index = 0, minDistancePx = 1 }) {
  const a = ownEntityByKind(before, kind, index);
  const b = ownEntityByKind(after, kind, index);
  if (!a || !b) return { ok: false, reason: `missing owned ${kind}[${index}]`, before: a, after: b };
  const distance = Math.hypot((a.x || 0) - (b.x || 0), (a.y || 0) - (b.y || 0));
  return {
    ok: distance >= minDistancePx,
    distance: round(distance),
    minDistancePx,
    before: { id: a.id, x: a.x, y: a.y, tick: before?.tick },
    after: { id: b.id, x: b.x, y: b.y, tick: after?.tick },
  };
}

function compareLocalAdvanced({ before, after, kind, index = 0, minDistancePx = 1 }) {
  const a = ownEntityByKind(before, kind, index);
  const b = ownEntityByKind(after, kind, index);
  if (!a || !b) return { ok: false, reason: `missing owned ${kind}[${index}]`, before: a, after: b };
  const distance = Math.hypot((a.x || 0) - (b.x || 0), (a.y || 0) - (b.y || 0));
  return {
    ok: distance >= minDistancePx,
    distance: round(distance),
    minDistancePx,
    before: { id: a.id, x: a.x, y: a.y, tick: before?.tick },
    after: { id: b.id, x: b.x, y: b.y, tick: after?.tick },
  };
}

function renderedSummary(summary) {
  return summary?.rendered || null;
}

function compareLocalOrderPlan({ summary, kind, index = 0, expected = [] }) {
  const entity = ownEntityByKind(summary, kind, index);
  if (!entity) return { ok: false, reason: `missing owned ${kind}[${index}]` };
  const actual = summarizePlan(entity.orderPlan || []);
  const normalizedExpected = summarizePlan(expected);
  return {
    ok: JSON.stringify(actual) === JSON.stringify(normalizedExpected),
    actual,
    expected: normalizedExpected,
  };
}

function assertOwnerSafeBaseline(summary) {
  const checks = [];
  const add = (name, ok, detail = null) => checks.push({ name, ok, detail });
  add("baselineImported", !!summary, summary || null);
  add("ownedIdsPresent", (summary?.ownedEntities || []).every((entity) => Number.isInteger(entity.id)));
  add("visibleObstaclesHaveNoIds", (summary?.visibleObstacles || []).every((obstacle) => obstacle.id == null));
  add("visibleObstaclesHaveNoOrders", (summary?.visibleObstacles || []).every((obstacle) => obstacle.orderPlan == null && obstacle.targetId == null));
  add("visibleObstaclesHaveNoEconomy", (summary?.visibleObstacles || []).every((obstacle) => obstacle.steel == null && obstacle.oil == null && obstacle.production == null));
  return { ok: checks.every((check) => check.ok), checks, summary };
}

function comparePredictionSummary(controller, step) {
  const checks = [];
  const add = (name, ok, actual, expected) => checks.push({ name, ok, actual, expected });
  if (step.enabled != null) {
    add("enabled", controller.enabled === step.enabled, controller.enabled, step.enabled);
  }
  if (step.mode != null) {
    add("mode", controller.mode === step.mode, controller.mode, step.mode);
  }
  if (step.pendingClientSeqs) {
    add(
      "pendingClientSeqs",
      JSON.stringify(controller.pendingClientSeqs || []) === JSON.stringify(step.pendingClientSeqs),
      controller.pendingClientSeqs || [],
      step.pendingClientSeqs,
    );
  }
  for (const [option, field] of [
    ["pendingCommandCount", "pendingCommandCount"],
    ["latestAckSeq", "latestAckSeq"],
    ["acknowledgedCount", "acknowledgedCount"],
    ["staleSnapshotCount", "staleSnapshotCount"],
    ["duplicateSnapshotCount", "duplicateSnapshotCount"],
    ["skippedSnapshotCount", "skippedSnapshotCount"],
    ["receiptCount", "receiptCount"],
    ["rejectionCount", "rejectionCount"],
    ["timedOutCount", "timedOutCount"],
    ["uiConfirmedCount", "uiConfirmedCount"],
    ["uiExpiredCount", "uiExpiredCount"],
    ["uiRejectedCount", "uiRejectedCount"],
  ]) {
    if (step[option] != null) add(field, controller[field] === step[option], controller[field], step[option]);
  }
  for (const [option, field] of [
    ["minAcknowledgedCount", "acknowledgedCount"],
    ["minStaleSnapshotCount", "staleSnapshotCount"],
    ["minDuplicateSnapshotCount", "duplicateSnapshotCount"],
    ["minSkippedSnapshotCount", "skippedSnapshotCount"],
    ["minReceiptCount", "receiptCount"],
    ["minRejectionCount", "rejectionCount"],
    ["minTimedOutCount", "timedOutCount"],
    ["minUiConfirmedCount", "uiConfirmedCount"],
    ["minUiExpiredCount", "uiExpiredCount"],
    ["minUiRejectedCount", "uiRejectedCount"],
  ]) {
    if (step[option] != null) add(field, (controller[field] || 0) >= step[option], controller[field], `>=${step[option]}`);
  }
  return { ok: checks.every((check) => check.ok), checks };
}

function compareOptimisticUiState(state, step) {
  const checks = [];
  const add = (name, ok, actual, expected) => checks.push({ name, ok, actual, expected });
  if (step.productionCount != null) {
    add("productionCount", (state.production || []).length === step.productionCount, (state.production || []).length, step.productionCount);
  }
  if (step.rallyCount != null) {
    add("rallyCount", (state.rally || []).length === step.rallyCount, (state.rally || []).length, step.rallyCount);
  }
  if (step.productionQueue != null) {
    add(
      "productionQueue",
      (state.production || [])[0]?.optimisticQueue === step.productionQueue,
      (state.production || [])[0]?.optimisticQueue,
      step.productionQueue,
    );
  }
  if (step.rallyPlanLength != null) {
    add("rallyPlanLength", (state.rally || [])[0]?.plan?.length === step.rallyPlanLength, (state.rally || [])[0]?.plan?.length, step.rallyPlanLength);
  }
  return { ok: checks.every((check) => check.ok), checks };
}

function round(value) {
  return Number.isFinite(value) ? Math.round(value * 100) / 100 : value;
}

async function scenarioFilesFor(args) {
  const all = fs.readdirSync(SCENARIO_DIR)
    .filter((name) => name.endsWith(".mjs"))
    .map((name) => path.join(SCENARIO_DIR, name))
    .sort();
  if (args.list) return all;
  if (args.scenarios.length === 0) {
    return DEFAULT_SCENARIOS.map((name) => path.join(SCENARIO_DIR, `${name}.mjs`));
  }
  return args.scenarios.flatMap((pattern) => {
    if (SCENARIO_GROUPS[pattern]) {
      return SCENARIO_GROUPS[pattern].map((name) => path.join(SCENARIO_DIR, `${name}.mjs`));
    }
    if (pattern.includes("*")) {
      const regex = new RegExp(`^${pattern.replaceAll(".", "\\.").replaceAll("*", ".*")}\\.mjs$`);
      return all.filter((file) => regex.test(path.basename(file)));
    }
    const file = pattern.endsWith(".mjs")
      ? path.resolve(pattern)
      : path.join(SCENARIO_DIR, `${pattern}.mjs`);
    if (!fs.existsSync(file)) throw new Error(`scenario not found: ${pattern}`);
    return [file];
  });
}

function parseArgs(argv) {
  const args = {
    scenarios: [],
    list: false,
    allowFailure: false,
    artifactRoot: undefined,
    baseUrl: process.env.RTS_URL,
    wsUrl: process.env.RTS_WS,
    chrome: process.env.CHROME,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--list") args.list = true;
    else if (arg === "--allow-failure") args.allowFailure = true;
    else if (arg === "--scenario") args.scenarios.push(argv[++i]);
    else if (arg === "--artifact-root") args.artifactRoot = argv[++i];
    else if (arg === "--base-url") args.baseUrl = argv[++i];
    else if (arg === "--ws-url") args.wsUrl = argv[++i];
    else if (arg === "--chrome") args.chrome = argv[++i];
    else if (arg === "--help") {
      printHelp();
      process.exit(0);
    } else if (!arg.startsWith("--")) {
      args.scenarios.push(arg);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function reproductionCommand(name) {
  return `node tests/tri_state/run.mjs --scenario ${name}`;
}

function printHelp() {
  console.log(`Usage:
  node tests/tri_state/run.mjs [--scenario name|glob] [--allow-failure]

Environment:
  RTS_URL   Browser base URL, default http://127.0.0.1:8081/
  RTS_WS    WebSocket URL, default ws://127.0.0.1:8081/ws
  CHROME    Chrome/Chromium executable for puppeteer-core
`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error(err.stack || err.message);
    process.exit(2);
  });
}
