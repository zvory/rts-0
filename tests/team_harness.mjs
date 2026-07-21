import { decodeServerMessage, parseServerFrame } from "../client/src/protocol.js";

export const URL = process.env.RTS_WS || "ws://127.0.0.1:8081/ws";

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const timeoutScaleRaw = process.env.RTS_TEST_TIMEOUT_SCALE
  ?? (process.env.CI || process.env.GITHUB_ACTIONS ? "3" : "1");
const timeoutScaleParsed = Number(timeoutScaleRaw);
const TIMEOUT_SCALE = Number.isFinite(timeoutScaleParsed) && timeoutScaleParsed > 0
  ? timeoutScaleParsed
  : 1;

function scaledTimeoutMs(timeoutMs) {
  return Math.ceil(timeoutMs * TIMEOUT_SCALE);
}

export function uniqueRoom(prefix = "itest") {
  return `${prefix}-${Math.floor(performance.now())}-${Math.floor(Math.random() * 1_000_000)}`;
}

export function createAssertions({ verbose = !!process.env.RTS_VERBOSE } = {}) {
  let failures = 0;
  return {
    ok(cond, msg) {
      if (!cond) {
        console.log("  FAIL " + msg);
        failures++;
      } else if (verbose) {
        console.log("  PASS " + msg);
      }
    },
    get failures() {
      return failures;
    },
  };
}

export class Client {
  constructor(tag, { url = URL } = {}) {
    this.tag = tag;
    this.ws = new WebSocket(url);
    this.ws.binaryType = "arraybuffer";
    this.playerId = null;
    this.lastSnapshot = null;
    this.msgs = [];
    this.rawSnapshots = [];
    this.waiters = [];
    this.nextClientSeq = 1;
    this.ws.onmessage = (event) => {
      const raw = parseServerFrame(event.data);
      if (raw.t === "snapshot") this.rawSnapshots.push(raw);
      const msg = decodeServerMessage(raw);
      this.msgs.push(msg);
      if (msg.t === "welcome") this.playerId = msg.playerId;
      if (msg.t === "snapshot") this.lastSnapshot = msg;
      this.waiters = this.waiters.filter((waiter) => {
        if (!waiter.test(msg)) return true;
        waiter.resolve(msg);
        return false;
      });
    };
    this.ws.onerror = (event) => console.log(`[${tag}] ws error`, event.message || event.type || event);
  }

  open() {
    return new Promise((resolve, reject) => {
      this.ws.onopen = () => resolve();
      this.ws.onclose = () => reject(new Error(`[${this.tag}] closed before open`));
    });
  }

  send(payload) {
    this.ws.send(JSON.stringify(payload));
  }

  command(cmd) {
    this.send({ t: "command", clientSeq: this.nextClientSeq++, cmd });
  }

  ready(ready = true) {
    this.send({ t: "ready", ready });
  }

  waitFor(test, timeoutMs = 5000, label = "message") {
    const hit = this.msgs.find(test);
    if (hit) return Promise.resolve(hit);
    return this.waitNext(test, timeoutMs, label);
  }

  waitNext(test, timeoutMs = 5000, label = "message") {
    return new Promise((resolve, reject) => {
      const effectiveTimeoutMs = scaledTimeoutMs(timeoutMs);
      const timeout = setTimeout(
        () => reject(new Error(`[${this.tag}] timeout waiting for ${label}`)),
        effectiveTimeoutMs,
      );
      this.waiters.push({
        test,
        resolve: (msg) => {
          clearTimeout(timeout);
          resolve(msg);
        },
      });
    });
  }

  close() {
    if (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING) {
      this.ws.close();
    }
  }
}

export async function connectClient(tag, opts) {
  const client = new Client(tag, opts);
  await client.open();
  await client.waitFor((msg) => msg.t === "welcome", 3000, `${tag} welcome`);
  return client;
}

export async function joinClient(tag, { room, name = tag, spectator = false } = {}) {
  const client = await connectClient(tag);
  client.send({ t: "join", name, room, ...(spectator ? { spectator: true } : {}) });
  return client;
}

export async function addAi(host, count = 1, { timeoutMs = 3000 } = {}) {
  const added = [];
  for (let i = 0; i < count; i++) {
    const beforeIds = new Set(lastLobby(host)?.players.map((player) => player.id) || []);
    host.send({ t: "addAi" });
    const lobby = await host.waitNext(
      (msg) => msg.t === "lobby" && msg.players.some((player) => player.isAi && !beforeIds.has(player.id)),
      timeoutMs,
      "lobby with added AI",
    );
    added.push(lobby.players.find((player) => player.isAi && !beforeIds.has(player.id)));
  }
  return added;
}

export async function addAiToTeam(host, teamId, { timeoutMs = 3000 } = {}) {
  const beforeIds = new Set(lastLobby(host)?.players.map((player) => player.id) || []);
  host.send({ t: "addAi", teamId });
  const lobby = await host.waitNext(
    (msg) => msg.t === "lobby" && msg.players.some((player) => player.isAi && !beforeIds.has(player.id)),
    timeoutMs,
    `lobby with added AI on team ${teamId}`,
  );
  return lobby.players.find((player) => player.isAi && !beforeIds.has(player.id));
}

export async function setTeamPreset(host, preset, { timeoutMs = 3000 } = {}) {
  host.send({ t: "setTeamPreset", preset });
  return host.waitNext(
    (msg) => msg.t === "lobby" && msg.teamPreset === preset,
    timeoutMs,
    `lobby with ${preset} preset`,
  );
}

export async function setTeam(host, id, teamId, { timeoutMs = 3000 } = {}) {
  host.send({ t: "setTeam", id, teamId });
  return host.waitNext(
    (msg) => msg.t === "lobby" && msg.players.some((player) => player.id === id && player.teamId === teamId),
    timeoutMs,
    `lobby with ${id} on team ${teamId}`,
  );
}

export async function removeAi(host, id, { timeoutMs = 3000 } = {}) {
  host.send({ t: "removeAi", id });
  return host.waitNext(
    (msg) => msg.t === "lobby" && !msg.players.some((player) => player.id === id),
    timeoutMs,
    "lobby after AI removal",
  );
}

export async function readyPlayers(clients, { timeoutMs = 3000 } = {}) {
  for (const client of clients) client.ready(true);
  return clients[0].waitFor((msg) => msg.t === "lobby" && msg.canStart, timeoutMs, "canStart");
}

export async function startMatch(host, participants, { timeoutMs = 6000 } = {}) {
  host.send({ t: "start" });
  const countdowns = await Promise.all(participants.map((client) =>
    client.waitFor((msg) => msg.t === "matchCountdown", 3000, `${client.tag} countdown`)
  ));
  participants.forEach((client, index) => {
    client.send({ t: "matchLoadReady", countdownId: countdowns[index].countdownId });
  });
  const starts = await Promise.all(participants.map((client) =>
    client.waitFor((msg) => msg.t === "start", timeoutMs, `${client.tag} start`)
  ));
  return { countdowns, starts };
}

export async function startMatchDirect(host, participants, { timeoutMs = 3000 } = {}) {
  host.send({ t: "start" });
  return Promise.all(participants.map((client) =>
    client.waitFor((msg) => msg.t === "start", timeoutMs, `${client.tag} start`)
  ));
}

export async function waitForGameOver(clients, { timeoutMs = 4000 } = {}) {
  return Promise.all(clients.map((client) =>
    client.waitFor((msg) => msg.t === "gameOver", timeoutMs, `${client.tag} gameOver`)
  ));
}

export function lastLobby(client) {
  return client.msgs.filter((msg) => msg.t === "lobby").at(-1) || null;
}

export function closeClients(...clients) {
  for (const client of clients.flat()) client?.close();
}

export function assertLobbyProtocol(ok, lobby, { expectedPlayers, hostId } = {}) {
  ok(lobby?.t === "lobby", "LOBBY: message has lobby tag");
  ok(typeof lobby.hostId === "number", `LOBBY: hostId is numeric (${lobby.hostId})`);
  if (hostId != null) ok(lobby.hostId === hostId, `LOBBY: host is expected player (${lobby.hostId})`);
  ok(typeof lobby.canStart === "boolean", `LOBBY: canStart is boolean (${lobby.canStart})`);
  ok(typeof lobby.teamPreset === "string" && lobby.teamPreset.length > 0,
    `LOBBY: teamPreset is present (${lobby.teamPreset})`);
  ok(Array.isArray(lobby.players), `LOBBY: players is an array (${lobby.players?.length})`);
  if (expectedPlayers != null) ok(lobby.players.length === expectedPlayers, `LOBBY: lists ${expectedPlayers} participants`);
  ok(Array.isArray(lobby.maps) && lobby.maps.length >= 1, `LOBBY: exposes selectable maps (${lobby.maps?.length})`);
  ok(lobby.maps.some((map) => map.name === lobby.map), `LOBBY: selected map is present (${lobby.map})`);
  for (const map of lobby.maps) {
    ok(typeof map.minPlayers === "number" && map.minPlayers >= 1,
      `LOBBY: map minPlayers is valid for ${map.name} (${map.minPlayers})`);
    ok(typeof map.maxPlayers === "number" && map.maxPlayers >= map.minPlayers,
      `LOBBY: map maxPlayers is valid for ${map.name} (${map.maxPlayers})`);
  }
  for (const player of lobby.players) {
    ok(typeof player.id === "number", `LOBBY: player id is numeric (${player.name}/${player.id})`);
    ok(typeof player.teamId === "number", `LOBBY: teamId is numeric for ${player.name} (${player.teamId})`);
    ok(typeof player.name === "string" && player.name.length > 0, `LOBBY: player has name (${player.name})`);
    ok(/^#/.test(player.color), `LOBBY: player has hex color (${player.color})`);
    ok(typeof player.ready === "boolean", `LOBBY: ready is boolean for ${player.name}`);
    ok(typeof player.isAi === "boolean", `LOBBY: isAi is boolean for ${player.name}`);
    ok(typeof player.isSpectator === "boolean", `LOBBY: isSpectator is boolean for ${player.name}`);
  }
}

export function assertCountdownProtocol(ok, countdown) {
  ok(countdown?.t === "matchCountdown", "COUNTDOWN: message has countdown tag");
  ok(Number.isInteger(countdown.countdownId) && countdown.countdownId > 0,
    `COUNTDOWN: carries a nonzero generation (${countdown.countdownId})`);
  ok(countdown.durationMs === 3000, `COUNTDOWN: duration is stable (${countdown.durationMs}ms)`);
  ok(Array.isArray(countdown.words) && countdown.words.join(" ") === "Drei! Zwei! Eins!",
    `COUNTDOWN: words are stable (${countdown.words?.join(" ")})`);
}

export function assertStartProtocol(ok, start, { playerId, expectedPlayers, spectator } = {}) {
  ok(start?.t === "start", "START: message has start tag");
  if (playerId != null) ok(start.playerId === playerId, `START: playerId matches recipient (${start.playerId})`);
  if (spectator != null) ok(start.spectator === spectator, `START: spectator flag is ${spectator}`);
  ok(start.map && start.map.terrain.length === start.map.width * start.map.height,
    `START: map terrain length matches dimensions (${start.map?.width}x${start.map?.height})`);
  ok(Array.isArray(start.map?.resources), `START: map resources is an array (${start.map?.resources?.length})`);
  ok(Array.isArray(start.players), `START: players is an array (${start.players?.length})`);
  if (expectedPlayers != null) ok(start.players.length === expectedPlayers, `START: lists ${expectedPlayers} active players`);
  for (const player of start.players || []) {
    ok(typeof player.id === "number", `START: player id is numeric (${player.id})`);
    ok(typeof player.teamId === "number" && player.teamId > 0,
      `START: player teamId is nonzero (${player.teamId})`);
    ok(typeof player.name === "string" && player.name.length > 0, `START: player has name (${player.name})`);
    ok(/^#/.test(player.color), `START: player has hex color (${player.color})`);
    ok(Number.isInteger(player.startTileX) && Number.isInteger(player.startTileY),
      `START: player has integer start tile (${player.startTileX},${player.startTileY})`);
    if ("isAi" in player) {
      ok(typeof player.isAi === "boolean", `START: isAi is boolean for ${player.name}`);
    }
  }
}

export function assertDistinctStartTiles(ok, start) {
  const seen = new Set();
  for (const player of start.players || []) {
    const key = `${player.startTileX},${player.startTileY}`;
    ok(!seen.has(key), `START: ${player.name} has a distinct start tile (${key})`);
    seen.add(key);
  }
}

export function assertScoreProtocol(ok, gameOver, { expectedPlayers } = {}) {
  ok(gameOver?.t === "gameOver", "SCORE: gameOver message has stable tag");
  ok(gameOver.winnerTeamId == null || typeof gameOver.winnerTeamId === "number",
    `SCORE: winnerTeamId is nullable numeric (${gameOver.winnerTeamId})`);
  ok(["won", "lost", "draw"].includes(gameOver.you), `SCORE: result verdict is stable (${gameOver.you})`);
  ok(Array.isArray(gameOver.scores), `SCORE: scores is an array (${gameOver.scores?.length})`);
  if (expectedPlayers != null) ok(gameOver.scores.length === expectedPlayers, `SCORE: lists ${expectedPlayers} players`);
  for (const score of gameOver.scores || []) {
    ok(typeof score.id === "number", `SCORE: player id is numeric (${score.id})`);
    ok(typeof score.teamId === "number" && score.teamId > 0,
      `SCORE: player teamId is nonzero for ${score.name} (${score.teamId})`);
    ok(typeof score.name === "string" && score.name.length > 0, `SCORE: player has name (${score.name})`);
    ok(typeof score.apm === "number", `SCORE: apm is numeric for ${score.name}`);
    ok(typeof score.unitScore === "number", `SCORE: unitScore is numeric for ${score.name}`);
    ok(typeof score.structureScore === "number", `SCORE: structureScore is numeric for ${score.name}`);
    ok(typeof score.unitsLost === "number", `SCORE: unitsLost is numeric for ${score.name}`);
    ok(typeof score.buildingsLost === "number", `SCORE: buildingsLost is numeric for ${score.name}`);
  }
}
