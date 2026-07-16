import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import puppeteer from "puppeteer-core";
import { summarizeSnapshot, ownEntityByKind } from "../diffs.mjs";

const DEFAULT_URL = process.env.RTS_URL || "http://127.0.0.1:8081/";
const DEFAULT_CHROME = process.env.CHROME ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

export class ClientLane {
  constructor({ scenario, room, artifacts, url = DEFAULT_URL, chrome = DEFAULT_CHROME }) {
    this.scenario = scenario;
    this.room = room;
    this.artifacts = artifacts;
    this.url = url;
    this.chrome = chrome;
    this.browser = null;
    this.page = null;
    this.profileDir = null;
    this.selection = [];
    this.issuedCommands = [];
    this.networkProfile = normalizeNetworkProfile(scenario.network);
  }

  async start() {
    this.profileDir = fs.mkdtempSync(path.join(os.tmpdir(), "rts-tri-state-chrome-"));
    this.browser = await puppeteer.launch({
      executablePath: this.chrome,
      headless: "new",
      args: ["--no-sandbox", "--window-size=1280,800", `--user-data-dir=${this.profileDir}`],
      defaultViewport: { width: 1280, height: 800 },
    });
    this.page = await this.browser.newPage();
    this.page.on("console", (message) => {
      if (message.type() === "error") {
        this.artifacts.client({ event: "console.error", text: message.text() });
      }
    });
    this.page.on("pageerror", (error) => {
      this.artifacts.client({ event: "page.error", message: error.message });
    });

    if (this.scenario.setup.kind === "devScenario") {
      await this.startDevScenario();
    } else {
      await this.startLiveRoom();
    }
  }

  async startLiveRoom() {
    const url = new URL(this.url);
    url.searchParams.set("rtsNoAutoPointerLock", "1");
    await this.page.evaluateOnNewDocument((prediction, networkProfile) => {
      window.localStorage.setItem("rts.prediction.enabled", prediction === "enabled" ? "1" : "0");
      installTriStateNetworkProfile(networkProfile);

      function installTriStateNetworkProfile(profile) {
        if (!profile || profile.mode === "direct") return;
        const NativeWebSocket = window.WebSocket;
        const events = [];
        const profileState = {
          rng: seededRng(profile.seed || 1),
          snapshotSeen: 0,
          snapshotDropped: 0,
          snapshotCoalesced: 0,
          pendingCoalesced: null,
          burstQueue: [],
          disconnectScheduled: false,
        };
        window.__rtsTriStateNetwork = { profile, events };

        class ProfiledWebSocket extends NativeWebSocket {
          constructor(...args) {
            super(...args);
            this.__rtsOnMessage = null;
            this.__rtsFlushTimer = null;
          }

          set onmessage(handler) {
            this.__rtsOnMessage = typeof handler === "function" ? handler : null;
            super.onmessage = (event) => deliverInbound(this, event, null);
          }

          get onmessage() {
            return this.__rtsOnMessage;
          }

          addEventListener(type, listener, options) {
            if (type === "message") {
              return super.addEventListener(type, (event) => deliverInbound(this, event, listener), options);
            }
            return super.addEventListener(type, listener, options);
          }

          send(data) {
            const delayMs = outboundDelayMs(data, profile, profileState);
            if (delayMs > 0) {
              events.push({ event: "command.delay", delayMs });
              window.setTimeout(() => super.send(data), delayMs);
              return;
            }
            return super.send(data);
          }
        }

        Object.defineProperty(ProfiledWebSocket, "OPEN", { value: NativeWebSocket.OPEN });
        Object.defineProperty(ProfiledWebSocket, "CONNECTING", { value: NativeWebSocket.CONNECTING });
        Object.defineProperty(ProfiledWebSocket, "CLOSING", { value: NativeWebSocket.CLOSING });
        Object.defineProperty(ProfiledWebSocket, "CLOSED", { value: NativeWebSocket.CLOSED });
        window.WebSocket = ProfiledWebSocket;

        function deliverInbound(socket, event, listener = null) {
          const parsed = parseMessage(event.data);
          if (parsed?.t !== "snapshot") {
            if (listener) listener.call(socket, event);
            if (socket.__rtsOnMessage) socket.__rtsOnMessage.call(socket, event);
            return;
          }
          const delivery = snapshotDelivery(profile, profileState);
          events.push({
            event: "snapshot.profile",
            tick: parsed.tick,
            action: delivery.action,
            delayMs: delivery.delayMs,
          });
          if (delivery.action === "drop") return;
          if (delivery.action === "coalesce") {
            profileState.pendingCoalesced = { socket, event, listener };
            if (!socket.__rtsFlushTimer) {
              socket.__rtsFlushTimer = window.setTimeout(() => {
                const pending = profileState.pendingCoalesced;
                profileState.pendingCoalesced = null;
                socket.__rtsFlushTimer = null;
                if (pending) dispatch(pending.socket, pending.event, pending.listener);
              }, delivery.delayMs);
            }
            return;
          }
          if (delivery.action === "burst") {
            profileState.burstQueue.push({ socket, event, listener });
            if (profileState.burstQueue.length < delivery.burstSize) return;
            const burst = profileState.burstQueue.splice(0);
            window.setTimeout(() => {
              for (const pending of burst) dispatch(pending.socket, pending.event, pending.listener);
            }, delivery.delayMs);
            return;
          }
          window.setTimeout(() => dispatch(socket, event, listener), delivery.delayMs);
        }

        function dispatch(socket, event, listener) {
          if (listener) listener.call(socket, event);
          if (socket.__rtsOnMessage) socket.__rtsOnMessage.call(socket, event);
        }

        function snapshotDelivery(profile, state) {
          state.snapshotSeen += 1;
          if (profile.snapshotDropEvery > 0 && state.snapshotSeen % profile.snapshotDropEvery === 0) {
            state.snapshotDropped += 1;
            return { action: "drop", delayMs: 0 };
          }
          let delayMs = ticksToMs(profile.snapshotLatencyTicks || 0, profile);
          if (profile.snapshotJitterTicks > 0) {
            const span = ticksToMs(profile.snapshotJitterTicks, profile);
            delayMs += Math.round((state.rng() * 2 - 1) * span);
          }
          if (profile.headOfLineEvery > 0 && state.snapshotSeen % profile.headOfLineEvery === 0) {
            delayMs += ticksToMs(profile.headOfLineTicks || 0, profile);
          }
          delayMs = Math.max(0, delayMs);
          if (profile.coalesceSnapshots) {
            state.snapshotCoalesced += 1;
            return { action: "coalesce", delayMs: Math.max(delayMs, ticksToMs(profile.coalesceWindowTicks || 1, profile)) };
          }
          if (profile.snapshotBurstSize > 1) {
            return { action: "burst", delayMs, burstSize: profile.snapshotBurstSize };
          }
          return { action: "delay", delayMs };
        }

        function outboundDelayMs(data, profile, state) {
          const parsed = parseMessage(data);
          if (parsed?.t !== "command") return 0;
          let delayMs = ticksToMs(profile.commandLatencyTicks || 0, profile);
          if (profile.commandJitterTicks > 0) {
            delayMs += Math.round((state.rng() * 2 - 1) * ticksToMs(profile.commandJitterTicks, profile));
          }
          return Math.max(0, delayMs);
        }

        function parseMessage(data) {
          try {
            return JSON.parse(String(data));
          } catch {
            return null;
          }
        }

        function ticksToMs(ticks, profile) {
          return Math.max(0, Number(ticks) || 0) * (Number(profile.tickMs) || 33);
        }

        function seededRng(seed) {
          let state = Number(seed) >>> 0;
          return () => {
            state = (1664525 * state + 1013904223) >>> 0;
            return state / 0x100000000;
          };
        }
      }
    }, this.scenario.setup.prediction, this.networkProfile);
    await this.page.goto(url.href, { waitUntil: "networkidle2", timeout: 15000 });
    // Live-room scenarios intentionally bypass the product lobby controls, so
    // request the now-demand-driven transport before issuing harness commands.
    await this.page.evaluate(() => window.__rts.ensureConnected());
    await this.page.waitForFunction(() => window.__rts?.net?.playerId != null, { timeout: 5000 });
    await this.page.evaluate((room) => {
      const app = window.__rts;
      app.net.join("Client", room);
      app.net.ready(true);
      app.net.start();
    }, this.room);
    await this.waitForMatch();
    await this.page.evaluate((prediction) => {
      window.__rts.setPredictionEnabled(prediction === "enabled");
    }, this.scenario.setup.prediction);
    await this.waitForSnapshot({ minTickDelta: 0 });
    await this.capture("initial");
  }

