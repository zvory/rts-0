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
  msg,
} from "../../client/src/protocol.js";
import { CameraNavigationInput } from "../../client/src/input/camera_navigation.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { ReplayCameraInput } from "../../client/src/replay_camera_input.js";
import { LivePauseOverlay } from "../../client/src/live_pause_overlay.js";
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
  const { ReplayViewer } = await import("../../client/src/replay_viewer.js");
  const { ReplayControls, RoomTimeControls } = await import("../../client/src/replay_controls.js");
  const { applyMatchUnitRanges } = await import("../../client/src/match_settings_toggles.js");
  const { shouldWarnBeforeUnload } = await import("../../client/src/app.js");
  const { dom } = await import("../../client/src/bootstrap.js");
  assert(ReplayViewer.prototype instanceof Match, "ReplayViewer reuses Match rendering lifecycle");
  assert(ReplayControls.prototype instanceof RoomTimeControls, "replay controls keep a neutral room-time base");
  assert(!("command" in ReplayCameraInput.prototype), "Replay camera input has no gameplay command API");
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
    assert(selectionArea.hidden, "replay viewer hides the selected-unit HUD area");
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
    labOperatorMatch.state = {
      spectator: true,
      controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } }),
    };
    labOperatorMatch.applySpectatorUi();
    assert(!selectionArea.hidden, "lab operator keeps the selected-unit HUD area visible");
    assert(!commandCard.hidden, "lab operator keeps the command card visible");

    selectionArea.hidden = false;
    commandCard.hidden = false;
    const labViewerMatch = Object.create(Match.prototype);
    labViewerMatch.replayViewer = false;
    labViewerMatch.state = {
      spectator: true,
      controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } }),
    };
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
      { kind: "spawnEntity", keepArmedOnWorldClick: true },
      { onWorldClick: () => {} },
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
      zoom: 1,
      calls: [],
      pans: [],
      setZoom(zoom, x, y) {
        this.calls.push({ zoom, x, y });
        this.zoom = zoom;
      },
      panByScreenDelta(dx, dy) {
        this.pans.push({ dx, dy });
      },
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
      const replayInput = new ReplayCameraInput(viewport, camera);
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
      assertApprox(camera.zoom, 1.12, 0.000001, "Replay mouse wheel zooms in");
      assert(camera.calls[0].x === 200 && camera.calls[0].y === 150, "Replay wheel zoom anchors on cursor");
      listeners.get("wheel")({
        deltaY: 100,
        clientX: 220,
        clientY: 180,
        preventDefault() {
          prevented += 1;
        },
      });
      assertApprox(camera.zoom, 1, 0.000001, "Replay mouse wheel zooms out");
      assert(prevented === 2, "Replay wheel zoom prevents page scroll");
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
    const helper = new CameraNavigationInput(viewport, { zoom: 1 }, {
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

  function fakeEl(tag = "div") {
    const el = {
      tagName: tag.toUpperCase(),
      children: [],
      dataset: {},
      style: { setProperty(name, value) { this[name] = value; } },
      hidden: false,
      textContent: "",
      className: "",
      _listeners: new Map(),
      classList: {
        add(cls) {
          if (!el.className.split(/\s+/).includes(cls)) el.className = `${el.className} ${cls}`.trim();
        },
        remove(cls) {
          el.className = el.className.split(/\s+/).filter((c) => c && c !== cls).join(" ");
        },
        toggle(cls, force) {
          const active = force === undefined ? !this.contains(cls) : !!force;
          if (active) this.add(cls);
          else this.remove(cls);
          return active;
        },
        contains(cls) {
          return el.className.split(/\s+/).includes(cls);
        },
      },
      setAttribute(name, value) {
        this[name] = value;
      },
      appendChild(child) {
        child.parentNode = this;
        this.children.push(child);
        return child;
      },
      replaceChildren(...children) {
        this.children = [];
        for (const child of children) this.appendChild(child);
      },
      addEventListener(type, handler) {
        this._listeners.set(type, handler);
      },
      removeEventListener(type, handler) {
        if (this._listeners.get(type) === handler) this._listeners.delete(type);
      },
      dispatchEvent(ev) {
        this._listeners.get(ev.type)?.(ev);
      },
      remove() {
        if (!this.parentNode) return;
        this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
        this.parentNode = null;
      },
      closest(selector) {
        if (selector.startsWith(".") && this.classList.contains(selector.slice(1))) return this;
        return this.parentNode?.closest?.(selector) || null;
      },
      getBoundingClientRect() {
        const px = (value, fallback) => {
          if (typeof value !== "string" || !value.endsWith("px")) return fallback;
          const parsed = Number.parseFloat(value);
          return Number.isFinite(parsed) ? parsed : fallback;
        };
        return {
          left: px(this.style.left, 0),
          top: px(this.style.top, 0),
          width: px(this.style.width, 200),
          height: px(this.style.height, 80),
        };
      },
      querySelector(selector) {
        return this.querySelectorAll(selector)[0] || null;
      },
      querySelectorAll(selector) {
        const out = [];
        const matches = (node) => {
          if (selector.includes(",")) {
            return selector.split(",").some((part) => {
              const trimmed = part.trim();
              return trimmed.startsWith(".") && node.classList?.contains(trimmed.slice(1));
            });
          }
          if (selector === ".spd-btn:not(.seek-btn)") {
            return node.classList?.contains("spd-btn") && !node.classList?.contains("seek-btn");
          }
          if (selector.startsWith("#")) return node.id === selector.slice(1);
          if (selector.startsWith(".")) return node.classList?.contains(selector.slice(1));
          return false;
        };
        const walk = (node) => {
          for (const child of node.children || []) {
            if (matches(child)) out.push(child);
            walk(child);
          }
        };
        walk(this);
        return out;
      },
    };
    return el;
  }

  globalThis.document.createElement = fakeEl;
  const replayControls = fakeEl("div");
  const speed2 = fakeEl("button");
  speed2.className = "spd-btn";
  speed2.dataset.speed = "2";
  const speed0 = fakeEl("button");
  speed0.className = "spd-btn room-time-pause-btn";
  speed0.dataset.speed = "0";
  const seekBack = fakeEl("button");
  seekBack.className = "spd-btn seek-btn";
  seekBack.dataset.seekBack = "90";
  const stepDev = fakeEl("button");
  stepDev.className = "spd-btn room-time-step-btn";
  stepDev.dataset.stepRoomTime = "";
  const concluded = fakeEl("span");
  concluded.id = "room-time-concluded";
  replayControls.appendChild(speed2);
  replayControls.appendChild(speed0);
  replayControls.appendChild(seekBack);
  replayControls.appendChild(stepDev);
  replayControls.appendChild(concluded);
  dom.roomTimeControls = replayControls;
  const replayNet = {
    speeds: [],
    seekBacks: [],
    seekTargets: [],
    selections: [],
    branches: 0,
    steps: 0,
    setRoomTimeSpeed(speed) {
      this.speeds.push(speed);
    },
    seekRoomTime(ticksBack) {
      this.seekBacks.push(ticksBack);
    },
    seekRoomTimeTo(tick) {
      this.seekTargets.push(tick);
    },
    setVisionSelection(selection) {
      this.selections.push(selection);
    },
    requestBranchFromTick() {
      this.branches += 1;
    },
    stepRoomTime() {
      this.steps += 1;
    },
  };
  const roomTimeState = {
    players: [
      { id: 1, name: "Alpha", color: "#f00" },
      { id: 2, name: "Bravo", color: "#0f0" },
    ],
  };
  const replayUi = new ReplayControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: true,
    capabilities: createRoomCapabilities({
      startPayload: {
        replay: { durationTicks: 1_000 },
        capabilities: {
          roomTime: {
            available: true,
            setSpeed: true,
            pause: true,
            seekRelative: true,
            seekAbsolute: true,
            timeline: true,
          },
          visibility: { visionSelection: true },
          actions: { branchFromTick: true },
        },
      },
    }),
  });
  assert(speed2.classList.contains("active"), "replay speed defaults can mark 2x active");
  assert(replayControls.classList.contains("replay-viewer-controls"), "replay viewer controls keep wrapper class");
  assert(replayControls.classList.contains("room-time-floating-panel"), "room-time controls mount as a floating panel");
  const dragHandle = replayControls.querySelector(".room-time-panel-drag-handle");
  assert(replayControls.querySelector(".room-time-panel-title")?.textContent === "Replay",
    "floating replay controls include a labeled drag handle");
  assert(replayControls.querySelector(".room-time-panel-body")?.querySelector(".seek-btn") === seekBack,
    "floating panel wraps the existing room-time buttons in its body");
  assert(!seekBack.hidden, "replay seek buttons stay visible in replay mode");
  assert(stepDev.hidden, "scenario step controls stay hidden in replay mode");
  dragHandle._listeners.get("pointerdown")({
    button: 0,
    isPrimary: true,
    pointerId: 7,
    clientX: 20,
    clientY: 30,
    currentTarget: dragHandle,
    preventDefault() {},
    stopPropagation() {},
  });
  windowListeners.get("pointermove")({
    pointerId: 7,
    clientX: 120,
    clientY: 80,
    preventDefault() {},
  });
  assert(replayControls.style.left === "112px" && replayControls.style.top === "62px",
    "dragging the floating room-time panel updates its screen position");
  windowListeners.get("pointerup")({ pointerId: 7 });
  assert(localStorageValues.has("rts.roomTimeControls.panel.v1"),
    "floating room-time panel position is persisted after drag");
  dragHandle._listeners.get("keydown")({
    key: "ArrowRight",
    preventDefault() {},
  });
  assert(replayControls.style.left === "136px", "drag handle arrow keys nudge the room-time panel");
  const collapsePanel = replayControls.querySelector(".room-time-panel-collapse");
  collapsePanel._listeners.get("click")({});
  assert(
    replayControls.dataset.collapsed === "true" &&
      replayControls.querySelector(".room-time-panel-body").hidden === true &&
      JSON.parse(localStorageValues.get("rts.roomTimeControls.panel.v1")).collapsed === true,
    "collapse hides and persists the floating room-time panel body",
  );
  assert(
    replayControls.querySelector(".room-time-panel-reset") === null,
    "floating room-time panel omits the position reset button",
  );
  dragHandle._listeners.get("keydown")({
    key: "Home",
    preventDefault() {},
  });
  assert(!localStorageValues.has("rts.roomTimeControls.panel.v1"), "reset clears the persisted room-time panel position");
  assert(replayControls.style.left === "" && replayControls.style.top === "",
    "reset returns the floating room-time panel to its default CSS position");
  assert(
    replayControls.dataset.collapsed === "false" &&
      replayControls.querySelector(".room-time-panel-body").hidden === false,
    "reset expands the floating room-time panel",
  );
  const pauseReplay = replayControls.querySelector(".replay-pause-btn");
  assert(pauseReplay?.textContent === "Pause", "replay viewer builds a pause button");
  const branchReplay = replayControls.querySelector(".replay-branch-btn");
  assert(branchReplay?.textContent === "Resume play from here", "replay branch button describes resuming from the current tick");
  replayControls._listeners.get("click")({ target: speed2 });
  assert(replayNet.speeds.at(-1) === 2, "speed click sends net.setRoomTimeSpeed");
  replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2 });
  replayControls._listeners.get("click")({ target: pauseReplay });
  assert(replayNet.speeds.at(-1) === 0, "replay pause button sends zero playback speed");
  assert(pauseReplay.textContent === "Resume", "paused replay button switches to resume");
  replayControls._listeners.get("click")({ target: pauseReplay });
  assert(replayNet.speeds.at(-1) === 2, "replay resume button restores the last non-zero speed");
  assert(pauseReplay.textContent === "Pause", "resumed replay button switches back to pause");
  replayControls._listeners.get("click")({ target: seekBack });
  assert(replayNet.seekBacks.at(-1) === 90, "seek click sends net.seekRoomTime");
  const visionButtons = replayControls.querySelectorAll(".vision-btn");
  assert(visionButtons.length === 3, "replay viewer builds all-player and per-player fog controls");
  replayUi.onVisionSelectionClick({ target: visionButtons[1], shiftKey: false });
  assert(
    replayNet.selections.at(-1).mode === "player" &&
      replayNet.selections.at(-1).playerId === 1,
    "single replay fog click sends a per-viewer player vision request",
  );
  replayUi.onVisionSelectionClick({ target: visionButtons[2], shiftKey: true });
  assert(
    replayNet.selections.at(-1).mode === "players" &&
      replayNet.selections.at(-1).playerIds.join(",") === "1,2",
    "shift-click replay fog controls send a selected-players request",
  );
  replayUi.onVisionSelectionClick({ target: visionButtons[0], shiftKey: false });
  assert(replayNet.selections.at(-1).mode === "all", "all replay fog control restores union vision");
  replayUi.applyRoomTimeState({
    currentTick: 100,
    durationTicks: 1_000,
    keyframeTicks: [0, 400, 800],
    speed: 2,
    paused: false,
    ended: false,
  });
  assert(
    replayControls.querySelectorAll(".room-time-timeline-mark").length === 3,
    "replay timeline renders server keyframe marks",
  );
  const timelineTrack = replayControls.querySelector(".room-time-timeline-track");
  replayUi.onRoomTimeTimelineClick({ currentTarget: timelineTrack, clientX: 100 });
  assert(replayNet.seekTargets.at(-1) === 500, "replay timeline click seeks to the clicked tick");
  assert(
    replayControls.querySelector(".room-time-tick-status").textContent.includes("Seeking 500"),
    "replay timeline shows a pending seek indicator",
  );
  replayUi.destroy();
  assert(replayControls.hidden, "destroy hides replay controls");
  assert(!replayControls.classList.contains("replay-viewer-controls"), "destroy clears replay wrapper class");
  assert(!seekBack.hidden, "destroy restores seek controls visible");
  assert(stepDev.hidden, "destroy restores scenario step controls hidden");
  assert(!replayControls.querySelector(".replay-pause-btn"), "destroy removes generated replay pause button");
  assert(!replayControls.querySelector(".replay-branch-btn"), "destroy removes generated replay branch button");
  assert(!replayControls.querySelector(".vision-selection-controls"), "destroy removes generated vision controls");
  assert(!replayControls.querySelector(".room-time-tick-status"), "destroy removes generated status");
  assert(!replayControls.querySelector(".room-time-timeline"), "destroy removes generated timeline");
  assert(!replayControls.querySelector(".room-time-panel-drag-handle"), "destroy removes floating room-time panel chrome");
  assert(replayControls.children.includes(seekBack), "destroy unwraps static room-time controls back onto the root");
  assert(replayControls._listeners.size === 0, "destroy removes room-time click listener");

  localStorageValues.set("rts.roomTimeControls.panel.v1", JSON.stringify({
    schemaVersion: 1,
    left: 260,
    top: 70,
    collapsed: true,
  }));
  globalThis.window.innerWidth = 390;
  const mobileRoomTimeControls = fakeEl("div");
  const mobileSpeed = fakeEl("button");
  mobileSpeed.className = "spd-btn";
  mobileSpeed.dataset.speed = "1";
  mobileRoomTimeControls.appendChild(mobileSpeed);
  dom.roomTimeControls = mobileRoomTimeControls;
  const mobileRoomTimeUi = new RoomTimeControls({
    net: replayNet,
    state: roomTimeState,
    capabilities: createRoomCapabilities({
      startPayload: {
        capabilities: {
          roomTime: { available: true, setSpeed: true },
        },
      },
    }),
  });
  assert(!mobileRoomTimeControls.style.left && !mobileRoomTimeControls.style.top,
    "room-time controls ignore saved desktop panel position on mobile widths");
  assert(
    mobileRoomTimeControls.dataset.collapsed === "true" &&
      mobileRoomTimeControls.querySelector(".room-time-panel-body").hidden === true,
    "room-time controls preserve saved collapsed state without restoring overlapping geometry",
  );
  mobileRoomTimeControls.querySelector(".room-time-panel-collapse")._listeners.get("click")({});
  const mobileRoomTimeStored = JSON.parse(localStorageValues.get("rts.roomTimeControls.panel.v1"));
  assert(
    mobileRoomTimeStored.left === 260 &&
      mobileRoomTimeStored.top === 70 &&
      mobileRoomTimeStored.collapsed === false,
    "room-time controls preserve saved desktop position when collapse is toggled on mobile",
  );
  globalThis.window.innerWidth = 1000;
  windowListeners.get("resize")();
  assert(
    mobileRoomTimeControls.style.left === "260px" &&
      mobileRoomTimeControls.style.top === "70px",
    "room-time controls restore saved desktop position after leaving mobile layout",
  );
  mobileRoomTimeUi.destroy();

  const visionSelectionOnlyControls = fakeEl("div");
  dom.roomTimeControls = visionSelectionOnlyControls;
  const visionSelectionOnlyUi = new ReplayControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: true,
    capabilities: createRoomCapabilities({
      startPayload: {
        replay: { durationTicks: 1_000 },
        capabilities: {
          roomTime: { available: true },
          visibility: { visionSelection: true },
        },
      },
    }),
  });
  assert(
    visionSelectionOnlyControls.querySelector(".vision-selection-controls"),
    "vision selection capability still builds replay fog controls",
  );
  assert(
    !visionSelectionOnlyControls.querySelector(".replay-branch-btn"),
    "vision selection alone does not build a replay branch button",
  );
  visionSelectionOnlyUi.destroy();

  const scenarioControls = fakeEl("div");
  const scenarioSpeed2 = fakeEl("button");
  scenarioSpeed2.className = "spd-btn";
  scenarioSpeed2.dataset.speed = "2";
  const scenarioSpeed0 = fakeEl("button");
  scenarioSpeed0.className = "spd-btn room-time-pause-btn";
  scenarioSpeed0.dataset.speed = "0";
  const scenarioStep = fakeEl("button");
  scenarioStep.className = "spd-btn room-time-step-btn";
  scenarioStep.dataset.stepRoomTime = "";
  const scenarioSeek = fakeEl("button");
  scenarioSeek.className = "spd-btn seek-btn";
  scenarioSeek.dataset.seekBack = "30";
  scenarioControls.appendChild(scenarioSpeed2);
  scenarioControls.appendChild(scenarioSpeed0);
  scenarioControls.appendChild(scenarioStep);
  scenarioControls.appendChild(scenarioSeek);
  dom.roomTimeControls = scenarioControls;
  const scenarioUi = new RoomTimeControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: false,
    capabilities: createRoomCapabilities({
      startPayload: {
        spectator: true,
        capabilities: {
          roomTime: {
            available: true,
            setSpeed: true,
            pause: true,
            step: true,
          },
        },
      },
    }),
  });
  assert(!scenarioSpeed2.hidden, "scenario mode shows positive speed controls when setSpeed is advertised");
  assert(scenarioSeek.hidden, "scenario mode hides replay seek buttons");
  assert(!scenarioSpeed0.hidden, "scenario mode shows pause controls when pause is advertised");
  assert(!scenarioStep.hidden, "scenario mode shows step controls");
  scenarioControls._listeners.get("click")({ target: scenarioSpeed2 });
  assert(replayNet.speeds.at(-1) === 2, "scenario speed click sends net.setRoomTimeSpeed");
  scenarioControls._listeners.get("click")({ target: scenarioStep });
  assert(replayNet.steps === 1, "scenario step sends net.stepRoomTime");
  scenarioControls._listeners.get("click")({ target: scenarioSpeed0 });
  assert(replayNet.speeds.at(-1) === 0, "scenario pause speed sends net.setRoomTimeSpeed");
  scenarioUi.destroy();

  const aiLiveControls = fakeEl("div");
  const aiLiveSpeed4 = fakeEl("button");
  aiLiveSpeed4.className = "spd-btn";
  aiLiveSpeed4.dataset.speed = "4";
  const aiLivePause = fakeEl("button");
  aiLivePause.className = "spd-btn room-time-pause-btn";
  aiLivePause.dataset.speed = "0";
  const aiLiveStep = fakeEl("button");
  aiLiveStep.className = "spd-btn room-time-step-btn";
  aiLiveStep.dataset.stepRoomTime = "";
  const aiLiveSeek = fakeEl("button");
  aiLiveSeek.className = "spd-btn seek-btn";
  aiLiveSeek.dataset.seekBack = "30";
  aiLiveControls.appendChild(aiLiveSpeed4);
  aiLiveControls.appendChild(aiLivePause);
  aiLiveControls.appendChild(aiLiveStep);
  aiLiveControls.appendChild(aiLiveSeek);
  dom.roomTimeControls = aiLiveControls;
  const aiLiveUi = new RoomTimeControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: false,
    capabilities: createRoomCapabilities({
      startPayload: {
        spectator: true,
        capabilities: {
          roomTime: {
            available: true,
            setSpeed: true,
            pause: true,
          },
        },
      },
    }),
  });
  assert(!aiLiveSpeed4.hidden, "AI-only live controls show positive speed controls");
  assert(!aiLivePause.hidden, "AI-only live controls show pause controls");
  assert(aiLiveStep.hidden, "AI-only live controls hide step without step capability");
  assert(aiLiveSeek.hidden, "AI-only live controls hide seek without seek capability");
  assert(!aiLiveControls.querySelector(".room-time-timeline"), "AI-only live controls do not build a timeline without seek");
  aiLiveControls._listeners.get("click")({ target: aiLiveSpeed4 });
  assert(replayNet.speeds.at(-1) === 4, "AI-only live speed click sends net.setRoomTimeSpeed");
  aiLiveControls._listeners.get("click")({ target: aiLivePause });
  assert(replayNet.speeds.at(-1) === 0, "AI-only live pause sends zero room-time speed");
  assert(aiLivePause.textContent === "Resume", "paused AI-only live control switches to resume");
  aiLiveControls._listeners.get("click")({ target: aiLivePause });
  assert(replayNet.speeds.at(-1) === 4, "AI-only live resume restores the last selected speed");
  const aiLiveSpeedsBeforeHiddenClicks = replayNet.speeds.length;
  const aiLiveSeeksBeforeHiddenClicks = replayNet.seekBacks.length;
  const aiLiveStepsBeforeHiddenClicks = replayNet.steps;
  aiLiveControls._listeners.get("click")({ target: aiLiveStep });
  aiLiveControls._listeners.get("click")({ target: aiLiveSeek });
  assert(replayNet.speeds.length === aiLiveSpeedsBeforeHiddenClicks, "AI-only live hidden step control is inert");
  assert(replayNet.seekBacks.length === aiLiveSeeksBeforeHiddenClicks, "AI-only live hidden seek control is inert");
  assert(replayNet.steps === aiLiveStepsBeforeHiddenClicks, "AI-only live hidden step does not send");
  aiLiveUi.destroy();

  const labControls = fakeEl("div");
  const labSpeed2 = fakeEl("button");
  labSpeed2.className = "spd-btn";
  labSpeed2.dataset.speed = "2";
  const labPause = fakeEl("button");
  labPause.className = "spd-btn room-time-pause-btn";
  labPause.dataset.speed = "0";
  const labStep = fakeEl("button");
  labStep.className = "spd-btn room-time-step-btn";
  labStep.dataset.stepRoomTime = "";
  const labSeek = fakeEl("button");
  labSeek.className = "spd-btn seek-btn";
  labSeek.dataset.seekBack = "30";
  labControls.appendChild(labSpeed2);
  labControls.appendChild(labPause);
  labControls.appendChild(labStep);
  labControls.appendChild(labSeek);
  dom.roomTimeControls = labControls;
  const labUi = new RoomTimeControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: false,
    capabilities: createRoomCapabilities({
      startPayload: {
        spectator: true,
        lab: { room: "sandbox", role: "operator" },
        capabilities: {
          roomTime: {
            available: true,
            setSpeed: true,
            pause: true,
            step: true,
            seekRelative: true,
            seekAbsolute: true,
            timeline: true,
          },
        },
      },
    }),
  });
  assert(!labSpeed2.hidden, "lab mode shows positive speed controls when setSpeed is advertised");
  assert(!labPause.hidden, "lab mode shows pause controls when pause is advertised");
  assert(!labStep.hidden, "lab mode shows step controls when step is advertised");
  assert(!labSeek.hidden, "lab mode shows relative seek controls when seekRelative is advertised");
  assert(!labControls.querySelector(".vision-selection-controls"), "lab room-time controls do not create vision selection controls");
  assert(!labControls.querySelector(".replay-branch-btn"), "lab room-time controls do not create replay branch controls");
  labControls._listeners.get("click")({ target: labStep });
  assert(replayNet.steps === 2, "lab step sends net.stepRoomTime through neutral controls");
  labUi.applyRoomTimeState({
    currentTick: 120,
    durationTicks: 600,
    keyframeTicks: [0, 200, 400],
    speed: 1,
    paused: false,
  });
  assert(
    labControls.querySelectorAll(".room-time-timeline-mark").length === 3,
    "lab timeline renders server keyframe marks through neutral room-time controls",
  );
  assert(
    labControls.querySelector(".room-time-tick-status").textContent.startsWith("Room time"),
    "lab room-time status uses neutral copy instead of replay copy",
  );
  labControls._listeners.get("click")({ target: labSeek });
  assert(replayNet.seekBacks.at(-1) === 30, "lab relative seek sends net.seekRoomTime through neutral controls");
  const labTimelineTrack = labControls.querySelector(".room-time-timeline-track");
  labUi.onRoomTimeTimelineClick({ currentTarget: labTimelineTrack, clientX: 100 });
  assert(replayNet.seekTargets.at(-1) === 300, "lab timeline click sends an absolute room-time seek");
  labControls._listeners.get("click")({ target: labPause });
  assert(replayNet.speeds.at(-1) === 0, "lab pause sends net.setRoomTimeSpeed");
  assert(labPause.textContent === "Resume", "paused lab room-time control switches to resume");
  labControls._listeners.get("click")({ target: labPause });
  assert(replayNet.speeds.at(-1) === 1, "lab resume restores the last positive room-time speed");
  labUi.destroy();

  const stepOnlyControls = fakeEl("div");
  const stepOnlySpeed = fakeEl("button");
  stepOnlySpeed.className = "spd-btn";
  stepOnlySpeed.dataset.speed = "2";
  const stepOnlyPause = fakeEl("button");
  stepOnlyPause.className = "spd-btn room-time-pause-btn";
  stepOnlyPause.dataset.speed = "0";
  const stepOnlyStep = fakeEl("button");
  stepOnlyStep.className = "spd-btn room-time-step-btn";
  stepOnlyStep.dataset.stepRoomTime = "";
  const stepOnlySeek = fakeEl("button");
  stepOnlySeek.className = "spd-btn seek-btn";
  stepOnlySeek.dataset.seekBack = "30";
  stepOnlyControls.appendChild(stepOnlySpeed);
  stepOnlyControls.appendChild(stepOnlyPause);
  stepOnlyControls.appendChild(stepOnlyStep);
  stepOnlyControls.appendChild(stepOnlySeek);
  dom.roomTimeControls = stepOnlyControls;
  const stepOnlyUi = new RoomTimeControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: false,
    capabilities: createRoomCapabilities({
      startPayload: {
        spectator: true,
        capabilities: {
          roomTime: {
            available: true,
            step: true,
          },
        },
      },
    }),
  });
  assert(stepOnlySpeed.hidden, "positive speed controls hide without setSpeed capability");
  assert(stepOnlyPause.hidden, "pause controls hide without pause capability");
  assert(!stepOnlyStep.hidden, "step controls show with step capability");
  assert(stepOnlySeek.hidden, "relative seek controls hide without seekRelative capability");
  const speedsBeforeStepOnlyClicks = replayNet.speeds.length;
  const seeksBeforeStepOnlyClicks = replayNet.seekBacks.length;
  const stepsBeforeStepOnlyClicks = replayNet.steps;
  stepOnlyControls._listeners.get("click")({ target: stepOnlySpeed });
  stepOnlyControls._listeners.get("click")({ target: stepOnlyPause });
  stepOnlyControls._listeners.get("click")({ target: stepOnlySeek });
  assert(replayNet.speeds.length === speedsBeforeStepOnlyClicks, "hidden speed/pause controls are inert without capability");
  assert(replayNet.seekBacks.length === seeksBeforeStepOnlyClicks, "hidden seek controls are inert without capability");
  stepOnlyControls._listeners.get("click")({ target: stepOnlyStep });
  assert(replayNet.steps === stepsBeforeStepOnlyClicks + 1, "step controls still send when step is advertised");
  stepOnlyUi.destroy();

  const noCapabilityControls = fakeEl("div");
  dom.roomTimeControls = noCapabilityControls;
  const noCapabilityUi = new ReplayControls({
    net: replayNet,
    state: roomTimeState,
    replayViewer: true,
    capabilities: createRoomCapabilities({ startPayload: { spectator: true, replay: {} } }),
  });
  assert(!noCapabilityControls._listeners.has("click"), "room-time controls need an advertised capability");
  assert(
    !noCapabilityControls.querySelector(".vision-selection-controls"),
    "replay identity alone does not build vision selection controls",
  );
  noCapabilityUi.destroy();

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
    let unpaused = false;
    const overlay = new LivePauseOverlay({ root, onUnpause: () => { unpaused = true; } });
    overlay.applyLivePauseState({ paused: true, pausedBy: 2, pauseLimit: 3, canUnpause: true });
    assert(root.children.length === 1, "live pause overlay mounts generated DOM");
    assert(!root.children[0].hidden, "live pause overlay shows when paused");
    const button = root.querySelector("#live-pause-unpause");
    assert(button && !button.hidden && !button.disabled, "live pause overlay enables unpause for pause-authorized viewers");
    button.listeners.click();
    assert(unpaused, "live pause overlay calls injected unpause action");
    overlay.applyLivePauseState({ paused: true, canUnpause: false });
    assert(button.hidden && button.disabled, "live pause overlay hides unpause without authority");
    overlay.applyLivePauseState({ paused: false });
    assert(root.children[0].hidden, "live pause overlay hides when running");
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
  noticeAudioMatch.camera = { x: 0, y: 0, viewW: 100, viewH: 100, zoom: 1 };
  noticeAudioMatch.state = { spectator: false };
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
    x: 512,
    y: 768,
  });
  assert(playedNotices.length === 0, "live spectator notice alerts do not play audio");
  assert(minimapPings === 2, "live spectator notice alerts still ping the minimap");
  noticeAudioMatch.state = { spectator: false };
  noticeAudioMatch.handleNotice({
    e: EVENT.NOTICE,
    msg: "alert:under_attack",
    severity: NOTICE_SEVERITY.ALERT,
    x: 512,
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
  livePauseStateMatch.livePauseState = { paused: false };
  livePauseStateMatch.predictionVisualSuspended = false;
  livePauseStateMatch.predictionAdapter = { pauseVisualClock() {} };
  livePauseStateMatch.state = {
    applyPredictionDisplayOverlay(overlay) {
      livePauseOverlays.push(overlay);
    },
  };
  livePauseStateMatch.publishPredictionDebug = () => {};
  livePauseStateMatch.livePauseOverlay = { applyLivePauseState() {} };
  livePauseStateMatch.syncLivePauseUi = () => {};
  livePauseStateMatch.applyLivePauseState({ paused: true, canPause: false, canUnpause: true });
  assert(livePauseStateMatch.predictionVisualSuspended, "entering live pause suspends prediction visuals");
  assert(livePauseOverlays.at(-1)?.predictedSnapshot === null, "entering live pause drops any predicted movement frame");
  livePauseStateMatch.applyLivePauseState({ paused: false, canPause: true, canUnpause: false });
  assert(
    livePauseStateMatch.predictionVisualSuspended,
    "leaving live pause keeps prediction suspended until the next authoritative snapshot",
  );

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
