// tests/client_contracts/match_shell_contracts.mjs
// Match shell collaborator contracts imported by ../client_contracts.mjs.

import { assert, assertApprox } from "./assertions.mjs";
import { withFakeSettingsDocument } from "./fakes.mjs";
import { MatchCombatAudio } from "../../client/src/match_combat_audio.js";
import {
  MatchNetReporter,
  cursorRuntimeReportFields,
  predictionReportFields,
} from "../../client/src/match_net_reporter.js";
import { buildMatchSettingsContext } from "../../client/src/match_settings_context.js";
import { pointerLockDiagnosticToast } from "../../client/src/match_pointer_lock_diagnostics.js";
import {
  EVENT,
  KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  WEAPON_KIND,
} from "../../client/src/protocol.js";

// Windows desktop runtime diagnostics
// ---------------------------------------------------------------------------
{
  const root = {
    __RTS_DESKTOP_RUNTIME: {
      shell: "tauri",
      platform: "windows",
      nativeCursorBackend: false,
      nativeCursorCapture: false,
      pointerLockDisabled: false,
    },
    __TAURI_INTERNALS__: { invoke() {} },
    __TAURI__: { core: { invoke() {} } },
  };
  const fields = cursorRuntimeReportFields(root);
  assert(fields.desktopRuntimePresent, "Windows net reports retain the desktop runtime flag");
  assert(!fields.nativeCursorBridgePresent, "Windows net reports keep the macOS cursor bridge absent");
  assert(!fields.nativeCursorSupported, "Windows net reports do not claim macOS native cursor support");
  assert(!fields.nativeCursorActive, "Windows net reports do not claim native cursor capture is active");
  assert(fields.tauriInternalsPresent, "Windows net reports retain Tauri internals diagnostics");
  assert(fields.tauriGlobalPresent, "Windows net reports retain the Tauri global diagnostic");
  assert(
    pointerLockDiagnosticToast({
      error: { name: "Error", message: "Pointer Lock request timeout without locking the target." },
      support: {
        desktopRuntime: root.__RTS_DESKTOP_RUNTIME,
        nativeCursor: { supported: false, backend: null },
      },
    }).includes("request timeout"),
    "Windows browser-lock diagnostics report the browser failure instead of claiming the native bridge is missing",
  );
}

