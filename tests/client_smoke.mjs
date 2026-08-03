// Headless client smoke test. Drives the real client in headless Chrome and asserts it
// loads, renders the PixiJS scene, and that the full UI command loop works end-to-end:
// lobby -> ready -> start -> render -> box-select -> worker build card (with Pump Jack in the
// former Depot slot) -> train-card rendering.
// Fails on ANY console/page error.
//
// Requires a local Chrome. `tests/run-all.sh` installs the repository-owned puppeteer-core
// dependency through the shared lockfile-keyed cache before running this script:
//   tests/run-all.sh --no-rust
//   node tests/client_smoke.mjs        (server must be running on :8081)
// Env: RTS_URL (default http://127.0.0.1:8081/), CHROME (path to a Chrome/Chromium binary).
import puppeteer from "puppeteer-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const BASE_URL = process.env.RTS_URL || "http://127.0.0.1:8081/";
const TEST_URL = (() => {
  const url = new URL(BASE_URL);
  url.searchParams.set("rtsNoAutoPointerLock", "1");
  return url.href;
})();
const CHROME = process.env.CHROME ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const consoleErrors = [];
const responseErrors = [];
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
  page.on("response", (response) => {
    const status = response.status();
    if (status < 400 || response.url().includes("favicon")) return;
    responseErrors.push(`${status}: ${response.url()}`);
  });

  await page.goto(TEST_URL, { waitUntil: "networkidle2", timeout: 15000 });
  await page.waitForSelector("#lobby-screen", { visible: true, timeout: 5000 });
  ok(true, "lobby screen visible on load");
  const discordBadge = await page.evaluate(() => {
    const badge = document.getElementById("discord-invite-badge");
    return {
      visible: !!badge && getComputedStyle(badge).display !== "none",
      href: badge?.href || "",
      target: badge?.target || "",
      label: badge?.getAttribute("aria-label") || "",
    };
  });
  ok(
    discordBadge.visible &&
      discordBadge.href === "https://discord.gg/v4jR3JbH" &&
      discordBadge.target === "_blank" &&
      /Join the Bewegungskrieg Discord server/.test(discordBadge.label),
    "pre-game shell exposes the Discord invite badge",
  );
  ok(await page.evaluate(() => !window.PIXI), "main thread does not load the Pixi runtime");
  ok(await page.evaluate(() => !!document.querySelector("#lobby-browser")),
    "pre-join lobby browser is visible on first paint");

  await page.click("#lobby-name", { clickCount: 3 });
  await page.type("#lobby-name", "Solo");
  await page.evaluate(() => {
    const room = "client-smoke-" + Date.now();
    window.__rts.lobby.elRoom.value = room;
    window.__rts.lobby._join();
  });
  await page.waitForFunction(() => document.querySelector("#lobby-players")?.children.length >= 1, { timeout: 5000 });
  ok(true, "joined room; lobby player list populated");
  await page.click("#lobby-name", { clickCount: 3 });
  await page.type("#lobby-name", "Renamed Solo");
  await page.waitForFunction(
    () => Array.from(document.querySelectorAll("#lobby-players .player-name"))
      .some((el) => el.textContent === "Renamed Solo"),
    { timeout: 5000 },
  );
  ok(true, "editing the joined lobby name updates the authoritative roster");
  const teamUi = await page.evaluate(() => {
    const rows = Array.from(document.querySelectorAll("#lobby-players .team-row"));
    const seat = document.querySelector("#lobby-players .lobby-seat");
    return {
      teamRows: rows.map((row) => row.textContent || ""),
      newTeamRows: rows.filter((row) => row.classList.contains("is-new-team")).length,
      draggableSeats: Array.from(document.querySelectorAll("#lobby-players .lobby-seat[draggable='true']")).length,
      hasModeSummary: !!document.querySelector("#lobby-mode-summary"),
      hasTeamMarks: !!document.querySelector("#lobby-players .lobby-team-mark"),
      hasLaunchCopy: /Launch|Ready check/.test(document.querySelector(".lobby-launch-panel")?.textContent || ""),
      mapSelectorInSummary: !!document.querySelector(".lobby-room #lobby-map-selector:not([hidden])"),
      hasNativeMapSelect: !!document.querySelector("select#lobby-map"),
      mapSelectorInSidePanel: !!document.querySelector(".lobby-form #lobby-map-selector"),
      hasSidebarAddAi: !!document.querySelector("#lobby-add-ai"),
      statusText: document.querySelector("#lobby-status")?.textContent || "",
      seatDisplay: seat ? getComputedStyle(seat).display : "",
      hasFactionControl: !!document.querySelector("#lobby-players .player-faction-select, #lobby-players .player-faction-label"),
      hasSeatMeta: !!document.querySelector("#lobby-players .lobby-seat-meta"),
      hasTeamCount: !!document.querySelector("#lobby-players .lobby-team-count"),
      shellColumns: getComputedStyle(document.querySelector(".lobby-shell")).gridTemplateColumns,
      lobbyChatVisible: getComputedStyle(document.querySelector("#lobby-chat-panel")).display === "grid",
      chatDocked: document.querySelector("#lobby-chat-dock > #chat-overlay") != null,
      mapPreviewBeforeDropdown:
        document.querySelector("#lobby-map-selector")?.children[0]?.classList.contains("lobby-map-preview") &&
        document.querySelector("#lobby-map-selector")?.children[1]?.classList.contains("lobby-map-control"),
    };
  });
  ok(teamUi.teamRows.some((text) => /Team/.test(text)) && teamUi.newTeamRows === 1,
    `lobby renders occupied teams plus one new-team row (${teamUi.teamRows.join(" | ")})`);
  ok(!teamUi.teamRows.some((text) => /Command group/.test(text)),
    "lobby team headers omit redundant command group copy");
  ok(!teamUi.teamRows.some((text) => /Allied command|Opposing command/.test(text)),
    "lobby team headers omit old Allied/Opposing command copy");
  ok(teamUi.draggableSeats >= 1, `host lobby seats are draggable (${teamUi.draggableSeats})`);
  ok(!teamUi.hasModeSummary && !teamUi.hasLaunchCopy && !teamUi.statusText,
    "lobby omits mode summary, launch header copy, and room/player status text");
  ok(!teamUi.mapSelectorInSummary && teamUi.mapSelectorInSidePanel && !teamUi.hasNativeMapSelect,
    "host custom map selector lives in the right-side match setup controls");
  ok(!teamUi.hasSidebarAddAi, "lobby keeps Add AI contextual to the team roster");
  ok(!teamUi.hasTeamMarks && teamUi.seatDisplay === "grid",
    `lobby teams have no color marks and player rows align with grid (${teamUi.seatDisplay})`);
  ok(!teamUi.hasFactionControl && !teamUi.hasSeatMeta && !teamUi.hasTeamCount,
    "dense roster omits faction controls, seat metadata, and team count badges");
  ok(teamUi.lobbyChatVisible && teamUi.chatDocked && teamUi.shellColumns.split(" ").length === 3,
    `joined desktop lobby uses roster, setup, and chat columns (${teamUi.shellColumns})`);
  ok(teamUi.mapPreviewBeforeDropdown,
    "map preview and creator credit render above the dropdown");

  await page.type("#chat-input", "Lobby ready");
  await page.click("#chat-send");
  await page.waitForFunction(
    () => Array.from(document.querySelectorAll("#chat-messages .chat-message"))
      .some((line) => line.textContent?.includes("Lobby ready")),
    { timeout: 5000 },
  );
  ok(true, "lobby chat sends through the room and renders in the docked panel");

  await page.click("#lobby-map-trigger");
  await page.hover('.lobby-map-option[data-map-name="Schone Tage"]');
  await page.waitForFunction(() => {
    const image = document.querySelector(".lobby-map-preview img");
    return image?.naturalWidth === 512 && image?.naturalHeight === 512;
  }, { timeout: 5000 });
  const mapPreview = await page.evaluate(() => ({
    name: document.querySelector(".lobby-map-preview figcaption strong")?.textContent || "",
    author: document.querySelector(".lobby-map-preview figcaption span")?.textContent || "",
    src: document.querySelector(".lobby-map-preview img")?.getAttribute("src") || "",
  }));
  ok(mapPreview.name === "Schone Tage" && mapPreview.author === "Created by oti"
    && mapPreview.src.endsWith("/assets/map-previews/schone-tage.jpg"),
  `map hover shows the authoritative preview and creator (${JSON.stringify(mapPreview)})`);
  await page.click('.lobby-map-option[data-map-name="Schone Tage"]');
  await page.waitForFunction(
    () => document.querySelector("#lobby-map-trigger")?.textContent?.includes("Schone Tage"),
    { timeout: 5000 },
  );
  ok(true, "custom map option updates through the authoritative lobby selection");
  await page.click("#lobby-map-trigger");
  await page.click('.lobby-map-option[data-map-name="Chokes"]');
  await page.waitForFunction(
    () => document.querySelector("#lobby-map-trigger")?.textContent?.includes("Chokes"),
    { timeout: 5000 },
  );

  await page.click("#lobby-ready");
  await page.waitForFunction(() => { const b = document.querySelector("#lobby-start"); return b && !b.disabled; }, { timeout: 5000 });
  ok(true, "Start enabled after readying up");
  await page.click("#lobby-start");
  await page.waitForFunction(() => { const g = document.getElementById("game-screen"); return g && !g.hidden; }, { timeout: 6000 });
  ok(true, "game screen shown after start");
  ok(
    await page.evaluate(() => getComputedStyle(document.getElementById("discord-invite-badge")).display === "none"),
    "Discord invite badge stays out of the game screen",
  );

  await page.waitForSelector("#viewport canvas", { timeout: 5000 });
  await page.waitForFunction(() => window.__rtsRenderWorkerStats?.backendInfo?.backend === "webgl", { timeout: 5000 });
  const workerBackend = await page.evaluate(() => window.__rtsRenderWorkerStats);
  ok(workerBackend.mode === "pixi-webgl-module-worker" && workerBackend.backendInfo.pixiVersion === "8.19.0",
    `sole Pixi module worker owns WebGL (v${workerBackend.backendInfo.pixiVersion})`);
  const pixiAnnulus = await page.evaluate(async () => {
    const [{ PIXI_WORKER_URL }, { drawFacingWedge }] = await Promise.all([
      import("/src/renderer/worker_environment.js"),
      import("/src/renderer/shared.js"),
    ]);
    const pixi = await import(PIXI_WORKER_URL);
    const canvas = document.createElement("canvas");
    const app = new pixi.Application();
    try {
      await app.init({
        canvas,
        width: 256,
        height: 256,
        preference: "webgl",
        autoStart: false,
        antialias: false,
        backgroundAlpha: 0,
      });
      const graphics = new pixi.Graphics();
      app.stage.addChild(graphics);
      drawFacingWedge(graphics, 128, 128, 90, 0, Math.PI * 2, 0x8eb7ff, 0.35, 0.7, 40);
      app.render();
      const gl = app.renderer.gl;
      const center = new Uint8Array(4);
      const band = new Uint8Array(4);
      gl.readPixels(128, 128, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, center);
      gl.readPixels(188, 128, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, band);
      return { rendered: true, centerAlpha: center[3], bandAlpha: band[3] };
    } catch (error) {
      return { rendered: false, error: error?.stack || error?.message || String(error) };
    } finally {
      app.destroy();
    }
  });
  ok(
    pixiAnnulus.rendered && pixiAnnulus.centerAlpha === 0 && pixiAnnulus.bandAlpha > 0,
    pixiAnnulus.rendered
      ? `Pixi full-circle mortar range preserves a transparent dead zone (center=${pixiAnnulus.centerAlpha}, band=${pixiAnnulus.bandAlpha})`
      : `Pixi full-circle mortar range renders without a worker-stopping geometry error (${pixiAnnulus.error})`,
  );
  const canvas = await page.evaluate(() => { const c = document.querySelector("#viewport canvas"); return c ? { w: c.width, h: c.height } : null; });
  ok(canvas && canvas.w > 0 && canvas.h > 0, `canvas mounted and sized (${canvas?.w}x${canvas?.h})`);

  await sleep(2500);
  const hud = await page.evaluate(() => ({
    m: document.getElementById("res-steel")?.textContent,
    s: document.getElementById("res-supply")?.textContent,
    gameTime: document.getElementById("game-timer")?.textContent,
    apm: document.getElementById("apm-counter")?.textContent,
  }));
  ok(parseInt(hud.m, 10) >= 50, `HUD shows steel (${hud.m})`);
  ok(/\d+\s*\/\s*\d+/.test(hud.s || ""), `HUD shows supply (${hud.s})`);
  ok(/^\d{2}:\d{2}$/.test(hud.gameTime || "") && hud.gameTime !== "00:00",
    `HUD game timer is visible and advancing (${hud.gameTime})`);
  ok(/^\d+$/.test(hud.apm || ""), `HUD APM counter is visible as a raw number (${hud.apm})`);

  const own = await page.evaluate(() => {
    const s = window.__rts.match.state, es = s.entitiesInterpolated(1).filter((e) => e.owner === s.playerId);
    return { resourceDepot: es.filter((e) => e.kind === "resource_depot").length, w: es.filter((e) => e.kind === "worker").length };
  });
  ok(own.resourceDepot === 1 && own.w === 6, `client sees own Resource Depot + 6 workers (resourceDepot=${own.resourceDepot}, workers=${own.w})`);

  await page.waitForFunction(() => {
    const wasm = window.__rtsPredictionDebug?.wasm;
    return wasm?.ready || wasm?.disabledReason;
  }, { timeout: 5000 }).catch(() => {});
  const predictionSmoke = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state;
    const wasm = window.__rtsPredictionDebug?.wasm || null;
    if (!wasm?.ready) return { ready: false, reason: wasm?.disabledReason || "not-ready" };
    const worker = s.entitiesInterpolated(1, { includePrediction: false })
      .find((e) => e.owner === s.playerId && e.kind === "worker");
    if (!worker) return { ready: true, worker: false };
    m.clientIntent.closeCommandCardMenu();
    s.setSelection([worker.id]);
    const before = { x: worker.x, y: worker.y };
    const issued = m.commandIssuer.issueCommand({
      c: "move",
      units: [worker.id],
      x: worker.x + 180,
      y: worker.y,
    });
    // Emulate three visual ticks without waiting long enough for the server echo.
    m.predictionAdapter.lastAdvanceAt -= 100;
    m.advancePredictionVisual();
    const predicted = s.entitiesInterpolated(1).find((e) => e.id === worker.id);
    const authoritative = s.entitiesInterpolated(1, { includePrediction: false }).find((e) => e.id === worker.id);
    return {
      ready: true,
      worker: true,
      issued,
      before,
      predicted: predicted ? { x: predicted.x, y: predicted.y } : null,
      authoritative: authoritative ? { x: authoritative.x, y: authoritative.y } : null,
      debug: window.__rtsPredictionDebug,
    };
  });
  ok(
    !predictionSmoke.ready || (
      predictionSmoke.worker &&
      predictionSmoke.issued?.predicted &&
      predictionSmoke.predicted?.x > predictionSmoke.before.x &&
      predictionSmoke.authoritative?.x === predictionSmoke.before.x
    ),
    predictionSmoke.ready
      ? `PREDICTION: owned move advances before authoritative echo (before=${predictionSmoke.before?.x}, predicted=${predictionSmoke.predicted?.x}, authoritative=${predictionSmoke.authoritative?.x}, issued=${JSON.stringify(predictionSmoke.issued)}, wasm=${JSON.stringify(predictionSmoke.debug?.wasm)})`
      : `PREDICTION: WASM adapter unavailable for smoke (${predictionSmoke.reason})`,
  );

  const predictionOffSmoke = await page.evaluate(() => {
    const app = window.__rts, m = app.match, s = m.state;
    app.setPredictionEnabled(false);
    const worker = s.entitiesInterpolated(1, { includePrediction: false })
      .find((e) => e.owner === s.playerId && e.kind === "worker");
    if (!worker) return { worker: false };
    const issued = m.commandIssuer.issueCommand({
      c: "move",
      units: [worker.id],
      x: worker.x,
      y: worker.y + 96,
    });
    return {
      worker: true,
      issued,
      enabled: m.prediction.enabled,
      pending: m.prediction.pendingCommandCount,
    };
  });
  ok(
    predictionOffSmoke.worker &&
      predictionOffSmoke.enabled === false &&
      predictionOffSmoke.issued?.sent &&
      Number.isInteger(predictionOffSmoke.issued?.clientSeq) &&
      predictionOffSmoke.issued.clientSeq > 0 &&
      predictionOffSmoke.issued?.predicted === false &&
      predictionOffSmoke.pending === 0,
    `PREDICTION OFF: command sends sequenced authoritative order (seq=${predictionOffSmoke.issued?.clientSeq}, pending=${predictionOffSmoke.pending})`,
  );

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
      m.commandIssuer.issueCommand({ c: "gather", units: [workers[i].id], node: steel[i].id });
    }
    return { workers: workers.length, nodes: steel.length, assigned: n };
  });
  ok(gather.assigned > 0, `assigned workers to steel (workers=${gather.workers}, nodes=${gather.nodes})`);
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("z");
  ok(
    await page.evaluate(() => window.__rts.match.clientIntent.commandCardMode === "workerBuild"),
    "worker build hotkey opened the build submenu",
  );
  await page.waitForSelector(
    '#command-card button[data-command-id="kriegsia.build.pump_jack"]',
    { timeout: 5000 },
  );
  const pumpJackSlot = await page.evaluate(() => {
    const button = document.querySelector('#command-card button[data-command-id="kriegsia.build.pump_jack"]');
    return {
      hasDepotButton: !!document.querySelector('#command-card button[data-command-id="kriegsia.build.depot"]'),
      hotkey: button?.dataset.hotkey || null,
      steelCost: button?.querySelector(".cmd-cost .c-steel")?.textContent || "",
      tooltip: button?.querySelector('.cmd-tooltip')?.textContent || "",
    };
  });
  ok(
    !pumpJackSlot.hasDepotButton &&
      pumpJackSlot.hotkey === "W" &&
      pumpJackSlot.steelCost === "100" &&
      pumpJackSlot.tooltip.includes("20s") &&
      pumpJackSlot.tooltip.includes("oil patch") &&
      pumpJackSlot.tooltip.includes("Extracts 2 Oil"),
    `BUILD: Pump Jack occupies W with 100 steel cost, build time, oil-patch placement, and extraction details (${JSON.stringify(pumpJackSlot)})`,
  );

  const trainBtn = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state;
    const resourceDepot = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "resource_depot");
    if (!resourceDepot) return false;
    m.clientIntent.closeCommandCardMenu();
    s.setSelection([resourceDepot.id]);
    m.hud.update();
    return !!document.querySelector('#command-card [data-hotkey="Q"]');
  });
  ok(trainBtn, "TRAIN CARD: selecting the Resource Depot shows a Worker train button");
  await page.waitForFunction(() => {
    const state = window.__rts?.match?.state;
    const button = document.querySelector('#command-card button[data-hotkey="Q"]');
    return state?.resources?.steel >= 50 &&
      button &&
      !button.disabled &&
      !button.classList.contains("unaffordable");
  }, { timeout: 10000 });
  await page.click('#command-card button[data-hotkey="Q"]');
  await page.waitForFunction(() => {
    const s = window.__rts.match.state;
    const resourceDepot = s.entityById([...s.selection][0]);
    return resourceDepot?.prodQueue > 0 && resourceDepot.prodProgress >= 0;
  }, { timeout: 6000 });
  const productionProgress = await page.evaluate(async () => {
    const match = window.__rts.match;
    const state = match.state;
    const id = [...state.selection][0];
    const before = state.entityById(id)?.prodProgress ?? 0;
    match.net.off("snapshot", match.onSnapshot);
    await new Promise((resolve) => setTimeout(resolve, 300));
    const after = state.entityById(id)?.prodProgress ?? 0;
    match.net.on("snapshot", match.onSnapshot);
    return {
      before,
      after,
      predicted: state.entityById(id)?.progressPredicted === true,
      queue: state.entityById(id)?.prodQueue ?? 0,
    };
  });
  ok(
    productionProgress.queue > 0 &&
      productionProgress.predicted &&
      productionProgress.after > productionProgress.before,
    `PRODUCTION PROGRESS: selected train bar advances during snapshot gap (before=${productionProgress.before}, after=${productionProgress.after}, predicted=${productionProgress.predicted})`,
  );

  await page.keyboard.down("Tab");
  await page.waitForFunction(() => !document.getElementById("tab-menu")?.hidden, { timeout: 2000 });
  const initialAutoBuild = await page.evaluate(() => ({
    authoritative: window.__rts.match.state.autoBuild,
    menu: window.__rts.match.tabMenu.status(),
  }));
  ok(
    initialAutoBuild.authoritative?.reserveSteel === 0 &&
      initialAutoBuild.authoritative?.reserveOil === 0 &&
      initialAutoBuild.menu.reservations.steel === 0 &&
      initialAutoBuild.menu.reservations.oil === 0,
    `TAB MENU: new players start with zero resource floors (${JSON.stringify(initialAutoBuild)})`,
  );
  await page.keyboard.press("2");
  await page.keyboard.press("3");
  await page.keyboard.press("4");
  await page.keyboard.press("4");
  await page.keyboard.press("q");
  await page.evaluate(() => window.__rts.hotkeyProfiles.setActiveProfile("preset.classicRts"));
  await page.keyboard.press("a");
  await page.waitForFunction(
    () => {
      const settings = window.__rts.match.state.autoBuild;
      return settings?.paused === false &&
        settings.reserveSteel === 50 &&
        settings.reserveOil === 100;
    },
    { timeout: 2000 },
  );
  const tabMenuPrototype = await page.evaluate(() => ({
    ...window.__rts.match.tabMenu.status(),
    repeatConsumed: !window.dispatchEvent(new KeyboardEvent("keydown", {
      code: "Tab",
      repeat: true,
      bubbles: true,
      cancelable: true,
    })),
  }));
  ok(
    tabMenuPrototype.visible &&
      tabMenuPrototype.repeatConsumed &&
      !tabMenuPrototype.paused &&
      tabMenuPrototype.pauseHotkey === "A" &&
      tabMenuPrototype.reservations.steel === 50 &&
      tabMenuPrototype.reservations.oil === 100,
    `TAB MENU: held Tab sends authoritative Grid/Classic pause and reserve hotkeys (${JSON.stringify(tabMenuPrototype)})`,
  );
  await page.evaluate(() => {
    window.__rts.hotkeyProfiles.setActiveProfile("preset.grid");
    window.__rts.match.tabMenu.render();
  });
  await page.keyboard.up("Tab");
  await page.waitForFunction(() => document.getElementById("tab-menu")?.hidden, { timeout: 2000 });

  const tabMenuButton = await page.$("#tab-menu-button");
  const tabMenuButtonBox = await tabMenuButton?.boundingBox();
  if (!tabMenuButtonBox) throw new Error("Auto-Build menu button is unavailable for pointer hold coverage.");
  await page.mouse.move(
    tabMenuButtonBox.x + tabMenuButtonBox.width / 2,
    tabMenuButtonBox.y + tabMenuButtonBox.height / 2,
  );
  await page.mouse.down();
  await page.waitForFunction(() => !document.getElementById("tab-menu")?.hidden, { timeout: 2000 });
  await page.mouse.up();
  await page.waitForFunction(
    () => document.getElementById("tab-menu")?.hidden && document.getElementById("settings-menu")?.hidden,
    { timeout: 2000 },
  );

  const separatedMenuControls = await page.evaluate(() => {
    const hamburger = document.getElementById("tab-menu-button");
    const settings = document.getElementById("settings-button");
    const hamburgerBox = hamburger?.getBoundingClientRect();
    const settingsBox = settings?.getBoundingClientRect();
    return {
      hamburgerLabel: hamburger?.getAttribute("aria-label"),
      settingsLabel: settings?.getAttribute("aria-label"),
      settingsText: settings?.textContent,
      hamburgerLeft: hamburgerBox?.left,
      settingsLeft: settingsBox?.left,
      settingsTop: settingsBox?.top,
    };
  });
  ok(
    separatedMenuControls.hamburgerLabel === "Hold for Auto-Build menu" &&
      separatedMenuControls.settingsLabel === "Settings" &&
      separatedMenuControls.settingsText === "⚙" &&
      separatedMenuControls.hamburgerLeft < separatedMenuControls.settingsLeft &&
      separatedMenuControls.settingsTop > 100,
    `MENU CONTROLS: Auto-Build hamburger and bottom-right Settings gear stay distinct (${JSON.stringify(separatedMenuControls)})`,
  );

  await page.click("#settings-button");
  await page.waitForFunction(() => !document.getElementById("settings-menu")?.hidden, { timeout: 2000 });
  await page.click('[data-settings-tab="hotkeys"]');
  await page.click("#hotkey-clone-profile");
  await page.click('#hotkey-command-card-preview [data-command-id="unit.move"]');
  await page.keyboard.press("b");
  await page.waitForFunction(() => {
    const save = document.getElementById("hotkey-save-profile");
    return save && !save.disabled;
  }, { timeout: 2000 });
  await page.click("#hotkey-save-profile");
  await page.waitForFunction(() => window.__rts?.hotkeyProfiles?.getActiveProfile?.()?.bindings?.["unit.move"] === "KeyB", { timeout: 2000 });
  ok(true, "HOTKEYS: settings editor saved a changed physical Move binding");
  await page.keyboard.press("Escape");
  await sleep(100);
  const afterMenuEscape = await page.evaluate(() => ({
    menuHidden: document.getElementById("settings-menu")?.hidden,
    selected: window.__rts.match.state.selection.size,
  }));
  ok(afterMenuEscape.menuHidden && afterMenuEscape.selected === 1,
     `ESCAPE: closes open settings menu without clearing selection (hidden=${afterMenuEscape.menuHidden}, selected=${afterMenuEscape.selected})`);

  const changedHotkey = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state;
    const worker = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "worker");
    if (!worker) return { worker: false, hotkey: null, target: null };
    m.clientIntent.closeCommandCardMenu();
    s.setSelection([worker.id]);
    m.hud.update();
    return {
      worker: true,
      hotkey: document.querySelector('#command-card [data-command-id="unit.move"]')?.dataset.hotkey || null,
      target: m.clientIntent.commandTarget,
    };
  });
  ok(changedHotkey.worker && changedHotkey.hotkey === "B",
    `HOTKEYS: live command card shows changed Move binding (${changedHotkey.hotkey})`);
  await page.keyboard.press("b");
  await sleep(150);
  ok(await page.evaluate(() => window.__rts.match.clientIntent.commandTarget === "move"),
    "HOTKEYS: changed Move binding activates the live command card");
  await page.keyboard.press("Escape");
  await sleep(100);
  ok(await page.evaluate(() => window.__rts.match.clientIntent.commandTarget == null && window.__rts.match.state.selection.size === 1),
    "HOTKEYS: Escape cancels changed-key Move targeting before gameplay cancel");

  await page.keyboard.press("Escape");
  await sleep(100);
  const afterGameplayEscape = await page.evaluate(() => ({
    menuHidden: document.getElementById("settings-menu")?.hidden,
    selected: window.__rts.match.state.selection.size,
    commandCardHidden: document.getElementById("command-card")?.hidden,
    commandSlots: document.querySelectorAll("#command-card .cmd-empty").length,
    commandButtons: document.querySelectorAll("#command-card button").length,
  }));
  ok(afterGameplayEscape.menuHidden && afterGameplayEscape.selected === 0,
     `ESCAPE: gameplay cancel clears selection without opening settings (hidden=${afterGameplayEscape.menuHidden}, selected=${afterGameplayEscape.selected})`);
  ok(!afterGameplayEscape.commandCardHidden && afterGameplayEscape.commandSlots === 9 && afterGameplayEscape.commandButtons === 0,
     `COMMAND CARD: empty selection keeps an inert 3x3 card (hidden=${afterGameplayEscape.commandCardHidden}, slots=${afterGameplayEscape.commandSlots}, buttons=${afterGameplayEscape.commandButtons})`);

  const beforePan = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state;
    const resourceDepot = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "resource_depot");
    if (resourceDepot) {
      m.clientIntent.closeCommandCardMenu();
      s.setSelection([resourceDepot.id]);
    }
    return {
      x: window.__rts.match.camera.x,
      y: window.__rts.match.camera.y,
      selected: s.selection.size,
    };
  });
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

  const editorPage = await browser.newPage();
  editorPage.on("console", (m) => { if (m.type() === "error") consoleErrors.push(m.text()); });
  editorPage.on("pageerror", (e) => pageErrors.push(e.message));
  editorPage.on("requestfailed", (r) => { if (!r.url().includes("favicon")) consoleErrors.push("requestfailed: " + r.url()); });
  editorPage.on("response", (response) => {
    const status = response.status();
    if (status >= 400 && !response.url().includes("favicon")) responseErrors.push(`${status}: ${response.url()}`);
  });
  await editorPage.setViewport({ width: 780, height: 600 });
  const editorUrl = new URL(BASE_URL);
  editorUrl.pathname = "/map-editor";
  editorUrl.search = "";
  await editorPage.goto(editorUrl.href, { waitUntil: "domcontentloaded", timeout: 15000 });
  await editorPage.waitForFunction(() => document.querySelectorAll(".map-editor-terrain-icon").length === 18, { timeout: 5000 });
  await editorPage.waitForFunction(() => window.__rtsRenderWorkerStats?.surface === "mapEditor"
    && window.__rtsRenderWorkerStats?.backendInfo?.backend === "webgl", { timeout: 5000 });
  const editorUi = await editorPage.evaluate(() => {
    const optionsWindow = document.querySelector(".map-editor-options-window");
    const toolsWindow = document.querySelector(".map-editor-tools-window");
    const layersWindow = document.querySelector(".map-editor-layers-window");
    const panel = toolsWindow?.querySelector(".map-editor-panel-body");
    const water = document.querySelector(".map-editor-terrain-button[data-terrain=water]");
    const optionsRect = optionsWindow?.getBoundingClientRect();
    const panelRect = toolsWindow?.getBoundingClientRect();
    const layersRect = layersWindow?.getBoundingClientRect();
    const layersMoveHandle = layersWindow?.querySelector(".lab-panel-drag-handle");
    layersMoveHandle?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    const movedLayersRect = layersWindow?.getBoundingClientRect();
    const noInitialStatus = document.querySelector(".map-editor-status") === null;
    water?.scrollIntoView({ block: "center" });
    const beforeScrollTop = panel?.scrollTop ?? -1;
    water?.click();
    const refreshedPanel = document.querySelector(".map-editor-tools-window .map-editor-panel-body");
    const floatingChrome = [
      [optionsWindow, "map editor options"],
      [layersWindow, "map editor layers"],
      [toolsWindow, "map editor tools"],
    ].every(([panelWindow, label]) =>
      panelWindow?.querySelector(".lab-panel-drag-handle")?.getAttribute("aria-label") === `Move ${label} panel`
      && panelWindow?.querySelector(".lab-panel-resize-handle")?.getAttribute("aria-label") === `Resize ${label} panel`
      && Boolean(panelWindow?.querySelector(".lab-panel-collapse")));
    return {
      beforeScrollTop,
      afterScrollTop: refreshedPanel?.scrollTop ?? -1,
      maxScroll: (refreshedPanel?.scrollHeight ?? 0) - (refreshedPanel?.clientHeight ?? 0),
      terrainPreviews: [...document.querySelectorAll(".map-editor-terrain-icon")]
        .map((icon) => ({ width: icon.width, height: icon.height })),
      headers: [...document.querySelectorAll(".map-editor-header")]
        .map((header) => header.textContent?.trim() || ""),
      floatingChrome,
      noInitialStatus,
      panelsDoNotOverlap: [optionsRect, layersRect, panelRect].every(Boolean) && [
        [optionsRect, layersRect],
        [optionsRect, panelRect],
        [layersRect, panelRect],
      ].every(([a, b]) => a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top),
      withinViewport: [optionsRect, layersRect, panelRect].every((rect) => rect &&
        rect.left >= 8 && rect.right <= window.innerWidth - 8 &&
        rect.top >= 8 && rect.bottom <= window.innerHeight - 8),
      noHorizontalOverflow: [...document.querySelectorAll(".map-editor-palette, .map-editor-player-picker")]
        .every((node) => node.scrollWidth <= node.clientWidth),
      actionButtons: [...document.querySelectorAll(".map-editor-options-window button")]
        .map((control) => control.textContent),
      zoom: (() => {
        const section = [...document.querySelectorAll(".map-editor-group")]
          .find((node) => node.querySelector("legend")?.textContent === "Zoom");
        const input = section?.querySelector("input[aria-label='Zoom percentage']");
        return section && input && {
          firstSection: section === document.querySelector(".map-editor-tools-window .map-editor-group"),
          buttons: [...section.querySelectorAll("button")].map((control) => control.textContent),
          min: input.min,
          max: input.max,
          value: Number(input.value),
        };
      })(),
      layers: [...document.querySelectorAll(".map-editor-layer-toggle")].map((label) => ({
        label: label.querySelector("span")?.textContent || "",
        description: label.querySelector("small")?.textContent || "",
        title: label.title,
        checked: !!label.querySelector("input[type=checkbox]")?.checked,
      })),
      layerPanel: (() => {
        const list = layersWindow?.querySelector(".map-editor-layer-list");
        const toggles = [...list?.querySelectorAll(".map-editor-layer-toggle") || []];
        return layersWindow && list && layersRect && {
          outsideTools: !toolsWindow?.contains(list),
          columns: getComputedStyle(list).gridTemplateColumns.split(" ").length,
          height: layersRect.height,
          maxToggleHeight: Math.max(...toggles.map((toggle) => toggle.getBoundingClientRect().height)),
          movePreservedSize: Math.abs(movedLayersRect.left - layersRect.left - 24) <= 1 &&
            Math.abs(movedLayersRect.width - layersRect.width) <= 1 &&
            Math.abs(movedLayersRect.height - layersRect.height) <= 1,
        };
      })(),
      overlayTools: (() => {
        const section = [...document.querySelectorAll(".map-editor-group")]
          .find((node) => node.querySelector("legend")?.textContent === "Gameplay overlays");
        return [...section?.querySelectorAll("button") || []].map((button) => button.textContent?.trim() || "");
      })(),
      symmetryTitle: document.querySelector("select[aria-label=Symmetry]")?.title || "",
      symmetryOptions: [...document.querySelector("select[aria-label=Symmetry]")?.options || []]
        .map((option) => option.textContent),
      doodadToolLabels: [...document.querySelectorAll(".map-editor-palette button")]
        .map((button) => button.textContent?.trim() || "")
        .filter((label) => ["Place", "Spray", "Erase", "Remove doodads", "Erase brush", "Delete selection", "Select / move"].includes(label)),
      blankMapWidth: (() => {
        const input = document.querySelector("input[aria-label='Map width']");
        return input && {
          type: input.type,
          value: input.value,
          min: input.min,
          max: input.max,
          width: input.getBoundingClientRect().width,
        };
      })(),
      blankMapHeight: (() => {
        const input = document.querySelector("input[aria-label='Map height']");
        return input && {
          type: input.type,
          value: input.value,
          min: input.min,
          max: input.max,
          width: input.getBoundingClientRect().width,
        };
      })(),
      clearanceSection: [...document.querySelectorAll(".map-editor-readout")]
        .find((node) => node.textContent === "Bases and starts reserve a passable grass area.")
        ?.closest("fieldset")?.querySelector("legend")?.textContent || "",
      hasRecipeTextarea: Boolean(document.querySelector("textarea[aria-label='Map recipe JSON']")),
    };
  });
  ok(
    editorUi.headers.some((header) => header.includes("Options")) &&
      editorUi.headers.some((header) => header.includes("Layers")) &&
      editorUi.headers.some((header) => header.includes("Tools")) &&
      editorUi.noInitialStatus &&
      editorUi.terrainPreviews.length === 18 &&
      editorUi.terrainPreviews.every((preview) => preview.width > 0 && preview.height > 0),
    `MAP EDITOR: separate Options/Layers/Tools panels omit initial status slop and show all 18 terrain previews (headers=${editorUi.headers.join("/")}, previews=${editorUi.terrainPreviews.length})`,
  );
  ok(
    editorUi.floatingChrome && editorUi.panelsDoNotOverlap && editorUi.withinViewport && editorUi.noHorizontalOverflow,
    "MAP EDITOR: three accessible floating panels do not overlap and terrain/start-base pickers stay within the viewport",
  );
  ok(
    editorUi.zoom?.firstSection &&
      ["Fill screen", "Fit to screen", "−", "+"].every((label) => editorUi.zoom.buttons.includes(label)) &&
      editorUi.zoom.min === "5" && editorUi.zoom.max === "400" && editorUi.zoom.value > 0,
    `MAP EDITOR: top Tools section exposes bounded framing, step, and percentage zoom controls (${JSON.stringify(editorUi.zoom)})`,
  );
  ok(
    editorUi.layers.length === 8 && editorUi.layers.every((layer) => layer.checked && layer.description) &&
      editorUi.layers.every((layer) => layer.title === `${layer.label} — ${layer.description}`) &&
      editorUi.layerPanel?.outsideTools && editorUi.layerPanel.columns === 2 &&
      editorUi.layerPanel.height < 180 && editorUi.layerPanel.maxToggleHeight < 32 &&
      editorUi.layerPanel.movePreservedSize &&
      ["Terrain & bases", "Stealth", "No vehicles", "Damage reduction", "Slowed movement", "Trees", "Gameplay doodads", "Decorative doodads"]
        .every((label) => editorUi.layers.some((layer) => layer.label === label)) &&
      ["Paint stealth", "Paint no vehicles", "Paint damage reduction", "Paint slowed movement", "Erase stealth", "Erase no vehicles", "Erase damage reduction", "Erase slowed movement"]
        .every((label) => editorUi.overlayTools.includes(label)) &&
      !editorUi.overlayTools.includes("Forest") && !editorUi.overlayTools.includes("Erase both"),
    `MAP EDITOR: compact floating Layers panel exposes eight independent visibility toggles (${JSON.stringify(editorUi.layerPanel)})`,
  );
  await editorPage.click("input[aria-label='Show Stealth']");
  await editorPage.waitForFunction(() => window.__mapEditor?.viewport?.layerVisibilitySnapshot?.().stealth === false);
  await editorPage.click("input[aria-label='Show Stealth']");
  await editorPage.waitForFunction(() => window.__mapEditor?.viewport?.layerVisibilitySnapshot?.().stealth === true);
  ok(true, "MAP EDITOR: layer checkbox changes reach the live worker presentation path");
  ok(
    editorUi.actionButtons.includes("Load map JSON") &&
      editorUi.actionButtons.includes("Export map JSON") &&
      editorUi.actionButtons.includes("Authoritative check") &&
      editorUi.actionButtons.includes("Route report") &&
      !editorUi.actionButtons.includes("Apply recipe JSON") &&
      !editorUi.hasRecipeTextarea &&
      !editorUi.actionButtons.includes("Save on this device") &&
      !editorUi.actionButtons.includes("Load saved map"),
    `MAP EDITOR: materialized map JSON actions omit agent recipe controls (${editorUi.actionButtons.join("/")})`,
  );
  ok(
    editorUi.maxScroll > 0 && editorUi.beforeScrollTop > 0 && editorUi.beforeScrollTop === editorUi.afterScrollTop,
    `MAP EDITOR: selecting terrain keeps sidebar scroll position (${editorUi.beforeScrollTop} -> ${editorUi.afterScrollTop})`,
  );
  ok(
    editorUi.symmetryTitle === "Symmetry applies to terrain and base moves." &&
      editorUi.symmetryOptions.includes("Half-turn (180°)") &&
      editorUi.symmetryOptions.includes("3-way rotation (120°, square-grid approximation)") &&
      editorUi.symmetryOptions.includes("Radial (4-way)") &&
      editorUi.symmetryOptions.includes("Diagonal ↘ (top-left ↔ bottom-right)") &&
      editorUi.symmetryOptions.includes("Diagonal ↙ (top-right ↔ bottom-left)") &&
      editorUi.blankMapWidth?.type === "number" &&
      editorUi.blankMapWidth.value === "126" &&
      editorUi.blankMapWidth.min === "16" &&
      editorUi.blankMapWidth.max === "256" &&
      editorUi.blankMapWidth.width <= 80 &&
      editorUi.blankMapHeight?.type === "number" &&
      editorUi.blankMapHeight.value === "126" &&
      editorUi.blankMapHeight.min === "16" &&
      editorUi.blankMapHeight.max === "256" &&
      editorUi.blankMapHeight.width <= 80 &&
      editorUi.clearanceSection === "Start and base locations",
    "MAP EDITOR: symmetry, independent blank-map dimensions, and grass-clearance controls are presented correctly",
  );
  ok(
    editorUi.doodadToolLabels.includes("Place") &&
      editorUi.doodadToolLabels.includes("Spray") &&
      editorUi.doodadToolLabels.includes("Erase") &&
      !editorUi.doodadToolLabels.some((label) => ["Remove doodads", "Erase brush", "Delete selection", "Select / move"].includes(label)),
    `MAP EDITOR: doodad tools expose only place, spray, and erase (${editorUi.doodadToolLabels.join(", ")})`,
  );
  await editorPage.setViewport({ width: 600, height: 360 });
  const mobileLayers = await editorPage.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => {
    const body = document.querySelector(".map-editor-layers-body");
    const lastToggle = body?.querySelector(".map-editor-layer-toggle:last-child");
    if (body) body.scrollTop = body.scrollHeight;
    const bodyRect = body?.getBoundingClientRect();
    const lastRect = lastToggle?.getBoundingClientRect();
    resolve({
      scrollable: body && body.scrollHeight > body.clientHeight && getComputedStyle(body).overflowY === "auto",
      lastToggleReachable: bodyRect && lastRect && lastRect.bottom <= bodyRect.bottom + 1,
    });
  })));
  ok(
    mobileLayers.scrollable && mobileLayers.lastToggleReachable,
    `MAP EDITOR: short mobile Layers panel scrolls to every toggle (${JSON.stringify(mobileLayers)})`,
  );
  await editorPage.close();

  ok(pageErrors.length === 0, `no uncaught page errors (${pageErrors.length})`);
  ok(consoleErrors.length === 0, `no console errors (${consoleErrors.length})`);
  ok(responseErrors.length === 0, `no HTTP error responses (${responseErrors.length})`);
  if (pageErrors.length) console.log("  -- pageErrors:\n" + pageErrors.map((e) => "     " + e).join("\n"));
  if (consoleErrors.length) console.log("  -- consoleErrors:\n" + consoleErrors.slice(0, 12).map((e) => "     " + e).join("\n"));
  if (responseErrors.length) console.log("  -- responseErrors:\n" + responseErrors.slice(0, 12).map((e) => "     " + e).join("\n"));
} finally {
  await browser.close();
}
if (failures > 0) console.log(`\nCLIENT SMOKE: ${failures} FAILURE(S) ❌`);
process.exit(failures === 0 ? 0 : 1);