  async startDevScenario() {
    const config = this.scenario.setup.devScenario || {
      id: "vehicle_small_block_baseline",
      unit: "scout_car",
      count: 5,
    };
    const url = new URL(this.url);
    url.searchParams.set("watchScenario", "1");
    url.searchParams.set("id", config.id);
    url.searchParams.set("unit", config.unit);
    url.searchParams.set("count", String(config.count));
    if (config.blocker) url.searchParams.set("blocker", config.blocker);
    url.searchParams.set("rtsNoAutoPointerLock", "1");
    await this.page.goto(url.href, { waitUntil: "domcontentloaded", timeout: 30000 });
    await this.waitForMatch();
    await this.setRoomTimeSpeed(1);
    await this.waitForSnapshot({ minTickDelta: 0 });
    await this.capture("initial");
  }

  async waitForMatch() {
    await this.page.waitForFunction(() => {
      const app = window.__rts;
      return !!app?.match?.state && document.getElementById("game-screen")?.hidden === false;
    }, { timeout: 8000 });
  }

  async selectOwn(kind, index = 0) {
    const selected = await this.page.evaluate(({ kind, index }) => {
      const state = window.__rts.match.state;
      const owned = state.entitiesInterpolated(1, { includePrediction: false })
        .filter((entity) => entity.owner === state.playerId && entity.kind === kind)
        .sort((a, b) => a.id - b.id);
      const entity = owned[index] || null;
      if (entity) state.setSelection([entity.id]);
      return entity ? { kind, index, id: entity.id } : null;
    }, { kind, index });
    if (!selected) throw new Error(`client missing owned ${kind}[${index}]`);
    this.selection = [selected];
    return this.selection;
  }

  async issue(command, args = {}) {
    if (!["move", "attackMove", "attack", "stop", "train", "setRally", "build", "invalidMove"].includes(command)) {
      throw new Error(`unsupported client command: ${command}`);
    }
    const selected = this.selection[0];
    if (!selected) throw new Error("client issue requires selectOwn first");
    const issued = await this.page.evaluate(({ command, args, selected }) => {
      const match = window.__rts.match;
      const state = match.state;
      const owned = state.entitiesInterpolated(1, { includePrediction: false })
        .filter((entity) => entity.owner === state.playerId && entity.kind === selected.kind)
        .sort((a, b) => a.id - b.id);
      const unit = owned[selected.index] || null;
      if (!unit) return { error: `selected ${selected.kind}[${selected.index}] disappeared` };
      state.setSelection([unit.id]);
      let cmd;
      if (command === "stop") {
        cmd = { c: "stop", units: [unit.id] };
      } else if (command === "train") {
        cmd = { c: "train", building: unit.id, unit: args.unit || "worker" };
      } else if (command === "setRally") {
        cmd = { c: "setRally", building: unit.id, x: args.x ?? unit.x + (args.dx ?? 0), y: args.y ?? unit.y + (args.dy ?? 0), kind: args.kind || "move" };
      } else if (command === "build") {
        cmd = {
          c: "build",
          units: [unit.id],
          building: args.building || "depot",
          tileX: args.tileX ?? 1,
          tileY: args.tileY ?? 1,
        };
      } else if (command === "invalidMove") {
        cmd = { c: "move", units: [999999999], x: args.x ?? unit.x + (args.dx ?? 0), y: args.y ?? unit.y + (args.dy ?? 0) };
      } else if (command === "attack") {
        cmd = { c: "attack", units: [unit.id], target: args.target ?? 999999999, queued: !!args.queued };
      } else {
        cmd = {
          c: command,
          units: [unit.id],
          x: args.x ?? unit.x + (args.dx ?? 0),
          y: args.y ?? unit.y + (args.dy ?? 0),
        };
      }
      if (args.queued) cmd.queued = true;
      const issued = match.commandIssuer.issueCommand(cmd);
      return {
        ...issued,
        command: cmd,
        latestKnownAuthoritativeTick: match.prediction?.debugSummary?.().latestAuthoritativeTick ?? null,
        predictionDebug: match.prediction?.debugSummary?.() || null,
      };
    }, { command, args, selected });
    if (issued?.error) throw new Error(issued.error);
    const record = {
      clientSeq: issued.clientSeq,
      kind: issued.command?.c || command,
      issueStep: this.issuedCommands.length,
      latestKnownAuthoritativeTick: issued.latestKnownAuthoritativeTick,
      command: issued.command,
    };
    this.issuedCommands.push(record);
    this.artifacts.client({ event: "command.issued", ...record, predictionDebug: issued.predictionDebug });
    return issued;
  }

