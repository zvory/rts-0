import { dom } from "./bootstrap.js";
import { TICK_HZ } from "./config.js";
import { createImmediateTouchButtonActivation } from "./panel_touch_activation.js";
import { VISION_SELECTION } from "./protocol.js";
import { FloatingRoomTimePanel } from "./room_time_panel.js";

const ROOM_TIME_CONFIRMATION_TIMEOUT_MS = 3_000;

export class RoomTimeControls {
  constructor({
    net,
    state,
    controlPolicy = null,
    replayViewer = false,
    capabilities = null,
    label = null,
    initialVisionSelection = null,
  }) {
    this.net = net;
    this.state = state;
    this.replayViewer = !!replayViewer;
    this.capabilities = capabilities || {};
    this.roomTime = this.capabilities.roomTime || {};
    this.visibility = this.capabilities.visibility || {};
    this.actions = this.capabilities.actions || {};
    this.label = label || (this.replayViewer ? "Replay" : "Room time");
    this.visionSelection = visionSelectionIds(initialVisionSelection, this.state?.players);
    this.omniscientVision = initialVisionSelection?.mode === VISION_SELECTION.OMNISCIENT;
    this.roomTimeState = null;
    this.roomTimeSeekPending = false;
    this.roomTimeSeekTargetTick = null;
    this.roomTimePending = null;
    this.roomTimePendingTimer = null;
    this.roomTimeTimedOutAction = null;
    this.roomTimeNotice = "";
    this.controlActivationBindings = [];
    this.timelineHoverBindings = [];
    this.roomTimeAccessDenied = controlPolicy?.kind === "lab" && controlPolicy.isOperator?.() === false;
    this.lastRoomTimeSpeed = 2;
    this.floatingPanel = null;

    if (!dom.roomTimeControls || (!this.roomTime.available && !this.visibility.visionSelection)) return;

    dom.roomTimeControls.hidden = false;
    dom.roomTimeControls.classList.toggle("replay-viewer-controls", this.replayViewer);
    dom.roomTimeControls.classList.add("room-time-controls");
    dom.roomTimeControls.setAttribute("aria-label", `${this.label} controls`);
    this.floatingPanel = new FloatingRoomTimePanel({ root: dom.roomTimeControls, label: this.label });
    this.floatingPanel.mount();
    for (const btn of dom.roomTimeControls.querySelectorAll(".spd-btn")) {
      const speed = parseFloat(btn.dataset.speed);
      if (Number.isFinite(speed) && speed > 0) btn.hidden = !this.roomTime.setSpeed;
    }
    for (const btn of dom.roomTimeControls.querySelectorAll(".seek-btn")) {
      btn.hidden = !this.roomTime.seekRelative;
    }
    for (const btn of dom.roomTimeControls.querySelectorAll(".room-time-pause-btn")) {
      btn.hidden = this.replayViewer || !this.roomTime.pause;
    }
    for (const btn of dom.roomTimeControls.querySelectorAll(".room-time-step-btn")) {
      btn.hidden = !this.roomTime.step;
    }
    this.bindStaticRoomTimeActivations();
    this.setRoomTimeSpeedActive(null);
    if (this.replayViewer && this.roomTime.pause) this.buildReplayPauseControl();
    if (this.replayViewer && this.actions.branchFromTick) this.buildBranchFromTickControl();
    if (this.visibility.visionSelection) this.buildVisionSelectionControls();
    if (this.roomTime.available) {
      this.buildRoomTimeStatus();
      if (this.roomTime.timeline && this.roomTime.seekAbsolute) this.buildRoomTimeTimeline();
      this.syncRoomTimePendingPresentation();
      this.updateRoomTimePauseButton();
      this.updateRoomTimeStatus();
    }
  }

  visionSelectionRequest() {
    if (this.omniscientVision) return { mode: VISION_SELECTION.OMNISCIENT };
    const playerIds = [...this.visionSelection].sort((a, b) => a - b);
    if (playerIds.length === 0) return { mode: VISION_SELECTION.ALL };
    if (playerIds.length === 1) {
      return { mode: VISION_SELECTION.PLAYER, playerId: playerIds[0] };
    }
    return { mode: VISION_SELECTION.PLAYERS, playerIds };
  }

