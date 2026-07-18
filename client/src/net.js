// Net — WebSocket wrapper with a tiny event emitter and typed send helpers.
// See docs/design/client-ui.md §4.1. All wire shapes come from protocol.js builders so the
// client and server stay in lockstep; this module owns no game logic.

import {
  S,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  decodeServerMessage,
  parseServerFrame,
  msg,
  cmd as cmdBuilders,
} from "./protocol.js";
import { ReportWindowAggregate } from "./report_window_aggregate.js";

export const SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES = 1280;
export const INITIAL_CONNECT_ATTEMPTS = 8;
export const INITIAL_CONNECT_RETRY_MS = 1000;
export const INITIAL_CONNECT_TIMEOUT_MS = 3000;
const SNAPSHOT_BYTE_SOURCE = "messagepack-application-payload";
const WEBSOCKET_COMPRESSION_NONE = "none";
const WEBSOCKET_COMPRESSION_PERMESSAGE_DEFLATE = "permessage-deflate";
const SUBSCRIBER_FAILURE_SIGNATURE_LIMIT = 32;
const SUBSCRIBER_EVENT_TYPE_LIMIT = 64;

const SNAPSHOT_BYTE_BUCKETS = Object.freeze([
  512,
  768,
  1024,
  SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES,
  1536,
  2048,
  3072,
  4096,
  6144,
  8192,
  12288,
  16384,
  24576,
  32768,
  49152,
  65536,
  98304,
  131072,
  196608,
  262144,
  393216,
  524288,
  786432,
  1048576,
]);

/**
 * Thin client transport over a single WebSocket connection.
 *
 * Incoming messages are dispatched by their `t` field (the S.* tags). Two
 * synthetic events, "open" and "close", mirror the underlying socket
 * lifecycle. The `welcome` message is intercepted to record our assigned
 * player id, and `pong` is used to compute round-trip latency.
 */
export class Net {
  /**
   * @param {string} url WebSocket url (derived from window.location in main.js).
   */
  constructor(url, diagnostics = null) {
    this.url = url;
    this.diagnostics = diagnostics;
    /** @type {WebSocket|null} */
    this.ws = null;
    /** Whether the current socket has emitted its open lifecycle event. */
    this._connected = false;
    /** @type {Map<string, Set<Function>>} type -> handlers */
    this._handlers = new Map();
    /** Weak identity keeps unsubscribed handlers collectible. */
    this._subscriberHandlerIds = new WeakMap();
    this._nextSubscriberHandlerId = 1;
    /** Bounded, payload-independent signatures already reported by this instance. */
    this._reportedSubscriberFailureSignatures = new Set();
    this._subscriberFailureReportingSaturated = false;
    /** @type {number|null} our assigned player id, set on `welcome`. */
    this._playerId = null;
    /** Most recently measured round-trip latency in ms (null until first pong). */
    this.latency = null;
    /** performance.now() stamp of the latest pong-derived latency sample. */
    this.latencyUpdatedAt = 0;
    /** performance.now() stamp of the last ping(), used to compute latency. */
    this._lastPingSent = null;
    this.snapshotReportStats = this.createSnapshotReportStats();
  }

  /** Our server-assigned player id, or null before the welcome message. */
  get playerId() {
    return this._playerId;
  }