  async waitForSnapshot({ minTickDelta = 1, timeoutMs = 10000 } = {}) {
    const startTick = await this.page.evaluate(() => window.__rts.match.state._cur?.tick ?? -1);
    if (minTickDelta <= 0 || startTick < 0) {
      await this.page.waitForFunction(
        () => window.__rts.match.state._cur?.tick != null,
        { timeout: timeoutMs },
      );
      return this.summary();
    }
    await this.page.waitForFunction(
      ({ startTick, minTickDelta }) => window.__rts.match.state._cur?.tick >= startTick + minTickDelta,
      { timeout: timeoutMs },
      { startTick, minTickDelta },
    );
    return this.summary();
  }

  async waitForAck(clientSeq, { timeoutMs = 5000 } = {}) {
    await this.page.waitForFunction(
      (clientSeq) => (window.__rts?.match?.state?._cur?.netStatus?.lastSimConsumedClientSeq || 0) >= clientSeq,
      { timeout: timeoutMs },
      clientSeq,
    );
    return this.summary();
  }

  async injectSnapshot(kind, options = {}) {
    const result = await this.page.evaluate(({ kind, options }) => {
      const match = window.__rts?.match;
      const cur = match?.state?._cur;
      if (!match?.prediction || !cur) return { error: "prediction controller or current snapshot unavailable" };
      const snapshot = structuredClone(cur);
      if (kind === "duplicate") {
        snapshot.netStatus = { ...(snapshot.netStatus || {}), lastSimConsumedClientSeq: options.ackSeq ?? snapshot.netStatus?.lastSimConsumedClientSeq ?? 0 };
      } else if (kind === "stale") {
        snapshot.tick = Math.max(0, Number(snapshot.tick || 0) - (options.tickBack ?? 1));
        snapshot.netStatus = { ...(snapshot.netStatus || {}), lastSimConsumedClientSeq: options.ackSeq ?? snapshot.netStatus?.lastSimConsumedClientSeq ?? 0 };
      } else if (kind === "skipped") {
        snapshot.tick = Number(snapshot.tick || 0) + (options.tickDelta ?? 3);
        snapshot.netStatus = { ...(snapshot.netStatus || {}), lastSimConsumedClientSeq: options.ackSeq ?? snapshot.netStatus?.lastSimConsumedClientSeq ?? 0 };
      } else {
        return { error: `unsupported injected snapshot kind: ${kind}` };
      }
      const before = match.prediction.debugSummary();
      const after = match.prediction.applyAuthoritativeSnapshot(snapshot);
      return { kind, snapshotTick: snapshot.tick, before, after };
    }, { kind, options });
    if (result?.error) throw new Error(result.error);
    this.artifacts.client({ event: "snapshot.injected", ...result });
    return result;
  }

