// tests/client_contracts/match_replay_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertApprox,
} from "./assertions.mjs";
import { withFakeOverlayDocument } from "./fakes.mjs";
import { HUD } from "../../client/src/hud.js";
import {
  EVENT,
  LAB_ROLE,
  MOVEMENT_PATH_DIAGNOSTICS,
  NOTICE_SEVERITY,
  S,
  msg,
} from "../../client/src/protocol.js";
import { CameraNavigationInput } from "../../client/src/input/camera_navigation.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { ReplayCameraInput } from "../../client/src/replay_camera_input.js";
import { LivePauseOverlay } from "../../client/src/live_pause_overlay.js";
import { notePredictionAuthoritativeSnapshot } from "../../client/src/match_live_pause.js";
import { createRoomCapabilities } from "../../client/src/room_capabilities.js";

{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const fallbackElement = {
    contains() { return true; },
    addEventListener() {},
    removeEventListener() {},
    setAttribute() {},
    querySelectorAll() { return []; },
    hidden: false,
    disabled: false,
    textContent: "",
    title: "",
  };
  const windowListeners = new Map();
  const localStorageValues = new Map();
  globalThis.window = {
    location: { protocol: "http:", host: "localhost", search: "" },
    innerWidth: 1000,
    innerHeight: 700,
    localStorage: {
      getItem(key) {
        return localStorageValues.has(key) ? localStorageValues.get(key) : null;
      },
      setItem(key, value) {
        localStorageValues.set(key, String(value));
      },
      removeItem(key) {
        localStorageValues.delete(key);
      },
    },
    addEventListener(type, handler) {
      windowListeners.set(type, handler);
    },
    removeEventListener(type, handler) {
      if (windowListeners.get(type) === handler) windowListeners.delete(type);
    },
    setTimeout(fn) {
      fn();
      return 1;
    },
  };
  globalThis.document = {
    hidden: false,
    hasFocus() { return true; },
    getElementById() { return fallbackElement; },
    createElement() { return { classList: { add() {} }, appendChild() {}, style: {} }; },
  };
  const { Match } = await import("../../client/src/match.js");
  const { MatchNoticePresenter } = await import("../../client/src/match_notice_presenter.js");
  const { ReplayViewer } = await import("../../client/src/replay_viewer.js");
  const { ReplayControls, RoomTimeControls } = await import("../../client/src/replay_controls.js");
  const { applyMatchUnitRanges } = await import("../../client/src/match_settings_toggles.js");
  const {
    App,
    shouldReportPlayerActivity,
    shouldReturnToLobbyBrowserAfterDisconnect,
    shouldWarnBeforeUnload,
  } = await import("../../client/src/app.js");
  const { dom } = await import("../../client/src/bootstrap.js");
  assert(ReplayViewer.prototype instanceof Match, "ReplayViewer reuses Match rendering lifecycle");
  assert(ReplayControls.prototype instanceof RoomTimeControls, "replay controls keep a neutral room-time base");
  assert(!("command" in ReplayCameraInput.prototype), "Replay camera input has no gameplay command API");
  {
    const handlers = new Map();
    const net = {
      on(type, handler) {
        if (!handlers.has(type)) handlers.set(type, new Set());
        handlers.get(type).add(handler);
      },
      off(type, handler) {
        handlers.get(type)?.delete(handler);
      },
      emit(type, message) {
        for (const handler of handlers.get(type) || []) handler(message);
      },
    };
    let finishRenderer = null;
    let installedClock = null;
    const renderer = {
      setRenderClock(clock) { installedClock = clock; },
      destroy() {},
    };
    const backend = {
      createRenderer() {
        return new Promise((resolve) => { finishRenderer = () => resolve(renderer); });
      },
    };
    class MatchFactoryProbe {
      constructor() {
        this.renderClock = { now: () => 123 };
        this.received = [];
        this.onSnapshot = (message) => this.received.push([S.SNAPSHOT, message.tick]);
        this.onCommandReceipt = (message) => this.received.push([S.COMMAND_RECEIPT, message.clientSeq]);
        this.onRoomTimeState = () => {};
        this.onLivePauseState = () => {};
        this.onObserverAnalysis = () => {};
      }
    }
    const creating = Match.create.call(
      MatchFactoryProbe,
      net,
      {},
      null,
      null,
      null,
      null,
      null,
      { rendererBackendBundle: backend },
    );
    net.emit(S.SNAPSHOT, { tick: 7 });
    net.emit(S.COMMAND_RECEIPT, { clientSeq: 9 });
    finishRenderer();
    const created = await creating;
    assert(created.received.length === 2 &&
      created.received[0][0] === S.SNAPSHOT && created.received[0][1] === 7 &&
      created.received[1][0] === S.COMMAND_RECEIPT && created.received[1][1] === 9,
      "Match.create replays opening events that arrive while Pixi initializes");
    assert(installedClock === created.renderClock,
      "Match.create installs the match clock through the renderer contract");
    assert([...handlers.values()].every((set) => set.size === 0),
      "Match.create removes temporary startup listeners after initialization");

    const protectedControlEvents = [];
    class SlowMatchFactoryProbe {
      constructor() {
        this.renderClock = { now: () => 456 };
        this.onSnapshot = (message) => protectedControlEvents.push([S.SNAPSHOT, message.tick]);
        this.onCommandReceipt = () => {};
        this.onRoomTimeState = (message) => protectedControlEvents.push([S.ROOM_TIME_STATE, message.cursor]);
        this.onLivePauseState = () => {};
        this.onObserverAnalysis = () => {};
      }
    }
    const slowCreating = Match.create.call(
      SlowMatchFactoryProbe,
      net,
      {},
      null,
      null,
      null,
      null,
      null,
      { rendererBackendBundle: backend },
    );
    net.emit(S.ROOM_TIME_STATE, { cursor: 11 });
    for (let tick = 1; tick <= 80; tick += 1) net.emit(S.SNAPSHOT, { tick });
    finishRenderer();
    await slowCreating;
    assert(protectedControlEvents.some(([type, cursor]) => type === S.ROOM_TIME_STATE && cursor === 11),
      "snapshot traffic cannot evict one-shot room state during slow renderer startup");
    assert(protectedControlEvents.at(-1)?.[1] === 80,
      "startup overflow retains the newest authoritative snapshot");

    let staleRendererDestroyed = false;
    let staleMatchConstructed = false;
    class StaleMatchFactoryProbe {
      constructor() { staleMatchConstructed = true; }
    }
    const staleResult = await Match.create.call(
      StaleMatchFactoryProbe,
      net,
      {},
      null,
      null,
      null,
      null,
      null,
      {
        isStartCurrent: () => false,
        rendererBackendBundle: {
          async createRenderer() {
            return { destroy() { staleRendererDestroyed = true; } };
          },
        },
      },
    );
    assert(staleResult === null && staleRendererDestroyed && !staleMatchConstructed,
      "Match.create discards a stale renderer before constructing match modules");
    assert([...handlers.values()].every((set) => set.size === 0),
      "Match.create removes temporary startup listeners when a start becomes stale");

    let failedRendererDestroyed = false;
    class FailingMatchFactoryProbe {
      constructor() {
        throw new Error("match construction failed");
      }
    }
    try {
      await Match.create.call(
        FailingMatchFactoryProbe,
        net,
        {},
        null,
        null,
        null,
        null,
        null,
        {
          rendererBackendBundle: {
            async createRenderer() {
              return { destroy() { failedRendererDestroyed = true; } };
            },
          },
        },
      );
      assert(false, "Match.create rejects when match construction fails");
    } catch (error) {
      assert(error.message === "match construction failed",
        "Match.create preserves the match construction failure");
    }
    assert(failedRendererDestroyed,
      "Match.create destroys an initialized renderer when match construction fails");
    assert([...handlers.values()].every((set) => set.size === 0),
      "Match.create removes temporary startup listeners after a failed initialization");
  }
  {
    let synced = 0;
    const unitRangeMatch = Object.create(Match.prototype);
    unitRangeMatch.state = { showUnitRangesEnabled: true };
    unitRangeMatch.syncSettingsToggleUi = () => { synced += 1; };
    applyMatchUnitRanges(unitRangeMatch, false);
    assert(!unitRangeMatch.state.showUnitRangesEnabled, "Match unit range helper updates the local display preference");

    let published = null;
    unitRangeMatch.onUnitRangesEnabledChange = (enabled) => { published = enabled; };
    unitRangeMatch.toggleUnitRangeOverlays();
    assert(unitRangeMatch.state.showUnitRangesEnabled && published === true,
      "Match unit range toggle publishes the persisted preference");
    assert(synced === 1, "Match unit range toggle refreshes the settings UI");
  }
  {
    const selectionArea = { hidden: false };
    const commandCard = { hidden: false };
    const giveUpConfirm = { hidden: false };
    dom.selectionArea = selectionArea;
    dom.commandCard = commandCard;
    dom.giveUpConfirm = giveUpConfirm;

    const replayMatch = Object.create(Match.prototype);
    replayMatch.replayViewer = true;
    replayMatch.state = { spectator: false };
    replayMatch.applySpectatorUi();
    assert(!selectionArea.hidden, "replay viewer shows the selected-unit HUD area for inspection");
    assert(commandCard.hidden, "replay viewer keeps command card hidden");
    assert(giveUpConfirm.hidden, "replay viewer hides give-up confirmation");

    selectionArea.hidden = true;
    commandCard.hidden = true;
    const liveMatch = Object.create(Match.prototype);
    liveMatch.replayViewer = false;
    liveMatch.state = { spectator: false };
    liveMatch.applySpectatorUi();
    assert(!selectionArea.hidden, "live player match restores the selected-unit HUD area");
    assert(!commandCard.hidden, "live player match restores the command card");

    selectionArea.hidden = true;
    commandCard.hidden = true;
    const labOperatorMatch = Object.create(Match.prototype);
    labOperatorMatch.replayViewer = false;
    labOperatorMatch.state = { spectator: true };
    labOperatorMatch.controlPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } });
    labOperatorMatch.applySpectatorUi();
    assert(!selectionArea.hidden, "lab operator keeps the selected-unit HUD area visible");
    assert(!commandCard.hidden, "lab operator keeps the command card visible");

    selectionArea.hidden = false;
    commandCard.hidden = false;
    const labViewerMatch = Object.create(Match.prototype);
    labViewerMatch.replayViewer = false;
    labViewerMatch.state = { spectator: true };
    labViewerMatch.controlPolicy = createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } });
    labViewerMatch.applySpectatorUi();
    assert(selectionArea.hidden, "read-only lab viewer hides the selected-unit HUD area");
    assert(commandCard.hidden, "read-only lab viewer hides the command card");

    const staleConfirm = { hidden: true };
    const staleConfirmButton = {
      disabled: true,
      textContent: "Giving up...",
      focus() { this.focused = true; },
    };
    dom.giveUpConfirm = staleConfirm;
    dom.giveUpConfirmButton = staleConfirmButton;
    const giveUpMatch = Object.create(Match.prototype);
    giveUpMatch.replayViewer = false;
    giveUpMatch.state = { spectator: false };
    giveUpMatch.giveUpSent = false;
    giveUpMatch.settings = null;
    giveUpMatch.openGiveUpConfirm();
    assert(!staleConfirm.hidden, "live player match opens the give-up confirmation");
    assert(!staleConfirmButton.disabled && staleConfirmButton.textContent === "Give up",
      "give-up confirmation resets stale pending button state before showing");
    giveUpMatch.closeMenus();
    assert(staleConfirm.hidden, "closeMenus hides the give-up confirmation");
    assert(!staleConfirmButton.disabled && staleConfirmButton.textContent === "Give up",
      "closeMenus resets give-up confirmation button state");

    const labToolMatch = Object.create(Match.prototype);
    labToolMatch.clientIntent = new ClientIntent();
    const labToolChanges = [];
    labToolMatch.publishLabToolChange = (change) => labToolChanges.push(change);
    let labToolWorldClick = null;
    const active = labToolMatch.armLabTool(
      { kind: "fieldPoint", payload: { xField: "spawn-x" } },
      { onWorldClick: (event) => { labToolWorldClick = event; } },
    );
    assert(labToolChanges.at(-1)?.type === "armed", "Match lab tool controller publishes armed state");
    labToolMatch.consumeLabToolWorldClick({
      tool: active,
      x: 44.5,
      y: 88.25,
      world: { x: 44.5, y: 88.25 },
      screen: { x: 10, y: 20 },
    });
    assert(labToolWorldClick?.tool.id === active.id, "Match lab tool controller routes world clicks with the active tool");
    assert(labToolWorldClick.x === 44.5 && labToolWorldClick.y === 88.25, "Match lab tool controller preserves exact world coordinates");
    assert(labToolMatch.clientIntent.activeLabTool === null, "Match lab tool controller clears consumed tools");
    assert(labToolChanges.at(-1)?.reason === "worldClick", "Match lab tool controller publishes world-click cancellation");
    const persistent = labToolMatch.armLabTool(
      { kind: "spawnEntity", payload: { owner: 1 }, keepArmedOnWorldClick: true },
      { onWorldClick: () => {} },
    );
    labToolMatch.clientIntent.updateLabToolPreview({ toolId: persistent.id, x: 8, y: 16 });
    const updatedPersistent = labToolMatch.updateLabToolPayload({ owner: 2 });
    assert(updatedPersistent === persistent, "Match updates persistent Lab tools without replacing their identity");
    assert(
      labToolChanges.at(-1)?.type === "updated" &&
        labToolMatch.clientIntent.labToolPreview?.payload?.owner === 2,
      "Match publishes Lab tool payload updates and preserves the live preview",
    );
    labToolMatch.consumeLabToolWorldClick({
      tool: persistent,
      x: 12,
      y: 16,
      world: { x: 12, y: 16 },
      screen: { x: 1, y: 2 },
    });
    assert(labToolMatch.clientIntent.activeLabTool?.id === persistent.id, "Match lab tool controller keeps persistent tools armed after world clicks");
    let labToolBoxSelection = null;
    const boxTool = labToolMatch.armLabTool(
      { kind: "removeSelectableUnits", consumeBoxSelection: true, keepArmedOnBoxSelection: true },
      { onBoxSelection: (event) => { labToolBoxSelection = event; } },
    );
    labToolMatch.consumeLabToolBoxSelection({
      tool: boxTool,
      entityIds: [31, 32],
      screenRect: { x: 10, y: 20, w: 40, h: 60 },
      worldRect: { minX: 10, minY: 20, maxX: 50, maxY: 80 },
    });
    assert(
      labToolBoxSelection?.tool.id === boxTool.id &&
        labToolBoxSelection.entityIds.join(",") === "31,32",
      "Match lab tool controller routes box selections with selected entity ids",
    );
    assert(labToolMatch.clientIntent.activeLabTool?.id === boxTool.id, "Match lab tool controller keeps persistent tools armed after box selections");
  }
  {
    const priorWindowForReplayInput = globalThis.window;
    const listeners = new Map();
    const options = new Map();
    const viewport = {
      addEventListener(type, handler, opts) {
        listeners.set(type, handler);
        options.set(type, opts);
      },
      removeEventListener(type, handler) {
        if (listeners.get(type) === handler) listeners.delete(type);
      },
      getBoundingClientRect() {
        return { left: 20, top: 30, width: 640, height: 480 };
      },
    };
    const camera = {
      calls: [],
      pans: [],
      dollyBy(factor, anchor) {
        this.calls.push({ factor, anchor });
      },
      panByScreenDelta(delta) {
        this.pans.push({ dx: delta.x, dy: delta.y });
      },
    };
    const replayState = {
      spectator: true,
      selection: new Set(),
      setSelection(ids) { this.selection = new Set(ids); },
      addToSelection(ids) { for (const id of ids) this.selection.add(id); },
      removeFromSelection(ids) { for (const id of ids) this.selection.delete(id); },
      clearSelection() { this.selection.clear(); },
    };
    globalThis.window = {
      addEventListener(type, handler) {
        listeners.set(`window:${type}`, handler);
      },
      removeEventListener(type, handler) {
        if (listeners.get(`window:${type}`) === handler) listeners.delete(`window:${type}`);
      },
    };
    try {
      const replayInput = new ReplayCameraInput(viewport, camera, replayState);
      replayInput.publishSelectionScene({
        version: 1,
        projection: {
          version: 1,
          viewport: { widthCssPx: 640, heightCssPx: 480 },
          project(point) { return { x: point.x, y: point.y, depth: 1 }; },
        },
        proxies: [
          {
            version: 1,
            id: 41,
            kind: "rifleman",
            owner: 1,
            anchor: { x: 200, y: 150, heightPx: 8 },
            footprint: { kind: "circle", radiusPx: 10 },
            minScreenRadiusCssPx: 6,
            interaction: { id: 41, kind: "rifleman", owner: 1, x: 200, y: 150, hp: 72, maxHp: 100 },
          },
          {
            version: 1,
            id: 42,
            kind: "tank",
            owner: 2,
            anchor: { x: 240, y: 170, heightPx: 16 },
            footprint: { kind: "circle", radiusPx: 16 },
            minScreenRadiusCssPx: 6,
            interaction: { id: 42, kind: "tank", owner: 2, x: 240, y: 170, hp: 240, maxHp: 300 },
          },
        ],
      });
      assert(options.get("wheel")?.passive === false, "Replay camera wheel listener is non-passive");
      let prevented = 0;
      listeners.get("wheel")({
        deltaY: -100,
        clientX: 220,
        clientY: 180,
        preventDefault() {
          prevented += 1;
        },
      });
      assertApprox(camera.calls[0].factor, 1.12, 0.000001, "Replay mouse wheel dollies in");
      assert(
        camera.calls[0].anchor.x === 200 && camera.calls[0].anchor.y === 150,
        "Replay wheel dolly anchors on cursor-local CSS pixels",
      );
      listeners.get("wheel")({
        deltaY: 100,
        clientX: 220,
        clientY: 180,
        preventDefault() {
          prevented += 1;
        },
      });
      assertApprox(camera.calls[1].factor, 1 / 1.12, 0.000001, "Replay mouse wheel dollies out");
      assert(prevented === 2, "Replay wheel dolly prevents page scroll");
      let dragPrevented = 0;
      listeners.get("mousedown")({
        button: 1,
        clientX: 120,
        clientY: 130,
        preventDefault() {
          dragPrevented += 1;
        },
      });
      listeners.get("window:mousemove")({
        clientX: 150,
        clientY: 160,
        preventDefault() {
          dragPrevented += 1;
        },
      });
      listeners.get("window:mouseup")({
        button: 1,
        preventDefault() {
          dragPrevented += 1;
        },
      });
      assert(camera.pans.length === 1, "Replay middle-drag pans through shared camera navigation");
      assert(camera.pans[0].dx === 30 && camera.pans[0].dy === 30, "Replay middle-drag uses screen delta");
      assert(dragPrevented === 3, "Replay middle-drag suppresses browser drag defaults");
      listeners.get("window:keydown")({
        code: "Space",
        preventDefault() {},
      });
      listeners.get("mousedown")({
        button: 0,
        clientX: 170,
        clientY: 175,
        preventDefault() {},
      });
      listeners.get("window:mousemove")({
        clientX: 160,
        clientY: 165,
        preventDefault() {},
      });
      listeners.get("window:mouseup")({
        button: 0,
        preventDefault() {},
      });
      listeners.get("window:keyup")({
        code: "Space",
        preventDefault() {},
      });
      assert(camera.pans.length === 2, "Replay Space+left-drag pans through shared camera navigation");
      assert(camera.pans[1].dx === -10 && camera.pans[1].dy === -10, "Replay Space+left-drag uses screen delta");
      listeners.get("mousedown")({ button: 0, clientX: 220, clientY: 180 });
      listeners.get("window:mouseup")({ button: 0, clientX: 220, clientY: 180 });
      assert(replayState.selection.has(41), "Replay spectators can select a presented unit without issuing an order");
      assert(!("command" in replayInput), "Replay selection keeps the input command-free");
      listeners.get("mousedown")({ button: 0, clientX: 170, clientY: 130 });
      listeners.get("window:mousemove")({ button: 0, clientX: 280, clientY: 230 });
      assert(replayInput.screenOverlay.snapshot().marquee?.w === 110, "Replay drag publishes a selection marquee");
      listeners.get("window:blur")();
      assert(replayInput.screenOverlay.snapshot().marquee === null, "Replay blur cancels an in-progress selection drag");
      listeners.get("window:mouseup")({ button: 0, clientX: 280, clientY: 230 });
      assert(replayState.selection.size === 1, "Replay blur prevents a stale drag release from changing selection");
      listeners.get("mousedown")({ button: 0, clientX: 170, clientY: 130 });
      listeners.get("window:mousemove")({ button: 0, clientX: 280, clientY: 230 });
      listeners.get("window:mouseup")({ button: 0, clientX: 280, clientY: 230 });
      assert(replayState.selection.size === 2, "Replay spectators can box-select visible units from either player");
      assert(replayInput.screenOverlay.snapshot().marquee === null, "Replay box selection clears its marquee on release");
      replayInput.destroy();
      assert(!listeners.has("wheel"), "Replay camera input removes wheel listener on destroy");
    } finally {
      if (priorWindowForReplayInput === undefined) delete globalThis.window;
      else globalThis.window = priorWindowForReplayInput;
    }
  }
  {
    const listeners = new Map();
    const viewport = {
      addEventListener(type, handler) {
        listeners.set(type, handler);
      },
      removeEventListener(type, handler) {
        if (listeners.get(type) === handler) listeners.delete(type);
      },
      getBoundingClientRect() {
        return { left: 0, top: 0, width: 800, height: 600 };
      },
    };
    const windowRef = {
      addEventListener(type, handler) {
        listeners.set(`window:${type}`, handler);
      },
      removeEventListener(type, handler) {
        if (listeners.get(`window:${type}`) === handler) listeners.delete(`window:${type}`);
      },
    };
    const helper = new CameraNavigationInput(viewport, {}, {
      installListeners: true,
      windowRef,
      panKeyCodes: CameraNavigationInput.replayPanKeyCodes(),
    });
    let prevented = 0;
    listeners.get("window:keydown")({
      code: "KeyW",
      preventDefault() {
        prevented += 1;
      },
    });
    assert(helper.keys.up, "Shared camera navigation maps configured pan keys");
    listeners.get("window:keyup")({
      code: "KeyW",
      preventDefault() {
        prevented += 1;
      },
    });
    assert(!helper.keys.up && prevented === 2, "Shared camera navigation releases configured pan keys");
    helper.destroy();
    assert(!listeners.has("window:keydown"), "Shared camera navigation removes key listeners on destroy");
  }
  assert(!shouldWarnBeforeUnload(), "lobby state does not warn before unload");
  assert(
    shouldReturnToLobbyBrowserAfterDisconnect(),
    "an ordinary lobby socket close returns to the main lobby browser",
  );
  assert(
    !shouldReturnToLobbyBrowserAfterDisconnect({ match: {} }),
    "an in-game socket close remains on the match disconnect path",
  );
  assert(
    !shouldReturnToLobbyBrowserAfterDisconnect({ requiresConnectionOnStart: true }),
    "an explicit connected launch keeps its dedicated disconnect handling",
  );
  assert(
    shouldReportPlayerActivity({ socketOpen: true, nowMs: 30000, lastReportMs: 0 }),
    "human input is reportable after the throttle interval",
  );
  assert(
    !shouldReportPlayerActivity({ socketOpen: false, nowMs: 30000, lastReportMs: 0 }),
    "human input is not reported without a socket",
  );
  assert(
    !shouldReportPlayerActivity({ socketOpen: true, nowMs: 29999, lastReportMs: 0 }),
    "human input reports are throttled",
  );
  {
    const app = Object.create(App.prototype);
    let activityCount = 0;
    app.socketOpen = true;
    app.lastPlayerActivityReportMs = 0;
    app.net = { activity() { activityCount += 1; return true; } };
    assert(app.reportPlayerActivity(30000), "eligible human input sends an activity notice");
    assert(!app.reportPlayerActivity(30001), "a sent activity notice re-arms the throttle");
    assert(activityCount === 1, "throttled activity emits one wire message");
  }
  {
    const app = Object.create(App.prototype);
    const starts = [];
    app.matchStartGeneration = 0;
    app.matchEndedGeneration = 0;
    app.startMatch = (payload, generation) => {
      starts.push({ payload, generation });
      return Promise.resolve();
    };
    app.onStart({ id: "first" });
    const firstPromise = app.matchStartPromise;
    app.onStart({ id: "second" });
    assert(starts[0].generation === 1 && starts[1].generation === 2,
      "overlapping async match starts receive monotonically increasing generations");
    assert(app.matchStartPromise !== firstPromise,
      "the app observes the newest async match-start promise instead of returning it to Net dispatch");
    await app.matchStartPromise;
  }
  {
    const app = Object.create(App.prototype);
    app.matchStartGeneration = 4;
    app.invalidatePendingMatchStart();
    assert(app.matchStartGeneration === 5,
      "returning to the lobby invalidates an in-flight asynchronous match start");
  }
  {
    const app = Object.create(App.prototype);
    app.matchStartGeneration = 3;
    app.matchEndedGeneration = 3;
    let stopped = 0;
    let destroyed = 0;
    const endedMatch = { stop() { stopped += 1; }, destroy() { destroyed += 1; } };
    assert(app.completeMatchStart(endedMatch, 3) && app.match === endedMatch && stopped === 1,
      "a match that ends during renderer startup attaches in a stopped state");
    const staleMatch = { stop() { stopped += 1; }, destroy() { destroyed += 1; } };
    assert(!app.completeMatchStart(staleMatch, 2) && destroyed === 1 && app.match === endedMatch,
      "a stale renderer startup destroys its match without replacing the current match");
  }
  {
    const app = Object.create(App.prototype);
    let resetCount = 0;
    let showCount = 0;
    let warningCount = 0;
    app.stopHeartbeat = () => {};
    app.socketOpen = true;
    app.intentionalIdleDisconnect = false;
    app.match = null;
    app.requiresConnectionOnStart = () => false;
    app.lobby = {
      resetToBrowser() { resetCount += 1; },
      show() { showCount += 1; },
    };
    app.showConnectionWarning = () => { warningCount += 1; };
    app.showConnectionLost = () => { warningCount += 1; };
    app.onClose();
    assert(resetCount === 1 && showCount === 1,
      "an ordinary lobby disconnect resets and shows the main lobby browser");
    assert(warningCount === 0, "an ordinary lobby disconnect does not show a warning");
  }
  {
    const connectionLost = { hidden: true };
    dom.connectionLost = connectionLost;
    const app = Object.create(App.prototype);
    app.lobby = null;
    app.labCatalog = null;
    app.showConnectionLost();
    assert(!connectionLost.hidden, "connection loss opens the persistent non-blocking notice");
    app.socketOpen = false;
    app.stopHeartbeat = () => {};
    app.net = { offline: true };
    app.onOpen();
    assert(connectionLost.hidden, "connection loss notice disappears when a socket reconnects");
  }
  assert(
    shouldWarnBeforeUnload({ match: { state: { spectator: false } } }),
    "live player match warns before unload",
  );
  assert(
    !shouldWarnBeforeUnload({ match: { state: { spectator: true } } }),
    "live spectator does not warn before unload",
  );
  assert(
    !shouldWarnBeforeUnload({ match: { state: { spectator: false }, labMetadata: { id: "lab" } } }),
    "lab match does not warn before unload",
  );
  assert(
    !shouldWarnBeforeUnload({ match: { state: { spectator: false }, replayViewer: true } }),
    "replay viewer does not warn before unload",
  );
  assert(
    !shouldWarnBeforeUnload({ match: { state: { spectator: false }, running: false } }),
    "resolved or stopped match does not warn before unload",
  );
  assert(!shouldWarnBeforeUnload({ inReplayPlayback: true }), "replay playback does not warn before unload");
  assert(
    !shouldWarnBeforeUnload({
      match: { state: { spectator: false } },
      allowUnloadWithoutWarning: true,
    }),
    "intentional app navigation bypasses unload warning",
  );

  const normalCapabilities = createRoomCapabilities({
    startPayload: { spectator: false, capabilities: { commands: { gameplay: true }, matchControls: { pause: true } } },
  });
  assert(!normalCapabilities.roomTime.available, "normal matches do not mount room-time controls");
  assert(normalCapabilities.commands.gameplay, "active players keep gameplay command affordances");
  assert(normalCapabilities.matchControls.pause, "active live players keep live pause affordances");

  const spectatorCapabilities = createRoomCapabilities({
    startPayload: {
      spectator: true,
      diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.ALL },
      capabilities: { commands: { gameplay: false }, matchControls: { pause: true } },
    },
  });
  assert(!spectatorCapabilities.commands.gameplay, "spectators get read-only command affordances");
  assert(
    spectatorCapabilities.diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.ALL,
    "capability parser keeps diagnostic affordances from the start payload",
  );
  assert(spectatorCapabilities.matchControls.pause, "spectators keep advertised live pause controls");

  withFakeOverlayDocument(({ FakeElement }) => {
    const root = new FakeElement("section");
    const settingsRoot = new FakeElement("div");
    let unpaused = false;
    const openedTabs = [];
    const overlay = new LivePauseOverlay({
      root,
      settingsRoot,
      onUnpause: () => { unpaused = true; },
      onOpenSettings: (tabId) => openedTabs.push(tabId),
      playerNameForId: (playerId) => playerId === 2 ? "Alex" : "",
    });
    overlay.applyLivePauseState({ paused: true, pausedBy: 2, pauseLimit: 3, canUnpause: true });
    assert(root.children.length === 1, "live pause overlay mounts generated DOM");
    assert(!root.children[0].hidden, "live pause overlay shows when paused");
    assert(root.querySelector(".live-pause-meta")?.textContent === "Paused by Alex", "live pause overlay resolves the pausing player's roster name");
    assert(settingsRoot.classList.contains("live-pause-active"), "live pause overlay raises settings above its screen blocker");
    root.querySelector("#live-pause-settings").listeners.click();
    root.querySelector("#live-pause-hotkeys").listeners.click();
    assert(openedTabs.join(",") === "game,hotkeys", "live pause overlay opens game settings and hotkey editing");
    const button = root.querySelector("#live-pause-unpause");
    assert(button && !button.hidden && !button.disabled, "live pause overlay enables unpause for pause-authorized viewers");
    button.listeners.click();
    assert(unpaused, "live pause overlay calls injected unpause action");
    overlay.applyLivePauseState({ paused: true, canUnpause: false });
    assert(button.hidden && button.disabled, "live pause overlay hides unpause without authority");
    overlay.applyLivePauseState({ paused: false });
    assert(root.children[0].hidden, "live pause overlay hides when running");
    assert(!settingsRoot.classList.contains("live-pause-active"), "live pause overlay restores normal settings stacking after unpause");
    overlay.destroy();
    assert(root.children.length === 0, "live pause overlay tears down DOM");
  });

  const noticeAudioMatch = Object.create(Match.prototype);
  const playedNotices = [];
  let minimapPings = 0;
  noticeAudioMatch.toast = () => {};
  noticeAudioMatch.audio = {
    play(id, opts) {
      playedNotices.push({ id, opts });
    },
  };
  noticeAudioMatch.minimap = {
    ping() {
      minimapPings += 1;
    },
    pulseBorder() {},
  };
  noticeAudioMatch.camera = {
    containsProjected(point, margin = 0) {
      return point.x >= -margin && point.x <= 100 + margin &&
        point.y >= -margin && point.y <= 100 + margin;
    },
  };
  noticeAudioMatch.state = { spectator: false };
  noticeAudioMatch.noticePresenter = new MatchNoticePresenter({
    toast: noticeAudioMatch.toast,
    minimap: noticeAudioMatch.minimap,
    audio: noticeAudioMatch.audio,
    isReplay: () => noticeAudioMatch.replayViewer,
    isSpectator: () => !!noticeAudioMatch.state?.spectator,
    pointInViewport: (x, y, margin) => noticeAudioMatch.pointInViewport(x, y, margin),
  });
  noticeAudioMatch.replayViewer = true;
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 512,
    y: 768,
  });
  assert(playedNotices.length === 0, "replay notice alerts do not play audio");
  assert(minimapPings === 1, "replay notice alerts still ping the minimap");
  noticeAudioMatch.replayViewer = false;
  noticeAudioMatch.state = { spectator: true };
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 1600,
    y: 768,
  });
  assert(playedNotices.length === 0, "live spectator notice alerts do not play audio");
  assert(minimapPings === 2, "live spectator notice alerts still ping the minimap");
  noticeAudioMatch.state = { spectator: false };
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 2700,
    y: 768,
  });
  assert(
    playedNotices[0]?.id === "notice_under_attack",
    "live notice alerts still play audio outside the current viewport",
  );

  const artilleryMarkerMatch = Object.create(Match.prototype);
  const artilleryMarkers = [];
  artilleryMarkerMatch.audio = null;
  artilleryMarkerMatch.minimap = {
    markArtilleryFiring(ev) {
      artilleryMarkers.push(ev);
    },
  };
  artilleryMarkerMatch.handleSnapshotEvents([
    { e: EVENT.ARTILLERY_FIRING, owner: 2, x: 288, y: 304, facing: 0.25 },
  ]);
  assert(
    artilleryMarkers.length === 1 &&
      artilleryMarkers[0].owner === 2 &&
      artilleryMarkers[0].x === 288,
    "artillery firing events are forwarded to the minimap marker layer",
  );

  const predictionPolicyMatch = Object.create(Match.prototype);
  predictionPolicyMatch.replayViewer = false;
  predictionPolicyMatch.state = {
    spectator: false,
    applyPredictionDisplayOverlay(overlay) {
      if (Object.prototype.hasOwnProperty.call(overlay || {}, "predictedSnapshot")) {
        this.predictedSnapshot = overlay.predictedSnapshot;
      }
      if (Object.prototype.hasOwnProperty.call(overlay || {}, "optimisticCommands")) {
        this.optimisticCommands = overlay.optimisticCommands;
      }
    },
  };
  predictionPolicyMatch.prediction = {
    enabled: true,
    predictor: null,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  predictionPolicyMatch.predictionInitToken = 0;
  let predictionAdapterInit = 0;
  let predictionAdapterDestroy = 0;
  let predictionAdapterId = 0;
  const makePredictionAdapter = () => {
    const adapter = {
      id: ++predictionAdapterId,
      ready: false,
      loading: false,
      destroyed: false,
      diagnostics: () => ({ ready: adapter.ready, loading: adapter.loading }),
      init: async () => {
        predictionAdapterInit += 1;
        adapter.loading = true;
        await Promise.resolve();
        adapter.loading = false;
        adapter.ready = true;
        return true;
      },
      destroy: () => {
        predictionAdapterDestroy += 1;
        adapter.destroyed = true;
        adapter.ready = false;
        adapter.loading = false;
      },
    };
    return adapter;
  };
  predictionPolicyMatch.createPredictionAdapter = makePredictionAdapter;
  predictionPolicyMatch.predictionAdapter = {
    ready: false,
    loading: false,
    diagnostics: () => ({ ready: false }),
    init: async () => true,
    destroy: () => { predictionAdapterDestroy += 1; },
  };
  predictionPolicyMatch.prediction.predictor = predictionPolicyMatch.predictionAdapter;
  predictionPolicyMatch.publishPredictionDebug = () => {};
  predictionPolicyMatch.mountSettings = () => {};
  predictionPolicyMatch.logPredictionStatus = () => {};
  predictionPolicyMatch.setPredictionEnabled(false);
  assert(!predictionPolicyMatch.prediction.enabled, "prediction setting can disable live prediction");
  assert(predictionPolicyMatch.state.predictedSnapshot === null, "disabling prediction clears local predicted overlay");
  assert(predictionPolicyMatch.state.optimisticCommands === null, "disabling prediction clears optimistic command UI");
  assert(predictionPolicyMatch.prediction.predictor === predictionPolicyMatch.predictionAdapter,
    "disabling prediction replaces the controller predictor with a fresh inactive adapter");
  assert(predictionAdapterDestroy === 1, "disabling prediction destroys the active WASM adapter");
  predictionPolicyMatch.setPredictionEnabled(true);
  await Promise.resolve();
  await Promise.resolve();
  assert(predictionPolicyMatch.prediction.enabled, "prediction setting can re-enable live prediction");
  assert(predictionAdapterInit === 1, "re-enabling prediction initializes the WASM adapter");
  assert(predictionPolicyMatch.predictionAdapter.ready, "re-enabled prediction owns a ready fresh adapter");

  const staleInitMatch = Object.create(Match.prototype);
  staleInitMatch.replayViewer = false;
  staleInitMatch.state = { spectator: false };
  staleInitMatch.predictionInitToken = 0;
  staleInitMatch.prediction = {
    enabled: true,
    predictor: null,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  let resolveStaleInit = null;
  const staleAdapter = {
    destroyed: false,
    ready: false,
    loading: true,
    diagnostics: () => ({ ready: staleAdapter.ready, loading: staleAdapter.loading }),
    init: () => new Promise((resolve) => {
      resolveStaleInit = () => {
        staleAdapter.loading = false;
        staleAdapter.ready = true;
        resolve(true);
      };
    }),
    destroy: () => {
      staleAdapter.destroyed = true;
      staleAdapter.ready = false;
      staleAdapter.loading = false;
    },
  };
  let freshAdapter = null;
  staleInitMatch.createPredictionAdapter = () => {
    freshAdapter = {
      destroyed: false,
      ready: false,
      loading: false,
      diagnostics: () => ({ ready: freshAdapter.ready, loading: freshAdapter.loading }),
      init: async () => {
        freshAdapter.ready = true;
        return true;
      },
      destroy: () => {
        freshAdapter.destroyed = true;
        freshAdapter.ready = false;
      },
    };
    return freshAdapter;
  };
  staleInitMatch.predictionAdapter = staleAdapter;
  staleInitMatch.prediction.predictor = staleAdapter;
  staleInitMatch.publishPredictionDebug = () => {};
  staleInitMatch.mountSettings = () => {};
  staleInitMatch.logPredictionStatus = () => {};
  staleInitMatch.initPredictionAdapter();
  staleInitMatch.setPredictionEnabled(false);
  staleInitMatch.setPredictionEnabled(true);
  await Promise.resolve();
  resolveStaleInit();
  await Promise.resolve();
  assert(staleAdapter.destroyed, "stale in-flight prediction init is destroyed after the toggle-off token changes");
  assert(freshAdapter.ready && !freshAdapter.destroyed, "stale init completion does not destroy the re-enabled adapter");

  const mismatchMatch = Object.create(Match.prototype);
  mismatchMatch.prediction = {
    enabled: true,
    reset({ enabled }) {
      this.enabled = enabled;
    },
  };
  mismatchMatch.predictionStateMismatchLogged = false;
  let mismatchStatus = null;
  mismatchMatch.logPredictionStatus = (status) => { mismatchStatus = status; };
  mismatchMatch.state = {};
  mismatchMatch.advancePredictionVisual();
  assert(!mismatchMatch.prediction.enabled, "stale cached state module disables prediction instead of crashing");
  assert(mismatchStatus === "disabled-state-mismatch", "state mismatch is logged for diagnostics");

  const pausePredictionMatch = Object.create(Match.prototype);
  const pausePredictionOverlays = [];
  let pauseVisualClockCalls = 0;
  let advanceVisualCalls = 0;
  pausePredictionMatch.livePauseState = { paused: true };
  pausePredictionMatch.predictionVisualSuspended = false;
  pausePredictionMatch.prediction = { enabled: true };
  pausePredictionMatch.predictionAdapter = {
    ready: true,
    pauseVisualClock() {
      pauseVisualClockCalls += 1;
    },
    advanceVisual() {
      advanceVisualCalls += 1;
      return { tick: 2, entities: [] };
    },
    diagnostics: () => ({}),
  };
  pausePredictionMatch.state = {
    applyPredictionDisplayOverlay(overlay) {
      pausePredictionOverlays.push(overlay);
    },
  };
  pausePredictionMatch.predictionStateCompatible = () => true;
  pausePredictionMatch.applyPredictionDisplayOverlay = Match.prototype.applyPredictionDisplayOverlay;
  pausePredictionMatch.publishPredictionDebug = () => {};
  pausePredictionMatch.advancePredictionVisual();
  assert(advanceVisualCalls === 0, "live pause stops per-frame movement prediction ticks");
  assert(pauseVisualClockCalls === 1, "live pause keeps the prediction visual clock synced to wall time");
  assert(
    pausePredictionOverlays.at(-1)?.predictedSnapshot === null,
    "live pause clears the predicted movement overlay",
  );

  pausePredictionMatch.livePauseState = { paused: false };
  pausePredictionMatch.predictionVisualSuspended = true;
  pausePredictionMatch.advancePredictionVisual();
  assert(advanceVisualCalls === 0, "prediction stays suspended until a post-unpause snapshot is applied");
  pausePredictionMatch.predictionVisualSuspended = false;
  pausePredictionMatch.advancePredictionVisual();
  assert(advanceVisualCalls === 1, "prediction resumes after the snapshot gate clears");

  const livePauseStateMatch = Object.create(Match.prototype);
  const livePauseOverlays = [];
  const progressPauseStates = [];
  livePauseStateMatch.livePauseState = { paused: false };
  livePauseStateMatch.predictionVisualSuspended = false;
  livePauseStateMatch.predictionAdapter = { pauseVisualClock() {} };
  livePauseStateMatch.state = {
    applyPredictionDisplayOverlay(overlay) {
      livePauseOverlays.push(overlay);
    },
    setProgressPredictionPaused(paused) {
      progressPauseStates.push(paused);
    },
  };
  livePauseStateMatch.publishPredictionDebug = () => {};
  livePauseStateMatch.livePauseOverlay = { applyLivePauseState() {} };
  livePauseStateMatch.syncLivePauseUi = () => {};
  const worldBedStates = [];
  livePauseStateMatch.combatAudio = {
    updateWorldCombatBed(active) { worldBedStates.push(active); },
  };
  livePauseStateMatch.applyLivePauseState({ paused: true, canPause: false, canUnpause: true });
  assert(livePauseStateMatch.predictionVisualSuspended, "entering live pause suspends prediction visuals");
  assert(livePauseOverlays.at(-1)?.predictedSnapshot === null, "entering live pause drops any predicted movement frame");
  assert(progressPauseStates.at(-1) === true, "live pause freezes progress prediction for a non-pausing client");
  assert(worldBedStates.at(-1) === false, "entering live pause fades out the world combat bed");
  livePauseStateMatch.applyLivePauseState({ paused: false, canPause: true, canUnpause: false });
  assert(
    livePauseStateMatch.predictionVisualSuspended,
    "leaving live pause keeps prediction suspended until the next authoritative snapshot",
  );
  assert(progressPauseStates.at(-1) === true, "unpause keeps progress frozen until an authoritative snapshot");
  notePredictionAuthoritativeSnapshot(livePauseStateMatch);
  assert(progressPauseStates.at(-1) === false, "the first post-unpause snapshot resumes progress prediction");

  livePauseStateMatch.roomTimeControls = { applyRoomTimeState() {} };
  livePauseStateMatch.applyRoomTimeState({ currentTick: 90, durationTicks: 600, speed: 0, paused: true });
  assert(worldBedStates.at(-1) === false, "room-time pause fades out the world combat bed");
  livePauseStateMatch.applyRoomTimeState({ currentTick: 600, durationTicks: 600, speed: 2, paused: false });
  assert(worldBedStates.at(-1) === false, "ended replay playback fades out the world combat bed");

  const manualPointerLockMatch = Object.create(Match.prototype);
  let toggledPointerLock = 0;
  let closedSettings = 0;
  manualPointerLockMatch.input = {
    pointerLocked: false,
    pointerLockSupported: () => true,
    togglePointerLock() {
      toggledPointerLock += 1;
    },
  };
  manualPointerLockMatch.closeSettingsMenu = () => {
    closedSettings += 1;
  };
  manualPointerLockMatch.syncPointerLockUi = () => {};
  manualPointerLockMatch.togglePointerLock();
  assert(toggledPointerLock === 1, "browser cursor-lock action remains manual");
  assert(closedSettings === 1, "manual cursor-lock request closes settings before requesting lock");

  let unsupportedToast = null;
  manualPointerLockMatch.input.pointerLockSupported = () => false;
  manualPointerLockMatch.toast = (msg) => {
    unsupportedToast = msg;
  };
  manualPointerLockMatch.togglePointerLock();
  assert(toggledPointerLock === 1, "unsupported cursor-lock action does not request Pointer Lock");
  assert(unsupportedToast === "Cursor lock is not supported by this browser.",
    "unsupported cursor lock surfaces the existing support message");

  const priorDesktopRuntime = globalThis.__RTS_DESKTOP_RUNTIME;
  const priorWindowSetTimeout = globalThis.window.setTimeout;
  const priorWindowClearTimeout = globalThis.window.clearTimeout;
  const priorDocumentAddEventListener = globalThis.document.addEventListener;
  const priorDocumentRemoveEventListener = globalThis.document.removeEventListener;
  const priorDocumentHasFocus = globalThis.document.hasFocus;
  const priorWindowSearch = globalThis.window.location.search;
  const documentListeners = new Map();
  const timers = [];
  const clearedTimers = [];
  globalThis.__RTS_DESKTOP_RUNTIME = {
    shell: "tauri",
    nativeCursorCapture: true,
    aggressiveCursorLock: true,
  };
  globalThis.window.setTimeout = (fn, ms) => {
    const id = timers.length + 1;
    timers.push({ id, fn, ms });
    return id;
  };
  globalThis.window.clearTimeout = (id) => {
    clearedTimers.push(id);
  };
  globalThis.document.addEventListener = (type, handler) => {
    documentListeners.set(type, handler);
  };
  globalThis.document.removeEventListener = (type, handler) => {
    if (documentListeners.get(type) === handler) documentListeners.delete(type);
  };
  try {
    const optInMatch = Object.create(Match.prototype);
    optInMatch.replayViewer = false;
    optInMatch.input = { requestPointerLock() {} };
    assert(optInMatch.shouldUseDesktopCursorAutoLock(), "Tauri native cursor runtime opts matches into aggressive cursor lock");
    optInMatch.replayViewer = true;
    assert(!optInMatch.shouldUseDesktopCursorAutoLock(), "replay viewers do not auto-lock the cursor");
    globalThis.window.location.search = "?rtsNoAutoPointerLock=1";
    optInMatch.replayViewer = false;
    assert(!optInMatch.shouldUseDesktopCursorAutoLock(), "rtsNoAutoPointerLock disables desktop cursor auto-lock");
    globalThis.window.location.search = "";

    const autoLockMatch = Object.create(Match.prototype);
    let requestedLocks = 0;
    let autoClosedSettings = 0;
    let syncedPointerUi = 0;
    let lockToast = null;
    autoLockMatch.replayViewer = false;
    autoLockMatch.desktopCursorAutoLockEnabled = true;
    autoLockMatch.desktopCursorAutoLockTimer = null;
    autoLockMatch.desktopCursorAutoLockInFlight = false;
    autoLockMatch.desktopCursorAutoLockFailures = 0;
    autoLockMatch.onDesktopCursorAutoLockSignal = autoLockMatch.handleDesktopCursorAutoLockSignal.bind(autoLockMatch);
    autoLockMatch.input = {
      pointerLocked: false,
      pointerLockSupported: () => true,
      requestPointerLock() {
        requestedLocks += 1;
        this.pointerLocked = true;
        autoLockMatch.handlePointerLockChange(true);
        return Promise.resolve(true);
      },
    };
    autoLockMatch.closeSettingsMenu = () => { autoClosedSettings += 1; };
    autoLockMatch.toast = (msg) => { lockToast = msg; };
    autoLockMatch.syncPointerLockUi = () => { syncedPointerUi += 1; };
    autoLockMatch.installDesktopCursorAutoLock();
    assert(timers[0]?.ms === 250, "desktop cursor auto-lock waits briefly after match mount");
    timers.shift().fn();
    await Promise.resolve();
    assert(requestedLocks === 1, "desktop cursor auto-lock requests capture after match mount");
    assert(autoClosedSettings === 1, "desktop cursor auto-lock closes settings after capture succeeds");
    assert(lockToast === "Cursor locked. Alt-Tab to leave the game.",
      "desktop cursor auto-lock explains Alt-Tab release");

    autoLockMatch.input.pointerLocked = false;
    autoLockMatch.handlePointerLockChange(false);
    assert(timers[0]?.ms === 120, "focused cursor unlock schedules a quick desktop relock");
    timers.shift().fn();
    await Promise.resolve();
    assert(requestedLocks === 2, "focused cursor unlock re-requests desktop cursor capture");
    assert(syncedPointerUi >= 2, "desktop cursor auto-lock keeps the settings UI synchronized");

    autoLockMatch.input.pointerLocked = false;
    autoLockMatch.handleDesktopCursorAutoLockSignal();
    const pendingTimer = autoLockMatch.desktopCursorAutoLockTimer;
    autoLockMatch.teardownDesktopCursorAutoLock();
    assert(clearedTimers.includes(pendingTimer), "desktop cursor auto-lock clears pending relock timers on teardown");
    assert(!windowListeners.has("focus") && !windowListeners.has("pageshow") && !documentListeners.has("visibilitychange"),
      "desktop cursor auto-lock removes focus and visibility listeners on teardown");
  } finally {
    if (priorDesktopRuntime === undefined) delete globalThis.__RTS_DESKTOP_RUNTIME;
    else globalThis.__RTS_DESKTOP_RUNTIME = priorDesktopRuntime;
    globalThis.window.setTimeout = priorWindowSetTimeout;
    if (priorWindowClearTimeout === undefined) delete globalThis.window.clearTimeout;
    else globalThis.window.clearTimeout = priorWindowClearTimeout;
    if (priorDocumentAddEventListener === undefined) delete globalThis.document.addEventListener;
    else globalThis.document.addEventListener = priorDocumentAddEventListener;
    if (priorDocumentRemoveEventListener === undefined) delete globalThis.document.removeEventListener;
    else globalThis.document.removeEventListener = priorDocumentRemoveEventListener;
    globalThis.document.hasFocus = priorDocumentHasFocus;
    globalThis.window.location.search = priorWindowSearch;
  }

  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}