// Match net-report/ping collaborator
// ---------------------------------------------------------------------------
{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorPerformance = globalThis.performance;
  const priorClearInterval = globalThis.clearInterval;
  const intervals = [];
  const cleared = [];
  let now = 1200;
  globalThis.window = {
    setInterval(handler, ms) {
      const id = intervals.length + 1;
      intervals.push({ id, handler, ms });
      return id;
    },
  };
  globalThis.clearInterval = (id) => cleared.push(id);
  globalThis.document = {
    hidden: true,
    hasFocus() { return false; },
  };
  globalThis.performance = { now: () => now };
  try {
    const pings = [];
    const reports = [];
    const diagnostics = [];
    let resetStats = 0;
    let resetFrame = 0;
    let resetSnapshot = 0;
    const net = {
      bufferedAmount: 456,
      ping() { pings.push("ping"); },
      netReport(report) { reports.push(report); },
      consumeSnapshotReportStats() {
        return {
          snapshotBytesTotal: 1024,
          snapshotBytesMax: 768,
          snapshotMessageCount: 2,
        };
      },
    };
    const health = {
      reportStartedAt: 1000,
      reportStats: {
        rttMaxMs: 31,
        badRttSamples: 1,
        snapshotGapMaxMs: 44,
        jitterSamples: 2,
        snapshots: 3,
        snapshotLateFrameCount: 1,
        predictedSnapshotLateFrameCount: 1,
        predictionActiveLateFrameCount: 1,
        commandBurstBucketMs: 250,
        commandBurstMax: 3,
        commandBurstFrameGapMaxMs: 24,
        commandBurstWorstFramePhase: "match.input",
        commandBurstWorstFramePhaseMs: 9,
        frameGapMaxMs: 25,
        frameCount: 2,
        frameTotalMs: 40,
      },
      metrics() {
        return {
          latencyMs: 28,
          jitterMs: 9,
          serverTickMs: 33,
          serverLagMs: 4,
          issues: {
            slowTick: { count: 5 },
            headOfLine: { count: 6 },
          },
        };
      },
      resetReportStats() { resetStats += 1; },
    };
    const reporter = new MatchNetReporter({
      net,
      health,
      frameProfiler: {
        reportSummary: () => ({
          frameWorkMaxMs: 12,
          frameRafDispatchMaxMs: 5,
          frameUnattributedMaxMs: 7,
          rendererMaxMs: 8,
          topRendererPhase: "renderer.units",
          topRendererPhaseMs: 8,
          topRenderDiagnosticGroup: "renderer.pixi.displayObject",
          topRenderDiagnosticGroupCount: 3,
          clientFramePhases: [{ label: "match.renderer", count: 2, maxMs: 8, p95Ms: 8 }],
          rendererFramePhases: [{ label: "renderer.units", count: 2, maxMs: 8, p95Ms: 8 }],
          renderDiagnosticCounters: [{
            label: "renderer.pixi.displayObject",
            samples: 3,
            frames: 2,
            total: 3,
            maxFrame: 2,
          }],
          context: {},
        }),
        resetReportWindow: () => { resetFrame += 1; },
      },
      snapshotProcessingReport: {
        snapshotApplySummary: () => ({ max: 3 }),
        predictionApplySummary: () => ({ max: 2 }),
        reset: () => { resetSnapshot += 1; },
      },
      diagnostics: {
        count(name, payload) { diagnostics.push({ name, payload }); },
      },
      matchRunId: "match-run-7",
      getLastSnapshotTick: () => 77,
      getPredictionReportFields: () => ({ predictionMode: "predicting", pendingCommandCount: 4 }),
    });

    reporter.startMatchPings();
    assert(pings.length === 1, "match net reporter sends an immediate ping when started");
    assert(intervals[0].ms === 2000, "match net reporter preserves ping cadence");
    intervals[0].handler();
    assert(pings.length === 2, "match net reporter ping interval uses the injected Net");
    reporter.stopMatchPings();
    assert(cleared.includes(1), "match net reporter clears the ping timer");

    reporter.startNetReports();
    assert(intervals[1].ms === 10000, "match net reporter preserves net-report cadence");
    now = 1250;
    reporter.sendNetReport();
    assert(reports[0].schemaVersion === 1, "match net reporter sends schema-versioned reports");
    assert(reports[0].matchRunId === "match-run-7", "match net reporter preserves match run id");
    assert(reports[0].matchTick === 77, "match net reporter reads the latest snapshot tick lazily");
    assert(reports[0].fpsEstimate === 50, "match net reporter derives fps estimate from frame stats");
    assert(reports[0].predictionMode === "predicting", "match net reporter merges prediction fields");
    assert(reports[0].commandBurstMax === 3, "match net reporter includes command burst density");
    assert(
      reports[0].predictedSnapshotLateFrameCount === 1,
      "match net reporter includes prediction coverage during late snapshot frames",
    );
    assert(
      reports[0].predictedSnapshotLateFramePctX100 === 10000 &&
        reports[0].predictionActiveLateFrameCount === 1,
      "match net reporter includes bounded late-snapshot prediction coverage context",
    );
    assert(
      reports[0].frameRafDispatchMaxMs === 5 &&
        reports[0].frameUnattributedMaxMs === 7 &&
        reports[0].topRendererPhase === "renderer.units",
      "match net reporter includes bounded local frame context",
    );
    assert(
      reports[0].renderDiagnosticCounters[0].label === "renderer.pixi.displayObject",
      "match net reporter includes grouped render diagnostics",
    );
    assert(reports[0].hidden === true && reports[0].focused === false, "match net reporter includes document state");
    assert(resetStats === 1 && resetFrame === 1 && resetSnapshot === 1, "match net reporter resets report windows after upload");
    assert(
      diagnostics[0]?.name === "client.send.netReport" && diagnostics[0].payload.pendingCommandCount === 4,
      "match net reporter preserves diagnostics counters",
    );
    reporter.stopNetReports();
    assert(cleared.includes(2), "match net reporter clears the net-report timer");
  } finally {
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorPerformance === undefined) delete globalThis.performance;
    else globalThis.performance = priorPerformance;
    if (priorClearInterval === undefined) delete globalThis.clearInterval;
    else globalThis.clearInterval = priorClearInterval;
  }
}