  /**
   * Open the WebSocket.
   * @returns {Promise<void>} resolves once the socket is open, rejects on a
   *   connection error that occurs before it opens.
   */
  async connect({
    attempts = INITIAL_CONNECT_ATTEMPTS,
    retryMs = INITIAL_CONNECT_RETRY_MS,
    attemptTimeoutMs = INITIAL_CONNECT_TIMEOUT_MS,
    wait = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)),
  } = {}) {
    const boundedAttempts = Math.max(1, Math.floor(attempts));
    let lastError = new Error("WebSocket connection failed");
    for (let attempt = 1; attempt <= boundedAttempts; attempt += 1) {
      try {
        await this._connectOnce(attemptTimeoutMs);
        return;
      } catch (err) {
        lastError = err;
        if (attempt < boundedAttempts) {
          this.diagnostics?.mark("ws.connect.retry", { attempt, attempts: boundedAttempts });
          await wait(retryMs);
        }
      }
    }
    throw lastError;
  }

  /** Close the current socket without scheduling a reconnect. */
  disconnect() {
    const ws = this.ws;
    if (!ws) return;
    this.ws = null;
    this._playerId = null;
    const wasConnected = this._connected;
    this._connected = false;
    try {
      ws.close();
    } catch {
      // The socket is already unusable; clearing our reference is sufficient.
    }
    // Surface the intentional transition immediately. The socket's eventual
    // close event is stale once `this.ws` is cleared and must not be allowed to
    // tear down a replacement connection opened in the meantime.
    if (wasConnected) this._emit("close");
  }

  _connectOnce(timeoutMs) {
    return new Promise((resolve, reject) => {
      let settled = false;
      let opened = false;
      this.diagnostics?.mark("ws.connect.start", { url: this.url });
      const ws = new WebSocket(this.url);
      ws.binaryType = "arraybuffer";
      this.ws = ws;
      const timeoutId = setTimeout(() => {
        if (settled) return;
        this.diagnostics?.mark("ws.connect.timeout", { timeoutMs });
        try {
          ws.close();
        } catch {
          settled = true;
          if (this.ws === ws) this.ws = null;
          reject(new Error("WebSocket connection timed out"));
        }
      }, Math.max(1, timeoutMs));

      ws.addEventListener("open", () => {
        if (this.ws !== ws) return;
        clearTimeout(timeoutId);
        opened = true;
        settled = true;
        this._connected = true;
        this.diagnostics?.mark("ws.open");
        this._emit("open");
        resolve();
      });

      ws.addEventListener("error", (ev) => {
        // An error before open means the connection never came up: reject the
        // connect() promise. Errors after open are surfaced via "close".
        if (!settled) {
          this.diagnostics?.mark("ws.error.before_open");
          try {
            ws.close();
          } catch {
            clearTimeout(timeoutId);
            settled = true;
            if (this.ws === ws) this.ws = null;
            reject(new Error("WebSocket connection failed"));
          }
        }
      });

      ws.addEventListener("close", () => {
        clearTimeout(timeoutId);
        const wasCurrent = this.ws === ws;
        if (wasCurrent) {
          this.ws = null;
          this._connected = false;
          this._playerId = null;
        }
        this.diagnostics?.mark("ws.close");
        if (opened && wasCurrent) this._emit("close");
        if (!settled) {
          settled = true;
          reject(new Error("WebSocket connection failed"));
        }
      });

      ws.addEventListener("message", (ev) => {
        if (this.ws === ws) this._onMessage(ev);
      });
    });
  }

  /**
   * Subscribe to a message type. `type` is a ServerMessage tag (S.*) or one of
   * the synthetic lifecycle events "open" / "close".
   * @param {string} type
   * @param {Function} handler invoked with the parsed message (or no args for
   *   the lifecycle events).
   */
  on(type, handler) {
    let set = this._handlers.get(type);
    if (!set) {
      set = new Set();
      this._handlers.set(type, set);
    }
    if (!this._subscriberHandlerIds.has(handler)) {
      this._subscriberHandlerIds.set(handler, this._nextSubscriberHandlerId);
      this._nextSubscriberHandlerId += 1;
    }
    set.add(handler);
  }

  /**
   * Unsubscribe a previously registered handler.
   * @param {string} type
   * @param {Function} handler
   */
  off(type, handler) {
    const set = this._handlers.get(type);
    if (set) set.delete(handler);
  }

  // --- typed send helpers (mirror protocol.js builders) -------------------

  /**
   * Join (or create) a room.
   * @param {string} name display name
   * @param {string} [room] room id; defaults to "main" via the builder.
   * @param {boolean} [spectator=false] join as an observer instead of a player.
   * @param {boolean} [replayOk=false] confirm joining replay playback if the room is in replay.
   */
  join(name, room, spectator = false, replayOk = false) {
    this._send(msg.join(name, room, spectator, replayOk));
  }

  /** Update our display name while waiting in the lobby. */
  setName(name) {
    this._send(msg.setName(name));
  }

  /**
   * Toggle our ready state in the lobby.
   * @param {boolean} isReady
   */
  ready(isReady) {
    this._send(msg.ready(isReady));
  }

  /** Ask the server to start the match (only honored from the room host). */
  start() {
    this._send(msg.start());
  }

  /** Deprecated compatibility command; the current server ignores team presets. */
  setTeamPreset(preset) {
    this._send(msg.setTeamPreset(preset));
  }

  /** Assign one lobby seat to a nonzero team id (host-only; ignored by the server otherwise). */
  setTeam(id, teamId) {
    this._send(msg.setTeam(id, teamId));
  }

  /** Select this player's own lobby faction. */
  setFaction(factionId) {
    this._send(msg.setFaction(factionId));
  }

  /** Add a computer opponent to the room (host-only; ignored by the server otherwise). */
  addAi(teamId = undefined, aiProfileId = undefined) {
    this._send(msg.addAi(teamId, aiProfileId));
  }

  /** Select the live AI profile for an existing AI opponent (host-only). */
  setAiProfile(id, aiProfileId) {
    this._send(msg.setAiProfile(id, aiProfileId));
  }

  /**
   * Remove a previously-added AI opponent by its player id (host-only).
   * @param {number} id the AI player's id from the lobby list.
   */
  removeAi(id) {
    this._send(msg.removeAi(id));
  }

  /**
   * Switch a player between player and spectator role while still in the lobby.
   * @param {boolean} spectator
   * @param {number|undefined} [id] optional host-targeted player id; omitted means self.
   */
  setSpectator(spectator, id = undefined) {
    this._send(msg.setSpectator(spectator, id));
  }

  /**
   * Issue a gameplay command.
   * @param {object} cmd a command object built via protocol.js `cmd.*`.
   */
  command(cmd, clientSeq) {
    if (!Number.isInteger(clientSeq) || clientSeq <= 0 || clientSeq > 0xffffffff) {
      throw new Error("Net.command requires a positive u32 clientSeq");
    }
    return this._send(msg.command(cmd, clientSeq));
  }

  /** Give up the active match and request the score screen. */
  giveUp() {
    this._send(msg.giveUp());
  }

  /** Request a server-authoritative live match pause. */
  pauseGame() {
    this._send(msg.pauseGame());
  }

  /** Request live match resume. */
  unpauseGame() {
    this._send(msg.unpauseGame());
  }

  /** Leave replay playback and ask the room to return to lobby. */
  returnToLobby() {
    this._send(msg.returnToLobby());
  }

  /**
   * Send a latency probe stamped with the current high-resolution time. The
   * matching pong is correlated by its echoed `ts` to compute `latency`.
   */
  ping() {
    const ts = performance.now();
    this._lastPingSent = ts;
    this._send(msg.ping(ts));
  }

  /**
   * Report client-observed network/render health to the server logs.
   * @param {object} report bounded aggregate metrics
   */
  netReport(report) {
    this._send(msg.netReport(report));
  }

  /** Report recent human input separately from automatic connection traffic. */
  activity() {
    return this._send(msg.activity());
  }

  createSnapshotReportStats() {
    return {
      bytesTotal: 0,
      bytesMax: 0,
      messageCount: 0,
      overSegmentBudgetCount: 0,
      byteSizes: new ReportWindowAggregate({
        buckets: SNAPSHOT_BYTE_BUCKETS,
        maxValue: SNAPSHOT_BYTE_BUCKETS.at(-1),
      }),
      parseMs: new ReportWindowAggregate(),
      decodeMs: new ReportWindowAggregate(),
      snapshotCodec: SNAPSHOT_CODEC.MESSAGEPACK_COMPACT,
      snapshotCodecVersion: SNAPSHOT_CODEC_VERSION,
      snapshotFrameKind: SNAPSHOT_FRAME_KIND.BINARY,
    };
  }

  consumeSnapshotReportStats() {
    const stats = this.snapshotReportStats;
    const bytes = stats.byteSizes.summary();
    const parse = stats.parseMs.summary();
    const decode = stats.decodeMs.summary();
    const extensions = websocketExtensions(this.ws);
    const out = {
      snapshotBytesTotal: stats.bytesTotal,
      snapshotBytesMax: stats.bytesMax,
      snapshotBytesAvg: stats.messageCount > 0 ? Math.round(stats.bytesTotal / stats.messageCount) : 0,
      snapshotMessageCount: stats.messageCount,
      snapshotByteSource: SNAPSHOT_BYTE_SOURCE,
      snapshotCodec: stats.snapshotCodec,
      snapshotCodecVersion: stats.snapshotCodecVersion,
      snapshotFrameKind: stats.snapshotFrameKind,
      snapshotBytesP95: bytes.p95,
      snapshotSegmentBudgetBytes: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES,
      snapshotOverSegmentBudgetCount: stats.overSegmentBudgetCount,
      snapshotOverSegmentBudgetPctX100:
        stats.messageCount > 0 ? Math.round((stats.overSegmentBudgetCount * 10000) / stats.messageCount) : 0,
      snapshotParseMaxMs: parse.max,
      snapshotParseP95Ms: parse.p95,
      snapshotDecodeMaxMs: decode.max,
      snapshotDecodeP95Ms: decode.p95,
      websocketExtensions: extensions,
      websocketCompression: websocketCompressionState(extensions),
    };
    stats.bytesTotal = 0;
    stats.bytesMax = 0;
    stats.messageCount = 0;
    stats.overSegmentBudgetCount = 0;
    stats.byteSizes.reset();
    stats.parseMs.reset();
    stats.decodeMs.reset();
    stats.snapshotCodec = SNAPSHOT_CODEC.MESSAGEPACK_COMPACT;
    stats.snapshotCodecVersion = SNAPSHOT_CODEC_VERSION;
    stats.snapshotFrameKind = SNAPSHOT_FRAME_KIND.BINARY;
    return out;
  }

  /** Bytes queued by the browser for this WebSocket, if available. */
  get bufferedAmount() {
    return this.ws?.bufferedAmount || 0;
  }

  /**
   * Set room-controlled time speed. 0 pauses rooms whose clock allows pause.
   * @returns {boolean} true when the WebSocket accepted the outgoing frame
   */
  setRoomTimeSpeed(speed) {
    return this._send(msg.setRoomTimeSpeed(speed));
  }

  /**
   * Advance room-controlled time by one authoritative simulation tick where allowed.
   * @returns {boolean} true when the WebSocket accepted the outgoing frame
   */
  stepRoomTime() {
    return this._send(msg.stepRoomTime());
  }

  /**
   * Rewind room-controlled time by `ticksBack` ticks. Pass a large value
   * (e.g. 2**31 - 1) to reset to the start.
   * @param {number} ticksBack
   * @returns {boolean} true when the WebSocket accepted the outgoing frame
   */
  seekRoomTime(ticksBack) {
    return this._send(msg.seekRoomTime(ticksBack));
  }

  /**
   * Seek room-controlled time to an absolute simulation tick where allowed.
   * @returns {boolean} true when the WebSocket accepted the outgoing frame
   */
  seekRoomTimeTo(tick) {
    return this._send(msg.seekRoomTimeTo(tick));
  }

  /**
   * Select replay fog perspective for this viewer only.
   * @param {object} selection vision selection payload from protocol.js builders/constants
   */
  setVisionSelection(selection) {
    this._send(msg.setVisionSelection(selection));
  }

  /**
   * Send a privileged lab operation envelope. The server replies with labResult.
   * @param {number} requestId positive request id allocated by LabClient
   * @param {object} op lab operation payload
   * @returns {boolean} true when the frame was sent
   */
  lab(requestId, op) {
    return this._send(msg.lab(requestId, op));
  }

  /** Request a practice branch room from the current authoritative replay tick. */
  requestBranchFromTick() {
    this._send(msg.requestBranchFromTick());
  }

  /** Claim an original replay player seat in branch staging. */
  claimBranchSeat(playerId) {
    this._send(msg.claimBranchSeat(playerId));
  }

  /** Release an original replay player seat in branch staging. */
  releaseBranchSeat(playerId) {
    this._send(msg.releaseBranchSeat(playerId));
  }

  /** Ask the server to start the staged replay branch. */
  startBranch() {
    this._send(msg.startBranch());
  }

  /**
   * Host selects a map by name (lobby phase only).
   * @param {string} map map display name
   */
  selectMap(map) {
    this._send(msg.selectMap(map));
  }

  // --- internals ----------------------------------------------------------

  /**
   * Serialize and send a message, guarding against sends before the socket is
   * open. Returns true if the message was sent.
   * @param {object} obj
   * @returns {boolean}
   */
  _send(obj) {
    const json = JSON.stringify(obj);
    const label = `client.send.${obj?.t || "unknown"}`;
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      this.diagnostics?.mark(`${label}.blocked`, {
        readyState: this.ws?.readyState ?? null,
        bytes: json.length,
      });
      return false;
    }
    if (obj?.t === "ping") this.diagnostics?.count(label, { bytes: json.length });
    else this.diagnostics?.mark(label, { bytes: json.length });
    try {
      this.ws.send(json);
      return true;
    } catch (error) {
      this.diagnostics?.mark(`${label}.blocked`, {
        readyState: this.ws?.readyState ?? null,
        bytes: json.length,
        reason: "send_failed",
        errorName: error?.name || "Error",
      });
      return false;
    }
  }

  /**
   * Parse an incoming frame, apply built-in side effects (welcome/pong), then
   * dispatch to handlers registered for its tag.
   * @param {MessageEvent} ev
   */
  _onMessage(ev) {
    const rawBytes = frameByteLength(ev.data);
    let m;
    let parseMs = 0;
    let decodeMs = 0;
    try {
      const parseStartedAt = performance.now();
      const raw = parseServerFrame(ev.data);
      parseMs = performance.now() - parseStartedAt;
      const decodeStartedAt = performance.now();
      m = decodeServerMessage(raw);
      decodeMs = performance.now() - decodeStartedAt;
    } catch (err) {
      // Ignore malformed frames rather than tearing down the connection.
      this.diagnostics?.mark("server.recv.malformed", { bytes: rawBytes });
      return;
    }
    if (!m || typeof m.t !== "string") return;
    const detail = { bytes: rawBytes };
    const label = `server.recv.${m.t}`;
    if (m.t === S.SNAPSHOT || m.t === S.PONG) this.diagnostics?.count(label, detail);
    else this.diagnostics?.mark(label, detail);
    if (m.t === S.SNAPSHOT) {
      const frameKind = snapshotFrameKindForData(ev.data);
      this.noteSnapshotFrame({
        bytes: rawBytes,
        parseMs,
        decodeMs,
        snapshotCodec:
          frameKind === SNAPSHOT_FRAME_KIND.BINARY
            ? SNAPSHOT_CODEC.MESSAGEPACK_COMPACT
            : SNAPSHOT_CODEC.COMPACT_JSON,
        snapshotCodecVersion: SNAPSHOT_CODEC_VERSION,
        frameKind,
      });
    }

    switch (m.t) {
      case S.WELCOME:
        if (typeof m.playerId === "number") this._playerId = m.playerId;
        break;
      case S.PONG:
        // Prefer the echoed ts so concurrent pings stay correctly paired.
        if (typeof m.ts === "number") {
          this.latency = performance.now() - m.ts;
        } else if (this._lastPingSent != null) {
          this.latency = performance.now() - this._lastPingSent;
        }
        this.latencyUpdatedAt = performance.now();
        break;
      default:
        break;
    }

    this._emit(m.t, m);
  }

  /**
   * Invoke all handlers for a type. Handler exceptions are isolated so one bad
   * subscriber cannot break dispatch for the rest.
   * @param {string} type
   * @param {*} [payload]
   */
  _emit(type, payload) {
    const set = this._handlers.get(type);
    if (!set) return;
    for (const handler of set) {
      try {
        handler(payload);
      } catch (err) {
        this._reportSubscriberFailure(type, handler, err);
      }
    }
  }

  _reportSubscriberFailure(type, handler, thrown) {
    try {
      const eventType = typeof type === "string"
        ? type.slice(0, SUBSCRIBER_EVENT_TYPE_LIMIT)
        : "unknown";
      const thrownValueClass = thrown instanceof Error ? "Error" : "NonError";
      const handlerId = this._subscriberHandlerIds.get(handler);
      const signature = JSON.stringify([eventType, thrownValueClass, handlerId]);
      if (this._reportedSubscriberFailureSignatures.has(signature)) return;

      if (this._reportedSubscriberFailureSignatures.size >= SUBSCRIBER_FAILURE_SIGNATURE_LIMIT) {
        if (this._subscriberFailureReportingSaturated) return;
        this._subscriberFailureReportingSaturated = true;
        try {
          console.error("[rts-net] subscriber failure reporting saturated");
        } catch {
          // Reporting must never interrupt subscriber delivery.
        }
        try {
          this.diagnostics?.mark("net.subscriberFailure.reportingSaturated");
        } catch {
          // Diagnostics are optional and isolated from console reporting.
        }
        return;
      }

      this._reportedSubscriberFailureSignatures.add(signature);
      const detail = Object.freeze({ eventType, thrownValueClass, handlerId });
      try {
        console.error("[rts-net] subscriber failure", detail);
      } catch {
        // Reporting must never interrupt subscriber delivery.
      }
      try {
        this.diagnostics?.mark("net.subscriberFailure", detail);
      } catch {
        // Diagnostics are optional and isolated from console reporting.
      }
    } catch {
      // Classification and bookkeeping are also untrusted failure boundaries.
    }
  }

  noteSnapshotFrame({ bytes, parseMs, decodeMs, snapshotCodec, snapshotCodecVersion, frameKind }) {
    const stats = this.snapshotReportStats;
    if (snapshotCodec) stats.snapshotCodec = snapshotCodec;
    if (Number.isInteger(snapshotCodecVersion) && snapshotCodecVersion > 0) {
      stats.snapshotCodecVersion = snapshotCodecVersion;
    }
    if (frameKind) stats.snapshotFrameKind = frameKind;
    const byteCount = Number(bytes);
    if (Number.isFinite(byteCount) && byteCount > 0) {
      stats.bytesTotal += byteCount;
      stats.bytesMax = Math.max(stats.bytesMax, byteCount);
      stats.byteSizes.add(byteCount);
      if (byteCount > SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES) {
        stats.overSegmentBudgetCount += 1;
      }
    }
    stats.messageCount += 1;
    stats.parseMs.add(parseMs);
    stats.decodeMs.add(decodeMs);
  }
}

function frameByteLength(data) {
  if (typeof data === "string") return data.length;
  if (data instanceof ArrayBuffer) return data.byteLength;
  if (ArrayBuffer.isView(data)) return data.byteLength;
  return undefined;
}

function snapshotFrameKindForData(data) {
  return typeof data === "string" ? SNAPSHOT_FRAME_KIND.TEXT : SNAPSHOT_FRAME_KIND.BINARY;
}

function websocketExtensions(ws) {
  return typeof ws?.extensions === "string" ? ws.extensions : "";
}

function websocketCompressionState(extensions) {
  return String(extensions || "")
    .toLowerCase()
    .split(",")
    .map((part) => part.trim().split(";")[0]?.trim())
    .includes(WEBSOCKET_COMPRESSION_PERMESSAGE_DEFLATE)
    ? WEBSOCKET_COMPRESSION_PERMESSAGE_DEFLATE
    : WEBSOCKET_COMPRESSION_NONE;
}