  roomTimeControlSurface() {
    return this.floatingPanel?.contentEl || dom.roomTimeControls?.querySelector(".room-time-panel-body") || dom.roomTimeControls;
  }

  bindStaticRoomTimeActivations() {
    for (const btn of dom.roomTimeControls?.querySelectorAll(".spd-btn") || []) {
      this.bindRoomTimeActivation(btn, (event) => this.onRoomTimeControlClick({ target: btn, originalEvent: event }));
    }
  }

  bindRoomTimeActivation(button, onActivate) {
    if (!button || this.controlActivationBindings.some(([bound]) => bound === button)) return;
    const activation = createImmediateTouchButtonActivation(onActivate);
    const listeners = [
      ["pointerdown", activation.pointerdown],
      ["pointerup", activation.pointerup],
      ["pointercancel", activation.pointercancel],
      ["pointerleave", activation.pointerleave],
      ["click", activation.click],
    ];
    for (const [type, handler] of listeners) button.addEventListener(type, handler);
    this.controlActivationBindings.push([button, activation, listeners]);
  }

  clearRoomTimeActivations() {
    for (const [button, activation, listeners] of this.controlActivationBindings) {
      activation.reset();
      for (const [type, handler] of listeners) button.removeEventListener(type, handler);
    }
    this.controlActivationBindings = [];
  }

  onRoomTimeControlClick(e) {
    const btn = e.target.closest(".spd-btn");
    if (!btn || btn.hidden || btn.disabled || this.roomTimeAccessDenied) return;
    if (btn.dataset.stepRoomTime !== undefined) {
      if (!this.roomTime.step) return;
      const currentTick = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : null;
      this.requestRoomTimeAction(
        { kind: "step", baselineTick: currentTick },
        () => this.net.stepRoomTime(),
      );
      return;
    }
    if (btn.dataset.seekBack !== undefined) {
      if (!this.roomTime.seekRelative) return;
      const ticksBack = parseInt(btn.dataset.seekBack, 10);
      if (!isFinite(ticksBack) || ticksBack <= 0) return;
      const currentTick = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
      this.requestRoomTimeAction(
        {
          kind: "seek",
          mode: "relative",
          baselineTick: currentTick,
          expectedTick: Math.max(0, currentTick - ticksBack),
        },
        () => this.net.seekRoomTime(ticksBack),
      );
      return;
    }
    if (btn.dataset.roomTimePauseToggle !== undefined || btn.classList.contains("room-time-pause-btn")) {
      if (!this.roomTime.pause) return;
      const speed = this.isRoomTimePaused() ? this.lastRoomTimeSpeed : 0;
      this.requestRoomTimeAction({ kind: "speed", expectedSpeed: speed }, () => this.net.setRoomTimeSpeed(speed));
      return;
    }
    const speed = parseFloat(btn.dataset.speed);
    if (!isFinite(speed)) return;
    if (speed === 0 && !this.roomTime.pause) return;
    if (speed > 0 && !this.roomTime.setSpeed) return;
    this.requestRoomTimeAction({ kind: "speed", expectedSpeed: speed }, () => this.net.setRoomTimeSpeed(speed));
  }

  applyRoomTimeState(state) {
    this.roomTimeState = state || null;
    const pending = this.roomTimePending;
    if (pending && this.roomTimeActionConfirmed(pending, state)) {
      this.clearRoomTimePending();
      this.roomTimeNotice = "";
    } else if (
      !pending &&
      this.roomTimeTimedOutAction &&
      this.roomTimeActionConfirmed(this.roomTimeTimedOutAction, state)
    ) {
      this.roomTimeTimedOutAction = null;
      this.roomTimeNotice = "";
    }
    if (Number.isFinite(state?.speed) && state.speed > 0) this.lastRoomTimeSpeed = state.speed;
    this.syncRoomTimePendingPresentation();
    const ended =
      state?.ended === true ||
      (Number.isFinite(state?.currentTick) &&
        Number.isFinite(state?.durationTicks) &&
        state.durationTicks > 0 &&
        state.currentTick >= state.durationTicks);
    this.setRoomTimeConcluded(ended);
    if (Number.isFinite(state?.speed)) this.setRoomTimeSpeedActive(state.speed);
    this.updateRoomTimePauseButton();
    this.updateRoomTimeStatus();
    this.updateRoomTimeTimeline();
  }

