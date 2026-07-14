// tests/client_contracts/room_time_controls_contracts.mjs
// Focused room-time/replay control contract assertions.

import { assert } from "./assertions.mjs";
import { LAB_ROLE } from "../../client/src/protocol.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { createRoomCapabilities } from "../../client/src/room_capabilities.js";
import { formatReplaySeekNotice } from "../../client/src/replay_seek_notice.js";

assert(
  formatReplaySeekNotice({ fromTick: 900, targetTick: 300 }) === "Seeking backward 20 seconds…",
  "replay seek notices describe authoritative backward distance in seconds",
);
assert(
  formatReplaySeekNotice({ fromTick: 300, targetTick: 345 }) === "Seeking forward 1.5 seconds…",
  "replay seek notices describe authoritative forward distance in seconds",
);
assert(
  formatReplaySeekNotice({ fromTick: 30, targetTick: 30 }) === "Seeking to the current replay position…",
  "replay seek notices handle an accepted no-distance seek",
);
assert(
  formatReplaySeekNotice({ fromTick: "invalid", targetTick: 30 }) === "",
  "replay seek notices ignore malformed protocol values",
);

const priorWindow = globalThis.window;
const priorDocument = globalThis.document;
const windowListeners = new Map();
const localStorageValues = new Map();
let coarsePointer = false;
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
  matchMedia(query) {
    return { matches: query === "(pointer: coarse)" && coarsePointer };
  },
};
globalThis.document = {
  hidden: false,
  hasFocus() { return true; },
  getElementById() { return fallbackElement; },
  createElement() { return { classList: { add() {} }, appendChild() {}, style: {} }; },
};

const { ReplayControls, RoomTimeControls } = await import("../../client/src/replay_controls.js");
const { dom } = await import("../../client/src/bootstrap.js");
const priorRoomTimeControls = dom.roomTimeControls;

try {
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
    return true;
  },
  seekRoomTime(ticksBack) {
    this.seekBacks.push(ticksBack);
    return true;
  },
  seekRoomTimeTo(tick) {
    this.seekTargets.push(tick);
    return true;
  },
  setVisionSelection(selection) {
    this.selections.push(selection);
  },
  requestBranchFromTick() {
    this.branches += 1;
    return true;
  },
  stepRoomTime() {
    this.steps += 1;
    return true;
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
assert(!speed2.classList.contains("active"), "replay speed stays unselected until room time is authoritative");
assert(speed2.disabled, "room-time actions stay inactive until the first authoritative state arrives");
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
replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2, paused: false });
speed2._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 2, "speed click sends net.setRoomTimeSpeed");
assert(speed2.classList.contains("active"), "speed click preserves its authoritative selection while confirmation is pending");
replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2 });

const originalSeekRoomTime = replayNet.seekRoomTime;
replayNet.seekRoomTime = (ticksBack) => {
  replayNet.seekBacks.push(ticksBack);
  return false;
};
seekBack._listeners.get("click")({});
assert(
  replayControls.querySelector(".room-time-tick-status").textContent.includes("was not sent") &&
    replayControls.dataset.roomTimePending === "false",
  "a blocked room-time send preserves authoritative presentation and explains the local failure",
);
replayNet.seekRoomTime = originalSeekRoomTime;

const actualDateNow = Date.now;
let roomTimeNow = 1_000;
Date.now = () => roomTimeNow;
const speedsBeforeTouch = replayNet.speeds.length;
speed2._listeners.get("pointerdown")({ button: 0, isPrimary: true, pointerId: 21, pointerType: "touch" });
speed2._listeners.get("pointerup")({
  pointerId: 21,
  pointerType: "touch",
  preventDefault() {},
  stopPropagation() {},
});
assert(
  replayNet.speeds.length === speedsBeforeTouch + 1 && replayControls.dataset.roomTimePending === "true",
  "a touch speed tap sends once and shows pending confirmation",
);
speed2._listeners.get("click")({ pointerType: "touch", detail: 1, preventDefault() {}, stopPropagation() {} });
assert(replayNet.speeds.length === speedsBeforeTouch + 1, "the synthesized touch click does not duplicate the speed request");
replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2, paused: false });

roomTimeNow += 1_000;
const speedsBeforeCancelledTouch = replayNet.speeds.length;
speed2._listeners.get("pointerdown")({ button: 0, isPrimary: true, pointerId: 22, pointerType: "touch" });
speed2._listeners.get("pointerleave")({ pointerId: 22, pointerType: "touch" });
speed2._listeners.get("pointerup")({ pointerId: 22, pointerType: "touch", preventDefault() {}, stopPropagation() {} });
assert(replayNet.speeds.length === speedsBeforeCancelledTouch, "a dragged or outside touch release invokes no room-time action");

const speedsBeforePen = replayNet.speeds.length;
pauseReplay._listeners.get("pointerdown")({ button: 0, isPrimary: true, pointerId: 23, pointerType: "pen" });
pauseReplay._listeners.get("pointerup")({
  pointerId: 23,
  pointerType: "pen",
  preventDefault() {},
  stopPropagation() {},
});
assert(replayNet.speeds.at(-1) === 0, "replay pause button sends zero playback speed");
assert(replayNet.speeds.length === speedsBeforePen + 1, "a pen pause tap sends exactly one request");
assert(pauseReplay.textContent === "Pause", "pause waits for authoritative confirmation before changing its label");
replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 0, paused: true });
assert(pauseReplay.textContent === "Resume", "paused replay button switches to resume");
pauseReplay._listeners.get("click")({ pointerType: "pen", detail: 1, preventDefault() {}, stopPropagation() {} });
assert(replayNet.speeds.length === speedsBeforePen + 1, "the synthesized pen click does not duplicate pause");

roomTimeNow += 1_000;
pauseReplay._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 2, "replay resume button restores the last non-zero speed");
replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2, paused: false });
assert(pauseReplay.textContent === "Pause", "resumed replay button switches back to pause");

replayNet.playerId = 41;
seekBack._listeners.get("click")({});
assert(replayNet.seekBacks.at(-1) === 90, "seek click sends net.seekRoomTime");
replayUi.applyRoomTimeState({ currentTick: 120, durationTicks: 1_000, speed: 2, paused: false });
assert(
  replayControls.dataset.roomTimePending === "true" &&
    replayControls.querySelector(".room-time-tick-status").textContent.includes("Seeking 30"),
  "a stale non-confirming authoritative update does not reject a pending seek",
);
replayUi.applyRoomTimeState({ currentTick: 34, durationTicks: 1_000, speed: 2, paused: false, controllerId: 99 });
assert(
  replayControls.dataset.roomTimePending === "true",
  "another viewer's authoritative rewind does not confirm the local seek",
);
replayUi.applyRoomTimeState({ currentTick: 35, durationTicks: 1_000, speed: 2, paused: false, controllerId: 41 });
assert(
  replayControls.dataset.roomTimePending === "false",
  "an authoritative rewind clears pending even when server progress makes the exact client prediction stale",
);
delete replayNet.playerId;
speed2._listeners.get("click")({});
replayUi.expireRoomTimePending();
assert(
  replayControls.querySelector(".room-time-tick-status").textContent.includes("check connection or permissions"),
  "a missing room-time confirmation reverts pending presentation with an actionable status",
);
replayUi.applyRoomTimeState({ currentTick: 30, durationTicks: 1_000, speed: 2, paused: false });
assert(
  !replayControls.querySelector(".room-time-tick-status").textContent.includes("check connection or permissions"),
  "a late matching authoritative state clears the timeout notice",
);
Date.now = actualDateNow;
const visionButtons = replayControls.querySelectorAll(".vision-btn");
assert(visionButtons.length === 3, "replay viewer builds all-player and per-player fog controls");
assert(branchReplay._listeners.has("pointerdown"), "branch action installs the scoped pointer activation path");
assert(visionButtons[1]._listeners.has("pointerdown"), "vision controls install the scoped pointer activation path");
branchReplay._listeners.get("pointerdown")({ button: 0, isPrimary: true, pointerId: 24, pointerType: "touch" });
branchReplay._listeners.get("pointerup")({
  pointerId: 24,
  pointerType: "touch",
  preventDefault() {},
  stopPropagation() {},
});
assert(replayNet.branches === 1, "a touch branch action invokes its request once");
branchReplay._listeners.get("click")({ pointerType: "touch", detail: 1, preventDefault() {}, stopPropagation() {} });
assert(replayNet.branches === 1, "the synthesized touch click does not duplicate the branch request");