{
  const fields = predictionReportFields({
    prediction: {
      debugSummary: () => ({
        mode: "predicting",
        pendingCommandCount: 7,
        ackLatencyMs: 13,
        maxCorrectionDistance: 22,
        correctionCount: 3,
        disableCount: 1,
        disableReasons: {
          "user-disabled": 1,
          "prediction-build-mismatch": 2,
          "wasm-unavailable": 3,
        },
      }),
      consumeCommandReportStats: () => ({
        commandsIssued: 9,
        commandIssueToServerReceiptMaxMs: 40,
        predictionReplayMaxMs: 11,
        predictionReplayMaxTicks: 7,
        predictionReplayBudgetExceededCount: 1,
      }),
    },
    predictionAdapter: {
      diagnostics: () => ({
        lastTickMs: 2,
        memoryBytes: 4096,
        lastReplayTicks: 5,
      }),
      consumeReportStats: () => ({
        predictionReplayMaxMs: 13,
        predictionReplayMaxTicks: 8,
        predictionReplayBudgetExceededCount: 2,
      }),
    },
  });
  assert(fields.predictionMode === "predicting", "prediction net-report fields preserve controller mode");
  assert(fields.pendingCommandCount === 7, "prediction net-report fields include pending command count");
  assert(fields.commandsIssued === 9, "prediction net-report fields include command report counters");
  assert(fields.predictionDisableUserCount === 1, "prediction net-report fields bucket user disables");
  assert(fields.predictionDisableCompatibilityCount === 2, "prediction net-report fields bucket compatibility disables");
  assert(fields.predictionDisableWasmCount === 3, "prediction net-report fields bucket WASM disables");
  assert(fields.wasmMemoryBytes === 4096, "prediction net-report fields include WASM diagnostics");
  assert(fields.predictionReplayMaxMs === 13, "prediction net-report fields include replay max milliseconds");
  assert(fields.predictionReplayMaxTicks === 8, "prediction net-report fields include replay max ticks");
  assert(fields.predictionReplayBudgetExceededCount === 3, "prediction net-report fields include replay budget exceeds");
}

// Match combat-audio collaborator
// ---------------------------------------------------------------------------
{
  const entities = new Map([
    [1, { id: 1, kind: KIND.MACHINE_GUNNER, owner: 1, x: 100, y: 140, targetId: 9 }],
    [3, { id: 3, kind: KIND.ARTILLERY, owner: 2, x: 220, y: 260 }],
  ]);
  const plays = [];
  const stopped = [];
  const combatAudio = new MatchCombatAudio({
    state: {
      playerId: 1,
      entityById: (id) => entities.get(id) || null,
    },
    audio: {
      pickVariant: (ids) => ids[0],
      play(id, opts) {
        plays.push({ id, opts });
        return true;
      },
      stopByKey(key) {
        stopped.push(key);
      },
    },
  });

  combatAudio.playAttackSound({ e: EVENT.ATTACK, from: 1 });
  assert(plays[0].id === "combat_mg_burst_02", "match combat audio picks machine-gun combat sound");
  assert(plays[0].opts.category === "combat_self", "match combat audio classifies own fire as combat_self");
  assertApprox(plays[0].opts.gain, 0.49, 0.0001, "machine-gun fire uses the quieter combat mix");
  assert(plays[0].opts.key === "combat:machine_gunner:1", "match combat audio keys looping machine-gunner bursts");
  combatAudio.stopInactiveMachineGunSounds();
  assert(stopped.length === 0, "match combat audio keeps active machine-gunner target audio");
  entities.set(1, { id: 1, kind: KIND.MACHINE_GUNNER, owner: 1, x: 100, y: 140 });
  combatAudio.stopInactiveMachineGunSounds();
  assert(stopped[0] === "combat:machine_gunner:1", "match combat audio stops stale machine-gunner target audio");
  combatAudio.playPointFireSound({ e: EVENT.MORTAR_LAUNCH, fromX: 12, fromY: 24 });
  assert(plays.at(-1).id === "combat_mortar_launch_04", "match combat audio routes mortar launches");
  assert(plays.at(-1).opts.x === 12 && plays.at(-1).opts.y === 24, "match combat audio preserves point-fire source position");
  assertApprox(plays.at(-1).opts.gain, 0.595, 0.0001, "mortar launches use the quieter combat mix");
  combatAudio.playPointFireSound({ e: EVENT.MORTAR_IMPACT, x: 48, y: 96 });
  assert(plays.at(-1).id === "combat_mortar_impact_01", "match combat audio routes mortar impacts");
  assert(plays.at(-1).opts.x === 48 && plays.at(-1).opts.y === 96, "mortar impact sound is spatialized at the landing point");
  assert(plays.at(-1).opts.category === "combat_other", "source-less mortar impacts avoid claiming self ownership");
  assertApprox(plays.at(-1).opts.gain, 0.7, 0.0001, "mortar impacts use the quieter combat mix");
  const playCountBeforeSelfReveal = plays.length;
  combatAudio.playAttackSound({ e: EVENT.ATTACK, from: 3, to: 3, weaponKind: WEAPON_KIND.ARTILLERY_GUN });
  assert(
    plays.length === playCountBeforeSelfReveal,
    "match combat audio keeps artillery self-reveal attack events silent",
  );

  const labPlays = [];
  const labEntities = new Map([
    [2, { id: 2, kind: KIND.RIFLEMAN, owner: 2, x: 140, y: 180 }],
  ]);
  const labCombatAudio = new MatchCombatAudio({
    state: {
      playerId: 1,
      entityById: (id) => labEntities.get(id) || null,
    },
    controlPolicy: {
      kind: "lab",
      feedbackOwner() {
        return 2;
      },
    },
    audio: {
      pickVariant: (ids) => ids[0],
      play(id, opts) {
        labPlays.push({ id, opts });
        return true;
      },
      stopByKey() {},
    },
  });
  labCombatAudio.playAttackSound({ e: EVENT.ATTACK, from: 2 });
  assert(labPlays[0].opts.category === "combat_self", "lab combat audio uses selected issue-as owner for self fire");
  assertApprox(labPlays[0].opts.gain, 0.175, 0.0001, "rifle fire uses the quieter combat mix");
}

