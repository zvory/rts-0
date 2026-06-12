import { decodeServerMessage } from "../../../client/src/protocol.js";
import { summarizeSnapshot, ownEntityByKind } from "../diffs.mjs";

const DEFAULT_WS = process.env.RTS_WS || "ws://127.0.0.1:8081/ws";

export class RemoteLane {
  constructor({ scenario, room, artifacts, url = DEFAULT_WS }) {
    this.scenario = scenario;
    this.room = room;
    this.artifacts = artifacts;
    this.url = url;
    this.ws = null;
    this.playerId = null;
    this.startInfo = null;
    this.lastSnapshot = null;
    this.messages = [];
    this.waiters = [];
    this.nextClientSeq = 1;
    this.selection = [];
  }

  async start() {
    this.ws = new WebSocket(this.url);
    this.ws.onmessage = (event) => this.onMessage(event);
    this.ws.onerror = (event) => {
      this.artifacts.remote({ event: "ws.error", message: event.message || event.type || "error" });
    };
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`remote lane timeout opening ${this.url}`)), 5000);
      this.ws.onopen = () => {
        clearTimeout(timer);
        resolve();
      };
      this.ws.onclose = () => reject(new Error("remote lane closed before open"));
    });
    await this.waitFor((message) => message.t === "welcome", 3000, "welcome");
    this.send({ t: "join", name: "Remote", room: this.room });
    await this.waitFor((message) => message.t === "lobby", 3000, "lobby join");
    if (this.scenario.setup.quickstart) {
      this.send({ t: "setQuickstart", enabled: true });
      await this.waitFor((message) => message.t === "lobby" && message.quickstart === true, 3000, "quickstart");
    }
    this.send({ t: "ready", ready: true });
    await this.waitFor((message) => message.t === "lobby" && message.canStart, 3000, "canStart");
    this.send({ t: "start" });
    this.startInfo = await this.waitFor((message) => message.t === "start", 6000, "start");
    await this.waitFor((message) => message.t === "snapshot" && message.entities?.length > 0, 3000, "initial snapshot");
    this.capture("initial");
  }

  onMessage(event) {
    let message;
    try {
      message = decodeServerMessage(JSON.parse(event.data));
    } catch (err) {
      this.artifacts.remote({ event: "decode.error", message: err.message });
      return;
    }
    this.messages.push(message);
    if (message.t === "welcome") this.playerId = message.playerId;
    if (message.t === "snapshot") {
      this.lastSnapshot = message;
      this.artifacts.remote({
        event: "snapshot",
        tick: message.tick,
        entityCount: message.entities?.length || 0,
        netStatus: message.netStatus || null,
      });
    } else if (message.t !== "pong") {
      this.artifacts.remote({ event: message.t, message: compactMessage(message) });
    }
    this.waiters = this.waiters.filter((waiter) => {
      if (!waiter.test(message)) return true;
      waiter.resolve(message);
      return false;
    });
  }

  send(message) {
    this.ws.send(JSON.stringify(message));
    this.artifacts.remote({ event: "send", message });
  }

  command(command) {
    const clientSeq = this.nextClientSeq++;
    this.send({ t: "command", clientSeq, cmd: command });
    return { clientSeq };
  }

  async selectOwn(kind, index = 0) {
    const entity = ownEntityByKind(this.summary(), kind, index);
    if (!entity) throw new Error(`remote missing owned ${kind}[${index}]`);
    this.selection = [{ kind, index, id: entity.id }];
    return this.selection;
  }

  async issue(command, args = {}) {
    if (command !== "move" && command !== "attackMove") {
      throw new Error(`unsupported remote command: ${command}`);
    }
    const unit = this.selectedEntity();
    const target = {
      x: unit.x + (args.dx ?? 0),
      y: unit.y + (args.dy ?? 0),
    };
    const cmd = {
      c: command,
      units: [unit.id],
      x: target.x,
      y: target.y,
    };
    if (args.queued) cmd.queued = true;
    const sent = this.command(cmd);
    return { command: cmd, ...sent };
  }

  selectedEntity() {
    const selected = this.selection[0];
    if (!selected) throw new Error("remote issue requires selectOwn first");
    const entity = ownEntityByKind(this.summary(), selected.kind, selected.index);
    if (!entity) throw new Error(`remote selected ${selected.kind}[${selected.index}] disappeared`);
    return entity;
  }

  async waitForSnapshot({ minTickDelta = 1, timeoutMs = 5000 } = {}) {
    const startTick = this.lastSnapshot?.tick ?? -1;
    return this.waitFor(
      (message) => message.t === "snapshot" && message.tick >= startTick + minTickDelta,
      timeoutMs,
      `snapshot +${minTickDelta}`,
    );
  }

  capture(label) {
    const summary = this.summary();
    this.artifacts.remote({ event: "capture", label, summary });
    return summary;
  }

  summary() {
    return summarizeSnapshot(this.lastSnapshot, this.playerId);
  }

  waitFor(test, timeoutMs, label) {
    const hit = this.messages.find(test);
    if (hit) return Promise.resolve(hit);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`remote timeout waiting for ${label}`)), timeoutMs);
      this.waiters.push({
        test,
        resolve: (message) => {
          clearTimeout(timer);
          resolve(message);
        },
      });
    });
  }

  async close() {
    if (!this.ws) return;
    this.ws.close();
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

function compactMessage(message) {
  if (!message || typeof message !== "object") return message;
  if (message.t === "lobby") {
    return {
      t: message.t,
      players: message.players?.map((player) => ({
        id: player.id,
        name: player.name,
        spectator: !!player.isSpectator,
      })),
      canStart: message.canStart,
      quickstart: message.quickstart,
      map: message.map,
    };
  }
  if (message.t === "start") {
    return {
      t: message.t,
      playerId: message.playerId,
      spectator: !!message.spectator,
      players: message.players?.length,
      map: message.map ? `${message.map.width}x${message.map.height}` : null,
    };
  }
  return message;
}
