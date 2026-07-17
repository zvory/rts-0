// tests/client_contracts/net_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { CLIENT_NET_REPORT_FIELDS } from "../client_net_report_fields.mjs";
import {
  assert,
  assertHasGetter,
  assertHasMethod,
  assertThrows,
} from "./assertions.mjs";
import {
  INITIAL_CONNECT_ATTEMPTS,
  INITIAL_CONNECT_RETRY_MS,
  INITIAL_CONNECT_TIMEOUT_MS,
  Net,
  SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES,
} from "../../client/src/net.js";
import { DEFAULT_AI_PROFILE_ID } from "../../client/src/lobby.js";
import { AI_PROFILES } from "../../client/src/lobby_view.js";
import {
  COMPACT_SNAPSHOT_VERSION,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  PREDICTION_PROTOCOL_VERSION,
  cmd,
  msg,
} from "../../client/src/protocol.js";

import { messagePackSnapshotFrame } from "./snapshot_frame_helpers.mjs";

// Net
// ---------------------------------------------------------------------------
{
  assert(INITIAL_CONNECT_ATTEMPTS > 1, "initial Net connection retries are bounded and enabled");
  assert(INITIAL_CONNECT_RETRY_MS > 0, "initial Net retry delay is nonzero");
  assert(INITIAL_CONNECT_TIMEOUT_MS > 0, "each initial Net attempt has a finite timeout");
  const NativeWebSocket = globalThis.WebSocket;
  const sockets = [];
  class StartupWebSocket {
    static OPEN = 1;
    static CLOSED = 3;

    constructor() {
      this.listeners = new Map();
      this.readyState = 0;
      sockets.push(this);
      queueMicrotask(() => {
        if (sockets.length === 1) this.emit("error", {});
        else {
          this.readyState = StartupWebSocket.OPEN;
          this.emit("open", {});
        }
      });
    }

    addEventListener(type, handler) {
      this.listeners.set(type, handler);
    }

    emit(type, event) {
      this.listeners.get(type)?.(event);
    }

    close() {
      this.readyState = StartupWebSocket.CLOSED;
      queueMicrotask(() => this.emit("close", {}));
    }
  }

  globalThis.WebSocket = StartupWebSocket;
  try {
    const retryNet = new Net("ws://example.test/ws");
    let waits = 0;
    await retryNet.connect({ attempts: 2, retryMs: 1, wait: async () => { waits += 1; } });
    assert(sockets.length === 2, "Net retries one failed initial WebSocket connection");
    assert(waits === 1, "Net waits once between sequential connection attempts");
    assert(sockets[0].readyState === StartupWebSocket.CLOSED, "failed socket closes before retry starts");
    assert(retryNet.ws === sockets[1], "Net retains only the successful retry socket");
  } finally {
    globalThis.WebSocket = NativeWebSocket;
  }
}