{
  const plays = [];
  const timers = [];
  const cleared = [];
  const entities = new Map([
    [8, { id: 8, kind: KIND.ARTILLERY, owner: 1, x: 120, y: 160 }],
  ]);
  const combatAudio = new MatchCombatAudio({
    state: {
      playerId: 1,
      entityById: (id) => entities.get(id) || null,
    },
    audio: {
      play(id, opts) {
        plays.push({ id, opts });
        return true;
      },
      stopByKey() {},
    },
    setTimer(handler, delay) {
      const timer = { handler, delay };
      timers.push(timer);
      return timer;
    },
    clearTimer(timer) {
      cleared.push(timer);
    },
  });

  combatAudio.playPointFireSound({
    e: EVENT.ARTILLERY_TARGET,
    from: 8,
    x: 512,
    y: 640,
    delayTicks: 150,
  });
  assert(plays[0].id === "combat_artillery_fire_05", "artillery target still plays its firing cue immediately");
  assertApprox(plays[0].opts.gain, 0.84, 0.0001, "artillery fire uses the quieter combat mix");
  assert(timers.length === 1, "artillery target schedules one landing cue");
  assert(
    Math.abs(timers[0].delay - 2191.678) < 0.01,
    "artillery landing starts 2.808 seconds before the authoritative five-second impact",
  );
  timers[0].handler();
  assert(plays.at(-1).id === "combat_artillery_landing_01", "scheduled artillery landing uses its dedicated cue");
  assert(plays.at(-1).opts.x === 512 && plays.at(-1).opts.y === 640, "landing cue is spatialized at the impact point");
  assert(plays.at(-1).opts.category === "combat_self", "own artillery landing uses the self combat bus");
  assertApprox(plays.at(-1).opts.gain, 0.7, 0.0001, "artillery landings use the quieter combat mix");
  combatAudio.playPointFireSound({
    e: EVENT.ARTILLERY_TARGET,
    from: 8,
    x: 704,
    y: 768,
    delayTicks: 150,
  });
  combatAudio.destroy();
  assert(cleared.length === 1 && cleared[0] === timers[1], "teardown cancels a pending artillery landing");
}

