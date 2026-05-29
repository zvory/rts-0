// Net — WebSocket wrapper with a tiny event emitter and typed send helpers.
// See DESIGN.md §4.1. All wire shapes come from protocol.js builders so the
// client and server stay in lockstep; this module owns no game logic.

import { S, msg, cmd as cmdBuilders } from "./protocol.js";

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
  constructor(url) {
    this.url = url;
    /** @type {WebSocket|null} */
    this.ws = null;
    /** @type {Map<string, Set<Function>>} type -> handlers */
    this._handlers = new Map();
    /** @type {number|null} our assigned player id, set on `welcome`. */
    this._playerId = null;
    /** Most recently measured round-trip latency in ms (null until first pong). */
    this.latency = null;
    /** performance.now() stamp of the last ping(), used to compute latency. */
    this._lastPingSent = null;
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
  connect() {
    return new Promise((resolve, reject) => {
      let settled = false;
      const ws = new WebSocket(this.url);
      this.ws = ws;

      ws.addEventListener("open", () => {
        settled = true;
        this._emit("open");
        resolve();
      });

      ws.addEventListener("error", (ev) => {
        // An error before open means the connection never came up: reject the
        // connect() promise. Errors after open are surfaced via "close".
        if (!settled) {
          settled = true;
          reject(new Error("WebSocket connection failed"));
        }
      });

      ws.addEventListener("close", () => {
        this._emit("close");
      });

      ws.addEventListener("message", (ev) => this._onMessage(ev));
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
   */
  join(name, room) {
    this._send(msg.join(name, room));
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

  /** Add a computer opponent to the room (host-only; ignored by the server otherwise). */
  addAi() {
    this._send(msg.addAi());
  }

  /**
   * Remove a previously-added AI opponent by its player id (host-only).
   * @param {number} id the AI player's id from the lobby list.
   */
  removeAi(id) {
    this._send(msg.removeAi(id));
  }

  /**
   * Issue a gameplay command.
   * @param {object} cmd a command object built via protocol.js `cmd.*`.
   */
  command(cmd) {
    this._send(msg.command(cmd));
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

  // --- internals ----------------------------------------------------------

  /**
   * Serialize and send a message, guarding against sends before the socket is
   * open. Returns true if the message was sent.
   * @param {object} obj
   * @returns {boolean}
   */
  _send(obj) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    this.ws.send(JSON.stringify(obj));
    return true;
  }

  /**
   * Parse an incoming frame, apply built-in side effects (welcome/pong), then
   * dispatch to handlers registered for its tag.
   * @param {MessageEvent} ev
   */
  _onMessage(ev) {
    let m;
    try {
      m = JSON.parse(ev.data);
    } catch (err) {
      // Ignore malformed frames rather than tearing down the connection.
      return;
    }
    if (!m || typeof m.t !== "string") return;

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
        // eslint-disable-next-line no-console
        console.error(`Net handler for "${type}" threw:`, err);
      }
    }
  }
}