{
  const net = new Net("ws://example.test/ws");
  assert(net instanceof Net, "Net constructor should return an instance");
  assertHasMethod(net, "connect", "Net");
  assertHasMethod(net, "disconnect", "Net");
  assertHasMethod(net, "on", "Net");
  assertHasMethod(net, "off", "Net");
  assertHasMethod(net, "join", "Net");
  assertHasMethod(net, "setName", "Net");
  assertHasMethod(net, "ready", "Net");
  assertHasMethod(net, "start", "Net");
  assertHasMethod(net, "giveUp", "Net");
  assertHasMethod(net, "pauseGame", "Net");
  assertHasMethod(net, "unpauseGame", "Net");
  assertHasMethod(net, "returnToLobby", "Net");
  assertHasMethod(net, "command", "Net");
  assertHasMethod(net, "ping", "Net");
  assertHasMethod(net, "netReport", "Net");
  assertHasMethod(net, "activity", "Net");
  assertHasGetter(net, "playerId", "Net");
  assert(net.playerId === null, "Net.playerId should be null before welcome");
  assertHasMethod(net, "addAi", "Net");
  assertHasMethod(net, "removeAi", "Net");
  assertHasMethod(net, "setTeamPreset", "Net");
  assertHasMethod(net, "setTeam", "Net");
  assertHasMethod(net, "setFaction", "Net");
  assertHasMethod(net, "setRoomTimeSpeed", "Net");
  assertHasMethod(net, "stepRoomTime", "Net");
  assertHasMethod(net, "seekRoomTime", "Net");
  assertHasMethod(net, "seekRoomTimeTo", "Net");
  assertHasMethod(net, "setVisionSelection", "Net");
  assertHasMethod(net, "lab", "Net");
  assertHasMethod(net, "requestBranchFromTick", "Net");
  assertHasMethod(net, "claimBranchSeat", "Net");
  assertHasMethod(net, "releaseBranchSeat", "Net");
  assertHasMethod(net, "startBranch", "Net");
  const sent = [];
  net.ws = {
    readyState: WebSocket.OPEN,
    bufferedAmount: 0,
    send(json) {
      sent.push(JSON.parse(json));
    },
  };
  assertThrows(() => net.command(cmd.stop([1])), "Net.command requires controller-provided clientSeq");
  net.command(cmd.stop([1]), 7);
  assert(sent[0].clientSeq === 7, "Net.command sends the provided clientSeq");
  net.pauseGame();
  net.unpauseGame();
  assert(sent[1].t === "pauseGame" && sent[2].t === "unpauseGame", "Net live pause helpers send exact tags");
  net.lab(12, { op: "setVision", vision: msg.labVisionAll() });
  assert(sent[3].t === "lab" && sent[3].requestId === 12, "Net.lab sends lab request envelopes");
  assert(net.setRoomTimeSpeed(2) === true, "Net reports a successful room-time speed send");
  assert(net.stepRoomTime() === true, "Net reports a successful room-time step send");
  net.ws.readyState = WebSocket.CLOSED;
  assert(net.setRoomTimeSpeed(2) === false, "Net reports a blocked room-time speed send");
  assert(net.stepRoomTime() === false, "Net reports a blocked room-time step send");
  net.ws.readyState = WebSocket.OPEN;
  const workingSend = net.ws.send;
  net.ws.send = () => { throw new Error("socket closed during send"); };
  assert(net.setRoomTimeSpeed(2) === false, "Net reports a synchronous WebSocket send failure");
  net.ws.send = workingSend;
  assert(
    msg.labExportScenario(13, "saved").op.name === "saved",
    "lab setup export builder includes a name",
  );
  assert(
    msg.labImportScenario(14, { schemaVersion: 1 }).op.scenario.schemaVersion === 1,
    "lab setup import builder includes a compatibility payload",
  );
  assert(!("replayOk" in msg.join("A", "main")), "join builder omits replayOk by default");
  assert(
    msg.join("A", "main", false, true).replayOk === true,
    "join builder can confirm replay joins",
  );
  const priorPerformance = globalThis.performance;
  let nowSamples = [0, 2, 2, 5, 10, 13, 13, 17];
  globalThis.performance = { now: () => nowSamples.shift() ?? 17 };
  try {
    const reportNet = new Net("ws://example.invalid");
    reportNet.ws = { extensions: "permessage-deflate; client_max_window_bits" };
    reportNet._onMessage({
      data: messagePackSnapshotFrame({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [1, 0, 0, 0, 0],
        e: [],
        n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 0, null],
      }),
    });
    reportNet._onMessage({
      data: messagePackSnapshotFrame({
        t: "snapshot",
        v: COMPACT_SNAPSHOT_VERSION,
        s: [2, 0, 0, 0, 0],
        e: [],
        n: [0, 0, 0, 0, 0, PREDICTION_PROTOCOL_VERSION, 0, null],
      }),
    });
    const stats = reportNet.consumeSnapshotReportStats();
    assert(stats.snapshotMessageCount === 2, "Net reports snapshot message count");
    assert(stats.snapshotBytesTotal > stats.snapshotBytesMax, "Net reports bounded snapshot byte totals");
    assert(
      stats.snapshotByteSource === "messagepack-application-payload",
      "Net labels MessagePack payload byte measurement source",
    );
    assert(stats.snapshotCodec === SNAPSHOT_CODEC.MESSAGEPACK_COMPACT, "Net reports snapshot codec");
    assert(stats.snapshotCodecVersion === SNAPSHOT_CODEC_VERSION, "Net reports snapshot codec version");
    assert(stats.snapshotFrameKind === SNAPSHOT_FRAME_KIND.BINARY, "Net reports binary snapshot frame kind");
    assert(
      stats.websocketExtensions.includes("permessage-deflate"),
      "Net reports browser WebSocket extension string",
    );
    assert(
      stats.websocketCompression === "permessage-deflate",
      "Net reports negotiated permessage-deflate state",
    );
    assert(stats.snapshotSegmentBudgetBytes === SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES, "Net reports snapshot packet budget");
    assert(stats.snapshotBytesP95 >= stats.snapshotBytesAvg, "Net reports snapshot byte p95");
    assert(stats.snapshotOverSegmentBudgetCount === 0, "small snapshots stay within packet budget");
    assert(stats.snapshotParseMaxMs === 3, "Net reports snapshot frame parse max");
    assert(stats.snapshotDecodeMaxMs === 4, "Net reports compact decode max");
    const resetStats = reportNet.consumeSnapshotReportStats();
    assert(resetStats.snapshotMessageCount === 0, "Net snapshot report stats reset");
    assert(resetStats.websocketCompression === "permessage-deflate", "Net keeps compression state after stats reset");
    assert(resetStats.snapshotOverSegmentBudgetCount === 0, "Net snapshot packet-budget stats reset");
    assert(resetStats.snapshotCodec === SNAPSHOT_CODEC.MESSAGEPACK_COMPACT, "Net snapshot codec default resets");
    assert(resetStats.snapshotFrameKind === SNAPSHOT_FRAME_KIND.BINARY, "Net snapshot frame kind default resets");

    reportNet.noteSnapshotFrame({
      bytes: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES + 1,
      parseMs: 0,
      decodeMs: 0,
      snapshotCodec: SNAPSHOT_CODEC.MESSAGEPACK_COMPACT,
      snapshotCodecVersion: SNAPSHOT_CODEC_VERSION,
      frameKind: SNAPSHOT_FRAME_KIND.BINARY,
    });
    const overBudget = reportNet.consumeSnapshotReportStats();
    assert(overBudget.snapshotBytesP95 > SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES, "Net reports over-budget byte p95");
    assert(overBudget.snapshotOverSegmentBudgetCount === 1, "Net counts over-budget snapshot frames");
    assert(overBudget.snapshotOverSegmentBudgetPctX100 === 10000, "Net reports over-budget snapshot percentage");
  } finally {
    globalThis.performance = priorPerformance;
  }
  for (const field of [
    "snapshotBytesTotal",
    "snapshotByteSource",
    "snapshotCodec",
    "snapshotCodecVersion",
    "snapshotFrameKind",
    "snapshotBytesP95",
    "snapshotSegmentBudgetBytes",
    "snapshotOverSegmentBudgetCount",
    "snapshotOverSegmentBudgetPctX100",
    "snapshotParseMaxMs",
    "snapshotDecodeP95Ms",
    "websocketExtensions",
    "websocketCompression",
    "snapshotApplyMaxMs",
    "predictionApplyP95Ms",
    "snapshotTickGapMax",
    "snapshotLateFrameCount",
    "predictedSnapshotLateFrameCount",
    "predictedSnapshotLateFramePctX100",
    "snapshotBurstMax",
    "frameWorkMaxMs",
    "frameWorkP95Ms",
    "frameRafDispatchMaxMs",
    "frameUnattributedP95Ms",
    "slowFrameCount",
    "worstFramePhase",
    "rendererMaxMs",
    "topRendererPhase",
    "clientFramePhases",
    "rendererFramePhases",
    "renderDiagnosticCounters",
    "entityCount",
    "devicePixelRatioX100",
    "commandBurstMax",
    "predictionDisableWasmCount",
    "predictionReplayMaxMs",
    "predictionReplayBudgetExceededCount",
  ]) {
    assert(CLIENT_NET_REPORT_FIELDS.includes(field), `client net-report field list includes ${field}`);
  }
  assert(msg.netReport({ schemaVersion: 1 }).t === "netReport", "net-report builder tag");
  assert(msg.netReport({ schemaVersion: 1 }).report.schemaVersion === 1, "net-report builder payload");
  assert(msg.activity().t === "activity", "human-activity builder tag");
  assert(msg.returnToLobby().t === "returnToLobby", "return-to-lobby builder tag");
  assert(msg.setRoomTimeSpeed(2).t === "setRoomTimeSpeed", "room-time speed builder tag");
  assert(msg.stepRoomTime().t === "stepRoomTime", "room-time step builder tag");
  assert(msg.seekRoomTime(90).ticksBack === 90, "room-time relative seek builder payload");
  assert(msg.seekRoomTimeTo(450).tick === 450, "room-time absolute seek builder payload");
  assert(msg.setTeamPreset("1v2").preset === "1v2", "team preset builder payload");
  assert(msg.setName("Renamed").name === "Renamed", "lobby name builder payload");
  assert(msg.setTeam(7, 2).teamId === 2, "team assignment builder payload");
  assert(msg.setFaction("ekat").factionId === "ekat", "faction selection builder payload");
  assert(DEFAULT_AI_PROFILE_ID === "ai_2_1", "lobby defaults to AI 2.1");
  assert(
    AI_PROFILES.length === 2 &&
      AI_PROFILES[0].id === "ai_2_1" &&
      AI_PROFILES[0].label === "AI 2.1" &&
      AI_PROFILES[1].id === "ai_turtle" &&
      AI_PROFILES[1].label === "AI Turtle",
    "lobby exposes the two supported AI profiles",
  );
  assert(msg.addAi(2).teamId === 2, "addAi builder can include teamId");
  assert(
    msg.addAi(2, DEFAULT_AI_PROFILE_ID).aiProfileId === DEFAULT_AI_PROFILE_ID,
    "addAi builder can include default aiProfileId",
  );
  assert(
    msg.addAi(2, "ai_2_1").aiProfileId === "ai_2_1",
    "addAi builder can request AI 2.1 explicitly",
  );
  assert(msg.requestBranchFromTick().t === "requestBranchFromTick", "replay branch builder tag");
  assert(msg.claimBranchSeat(7).t === "claimBranchSeat", "branch seat claim builder tag");
  assert(msg.releaseBranchSeat(7).t === "releaseBranchSeat", "branch seat release builder tag");
  assert(msg.startBranch().t === "startBranch", "branch start builder tag");
  assert(msg.visionSelectionAll().t === "setVisionSelection", "replay all-vision builder tag");
  assert(msg.visionSelectionAll().selection.mode === "all", "replay all-vision builder payload");
  assert(
    msg.visionSelectionPlayer(7).selection.playerId === 7,
    "replay single-player vision builder payload",
  );
  assert(
    msg.visionSelectionPlayers([1, 2]).selection.playerIds.join(",") === "1,2",
    "replay subset vision builder payload",
  );
}

