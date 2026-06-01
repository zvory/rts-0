// Headless client smoke test. Drives the real client in headless Chrome and asserts it
// loads, renders the PixiJS scene, and that the full UI command loop works end-to-end:
// lobby -> ready -> start -> render -> box-select -> mine enough steel -> build placement
// (round-trips through the server and the depot appears) -> train-card rendering.
// Fails on ANY console/page error.
//
// Requires puppeteer-core and a local Chrome:
//   cd tests && npm install
//   node client_smoke.mjs              (server must be running on :8081)
// Env: RTS_URL (default http://127.0.0.1:8081/), CHROME (path to a Chrome/Chromium binary).
import puppeteer from "puppeteer-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const URL = process.env.RTS_URL || "http://127.0.0.1:8081/";
const CHROME = process.env.CHROME ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const consoleErrors = [];
const pageErrors = [];
let failures = 0;
const VERBOSE = !!process.env.RTS_VERBOSE;
const ok = (c, m) => { if (!c) { console.log("  FAIL " + m); failures++; } else if (VERBOSE) { console.log("  PASS " + m); } };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chromeProfileDir = fs.mkdtempSync(path.join(os.tmpdir(), "rts-chrome-"));

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: "new",
  args: ["--no-sandbox", "--window-size=1440,900", `--user-data-dir=${chromeProfileDir}`],
  defaultViewport: { width: 1440, height: 900 },
});