  requestRoomTimeAction(pending, send) {
    if (this.roomTimePending) return false;
    this.roomTimeTimedOutAction = null;
    const sent = send?.() === true;
    if (!sent) {
      this.roomTimeNotice = "Room time command was not sent.";
      this.updateRoomTimeStatus();
      return false;
    }

    this.roomTimeNotice = "";
    this.roomTimePending = Number.isFinite(this.net?.playerId)
      ? { ...pending, controllerId: this.net.playerId }
      : pending;
    if (this.roomTimePending.kind === "seek") {
      this.roomTimeSeekPending = true;
      this.roomTimeSeekTargetTick = this.roomTimePending.expectedTick;
      this.setRoomTimeConcluded(false);
    }
    this.syncRoomTimePendingPresentation();
    this.updateRoomTimeStatus();
    this.updateRoomTimeTimeline();
    this.roomTimePendingTimer = typeof globalThis.setTimeout === "function"
      ? globalThis.setTimeout(() => this.expireRoomTimePending(), ROOM_TIME_CONFIRMATION_TIMEOUT_MS)
      : null;
    return true;
  }

  roomTimeActionConfirmed(pending, state) {
    if (!state) return false;
    if (pending.kind === "speed") {
      return Number.isFinite(state.speed) && Math.abs(state.speed - pending.expectedSpeed) < 0.001;
    }
    if (pending.kind === "seek") {
      if (!Number.isFinite(state.currentTick) || !Number.isFinite(pending.expectedTick)) return false;
      if (Number.isFinite(pending.controllerId) && state.controllerId !== pending.controllerId) return false;
      if (state.currentTick === pending.expectedTick) return true;
      if (!Number.isFinite(pending.baselineTick)) return false;
      if (pending.mode === "relative") return state.currentTick < pending.baselineTick;
      return (
        Math.abs(state.currentTick - pending.expectedTick) <
        Math.abs(pending.baselineTick - pending.expectedTick)
      );
    }
    if (pending.kind === "step") {
      if (
        Number.isFinite(pending.controllerId) &&
        Number.isFinite(state.controllerId) &&
        state.controllerId !== pending.controllerId
      ) return false;
      return (
        Number.isFinite(pending.baselineTick) &&
        Number.isFinite(state.currentTick) &&
        state.currentTick > pending.baselineTick
      );
    }
    return false;
  }

  expireRoomTimePending() {
    if (!this.roomTimePending) return;
    this.roomTimeTimedOutAction = this.roomTimePending;
    this.clearRoomTimePending();
    this.roomTimeNotice = "Room time unavailable or unchanged — check connection or permissions.";
    this.updateRoomTimeStatus();
  }

  clearRoomTimePending() {
    if (this.roomTimePendingTimer != null) globalThis.clearTimeout?.(this.roomTimePendingTimer);
    this.roomTimePendingTimer = null;
    this.roomTimePending = null;
    this.roomTimeSeekPending = false;
    this.roomTimeSeekTargetTick = null;
    this.syncRoomTimePendingPresentation();
  }

  syncRoomTimePendingPresentation() {
    const root = dom.roomTimeControls;
    if (!root) return;
    const pending = !!this.roomTimePending;
    root.dataset.roomTimePending = pending ? "true" : "false";
    root.setAttribute("aria-busy", pending ? "true" : "false");
    const awaitingAuthority = !this.roomTimeState;
    for (const btn of root.querySelectorAll(".spd-btn")) {
      btn.disabled = pending || this.roomTimeAccessDenied || awaitingAuthority;
    }
    const timeline = root.querySelector(".room-time-timeline-track");
    if (timeline) {
      timeline.disabled = pending || this.roomTimeAccessDenied || awaitingAuthority;
      if (timeline.disabled) this.hideRoomTimeTimelineHover();
    }
    root.dataset.roomTimeAccessDenied = this.roomTimeAccessDenied ? "true" : "false";
    root.dataset.roomTimeAwaitingAuthority = awaitingAuthority ? "true" : "false";
  }

  noteSnapshotTick(tick) {
    if (!this.roomTime.available || !Number.isFinite(tick)) return;
    if (!this.roomTimeState) return;
    this.roomTimeState = { ...this.roomTimeState, currentTick: tick };
    this.updateRoomTimeStatus();
    this.updateRoomTimeTimeline();
  }

  setRoomTimeConcluded(concluded) {
    const status = dom.roomTimeControls?.querySelector("#room-time-concluded");
    if (!status) return;
    status.textContent = this.replayViewer ? "Replay Concluded" : "Room Time Ended";
    status.hidden = !concluded;
  }

  setRoomTimeSpeedActive(speed) {
    if (!dom.roomTimeControls) return;
    for (const btn of dom.roomTimeControls.querySelectorAll(".spd-btn:not(.seek-btn)")) {
      if (btn.dataset.speed === undefined) continue;
      const btnSpeed = parseFloat(btn.dataset.speed);
      btn.classList.toggle(
        "active",
        Number.isFinite(speed) && Number.isFinite(btnSpeed) && Math.abs(btnSpeed - speed) < 0.001,
      );
    }
  }

  isRoomTimePaused() {
    return this.roomTimeState?.paused === true || this.roomTimeState?.speed === 0;
  }

  updateRoomTimePauseButton() {
    if (!dom.roomTimeControls) return;
    const paused = this.isRoomTimePaused();
    for (const btn of dom.roomTimeControls.querySelectorAll(".replay-pause-btn, .room-time-pause-btn")) {
      btn.textContent = paused ? "Resume" : "Pause";
      btn.title = paused ? `Resume ${this.label.toLowerCase()} at ${this.lastRoomTimeSpeed}x.` : `Pause ${this.label.toLowerCase()}.`;
      btn.classList.toggle("active", paused);
    }
  }