{
  const net = new Net("ws://example.test/ws");
  let closes = 0;
  net._connected = true;
  net.on("close", () => { closes += 1; });
  net._playerId = 7;
  let socketCloses = 0;
  net.ws = { close() { socketCloses += 1; } };
  net.disconnect();
  assert(socketCloses === 1, "Net.disconnect closes the current socket");
  assert(closes === 1, "Net.disconnect emits one immediate close lifecycle event");
  assert(net.ws === null && net.playerId === null, "Net.disconnect clears pre-join connection state");
}

{
  const NativeWebSocket = globalThis.WebSocket;
  const sockets = [];
  class LifecycleWebSocket {
    static OPEN = 1;

    constructor() {
      this.listeners = new Map();
      this.readyState = 0;
      sockets.push(this);
    }

    addEventListener(type, handler) {
      this.listeners.set(type, handler);
    }

    emit(type, event = {}) {
      this.listeners.get(type)?.(event);
    }

    close() {
      this.readyState = 3;
    }
  }

  globalThis.WebSocket = LifecycleWebSocket;
  try {
    const net = new Net("ws://example.test/ws");
    let closes = 0;
    let messages = 0;
    net.on("close", () => { closes += 1; });
    net._onMessage = () => { messages += 1; };

    const firstConnect = net.connect({ attempts: 1 });
    sockets[0].readyState = LifecycleWebSocket.OPEN;
    sockets[0].emit("open");
    await firstConnect;
    const staleSocket = sockets[0];

    net.disconnect();
    assert(closes === 1, "intentional disconnect transitions listeners immediately");

    const secondConnect = net.connect({ attempts: 1 });
    sockets[1].readyState = LifecycleWebSocket.OPEN;
    sockets[1].emit("open");
    await secondConnect;
    staleSocket.emit("close");
    staleSocket.emit("message", { data: "stale" });
    sockets[1].emit("message", { data: "current" });

    assert(closes === 1, "a stale socket close does not close the replacement lifecycle");
    assert(net.ws === sockets[1], "a stale socket close preserves the replacement socket");
    assert(messages === 1, "messages from a stale socket are ignored after reconnect");
  } finally {
    globalThis.WebSocket = NativeWebSocket;
  }
}

// ---------------------------------------------------------------------------