// Match settings context collaborator
// ---------------------------------------------------------------------------
withFakeSettingsDocument(() => {
  let pauseSent = 0;
  let giveUpOpened = 0;
  let predictionToggled = false;
  let pointerLockToggled = 0;
  let debugToggled = 0;
  let unitRangeToggled = 0;
  const context = buildMatchSettingsContext({
    replayViewer: false,
    state: { spectator: false, debugPathOverlaysEnabled: true, showUnitRangesEnabled: false },
    capabilities: {
      matchControls: { pause: true },
      diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.ALL },
    },
    livePauseState: { paused: false, canPause: true, pausesRemaining: 2 },
    giveUpSent: false,
    audio: {},
    hotkeyProfiles: null,
    prediction: { enabled: true },
    predictionAdapter: { ready: true, loading: false },
    input: { pointerLocked: false, pointerLockSupported: () => true },
    onPauseGame: () => { pauseSent += 1; },
    onGiveUpOpen: () => { giveUpOpened += 1; },
    onPredictionEnabledChange: (enabled) => { predictionToggled = enabled; },
    onPointerLockToggle: () => { pointerLockToggled += 1; },
    onDebugPathToggle: () => { debugToggled += 1; },
    onUnitRangeToggle: () => { unitRangeToggled += 1; },
    livePauseActionLabel: () => "Pause (2)",
    livePauseActionTitle: () => "2 pauses remaining.",
  });
  assert(context.kind === "match" && !context.spectator && !context.replay, "match settings context identifies live player matches");
  const [pauseAction, giveUpAction] = context.actions;
  const pauseButton = pauseAction.render();
  const giveUpButton = giveUpAction.render();
  assert(pauseButton.id === "live-pause-open" && pauseButton.textContent === "Pause (2)", "match settings context wires pause action label");
  pauseButton.listeners.click();
  giveUpButton.listeners.click();
  assert(pauseSent === 1 && giveUpOpened === 1, "match settings context preserves pause and give-up callbacks");

  const gameTab = context.tabs.find((tab) => tab.id === "game");
  const debugTab = context.tabs.find((tab) => tab.id === "debug");
  const root = document.createElement("div");
  gameTab.render(root, context);
  root.children.find((child) => child.id === "prediction-toggle").listeners.click();
  root.children.find((child) => child.id === "pointer-lock-toggle").listeners.click();
  root.children.find((child) => child.id === "unit-range-toggle").listeners.click();
  debugTab.render(root, context);
  root.children.find((child) => child.id === "debug-path-toggle").listeners.click();
  assert(predictionToggled === false, "match settings context toggles prediction through the injected callback");
  assert(pointerLockToggled === 1, "match settings context toggles pointer lock through the injected callback");
  assert(unitRangeToggled === 1, "match settings context toggles unit ranges through the injected callback");
  assert(debugToggled === 1, "match settings context toggles debug paths through the injected callback");
});

withFakeSettingsDocument(() => {
  let pauseSent = 0;
  const context = buildMatchSettingsContext({
    replayViewer: false,
    state: { spectator: true },
    capabilities: {
      matchControls: { pause: true },
      diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.NONE },
    },
    livePauseState: { paused: false, canPause: true, pausesRemaining: 2 },
    giveUpSent: false,
    audio: {},
    hotkeyProfiles: null,
    prediction: { enabled: false },
    input: null,
    onPauseGame: () => { pauseSent += 1; },
    livePauseActionLabel: () => "Pause (2)",
  });
  assert(context.kind === "spectator" && context.spectator, "match settings context identifies live spectators");
  const [pauseAction, giveUpAction] = context.actions;
  const pauseButton = pauseAction.render();
  assert(pauseButton.id === "live-pause-open" && pauseButton.textContent === "Pause (2)",
    "match settings context shows spectator live pause action");
  assert(giveUpAction.render() === null, "match settings context still hides give-up for spectators");
  pauseButton.listeners.click();
  assert(pauseSent === 1, "match settings context wires spectator pause callback");
  assert(
    !context.tabs.some((tab) => tab.id === "replay-controls"),
    "live spectator camera controls stay out of match settings",
  );
});

withFakeSettingsDocument(() => {
  let returned = 0;
  const context = buildMatchSettingsContext({
    replayViewer: true,
    state: { spectator: true },
    capabilities: {
      matchControls: { pause: false },
      diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.NONE },
    },
    livePauseState: { paused: false, canPause: false },
    giveUpSent: false,
    audio: {},
    hotkeyProfiles: null,
    prediction: { enabled: false },
    input: null,
    onBackToLobby: () => { returned += 1; },
  });
  assert(context.kind === "replay" && context.replay, "match settings context identifies replay viewers");
  const button = context.actions[1].render();
  assert(button.id === "back-to-lobby-open" && button.textContent === "Back to Lobby",
    "match settings context shows replay back-to-lobby action in the leave slot");
  button.listeners.click();
  assert(returned === 1, "match settings context wires replay back-to-lobby callback");
});

withFakeSettingsDocument(() => {
  let returned = 0;
  const context = buildMatchSettingsContext({
    replayViewer: false,
    labMetadata: { role: "operator" },
    state: { spectator: true },
    capabilities: {
      matchControls: { pause: false },
      diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.NONE },
    },
    livePauseState: { paused: false, canPause: false },
    giveUpSent: false,
    audio: {},
    hotkeyProfiles: null,
    prediction: { enabled: false },
    input: null,
    onBackToLobby: () => { returned += 1; },
  });
  assert(context.kind === "lab" && context.spectator, "match settings context identifies lab sessions");
  const button = context.actions[1].render();
  assert(button.id === "back-to-lobby-open" && button.textContent === "Back to Lobby",
    "match settings context shows lab back-to-lobby action in the leave slot");
  button.listeners.click();
  assert(returned === 1, "match settings context wires lab back-to-lobby callback");
});