try {
  const page = await browser.newPage();
  page.on("console", (m) => { if (m.type() === "error") consoleErrors.push(m.text()); });
  page.on("pageerror", (e) => pageErrors.push(e.message));
  page.on("requestfailed", (r) => { if (!r.url().includes("favicon")) consoleErrors.push("requestfailed: " + r.url()); });

  await page.goto(URL, { waitUntil: "networkidle2", timeout: 15000 });
  await page.waitForSelector("#lobby-screen", { visible: true, timeout: 5000 });
  ok(true, "lobby screen visible on load");
  ok(await page.evaluate(() => !!window.PIXI), `PixiJS loaded (v${await page.evaluate(() => window.PIXI?.VERSION)})`);

  await page.click("#lobby-name", { clickCount: 3 });
  await page.type("#lobby-name", "Solo");
  await page.click("#lobby-room", { clickCount: 3 });
  await page.type("#lobby-room", "client-smoke-" + Date.now());
  await page.click("#lobby-join");
  await page.waitForFunction(() => document.querySelector("#lobby-players")?.children.length >= 1, { timeout: 5000 });
  ok(true, "joined room; lobby player list populated");

  await page.click("#lobby-ready");
  await page.waitForFunction(() => { const b = document.querySelector("#lobby-start"); return b && !b.disabled; }, { timeout: 5000 });
  ok(true, "Start enabled after readying up");
  await page.click("#lobby-start");
  await page.waitForFunction(() => { const g = document.getElementById("game-screen"); return g && !g.hidden; }, { timeout: 6000 });
  ok(true, "game screen shown after start");

  await page.waitForSelector("#viewport canvas", { timeout: 5000 });
  const canvas = await page.evaluate(() => { const c = document.querySelector("#viewport canvas"); return c ? { w: c.width, h: c.height } : null; });
  ok(canvas && canvas.w > 0 && canvas.h > 0, `canvas mounted and sized (${canvas?.w}x${canvas?.h})`);

  await sleep(2500);
  const hud = await page.evaluate(() => ({ m: document.getElementById("res-steel")?.textContent, s: document.getElementById("res-supply")?.textContent }));
  ok(parseInt(hud.m, 10) >= 50, `HUD shows steel (${hud.m})`);
  ok(/\d+\s*\/\s*\d+/.test(hud.s || ""), `HUD shows supply (${hud.s})`);

  const own = await page.evaluate(() => {
    const s = window.__rts.match.state, es = s.entitiesInterpolated(1).filter((e) => e.owner === s.playerId);
    return { industrialCenter: es.filter((e) => e.kind === "industrial_center").length, w: es.filter((e) => e.kind === "worker").length };
  });
  ok(own.industrialCenter === 1 && own.w === 4, `client sees own Industrial Center + 4 workers (industrialCenter=${own.industrialCenter}, workers=${own.w})`);

  // Interpolation must be live: GameState exposes recv timestamps so alpha isn't pinned to 1.
  const interp = await page.evaluate(() => {
    const s = window.__rts.match.state;
    return { prev: typeof s.prevRecvTime, curr: typeof s.currRecvTime,
             distinct: s.prevRecvTime != null && s.currRecvTime != null && s.prevRecvTime !== s.currRecvTime };
  });
  ok(interp.curr === "number" && interp.prev === "number" && interp.distinct,
     `INTERP: GameState exposes two distinct recv timestamps (prev=${interp.prev}, curr=${interp.curr})`);

  const vp = await page.$("#viewport");
  const box = await vp.boundingBox();
  await page.mouse.move(box.x + 60, box.y + 60);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 120, box.y + box.height - 160, { steps: 10 });
  await page.mouse.up();
  await sleep(250);
  ok(await page.evaluate(() => window.__rts.match.state.selection.size) >= 1, "box-select selected own units");

  const gather = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state;
    const workers = s.selectedEntities().filter((e) => e.owner === s.playerId && e.kind === "worker");
    const steel = s.entitiesInterpolated(1)
      .filter((e) => e.kind === "steel")
      .sort((a, b) => a.id - b.id);
    const n = Math.min(workers.length, steel.length);
    for (let i = 0; i < n; i++) {
      m.net.command({ c: "gather", units: [workers[i].id], node: steel[i].id });
    }
    return { workers: workers.length, nodes: steel.length, assigned: n };
  });
  ok(gather.assigned > 0, `assigned workers to steel before building (workers=${gather.workers}, nodes=${gather.nodes})`);
  await page.waitForFunction(() => {
    const btn = document.querySelector('#command-card button[data-hotkey="S"]');
    return btn && !btn.disabled && /Supply Depot/.test(btn.textContent || "");
  }, { timeout: 30000 });
  ok(true, "Depot build button became affordable after mining");

  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("s");
  await sleep(150);
  ok(await page.evaluate(() => window.__rts.match.state.placement?.building) === "depot", "build hotkey entered placement mode");

  const target = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state, map = s.map, ts = map.tileSize, PASS = { 0: true, 1: false, 2: false };
    const me = s.players.find((p) => p.id === s.playerId), hx = me.startTileX, hy = me.startTileY;
    for (let r = 3; r <= 7; r++) for (let dx = -r; dx <= r; dx++) for (let dy = -r; dy <= r; dy++) {
      if (Math.abs(dx) < 3 && Math.abs(dy) < 3) continue;
      const tx = hx + dx, ty = hy + dy;
      if (tx < 1 || ty < 1 || tx >= map.width - 2 || ty >= map.height - 2) continue;
      let okt = true;
      for (let ax = 0; ax < 2; ax++) for (let ay = 0; ay < 2; ay++) if (!PASS[map.terrain[(ty + ay) * map.width + (tx + ax)]]) okt = false;
      if (!okt) continue;
      const sp = m.camera.worldToScreen((tx + 1) * ts, (ty + 1) * ts);
      return { tx, ty, sx: sp.x, sy: sp.y };
    }
    return null;
  });
  ok(target != null, `found a valid placement tile (${target?.tx},${target?.ty})`);
  await page.mouse.move(box.x + target.sx, box.y + target.sy, { steps: 4 });
  await sleep(150);
  await page.mouse.click(box.x + target.sx, box.y + target.sy);
  let depot = false;
  for (let i = 0; i < 20 && !depot; i++) {
    await sleep(200);
    depot = await page.evaluate(() => { const s = window.__rts.match.state; return s.entitiesInterpolated(1).some((e) => e.owner === s.playerId && e.kind === "depot"); });
  }
  ok(depot, "BUILD: placing a Supply Depot created an own depot entity (server round-trip)");

  const trainBtn = await page.evaluate(() => {
    const s = window.__rts.match.state, industrialCenter = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "industrial_center");
    if (!industrialCenter) return false;
    s.setSelection([industrialCenter.id]); window.__rts.match.hud.update();
    return !!document.querySelector('#command-card [data-hotkey="Q"]');
  });
  ok(trainBtn, "TRAIN CARD: selecting the Industrial Center shows a Worker train button");

  const beforePan = await page.evaluate(() => ({
    x: window.__rts.match.camera.x,
    y: window.__rts.match.camera.y,
    selected: window.__rts.match.state.selection.size,
  }));
  await page.keyboard.down("Space");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 - 120, box.y + box.height / 2 - 80, { steps: 6 });
  await page.mouse.up();
  await page.keyboard.up("Space");
  await sleep(100);
  const afterPan = await page.evaluate(() => ({
    x: window.__rts.match.camera.x,
    y: window.__rts.match.camera.y,
    selected: window.__rts.match.state.selection.size,
  }));
  ok(afterPan.x !== beforePan.x || afterPan.y !== beforePan.y,
     `CAMERA: Space+drag pans the viewport (${beforePan.x.toFixed(1)},${beforePan.y.toFixed(1)} -> ${afterPan.x.toFixed(1)},${afterPan.y.toFixed(1)})`);
  ok(afterPan.selected === beforePan.selected, "CAMERA: Space+drag does not change selection");

  ok(pageErrors.length === 0, `no uncaught page errors (${pageErrors.length})`);
  ok(consoleErrors.length === 0, `no console errors (${consoleErrors.length})`);
  if (pageErrors.length) console.log("  -- pageErrors:\n" + pageErrors.map((e) => "     " + e).join("\n"));
  if (consoleErrors.length) console.log("  -- consoleErrors:\n" + consoleErrors.slice(0, 12).map((e) => "     " + e).join("\n"));
} finally {
  await browser.close();
}
if (failures > 0) console.log(`\nCLIENT SMOKE: ${failures} FAILURE(S) ❌`);
process.exit(failures === 0 ? 0 : 1);