  async setSnapshotDelivery(enabled) {
    const result = await this.page.evaluate((enabled) => {
      const match = window.__rts?.match;
      if (!match?.net || !match?.onSnapshot) return { error: "match snapshot handler unavailable" };
      if (enabled) match.net.on("snapshot", match.onSnapshot);
      else match.net.off("snapshot", match.onSnapshot);
      return { enabled };
    }, enabled);
    if (result?.error) throw new Error(result.error);
    this.artifacts.client({ event: "snapshot.delivery", enabled });
    return result;
  }

  async recordSocketReceipt(clientSeq, detail = {}) {
    const result = await this.page.evaluate(({ clientSeq, detail }) => {
      const controller = window.__rts?.match?.prediction;
      if (!controller) return { error: "prediction controller unavailable" };
      return controller.recordSocketReceipt(clientSeq, detail);
    }, { clientSeq, detail });
    if (result?.error) throw new Error(result.error);
    this.artifacts.client({ event: "socket.receipt", clientSeq, detail, predictionDebug: result });
    return result;
  }

  async recordCommandRejection(clientSeq, reason) {
    const result = await this.page.evaluate(({ clientSeq, reason }) => {
      const match = window.__rts?.match;
      const controller = match?.prediction;
      if (!controller) return { error: "prediction controller unavailable" };
      const summary = controller.recordCommandRejection(clientSeq, reason);
      match.state?.applyPredictionDisplayOverlay?.(controller.predictionDisplayOverlay());
      return summary;
    }, { clientSeq, reason });
    if (result?.error) throw new Error(result.error);
    this.artifacts.client({ event: "command.rejection", clientSeq, reason, predictionDebug: result });
    return result;
  }

  async expireCommands(elapsedMs = 20000) {
    const result = await this.page.evaluate((elapsedMs) => {
      const controller = window.__rts?.match?.prediction;
      if (!controller) return { error: "prediction controller unavailable" };
      const expired = controller.expireTimedOutCommands(performance.now() + elapsedMs);
      return { expired, summary: controller.debugSummary() };
    }, elapsedMs);
    if (result?.error) throw new Error(result.error);
    this.artifacts.client({ event: "command.timeout", elapsedMs, ...result });
    return result;
  }

  async networkDebug() {
    return this.page.evaluate(() => window.__rtsTriStateNetwork || null);
  }

  async setRoomTimeSpeed(speed) {
    await this.page.evaluate((speed) => window.__rts.match.net.setRoomTimeSpeed(speed), speed);
    if (speed === 0) {
      await this.page.waitForFunction(
        () => window.__rts?.match?.roomTimeControls?.roomTimeState?.paused === true,
        { timeout: 5000 },
      );
    }
    this.artifacts.client({ event: "setRoomTimeSpeed", speed });
  }

  async stepRoomTime() {
    await this.page.evaluate(() => window.__rts.match.net.stepRoomTime());
    this.artifacts.client({ event: "stepRoomTime" });
  }

  async capture(label) {
    const summary = await this.summary();
    this.artifacts.client({
      event: "capture",
      label,
      summary,
      predictionDebug: await this.predictionDebug(),
      selection: await this.selectionDebug(),
      network: await this.networkDebug(),
    });
    return summary;
  }

  async summary() {
    const snapshot = await this.page.evaluate(() => {
      const match = window.__rts?.match;
      const state = match?.state;
      if (!state?._cur) return null;
      return {
        ...state._cur,
        entities: state.entitiesInterpolated(1, { includePrediction: false }),
      };
    });
    const renderedSnapshot = await this.page.evaluate(() => {
      const match = window.__rts?.match;
      const state = match?.state;
      if (!state?._cur) return null;
      return {
        ...state._cur,
        entities: state.entitiesInterpolated(1, { includePrediction: true }),
      };
    });
    const playerId = await this.page.evaluate(() => window.__rts?.match?.state?.playerId ?? null);
    const summary = summarizeSnapshot(snapshot, playerId);
    if (summary) {
      summary.issuedCommands = this.issuedCommands.map(compactIssuedCommand);
      summary.rendered = summarizeSnapshot(renderedSnapshot, playerId);
    }
    return summary;
  }

