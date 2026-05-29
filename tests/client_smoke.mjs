// Headless client smoke test. Drives the real client in headless Chrome and asserts it
// loads, renders the PixiJS scene, and that the full UI command loop works end-to-end:
// lobby -> ready -> start -> render -> box-select -> build placement (round-trips through
// the server and the depot appears) -> train-card rendering. Fails on ANY console/page error.
//
// Requires puppeteer-core and a local Chrome:
//   cd tests && npm install
//   node client_smoke.mjs              (server must be running on :8080)
// Env: RTS_URL (default http://127.0.0.1:8080/), CHROME (path to a Chrome/Chromium binary).
import puppeteer from "puppeteer-core";

const URL = process.env.RTS_URL || "http://127.0.0.1:8080/";
const CHROME = process.env.CHROME ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const consoleErrors = [];
const pageErrors = [];
let failures = 0;
const ok = (c, m) => { console.log((c ? "  PASS " : "  FAIL ") + m); if (!c) failures++; };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: "new",
  args: ["--no-sandbox", "--window-size=1440,900", "--user-data-dir=/tmp/rts-chrome-profile"],
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
  const hud = await page.evaluate(() => ({ m: document.getElementById("res-minerals")?.textContent, s: document.getElementById("res-supply")?.textContent }));
  ok(parseInt(hud.m, 10) >= 50, `HUD shows minerals (${hud.m})`);
  ok(/\d+\s*\/\s*\d+/.test(hud.s || ""), `HUD shows supply (${hud.s})`);

  const own = await page.evaluate(() => {
    const s = window.__rts.match.state, es = s.entitiesInterpolated(1).filter((e) => e.owner === s.playerId);
    return { hq: es.filter((e) => e.kind === "hq").length, w: es.filter((e) => e.kind === "worker").length };
  });
  ok(own.hq === 1 && own.w === 4, `client sees own HQ + 4 workers (hq=${own.hq}, workers=${own.w})`);

  const vp = await page.$("#viewport");
  const box = await vp.boundingBox();
  await page.mouse.move(box.x + 60, box.y + 60);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 120, box.y + box.height - 160, { steps: 10 });
  await page.mouse.up();
  await sleep(250);
  ok(await page.evaluate(() => window.__rts.match.state.selection.size) >= 1, "box-select selected own units");

  await page.click('#command-card [data-hotkey="S"]');
  await sleep(150);
  ok(await page.evaluate(() => window.__rts.match.state.placement?.building) === "depot", "build button entered placement mode");

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
    const s = window.__rts.match.state, hq = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "hq");
    if (!hq) return false;
    s.setSelection([hq.id]); window.__rts.match.hud.update();
    return !!document.querySelector('#command-card [data-hotkey="W"]');
  });
  ok(trainBtn, "TRAIN CARD: selecting the HQ shows a Worker train button");

  ok(pageErrors.length === 0, `no uncaught page errors (${pageErrors.length})`);
  ok(consoleErrors.length === 0, `no console errors (${consoleErrors.length})`);
  if (pageErrors.length) console.log("  -- pageErrors:\n" + pageErrors.map((e) => "     " + e).join("\n"));
  if (consoleErrors.length) console.log("  -- consoleErrors:\n" + consoleErrors.slice(0, 12).map((e) => "     " + e).join("\n"));
} finally {
  await browser.close();
}
console.log(`\n${failures === 0 ? "CLIENT SMOKE: ALL PASS ✅" : "CLIENT SMOKE: " + failures + " FAILURE(S) ❌"}`);
process.exit(failures === 0 ? 0 : 1);
