#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { ArtifactWriter } from "./artifacts.mjs";
import { serializableScenario } from "./dsl.mjs";
import { compareOwnedOrderPlan, compareOwnedPosition } from "./diffs.mjs";
import { LocalLaneUnavailable } from "./lanes/local_lane.mjs";

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
    if (result.status !== "passed") failed += 1;
  }
  if (failed > 0 && !args.allowFailure) process.exit(1);
}

export async function runScenario(scenario, args = {}) {
  const artifacts = new ArtifactWriter(scenario.name, { root: args.artifactRoot });
  artifacts.writeScenario(serializableScenario(scenario));
  const command = reproductionCommand(scenario.name);
  const local = new LocalLaneUnavailable({ artifacts });
  const lanes = { local };
  const roomPrefix = `tri-state-${scenario.name}-${Date.now()}-${Math.floor(Math.random() * 10000)}`;
  let status = "passed";
  let failure = null;
  try {
    await local.start();
    if (scenario.setup.kind === "liveRoom") {
      const { RemoteLane } = await import("./lanes/remote_lane.mjs");
      const { ClientLane } = await import("./lanes/client_lane.mjs");
      lanes.remote = new RemoteLane({ scenario, room: `${roomPrefix}-remote`, artifacts, url: args.wsUrl });
      lanes.client = new ClientLane({ scenario, room: `${roomPrefix}-client`, artifacts, url: args.baseUrl, chrome: args.chrome });
      await lanes.remote.start();
      await lanes.client.start();
    } else if (scenario.setup.kind === "devScenario") {
      const { ClientLane } = await import("./lanes/client_lane.mjs");
      lanes.client = new ClientLane({ scenario, room: `${roomPrefix}-client`, artifacts, url: args.baseUrl, chrome: args.chrome });
      await lanes.client.start();
    } else if (scenario.setup.kind !== "artifactOnly") {
      throw new Error(`unsupported setup kind: ${scenario.setup.kind}`);
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
        "The local lane is intentionally unavailable until Phase 3.5.",
      ],
    });
  }
  return { status, artifactDir: artifacts.dir, failure };
}

async function executeStep(context, step) {
  const { lanes, artifacts } = context;
  switch (step.op) {
    case "selectOwn":
      await lanes.remote?.selectOwn(step.kind, step.index);
      await lanes.client?.selectOwn(step.kind, step.index);
      break;
    case "issue":
      if (lanes.remote) artifacts.timeline({ event: "remote.issue", result: await lanes.remote.issue(step.command, step.args) });
      if (lanes.client) artifacts.timeline({ event: "client.issue", result: await lanes.client.issue(step.command, step.args) });
      break;
    case "issueBurst":
      for (let i = 0; i < step.commands.length; i += 1) {
        const entry = step.commands[i];
        if (entry.select) {
          await lanes.remote?.selectOwn(entry.select.kind, entry.select.index || 0);
          await lanes.client?.selectOwn(entry.select.kind, entry.select.index || 0);
        }
        if (lanes.remote) artifacts.timeline({ event: "remote.issue", burstIndex: i, result: await lanes.remote.issue(entry.command, entry.args || {}) });
        if (lanes.client) artifacts.timeline({ event: "client.issue", burstIndex: i, result: await lanes.client.issue(entry.command, entry.args || {}) });
      }
      break;
    case "waitForSnapshot":
      await Promise.all([
        lanes.remote?.waitForSnapshot(step),
        lanes.client?.waitForSnapshot(step),
      ].filter(Boolean));
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
      await lanes.client.waitForSnapshot({ minTickDelta: 1, timeoutMs: 3000 });
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

function comparePredictionSummary(controller, step) {
  const checks = [];
  const add = (name, ok, actual, expected) => checks.push({ name, ok, actual, expected });
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
  ]) {
    if (step[option] != null) add(field, (controller[field] || 0) >= step[option], controller[field], `>=${step[option]}`);
  }
  return { ok: checks.every((check) => check.ok), checks };
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