  async predictionDebug() {
    return this.page.evaluate(() => {
      const match = window.__rts?.match;
      return {
        controller: match?.prediction?.debugSummary?.() || null,
        progress: match?.state?.progressPredictionDebug?.() || null,
        wasm: match?.predictionAdapter?.diagnostics?.() || null,
        published: window.__rtsPredictionDebug || null,
      };
    });
  }

  async optimisticCommandState() {
    return this.page.evaluate(() => {
      const match = window.__rts?.match;
      return match?.prediction?.optimisticUiState?.() || { production: [], rally: [] };
    });
  }

  async waitForPredictionReady({ timeoutMs = 8000 } = {}) {
    await this.page.evaluate(() => {
      const match = window.__rts?.match;
      if (match?.prediction?.enabled && !match?.predictionAdapter?.ready && !match?.predictionAdapter?.loading) {
        match.initPredictionAdapter?.();
      }
    });
    await this.page.waitForFunction(() => {
      const match = window.__rts?.match;
      return !!match?.prediction?.enabled && !!match?.predictionAdapter?.ready;
    }, { timeout: timeoutMs });
    return this.predictionDebug();
  }

  async advancePredictionVisual() {
    const result = await this.page.evaluate(() => {
      const match = window.__rts?.match;
      if (!match?.advancePredictionVisual) return { error: "match prediction visual method unavailable" };
      match.advancePredictionVisual();
      return {
        predictionDebug: {
          controller: match.prediction?.debugSummary?.() || null,
          wasm: match.predictionAdapter?.diagnostics?.() || null,
        },
      };
    });
    if (result?.error) throw new Error(result.error);
    this.artifacts.client({ event: "prediction.advanceVisual", ...result });
    return result;
  }

  async startPayload() {
    return this.page.evaluate(() => window.__rts?.match?.state?.startInfo || null);
  }

  async currentSnapshot() {
    return this.page.evaluate(() => window.__rts?.match?.state?._cur || null);
  }

  async selectionDebug() {
    return this.page.evaluate(() => {
      const state = window.__rts?.match?.state;
      return state ? Array.from(state.selection || []) : [];
    });
  }

  async currentTick() {
    return this.page.evaluate(() => {
      const roomTimeTick = window.__rts?.match?.roomTimeControls?.roomTimeState?.currentTick;
      if (Number.isFinite(roomTimeTick)) return roomTimeTick;
      return window.__rts?.match?.state?._cur?.tick ?? null;
    });
  }

  async ownEntity(kind, index = 0) {
    return ownEntityByKind(await this.summary(), kind, index);
  }

  async close() {
    if (this.browser) await this.browser.close();
  }
}

function normalizeNetworkProfile(network = {}) {
  if (!network || network.mode == null || network.mode === "direct") return { mode: "direct" };
  const tickMs = Number(network.tickMs ?? process.env.RTS_TEST_TICK_MS ?? 33) || 33;
  return {
    mode: "profile",
    name: network.name || "unnamed-profile",
    tickMs,
    seed: Number(network.seed ?? 1) || 1,
    commandLatencyTicks: Number(network.commandLatencyTicks ?? 0) || 0,
    commandJitterTicks: Number(network.commandJitterTicks ?? 0) || 0,
    snapshotLatencyTicks: Number(network.snapshotLatencyTicks ?? 0) || 0,
    snapshotJitterTicks: Number(network.snapshotJitterTicks ?? 0) || 0,
    snapshotDropEvery: Number(network.snapshotDropEvery ?? 0) || 0,
    snapshotBurstSize: Number(network.snapshotBurstSize ?? 0) || 0,
    coalesceSnapshots: !!network.coalesceSnapshots,
    coalesceWindowTicks: Number(network.coalesceWindowTicks ?? 1) || 1,
    headOfLineEvery: Number(network.headOfLineEvery ?? 0) || 0,
    headOfLineTicks: Number(network.headOfLineTicks ?? 0) || 0,
  };
}

function compactIssuedCommand(entry) {
  return {
    clientSeq: entry.clientSeq,
    kind: entry.kind,
    issueStep: entry.issueStep,
    latestKnownAuthoritativeTick: entry.latestKnownAuthoritativeTick,
  };
}
