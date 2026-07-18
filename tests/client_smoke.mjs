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
  ok(await page.evaluate(() => !!window.PIXI), `PixiJS loaded (v${await page.evaluate(() => window.PIXI?.VERSION)})`);
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
      mapSelectInSummary: !!document.querySelector(".lobby-status-grid #lobby-map:not([hidden])"),
      mapSelectInSidePanel: !!document.querySelector(".lobby-form #lobby-map"),
      hasSidebarAddAi: !!document.querySelector("#lobby-add-ai"),
      statusText: document.querySelector("#lobby-status")?.textContent || "",
      seatDisplay: seat ? getComputedStyle(seat).display : "",
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
  ok(teamUi.mapSelectInSummary && !teamUi.mapSelectInSidePanel,
    "host map selector renders in the summary row instead of the setup panel");
  ok(!teamUi.hasSidebarAddAi, "lobby keeps Add AI contextual to the team roster");
  ok(!teamUi.hasTeamMarks && teamUi.seatDisplay === "grid",
    `lobby teams have no color marks and player rows align with grid (${teamUi.seatDisplay})`);

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
    return { cityCentre: es.filter((e) => e.kind === "city_centre").length, w: es.filter((e) => e.kind === "worker").length };
  });
  ok(own.cityCentre === 1 && own.w === 6, `client sees own City Centre + 6 workers (cityCentre=${own.cityCentre}, workers=${own.w})`);

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
      ? `PREDICTION: owned move advances before authoritative echo (before=${predictionSmoke.before?.x}, predicted=${predictionSmoke.predicted?.x}, authoritative=${predictionSmoke.authoritative?.x})`
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
  await sleep(150);
  ok(
    await page.evaluate(() => window.__rts.match.clientIntent.commandCardMode === "workerBuild"),
    "worker build hotkey opened the build submenu",
  );
  const pumpJackSlot = await page.evaluate(() => {
    const button = document.querySelector('#command-card button[data-command-id="kriegsia.build.pump_jack"]');
    return {
      hasDepotButton: !!document.querySelector('#command-card button[data-command-id="kriegsia.build.depot"]'),
      hotkey: button?.dataset.hotkey || null,
      tooltip: button?.querySelector('.cmd-tooltip')?.textContent || "",
    };
  });
  ok(
    !pumpJackSlot.hasDepotButton &&
      pumpJackSlot.hotkey === "W" &&
      pumpJackSlot.tooltip.includes("50") &&
      pumpJackSlot.tooltip.includes("20s") &&
      pumpJackSlot.tooltip.includes("oil patch") &&
      pumpJackSlot.tooltip.includes("Extracts 2 Oil"),
    "BUILD: Pump Jack occupies W with cost, build time, oil-patch placement, and extraction details",
  );

  const trainBtn = await page.evaluate(() => {
    const m = window.__rts.match, s = m.state;
    const cityCentre = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "city_centre");
    if (!cityCentre) return false;
    m.clientIntent.closeCommandCardMenu();
    s.setSelection([cityCentre.id]);
    m.hud.update();
    return !!document.querySelector('#command-card [data-hotkey="Q"]');
  });
  ok(trainBtn, "TRAIN CARD: selecting the City Centre shows a Worker train button");
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
    const cityCentre = s.entityById([...s.selection][0]);
    return cityCentre?.prodQueue > 0 && cityCentre.prodProgress >= 0;
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
    const cityCentre = s.entitiesInterpolated(1).find((e) => e.owner === s.playerId && e.kind === "city_centre");
    if (cityCentre) {
      m.clientIntent.closeCommandCardMenu();
      s.setSelection([cityCentre.id]);
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
  await editorPage.setViewport({ width: 1280, height: 600 });
  const editorUrl = new URL(BASE_URL);
  editorUrl.pathname = "/map-editor";
  editorUrl.search = "";
  await editorPage.goto(editorUrl.href, { waitUntil: "domcontentloaded", timeout: 15000 });
  await editorPage.waitForFunction(() => document.querySelectorAll(".map-editor-terrain-icon").length === 8, { timeout: 5000 });
  const editorUi = await editorPage.evaluate(() => {
    const panelWindow = document.querySelector(".map-editor-panel");
    const panel = document.querySelector(".map-editor-panel-body");
    const water = document.querySelector(".map-editor-terrain-button[data-terrain=water]");
    const dragHandle = panelWindow?.querySelector(".lab-panel-drag-handle");
    const resizeHandle = panelWindow?.querySelector(".lab-panel-resize-handle");
    const collapseButton = panelWindow?.querySelector(".lab-panel-collapse");
    const panelRect = panelWindow?.getBoundingClientRect();
    water?.scrollIntoView({ block: "center" });
    const beforeScrollTop = panel?.scrollTop ?? -1;
    water?.click();
    const refreshedPanel = document.querySelector(".map-editor-panel-body");
    return {
      beforeScrollTop,
      afterScrollTop: refreshedPanel?.scrollTop ?? -1,
      maxScroll: (refreshedPanel?.scrollHeight ?? 0) - (refreshedPanel?.clientHeight ?? 0),
      terrainPreviews: [...document.querySelectorAll(".map-editor-terrain-icon")]
        .map((icon) => ({ width: icon.width, height: icon.height })),
      header: document.querySelector(".map-editor-header")?.textContent?.trim() || "",
      floatingChrome: dragHandle?.getAttribute("aria-label") === "Move map editor panel" &&
        resizeHandle?.getAttribute("aria-label") === "Resize map editor panel" &&
        Boolean(collapseButton),
      withinViewport: panelRect && panelRect.bottom <= window.innerHeight - 11,
      noHorizontalOverflow: [...document.querySelectorAll(".map-editor-palette, .map-editor-player-picker")]
        .every((node) => node.scrollWidth <= node.clientWidth),
      symmetryTitle: document.querySelector("select[aria-label=Symmetry]")?.title || "",
      symmetryOptions: [...document.querySelector("select[aria-label=Symmetry]")?.options || []]
        .map((option) => option.textContent),
      blankMapSize: (() => {
        const input = document.querySelector("input[aria-label='Blank map size']");
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
    };
  });
  ok(
    editorUi.header.includes("Map Editor") &&
      editorUi.terrainPreviews.length === 8 &&
      editorUi.terrainPreviews.every((preview) => preview.width > 0 && preview.height > 0),
    `MAP EDITOR: terrain buttons show eight rendered terrain previews (header=${editorUi.header}, previews=${editorUi.terrainPreviews.length})`,
  );
  ok(
    editorUi.floatingChrome && editorUi.withinViewport && editorUi.noHorizontalOverflow,
    "MAP EDITOR: accessible floating chrome and terrain/start-base pickers stay within the viewport",
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
      editorUi.blankMapSize?.type === "number" &&
      editorUi.blankMapSize.value === "126" &&
      editorUi.blankMapSize.min === "16" &&
      editorUi.blankMapSize.max === "166" &&
      editorUi.blankMapSize.width <= 80 &&
      editorUi.clearanceSection === "Start and base locations",
    "MAP EDITOR: symmetry, custom blank-map size, and grass-clearance controls are presented correctly",
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