visionButtons[1]._listeners.get("click")({});
assert(
  replayNet.selections.at(-1).mode === "player" &&
    replayNet.selections.at(-1).playerId === 1,
  "single replay fog click sends a per-viewer player vision request",
);
visionButtons[2]._listeners.get("click")({ shiftKey: true });
assert(
  replayNet.selections.at(-1).mode === "players" &&
    replayNet.selections.at(-1).playerIds.join(",") === "1,2",
  "shift-click replay fog controls send a selected-players request",
);
visionButtons[0]._listeners.get("click")({});
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
assert(timelineTrack._listeners.has("pointerdown"), "timeline seek installs the scoped pointer activation path");
timelineTrack._listeners.get("pointerdown")({ button: 0, isPrimary: true, pointerId: 25, pointerType: "touch" });
timelineTrack._listeners.get("pointerup")({
  pointerId: 25,
  pointerType: "touch",
  clientX: 100,
  preventDefault() {},
  stopPropagation() {},
});
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
globalThis.window.innerHeight = 844;
coarsePointer = true;
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
  "room-time controls ignore saved desktop panel position in mobile-debug presentation");
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
coarsePointer = false;
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
scenarioUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 2, paused: false });
scenarioSpeed2._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 2, "scenario speed click sends net.setRoomTimeSpeed");
scenarioUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 2, paused: false });
scenarioStep._listeners.get("click")({});
assert(replayNet.steps === 1, "scenario step sends net.stepRoomTime");
scenarioUi.applyRoomTimeState({ currentTick: 3, durationTicks: 0, speed: 2, paused: false });
scenarioSpeed0._listeners.get("click")({});
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
aiLiveUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 4, paused: false });
aiLiveSpeed4._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 4, "AI-only live speed click sends net.setRoomTimeSpeed");
aiLiveUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 4, paused: false });
aiLivePause._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 0, "AI-only live pause sends zero room-time speed");
aiLiveUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 0, paused: true });
assert(aiLivePause.textContent === "Resume", "paused AI-only live control switches to resume");
aiLivePause._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 4, "AI-only live resume restores the last selected speed");
aiLiveUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 4, paused: false });
const aiLiveSpeedsBeforeHiddenClicks = replayNet.speeds.length;
const aiLiveSeeksBeforeHiddenClicks = replayNet.seekBacks.length;
const aiLiveStepsBeforeHiddenClicks = replayNet.steps;
aiLiveStep._listeners.get("click")({});
aiLiveSeek._listeners.get("click")({});
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
labUi.applyRoomTimeState({
  currentTick: 119,
  durationTicks: 600,
  keyframeTicks: [0, 200, 400],
  speed: 1,
  paused: true,
});
labStep._listeners.get("click")({});
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
labSeek._listeners.get("click")({});
assert(replayNet.seekBacks.at(-1) === 30, "lab relative seek sends net.seekRoomTime through neutral controls");
labUi.applyRoomTimeState({ currentTick: 90, durationTicks: 600, speed: 1, paused: false });
const labTimelineTrack = labControls.querySelector(".room-time-timeline-track");
labUi.onRoomTimeTimelineClick({ currentTarget: labTimelineTrack, clientX: 100 });
assert(replayNet.seekTargets.at(-1) === 300, "lab timeline click sends an absolute room-time seek");
labUi.applyRoomTimeState({ currentTick: 301, durationTicks: 600, speed: 1, paused: false });
assert(
  labControls.dataset.roomTimePending === "false",
  "an absolute seek confirms when the authoritative tick moved toward a clamped or stale target",
);
labPause._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 0, "lab pause sends net.setRoomTimeSpeed");
labUi.applyRoomTimeState({ currentTick: 300, durationTicks: 600, speed: 0, paused: true });
assert(labPause.textContent === "Resume", "paused lab room-time control switches to resume");
labPause._listeners.get("click")({});
assert(replayNet.speeds.at(-1) === 1, "lab resume restores the last positive room-time speed");
labUi.destroy();

const readOnlyLabControls = fakeEl("div");
const readOnlyLabSpeed = fakeEl("button");
readOnlyLabSpeed.className = "spd-btn";
readOnlyLabSpeed.dataset.speed = "2";
readOnlyLabControls.appendChild(readOnlyLabSpeed);
dom.roomTimeControls = readOnlyLabControls;
const readOnlyLabUi = new RoomTimeControls({
  net: replayNet,
  state: {
    ...roomTimeState,
    controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } }),
  },
  capabilities: createRoomCapabilities({
    startPayload: {
      capabilities: { roomTime: { available: true, setSpeed: true } },
    },
  }),
});
const speedsBeforeReadOnlyLabClick = replayNet.speeds.length;
readOnlyLabSpeed._listeners.get("click")({});
assert(
  readOnlyLabSpeed.disabled &&
    replayNet.speeds.length === speedsBeforeReadOnlyLabClick &&
    readOnlyLabControls.querySelector(".room-time-tick-status").textContent.includes("operator access required"),
  "read-only lab room-time controls stay inactive with an authorization-specific status instead of timing out",
);
readOnlyLabUi.destroy();

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
stepOnlyUi.applyRoomTimeState({ currentTick: 0, durationTicks: 0, speed: 0, paused: true });
const speedsBeforeStepOnlyClicks = replayNet.speeds.length;
const seeksBeforeStepOnlyClicks = replayNet.seekBacks.length;
const stepsBeforeStepOnlyClicks = replayNet.steps;
stepOnlySpeed._listeners.get("click")({});
stepOnlyPause._listeners.get("click")({});
stepOnlySeek._listeners.get("click")({});
assert(replayNet.speeds.length === speedsBeforeStepOnlyClicks, "hidden speed/pause controls are inert without capability");
assert(replayNet.seekBacks.length === seeksBeforeStepOnlyClicks, "hidden seek controls are inert without capability");
stepOnlyStep._listeners.get("click")({});
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

} finally {
  dom.roomTimeControls = priorRoomTimeControls;
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}
