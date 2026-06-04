// Regression tests for the hardening fixes (run against a live server on :8081):
//  - build command with overflow-range tile coords must NOT crash the room
//  - a giant/duplicated units[] array must NOT stall the room (DoS guard)
//  - a join rejected mid-match must NOT wedge the socket (can still join another room)
// Usage: start the server, then `node tests/regression.mjs`.
import { decodeServerMessage } from "../client/src/protocol.js";

const URL = process.env.RTS_WS || "ws://127.0.0.1:8081/ws";
let failures = 0;
const VERBOSE = !!process.env.RTS_VERBOSE;
const ok = (c, m) => { if (!c) { console.log("  FAIL " + m); failures++; } else if (VERBOSE) { console.log("  PASS " + m); } };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

class Client {
  constructor() {
    this.ws = new WebSocket(URL); this.playerId = null; this.lastSnapshot = null; this.msgs = []; this.waiters = [];
    this.ws.onmessage = (e) => {
      const m = decodeServerMessage(JSON.parse(e.data)); this.msgs.push(m);
      if (m.t === "welcome") this.playerId = m.playerId;
      if (m.t === "snapshot") this.lastSnapshot = m;
      this.waiters = this.waiters.filter((w) => !w.test(m) || (w.resolve(m), false));
    };
  }
  open() { return new Promise((res, rej) => { this.ws.onopen = () => res(); this.ws.onerror = (e) => rej(e); }); }
  send(o) { this.ws.send(JSON.stringify(o)); }
  waitFor(test, t = 5000, label = "msg") {
    const hit = this.msgs.find(test); if (hit) return Promise.resolve(hit);
    return new Promise((resolve, reject) => {
      const to = setTimeout(() => reject(new Error("timeout: " + label)), t);
      this.waiters.push({ test, resolve: (m) => { clearTimeout(to); resolve(m); } });
    });
  }
  waitNext(test, t = 5000, label = "msg") {
    return new Promise((resolve, reject) => {
      const to = setTimeout(() => reject(new Error("timeout: " + label)), t);
      this.waiters.push({ test, resolve: (m) => { clearTimeout(to); resolve(m); } });
    });
  }
}

async function soloStart(room) {
  const c = new Client(); await c.open();
  await c.waitFor((m) => m.t === "welcome");
  c.send({ t: "join", name: "Reg", room });
  await c.waitFor((m) => m.t === "lobby");
  c.send({ t: "ready", ready: true });
  await c.waitFor((m) => m.t === "lobby" && m.canStart);
  c.send({ t: "start" });
  await c.waitFor((m) => m.t === "start");
  const snap = await c.waitFor((m) => m.t === "snapshot" && m.entities.length > 0);
  return { c, snap };
}

(async () => {
  // 1) Malicious build with overflow-range coordinates must not crash the room.
  {
    const room = "reg-build-" + Math.floor(performance.now());
    const { c, snap } = await soloStart(room);
    const worker = snap.entities.find((e) => e.owner === c.playerId && e.kind === "worker");
    c.send({ t: "command", cmd: { c: "build", worker: worker.id, building: "industrial_center", tileX: 4294967295, tileY: 0 } });
    const tickBefore = c.lastSnapshot.tick;
    await sleep(1500);
    const alive = c.lastSnapshot && c.lastSnapshot.tick > tickBefore;
    ok(alive, `OVERFLOW BUILD: room still ticking after huge tile coords (tick ${tickBefore} -> ${c.lastSnapshot?.tick})`);
    c.ws.close();
  }

  // 2a) Heavily-duplicated units[] (under the frame cap) must not stall the room: the
  //     per-command dedupe collapses N copies of one id into a single pathfind.
  {
    const room = "reg-dos-" + Math.floor(performance.now());
    const { c, snap } = await soloStart(room);
    const worker = snap.entities.find((e) => e.owner === c.playerId && e.kind === "worker");
    const units = new Array(20000).fill(worker.id); // 20k repeated owned id (~tens of KB, under cap)
    const tickBefore = c.lastSnapshot.tick;
    const t0 = performance.now();
    c.send({ t: "command", cmd: { c: "move", units, x: 1500, y: 1500 } });
    await sleep(2000);
    const dt = performance.now() - t0;
    ok(c.lastSnapshot && c.lastSnapshot.tick > tickBefore + 5,
       `DOS GUARD (dedupe): room kept ticking after 20k-id move (tick ${tickBefore} -> ${c.lastSnapshot?.tick} in ${Math.round(dt)}ms)`);
    c.ws.close();
  }

  // 2b) An oversized command frame must be rejected without taking the server down.
  {
    const room = "reg-frame-" + Math.floor(performance.now());
    const { c, snap } = await soloStart(room);
    const worker = snap.entities.find((e) => e.owner === c.playerId && e.kind === "worker");
    const huge = new Array(500000).fill(worker.id); // ~1MB JSON, exceeds the WS frame cap
    c.send({ t: "command", cmd: { c: "move", units: huge, x: 10, y: 10 } });
    await sleep(800);
    // Server must still be healthy: a brand-new connection still gets a welcome.
    const probe = new Client();
    await probe.open();
    const w = await probe.waitFor((m) => m.t === "welcome", 3000, "probe welcome");
    ok(w.playerId != null, `FRAME CAP: server healthy after an oversized command frame (new client welcome id=${w.playerId})`);
    probe.ws.close();
    c.ws.close();
  }

  // 3) A join rejected mid-match must not wedge the socket.
  {
    const room = "reg-join-" + Math.floor(performance.now());
    // Start a 2-player match in `room` so it is InGame.
    const A = new Client(); await A.open(); await A.waitFor((m) => m.t === "welcome");
    A.send({ t: "join", name: "A", room }); await A.waitFor((m) => m.t === "lobby");
    const B = new Client(); await B.open(); await B.waitFor((m) => m.t === "welcome");
    B.send({ t: "join", name: "B", room });
    await A.waitFor((m) => m.t === "lobby" && m.players.length === 2);
    A.send({ t: "ready", ready: true }); B.send({ t: "ready", ready: true });
    await A.waitFor((m) => m.t === "lobby" && m.canStart);
    A.send({ t: "start" }); await A.waitFor((m) => m.t === "start");

    // C tries to join the in-progress room -> should be rejected with an error.
    const C = new Client(); await C.open(); await C.waitFor((m) => m.t === "welcome");
    C.send({ t: "join", name: "C", room });
    await C.waitFor((m) => m.t === "error", 4000, "C rejection error");
    ok(true, "REJECTED JOIN: mid-match join returned an error");
    // C should NOT be wedged: joining a different room must now work.
    const other = room + "-other";
    C.send({ t: "join", name: "C", room: other });
    const lob = await C.waitFor((m) => m.t === "lobby" && m.room === other, 4000, "C lobby for other room");
    ok(lob.room === other, `NOT WEDGED: C joined a different room after rejection (room=${lob.room})`);
    A.ws.close(); B.ws.close(); C.ws.close();
  }

  // 4) Combat events hidden in fog must not leak to an uninvolved player. Audio trusts the
  //    snapshot stream, so a leaked attack/death event would become an audible map reveal.
  {
    const room = "reg-fog-events-" + Math.floor(performance.now());
    const clients = [new Client(), new Client(), new Client(), new Client()];
    for (const c of clients) {
      await c.open();
      await c.waitFor((m) => m.t === "welcome", 3000, "fog welcome");
      c.send({ t: "join", name: "Fog" + c.playerId, room });
    }
    await clients[0].waitFor((m) => m.t === "lobby" && m.players.length === 4, 4000, "fog lobby(4)");
    for (const c of clients) c.send({ t: "ready", ready: true });
    await clients[0].waitFor((m) => m.t === "lobby" && m.canStart, 4000, "fog canStart");
    clients[0].send({ t: "start" });
    const starts = await Promise.all(clients.map((c) => c.waitFor((m) => m.t === "start", 4000, "fog start")));
    const snaps = await Promise.all(clients.map((c) => c.waitFor((m) => m.t === "snapshot" && m.entities.length > 0, 4000, "fog first snapshot")));

    const observer = clients[0];
    const observerId = observer.playerId;
    const playerMeta = starts[0].players;
    const hiddenCandidates = clients
      .slice(1)
      .filter((c) => !snaps[0].entities.some((e) => e.owner === c.playerId));

    let pair = null;
    let bestDist = Infinity;
    for (let i = 0; i < hiddenCandidates.length; i++) {
      for (let j = i + 1; j < hiddenCandidates.length; j++) {
        const a = playerMeta.find((p) => p.id === hiddenCandidates[i].playerId);
        const b = playerMeta.find((p) => p.id === hiddenCandidates[j].playerId);
        if (!a || !b) continue;
        const dx = a.startTileX - b.startTileX;
        const dy = a.startTileY - b.startTileY;
        const d = Math.sqrt(dx * dx + dy * dy);
        if (d < bestDist) {
          bestDist = d;
          pair = [hiddenCandidates[i], hiddenCandidates[j], a, b];
        }
      }
    }

    if (!pair) {
      ok(false, "FOG EVENTS: test setup found no hidden non-observer pair");
      for (const c of clients) c.ws.close();
    } else {
      const [left, right, leftMeta, rightMeta] = pair;
      const ts = starts[0].map.tileSize;
      const targetX = ((leftMeta.startTileX + rightMeta.startTileX) / 2 + 0.5) * ts;
      const targetY = ((leftMeta.startTileY + rightMeta.startTileY) / 2 + 0.5) * ts;
      const leftSnap = snaps[clients.indexOf(left)];
      const rightSnap = snaps[clients.indexOf(right)];
      const leftWorkers = leftSnap.entities.filter((e) => e.owner === left.playerId && e.kind === "worker").map((e) => e.id);
      const rightWorkers = rightSnap.entities.filter((e) => e.owner === right.playerId && e.kind === "worker").map((e) => e.id);
      const observerStartIndex = observer.msgs.length;
      left.send({ t: "command", cmd: { c: "attackMove", units: leftWorkers, x: targetX, y: targetY } });
      right.send({ t: "command", cmd: { c: "attackMove", units: rightWorkers, x: targetX, y: targetY } });

      let combatSeen = false;
      const deadline = performance.now() + 45000;
      while (performance.now() < deadline && !combatSeen) {
        await sleep(1000);
        combatSeen = [left, right].some((c) =>
          c.msgs.some((m) => m.t === "snapshot" && (m.events || []).some((ev) => ev.e === "attack" || ev.e === "death")),
        );
      }

      ok(combatSeen, "FOG EVENTS: hidden pair produced combat events for involved players");
      const hiddenIds = new Set([...leftWorkers, ...rightWorkers]);
      const leaked = observer.msgs.slice(observerStartIndex).some((m) =>
        m.t === "snapshot" && (m.events || []).some((ev) =>
          (typeof ev.from === "number" && hiddenIds.has(ev.from)) ||
          (typeof ev.to === "number" && hiddenIds.has(ev.to)) ||
          (typeof ev.id === "number" && hiddenIds.has(ev.id))
        ),
      );
      ok(!leaked, `FOG EVENTS: observer ${observerId} received no hidden attack/death/build event ids`);
      for (const c of clients) c.ws.close();
    }
  }

  await sleep(200);
  if (failures > 0) console.log(`\nREGRESSION: ${failures} FAILURE(S) ❌`);
  process.exit(failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });
