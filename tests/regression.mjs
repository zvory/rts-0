// Regression tests for the hardening fixes (run against a live server on :8081):
//  - build command with overflow-range tile coords must NOT crash the room
//  - a giant/duplicated units[] array must NOT stall the room (DoS guard)
//  - a join rejected mid-match must NOT wedge the socket (can still join another room)
// Usage: start the server, then `node tests/regression.mjs`.
const URL = process.env.RTS_WS || "ws://127.0.0.1:8081/ws";
let failures = 0;
const ok = (c, m) => { console.log((c ? "  PASS " : "  FAIL ") + m); if (!c) failures++; };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

class Client {
  constructor() {
    this.ws = new WebSocket(URL); this.playerId = null; this.lastSnapshot = null; this.msgs = []; this.waiters = [];
    this.ws.onmessage = (e) => {
      const m = JSON.parse(e.data); this.msgs.push(m);
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

  await sleep(200);
  console.log(`\n${failures === 0 ? "REGRESSION: ALL PASS ✅" : "REGRESSION: " + failures + " FAILURE(S) ❌"}`);
  process.exit(failures === 0 ? 0 : 1);
})().catch((e) => { console.log("TEST ERROR:", e.message); process.exit(2); });
