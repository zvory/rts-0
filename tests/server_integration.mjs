// End-to-end server integration test — no dependencies (uses Node's built-in global
// WebSocket, Node >= 22). Drives two clients through the full lifecycle and asserts the
// authoritative pipeline: lobby/host/colors -> ready/canStart -> start (map + per-player
// payload) -> initial economy -> fog of war -> gather -> train -> disconnect/win.
//
// Usage: start the server (`cd server && cargo run`), then `node tests/server_integration.mjs`.
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
const URL = process.env.RTS_WS || "ws://127.0.0.1:8081/ws";
const ROOM = "itest-" + Math.floor(performance.now());

let failures = 0;
const VERBOSE = !!process.env.RTS_VERBOSE;
const ok = (cond, msg) => { if (!cond) { console.log("  FAIL " + msg); failures++; } else if (VERBOSE) { console.log("  PASS " + msg); } };

class Client {
  constructor(tag) {
    this.tag = tag;
    this.ws = new WebSocket(URL);
    this.playerId = null;
    this.lastSnapshot = null;
    this.msgs = [];
    this.waiters = [];
    this.ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      this.msgs.push(m);
      if (m.t === "welcome") this.playerId = m.playerId;
      if (m.t === "snapshot") this.lastSnapshot = m;
      this.waiters = this.waiters.filter((w) => !w.test(m) || (w.resolve(m), false));
    };
    this.ws.onerror = (e) => console.log(`[${tag}] ws error`, e.message || e.type || e);
  }
  open() { return new Promise((res, rej) => { this.ws.onopen = () => res(); this.ws.onclose = () => rej(new Error("closed before open")); }); }
  send(o) { this.ws.send(JSON.stringify(o)); }
  waitFor(test, timeoutMs = 5000, label = "message") {
    const hit = this.msgs.find(test);
    if (hit) return Promise.resolve(hit);
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(`[${this.tag}] timeout waiting for ${label}`)), timeoutMs);
      this.waiters.push({ test, resolve: (m) => { clearTimeout(t); resolve(m); } });
    });
  }
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  const A = new Client("A"); await A.open();
  await A.waitFor((m) => m.t === "welcome", 3000, "A welcome");
  ok(A.playerId != null, `A got welcome playerId=${A.playerId}`);
  A.send({ t: "join", name: "Alpha", room: ROOM });
  await A.waitFor((m) => m.t === "lobby", 3000, "A lobby");

  const B = new Client("B"); await B.open();
  await B.waitFor((m) => m.t === "welcome", 3000, "B welcome");
  B.send({ t: "join", name: "Bravo", room: ROOM });

  const lob = await A.waitFor((m) => m.t === "lobby" && m.players.length === 2, 3000, "A lobby(2)");
  ok(lob.players.length === 2, `lobby shows 2 players: ${lob.players.map((p) => p.name).join(", ")}`);
  ok(lob.hostId === A.playerId, `host is A (${lob.hostId})`);
  ok(lob.players.every((p) => /^#/.test(p.color)), `players have hex colors: ${lob.players.map((p) => p.color).join(",")}`);

  A.send({ t: "ready", ready: true });
  B.send({ t: "ready", ready: true });
  await A.waitFor((m) => m.t === "lobby" && m.canStart, 3000, "canStart");
  ok(true, "canStart after both ready");
  A.send({ t: "start" });

  const startA = await A.waitFor((m) => m.t === "start", 3000, "A start");
  const startB = await B.waitFor((m) => m.t === "start", 3000, "B start");
  ok(startA.map.terrain.length === startA.map.width * startA.map.height,
     `start map ${startA.map.width}x${startA.map.height}, terrain len=${startA.map.terrain.length}`);
  ok(startA.players.length === 2, `start lists 2 players`);
  ok(startA.playerId === A.playerId && startB.playerId === B.playerId, `each start carries own playerId`);
  const a = startA.players.find((p) => p.id === A.playerId);
  const b = startA.players.find((p) => p.id === B.playerId);
  ok(a && b && (a.startTileX !== b.startTileX || a.startTileY !== b.startTileY),
     `players start at distinct tiles A=(${a?.startTileX},${a?.startTileY}) B=(${b?.startTileX},${b?.startTileY})`);

  const snap = await A.waitFor((m) => m.t === "snapshot" && m.entities.length > 0, 3000, "A snapshot");
  ok(snap.steel === 50, `A starts with 50 steel (${snap.steel})`);
  ok(snap.oil === 0, `A starts with 0 oil (${snap.oil})`);
  ok(snap.supplyCap === 10, `A supply cap = 10 (${snap.supplyCap})`);
  ok(snap.supplyUsed === 4, `A supply used = 4 (${snap.supplyUsed})`);
  const mine = snap.entities.filter((e) => e.owner === A.playerId);
  ok(mine.filter((e) => e.kind === "industrial_center").length === 1, `A owns 1 Industrial Center`);
  const workers = mine.filter((e) => e.kind === "worker");
  ok(workers.length === 4, `A owns 4 workers (${workers.length})`);
  const steelNodes = snap.entities.filter((e) => e.kind === "steel");
  ok(steelNodes.length > 0 && steelNodes[0].owner === 0, `A sees neutral steel nodes (${steelNodes.length})`);
  ok(!snap.entities.some((e) => e.owner === B.playerId), `FOG: A cannot see B at start`);

  A.send({ t: "command", cmd: { c: "gather", units: workers.map((w) => w.id), node: steelNodes[0].id } });
  let sawLatch = false, peak = snap.steel;
  for (let i = 0; i < 30; i++) {
    await sleep(500);
    if (A.lastSnapshot) {
      peak = Math.max(peak, A.lastSnapshot.steel);
      if (A.lastSnapshot.entities.some((e) => e.owner === A.playerId && e.kind === "worker" && e.latchedNode)) sawLatch = true;
      if (A.lastSnapshot.steel > 50) break;
    }
  }
  ok(peak > 50, `GATHER: steel rose above 50 (peak=${peak})`);
  ok(sawLatch, `GATHER: a worker latched onto steel`);

  const beforeTrain = A.lastSnapshot.steel;
  A.send({ t: "command", cmd: { c: "train", building: mine.find((e) => e.kind === "industrial_center").id, unit: "worker" } });
  await sleep(1200);
  // The 4 workers keep mining during this 1.2s window, so income partially offsets the 50 spent.
  // At 30 Hz that window is ~36 ticks — enough for a couple of attached-mining ticks — so allow a
  // generous income margin; the point is only that the train was charged (~50 deducted).
  const trainIncomeMargin = 25;
  ok(A.lastSnapshot.steel <= beforeTrain - 50 + trainIncomeMargin, `TRAIN: steel dropped ~50 (before=${beforeTrain}, after=${A.lastSnapshot.steel})`);
  const industrialCenter = A.lastSnapshot.entities.find((e) => e.kind === "industrial_center" && e.owner === A.playerId);
  ok(industrialCenter && (industrialCenter.prodKind === "worker" || (industrialCenter.prodQueue || 0) >= 1), `TRAIN: Industrial Center shows production (queue=${industrialCenter?.prodQueue})`);

  B.ws.close();
  const over = await A.waitFor((m) => m.t === "gameOver", 4000, "A gameOver");
  ok(over.you === "won", `WIN: A wins after B disconnects (you=${over.you})`);

  A.ws.close();
  await sleep(200);
  if (failures > 0) console.log(`\n${failures} FAILURE(S) ❌`);
  process.exit(failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });
