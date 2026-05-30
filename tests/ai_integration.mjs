// End-to-end test for the optional AI opponents — no dependencies (Node >= 22 built-in
// WebSocket). Drives a single host through the lobby AI controls and asserts:
//   - addAi seats a computer opponent (isAi=true, hex color, always ready, not the host)
//   - the room can start with only the human ready (AIs don't gate readiness)
//   - removeAi unseats an AI
//   - addAi is host-only (a non-host's addAi is ignored)
//   - a 1-human + 1-AI match starts as a real 2-player match (distinct start tiles) and stays
//     live (snapshots keep flowing) — the AI's actual build/attack behavior is verified
//     deterministically by the Rust unit test `game::tests::ai_builds_economy_and_attacks`.
//
// Usage: start the server (`cd server && cargo run`), then `node tests/ai_integration.mjs`.
// Override the endpoint with RTS_WS (default ws://127.0.0.1:8081/ws).
const URL = process.env.RTS_WS || "ws://127.0.0.1:8081/ws";
const ROOM = "ai-itest-" + Math.floor(performance.now());

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
  // Wait for a NEW message matching `test` (ignores already-received ones, so repeated lobby
  // updates with the same shape can be awaited in sequence).
  waitNext(test, timeoutMs = 5000, label = "message") {
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(`[${this.tag}] timeout waiting for ${label}`)), timeoutMs);
      this.waiters.push({ test, resolve: (m) => { clearTimeout(t); resolve(m); } });
    });
  }
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  const A = new Client("A"); await A.open();
  await A.waitNext((m) => m.t === "welcome", 3000, "A welcome");
  A.send({ t: "join", name: "Host", room: ROOM });
  const solo = await A.waitNext((m) => m.t === "lobby", 3000, "A lobby");
  ok(solo.players.length === 1 && solo.hostId === A.playerId, `host alone in lobby (host=${solo.hostId})`);

  // Add an AI opponent.
  A.send({ t: "addAi" });
  const withAi = await A.waitNext((m) => m.t === "lobby" && m.players.length === 2, 3000, "lobby with AI");
  const ai = withAi.players.find((p) => p.isAi);
  const human = withAi.players.find((p) => p.id === A.playerId);
  ok(!!ai, "addAi seated a computer opponent");
  ok(ai && ai.isAi === true && human && human.isAi === false, "isAi flag distinguishes AI from human");
  ok(ai && /^#/.test(ai.color) && ai.color !== human.color, `AI has a distinct hex color (${ai?.color} vs ${human?.color})`);
  ok(ai && ai.ready === true, "AI is always ready");
  ok(ai && ai.id !== A.playerId, "AI got its own player id");

  // Add a second AI, then remove the first — exercises both controls and the cap accounting.
  A.send({ t: "addAi" });
  const withTwo = await A.waitNext((m) => m.t === "lobby" && m.players.length === 3, 3000, "lobby with 2 AI");
  ok(withTwo.players.filter((p) => p.isAi).length === 2, "second addAi seated a third player");
  A.send({ t: "removeAi", id: ai.id });
  const removed = await A.waitNext((m) => m.t === "lobby" && m.players.length === 2, 3000, "lobby after remove");
  ok(!removed.players.some((p) => p.id === ai.id), "removeAi unseated the targeted AI");

  // A non-host cannot add AIs: B joins, sends addAi, and the player count must not change.
  const B = new Client("B"); await B.open();
  await B.waitNext((m) => m.t === "welcome", 3000, "B welcome");
  B.send({ t: "join", name: "Guest", room: ROOM });
  await A.waitNext((m) => m.t === "lobby" && m.players.length === 3, 3000, "lobby with B");
  B.send({ t: "addAi" });
  await sleep(400);
  const last = A.msgs.filter((m) => m.t === "lobby").at(-1);
  ok(last.players.length === 3, `non-host addAi ignored (still ${last.players.length} players)`);
  // Drop B so the start is a clean 1-human + 1-AI match.
  B.ws.close();
  await A.waitNext((m) => m.t === "lobby" && m.players.length === 2, 3000, "lobby after B leaves");

  // Start: only the host needs to be ready (the AI doesn't gate canStart).
  A.send({ t: "ready", ready: true });
  await A.waitNext((m) => m.t === "lobby" && m.canStart, 3000, "canStart with just host ready");
  ok(true, "match can start with one human ready + one AI");
  A.send({ t: "start" });

  const start = await A.waitNext((m) => m.t === "start", 3000, "A start");
  ok(start.players.length === 2, `start lists 2 players (human + AI) (${start.players.length})`);
  const sa = start.players.find((p) => p.id === A.playerId);
  const sai = start.players.find((p) => p.id !== A.playerId);
  ok(sa && sai && (sa.startTileX !== sai.startTileX || sa.startTileY !== sai.startTileY),
     `human and AI start at distinct tiles`);

  // The match is live: confirm the human keeps receiving snapshots and its Industrial Center is present. (The
  // AI's economy/attack behavior is covered by the Rust unit test — fog hides the AI base from
  // the human here, so there's nothing fast to observe over the wire beyond a running match.)
  const firstSnap = await A.waitNext((m) => m.t === "snapshot" && m.entities.length > 0, 3000, "first snapshot");
  ok(firstSnap.entities.some((e) => e.owner === A.playerId && e.kind === "industrial_center"), "human owns its Industrial Center in-match");
  const tick0 = firstSnap.tick;
  await sleep(1500);
  ok(A.lastSnapshot && A.lastSnapshot.tick > tick0, `match advancing (tick ${tick0} -> ${A.lastSnapshot?.tick})`);
  ok(!A.msgs.some((m) => m.t === "gameOver"), "match still running (not prematurely resolved)");

  A.ws.close();
  await sleep(200);
  if (failures > 0) console.log(`\n${failures} FAILURE(S) ❌`);
  process.exit(failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });
