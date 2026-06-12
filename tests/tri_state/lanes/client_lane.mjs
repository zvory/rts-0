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
    await this.page.evaluateOnNewDocument((prediction) => {
      window.localStorage.setItem("rts.prediction.enabled", prediction === "enabled" ? "1" : "0");
    }, this.scenario.setup.prediction);
    await this.page.goto(url.href, { waitUntil: "networkidle2", timeout: 15000 });
    await this.page.waitForFunction(() => window.__rts?.net?.playerId != null, { timeout: 5000 });
    await this.page.evaluate((room, quickstart) => {
      const app = window.__rts;
      app.net.join("Client", room);
      if (quickstart) app.net.setQuickstart(true);
      app.net.ready(true);
      app.net.start();
    }, this.room, this.scenario.setup.quickstart);
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
    await this.page.goto(url.href, { waitUntil: "networkidle2", timeout: 15000 });
    await this.waitForMatch();
    await this.setReplaySpeed(1);
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
    if (!["move", "attackMove", "stop", "train", "setRally", "build", "invalidMove"].includes(command)) {
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
      const controller = window.__rts?.match?.prediction;
      if (!controller) return { error: "prediction controller unavailable" };
      return controller.recordCommandRejection(clientSeq, reason);
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

  async setReplaySpeed(speed) {
    await this.page.evaluate((speed) => window.__rts.match.net.setReplaySpeed(speed), speed);
    this.artifacts.client({ event: "setReplaySpeed", speed });
  }

  async stepDevTick() {
    await this.page.evaluate(() => window.__rts.match.net.stepDevTick());
    this.artifacts.client({ event: "stepDevTick" });
  }

  async capture(label) {
    const summary = await this.summary();
    this.artifacts.client({
      event: "capture",
      label,
      summary,
      predictionDebug: await this.predictionDebug(),
      selection: await this.selectionDebug(),
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
    const playerId = await this.page.evaluate(() => window.__rts?.match?.state?.playerId ?? null);
    const summary = summarizeSnapshot(snapshot, playerId);
    if (summary) summary.issuedCommands = this.issuedCommands.map(compactIssuedCommand);
    return summary;
  }

  async predictionDebug() {
    return this.page.evaluate(() => {
      const match = window.__rts?.match;
      return {
        controller: match?.prediction?.debugSummary?.() || null,
        wasm: match?.predictionAdapter?.diagnostics?.() || null,
        published: window.__rtsPredictionDebug || null,
      };
    });
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
    return this.page.evaluate(() => window.__rts?.match?.state?._cur?.tick ?? null);
  }

  async ownEntity(kind, index = 0) {
    return ownEntityByKind(await this.summary(), kind, index);
  }

  async close() {
    if (this.browser) await this.browser.close();
  }
}

function compactIssuedCommand(entry) {
  return {
    clientSeq: entry.clientSeq,
    kind: entry.kind,
    issueStep: entry.issueStep,
    latestKnownAuthoritativeTick: entry.latestKnownAuthoritativeTick,
  };
}