  buildReplayPauseControl() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".replay-pause-btn")) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;
    const pause = document.createElement("button");
    pause.type = "button";
    pause.className = "spd-btn replay-pause-btn";
    pause.dataset.roomTimePauseToggle = "1";
    pause.textContent = "Pause";
    pause.title = "Pause replay playback.";
    this.bindRoomTimeActivation(pause, (event) => this.onRoomTimeControlClick({ target: pause, originalEvent: event }));
    surface.appendChild(pause);
  }

  buildBranchFromTickControl() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".replay-branch-btn")) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;
    const resume = document.createElement("button");
    resume.type = "button";
    resume.className = "spd-btn replay-branch-btn";
    resume.textContent = "Resume play from here";
    resume.title = "Create a practice branch from the current replay tick.";
    this.bindRoomTimeActivation(resume, () => {
      if (!resume.hidden && !resume.disabled) this.net.requestBranchFromTick();
    });
    surface.appendChild(resume);
  }

  buildVisionSelectionControls() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".vision-selection-controls")) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;

    const group = document.createElement("div");
    group.className = "vision-selection-controls";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", "Observer perspective");

    const all = document.createElement("button");
    all.type = "button";
    all.className = "spd-btn vision-btn";
    all.dataset.vision = "all";
    all.textContent = "All";
    all.title = "Show the union of all players' vision.";
    this.bindRoomTimeActivation(all, (event) => this.onVisionSelectionClick({
      target: all,
      shiftKey: event?.shiftKey,
      metaKey: event?.metaKey,
      ctrlKey: event?.ctrlKey,
    }));
    group.appendChild(all);

    for (const player of this.state.players) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "spd-btn vision-btn";
      btn.dataset.playerId = String(player.id);
      btn.textContent = player.name || `P${player.id}`;
      btn.title = "Click for this player. Shift-click to combine players.";
      btn.style.setProperty("--player-color", player.color || "#aaa");
      this.bindRoomTimeActivation(btn, (event) => this.onVisionSelectionClick({
        target: btn,
        shiftKey: event?.shiftKey,
        metaKey: event?.metaKey,
        ctrlKey: event?.ctrlKey,
      }));
      group.appendChild(btn);
    }

    const omniscient = document.createElement("button");
    omniscient.type = "button";
    omniscient.className = "spd-btn vision-btn";
    omniscient.dataset.vision = "omniscient";
    omniscient.textContent = "Omniscient";
    omniscient.title = "Show the complete world and every owner's private details.";
    this.bindRoomTimeActivation(omniscient, (event) => this.onVisionSelectionClick({
      target: omniscient,
      shiftKey: event?.shiftKey,
      metaKey: event?.metaKey,
      ctrlKey: event?.ctrlKey,
    }));
    group.appendChild(omniscient);

    surface.appendChild(group);
    this.syncVisionSelectionButtons();
  }

  buildRoomTimeStatus() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".room-time-tick-status")) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;
    const status = document.createElement("span");
    status.className = "room-time-status room-time-tick-status";
    status.textContent = `${this.label} 0 / 0`;
    surface.appendChild(status);
  }

  buildRoomTimeTimeline() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".room-time-timeline")) return;
    if (!this.roomTime.timeline || !this.roomTime.seekAbsolute) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;

    const wrap = document.createElement("div");
    wrap.className = "room-time-timeline";

    const track = document.createElement("button");
    track.type = "button";
    track.className = "room-time-timeline-track";
    track.setAttribute("aria-label", `Seek ${this.label.toLowerCase()} timeline`);
    track.setAttribute("aria-describedby", "room-time-timeline-hover");
    this.bindRoomTimeActivation(track, (event) => this.onRoomTimeTimelineClick({
      currentTarget: track,
      clientX: event?.clientX,
    }));

    const hover = document.createElement("span");
    hover.id = "room-time-timeline-hover";
    hover.className = "room-time-timeline-hover";
    hover.setAttribute("role", "tooltip");
    hover.hidden = true;
    const onPointerMove = (event) => {
      if (event?.pointerType === "touch") {
        this.hideRoomTimeTimelineHover();
        return;
      }
      this.updateRoomTimeTimelineHover({
        currentTarget: track,
        clientX: event?.clientX,
      });
    };
    const onPointerLeave = () => this.hideRoomTimeTimelineHover();
    track.addEventListener("pointermove", onPointerMove);
    track.addEventListener("pointerleave", onPointerLeave);
    this.timelineHoverBindings.push(
      [track, "pointermove", onPointerMove],
      [track, "pointerleave", onPointerLeave],
    );

    const progress = document.createElement("span");
    progress.className = "room-time-timeline-progress";
    track.appendChild(progress);

    const marks = document.createElement("span");
    marks.className = "room-time-timeline-marks";
    track.appendChild(marks);

    wrap.appendChild(track);
    wrap.appendChild(hover);
    surface.appendChild(wrap);
    this.updateRoomTimeTimeline();
  }

  roomTimeTimelineTarget(track, clientX) {
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    if (!track || duration <= 0 || !Number.isFinite(clientX)) return null;
    const rect = track.getBoundingClientRect();
    if (!rect.width) return null;
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return { ratio, tick: Math.round(ratio * duration), trackWidth: rect.width };
  }

  formatRoomTimeTimelineTarget(tick) {
    const safeTick = Number.isFinite(tick) ? Math.max(0, Math.round(tick)) : 0;
    const totalSeconds = Math.floor(safeTick / TICK_HZ);
    const seconds = totalSeconds % 60;
    const totalMinutes = Math.floor(totalSeconds / 60);
    const minutes = totalMinutes % 60;
    const hours = Math.floor(totalMinutes / 60);
    const two = (value) => String(value).padStart(2, "0");
    const time = hours > 0
      ? `${hours}:${two(minutes)}:${two(seconds)}`
      : `${two(minutes)}:${two(seconds)}`;
    return `${time} · tick ${safeTick}`;
  }

  updateRoomTimeTimelineHover(ev) {
    const track = ev.currentTarget;
    if (track?.disabled || this.roomTimeAccessDenied) {
      this.hideRoomTimeTimelineHover();
      return;
    }
    const target = this.roomTimeTimelineTarget(track, ev.clientX);
    const hover = dom.roomTimeControls?.querySelector(".room-time-timeline-hover");
    if (!target || !hover) {
      this.hideRoomTimeTimelineHover();
      return;
    }
    const text = this.formatRoomTimeTimelineTarget(target.tick);
    hover.textContent = text;
    hover.hidden = false;
    const hoverWidth = Number.isFinite(hover.offsetWidth) ? hover.offsetWidth : 0;
    const halfHoverWidth = Math.min(target.trackWidth / 2, hoverWidth / 2);
    const targetX = target.ratio * target.trackWidth;
    const clampedX = Math.max(
      halfHoverWidth,
      Math.min(target.trackWidth - halfHoverWidth, targetX),
    );
    hover.style.setProperty("--room-time-hover", `${clampedX}px`);
  }

  hideRoomTimeTimelineHover() {
    const hover = dom.roomTimeControls?.querySelector(".room-time-timeline-hover");
    if (!hover) return;
    hover.hidden = true;
  }

  onRoomTimeTimelineClick(ev) {
    if (!this.roomTime.timeline || !this.roomTime.seekAbsolute) return;
    const track = ev.currentTarget;
    if (track?.disabled || this.roomTimeAccessDenied) return;
    const target = this.roomTimeTimelineTarget(track, ev.clientX);
    if (!target) return;
    const tick = target.tick;
    this.hideRoomTimeTimelineHover();
    const baselineTick = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : null;
    this.requestRoomTimeAction(
      { kind: "seek", mode: "absolute", baselineTick, expectedTick: tick },
      () => this.net.seekRoomTimeTo(tick),
    );
  }

  updateRoomTimeTimeline() {
    const timeline = dom.roomTimeControls?.querySelector(".room-time-timeline");
    if (!timeline) return;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const ratio = duration > 0 ? Math.max(0, Math.min(1, current / duration)) : 0;
    const progress = timeline.querySelector(".room-time-timeline-progress");
    progress?.style.setProperty("--room-time-progress", `${ratio * 100}%`);

    const marks = timeline.querySelector(".room-time-timeline-marks");
    if (!marks) return;
    const keyframeTicks = Array.isArray(this.roomTimeState?.keyframeTicks) ? this.roomTimeState.keyframeTicks : [];
    const normalized = [...new Set(keyframeTicks)]
      .filter((tick) => Number.isFinite(tick) && tick >= 0 && (duration <= 0 || tick <= duration))
      .sort((a, b) => a - b);
    const signature = `${duration}:${normalized.join(",")}`;
    if (marks.dataset.signature === signature) return;
    marks.dataset.signature = signature;
    marks.replaceChildren();
    for (const tick of normalized) {
      const mark = document.createElement("span");
      mark.className = "room-time-timeline-mark";
      const left = duration > 0 ? (tick / duration) * 100 : 0;
      mark.style.left = `${Math.max(0, Math.min(100, left))}%`;
      mark.title = `Keyframe ${tick}`;
      marks.appendChild(mark);
    }
  }

  onVisionSelectionClick(ev) {
    const btn = ev.target.closest(".vision-btn");
    if (!btn || btn.hidden || btn.disabled) return;
    if (btn.dataset.vision === "all") {
      this.omniscientVision = false;
      this.visionSelection.clear();
      this.net.setVisionSelection(this.visionSelectionRequest());
      this.syncVisionSelectionButtons();
      return;
    }
    if (btn.dataset.vision === "omniscient") {
      this.omniscientVision = true;
      this.visionSelection.clear();
      this.net.setVisionSelection(this.visionSelectionRequest());
      this.syncVisionSelectionButtons();
      return;
    }

    const id = Number(btn.dataset.playerId);
    if (!Number.isFinite(id)) return;
    this.omniscientVision = false;
    if (ev.shiftKey || ev.metaKey || ev.ctrlKey) {
      if (this.visionSelection.has(id)) this.visionSelection.delete(id);
      else this.visionSelection.add(id);
    } else {
      this.visionSelection.clear();
      this.visionSelection.add(id);
    }
    this.net.setVisionSelection(this.visionSelectionRequest());
    this.syncVisionSelectionButtons();
  }

  syncVisionSelectionButtons() {
    if (!dom.roomTimeControls) return;
    const allActive = !this.omniscientVision && this.visionSelection.size === 0;
    for (const btn of dom.roomTimeControls.querySelectorAll(".vision-btn")) {
      if (btn.dataset.vision === "all") {
        btn.classList.toggle("active", allActive);
        continue;
      }
      if (btn.dataset.vision === "omniscient") {
        btn.classList.toggle("active", this.omniscientVision);
        continue;
      }
      const id = Number(btn.dataset.playerId);
      btn.classList.toggle("active", this.visionSelection.has(id));
    }
  }

  updateRoomTimeStatus() {
    const status = dom.roomTimeControls?.querySelector(".room-time-tick-status");
    if (!status) return;
    if (this.roomTimeAccessDenied) {
      status.textContent = `${this.label} unavailable — operator access required.`;
      return;
    }
    if (!this.roomTimeState) {
      const pending = this.roomTimePending ? " · Waiting for confirmation..." : "";
      const notice = this.roomTimeNotice ? ` · ${this.roomTimeNotice}` : "";
      status.textContent = `${this.label} awaiting authoritative state${pending}${notice}`;
      return;
    }
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const speed = Number.isFinite(this.roomTimeState?.speed) ? this.roomTimeState.speed : this.lastRoomTimeSpeed;
    const pending = this.roomTimePending
      ? this.roomTimeSeekPending
        ? ` · Seeking${Number.isFinite(this.roomTimeSeekTargetTick) ? ` ${this.roomTimeSeekTargetTick}` : ""}...`
        : " · Waiting for confirmation..."
      : "";
    const notice = this.roomTimeNotice ? ` · ${this.roomTimeNotice}` : "";
    status.textContent = `${this.label} ${current} / ${duration} @ ${speed}x${pending}${notice}`;
  }

  destroy() {
    if (!dom.roomTimeControls) return;
    this.clearRoomTimePending();
    this.roomTimeTimedOutAction = null;
    this.clearRoomTimeActivations();
    for (const [target, type, handler] of this.timelineHoverBindings) {
      target.removeEventListener(type, handler);
    }
    this.timelineHoverBindings = [];
    dom.roomTimeControls.hidden = true;
    this.setRoomTimeConcluded(false);
    for (const btn of dom.roomTimeControls.querySelectorAll(".spd-btn")) {
      const speed = parseFloat(btn.dataset.speed);
      if (Number.isFinite(speed) && speed > 0) btn.hidden = false;
    }
    for (const btn of dom.roomTimeControls.querySelectorAll(".seek-btn")) btn.hidden = false;
    for (const btn of dom.roomTimeControls.querySelectorAll(".room-time-pause-btn, .room-time-step-btn")) {
      btn.hidden = true;
      if (btn.classList.contains("room-time-pause-btn")) {
        btn.textContent = "Pause";
        btn.title = "Pause room time";
        btn.classList.remove("active");
      }
    }
    for (const btn of dom.roomTimeControls.querySelectorAll(".spd-btn")) btn.disabled = false;
    delete dom.roomTimeControls.dataset.roomTimePending;
    delete dom.roomTimeControls.dataset.roomTimeAccessDenied;
    delete dom.roomTimeControls.dataset.roomTimeAwaitingAuthority;
    dom.roomTimeControls.removeAttribute?.("aria-busy");
    dom.roomTimeControls.classList.remove("replay-viewer-controls");
    dom.roomTimeControls.classList.remove("room-time-controls");
    dom.roomTimeControls.removeAttribute?.("aria-label");
    dom.roomTimeControls.querySelector(".replay-pause-btn")?.remove();
    dom.roomTimeControls.querySelector(".replay-branch-btn")?.remove();
    dom.roomTimeControls.querySelector(".vision-selection-controls")?.remove();
    dom.roomTimeControls.querySelector(".room-time-tick-status")?.remove();
    dom.roomTimeControls.querySelector(".room-time-timeline")?.remove();
    this.floatingPanel?.destroy();
    this.floatingPanel = null;
  }
}

function visionSelectionIds(selection, players) {
  const knownIds = new Set(
    (Array.isArray(players) ? players : [])
      .map((player) => Number(player?.id))
      .filter((id) => Number.isInteger(id) && id > 0),
  );
  if (!selection || selection.mode === VISION_SELECTION.ALL) return new Set();

  const candidates = selection.mode === VISION_SELECTION.PLAYER
    ? [selection.playerId]
    : selection.mode === VISION_SELECTION.PLAYERS
      ? selection.playerIds
      : [];
  return new Set(
    (Array.isArray(candidates) ? candidates : [])
      .map((id) => Number(id))
      .filter((id) => knownIds.has(id)),
  );
}

export class ReplayControls extends RoomTimeControls {}
