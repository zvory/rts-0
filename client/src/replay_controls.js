import { dom } from "./bootstrap.js";
import { REPLAY_VISION } from "./protocol.js";
import { FloatingRoomTimePanel } from "./room_time_panel.js";

export class RoomTimeControls {
  constructor({ net, state, replayViewer = false, capabilities = null, label = null }) {
    this.net = net;
    this.state = state;
    this.replayViewer = !!replayViewer;
    this.capabilities = capabilities || {};
    this.roomTime = this.capabilities.roomTime || {};
    this.visibility = this.capabilities.visibility || {};
    this.actions = this.capabilities.actions || {};
    this.label = label || (this.replayViewer ? "Replay" : "Room time");
    this.replayVisionSelection = new Set();
    this.roomTimeState = null;
    this.roomTimeSeekPending = false;
    this.roomTimeSeekTargetTick = null;
    this.roomTimeHandler = null;
    this.lastRoomTimeSpeed = 2;
    this.floatingPanel = null;

    if (!dom.roomTimeControls || !this.roomTime.available) return;

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
    this.roomTimeHandler = (e) => this.onRoomTimeControlClick(e);
    dom.roomTimeControls.addEventListener("click", this.roomTimeHandler);
    this.setRoomTimeSpeedActive(this.replayViewer ? 2 : null);
    if (this.replayViewer && this.roomTime.pause) this.buildReplayPauseControl();
    if (this.replayViewer && this.actions.replayBranch) this.buildReplayBranchControl();
    if (this.visibility.replayVision) this.buildReplayVisionControls();
    this.buildRoomTimeStatus();
    if (this.roomTime.timeline && this.roomTime.seekAbsolute) this.buildRoomTimeTimeline();
    this.updateRoomTimePauseButton();
  }

  roomTimeControlSurface() {
    return this.floatingPanel?.contentEl || dom.roomTimeControls?.querySelector(".room-time-panel-body") || dom.roomTimeControls;
  }

  onRoomTimeControlClick(e) {
    const btn = e.target.closest(".spd-btn");
    if (!btn || btn.hidden || btn.disabled) return;
    if (btn.dataset.stepRoomTime !== undefined) {
      if (!this.roomTime.step) return;
      this.net.stepRoomTime();
      return;
    }
    if (btn.dataset.seekBack !== undefined) {
      if (!this.roomTime.seekRelative) return;
      const ticksBack = parseInt(btn.dataset.seekBack, 10);
      if (!isFinite(ticksBack) || ticksBack <= 0) return;
      this.setRoomTimeConcluded(false);
      const currentTick = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
      this.setRoomTimeSeekPending(Math.max(0, currentTick - ticksBack));
      this.net.seekRoomTime(ticksBack);
      return;
    }
    if (btn.dataset.roomTimePauseToggle !== undefined || btn.classList.contains("room-time-pause-btn")) {
      if (!this.roomTime.pause) return;
      const speed = this.isRoomTimePaused() ? this.lastRoomTimeSpeed : 0;
      this.net.setRoomTimeSpeed(speed);
      this.roomTimeState = { ...(this.roomTimeState || {}), speed, paused: speed === 0 };
      this.setRoomTimeSpeedActive(speed);
      this.updateRoomTimePauseButton();
      this.updateRoomTimeStatus();
      return;
    }
    const speed = parseFloat(btn.dataset.speed);
    if (!isFinite(speed)) return;
    if (speed === 0 && !this.roomTime.pause) return;
    if (speed > 0 && !this.roomTime.setSpeed) return;
    if (speed > 0) this.lastRoomTimeSpeed = speed;
    this.net.setRoomTimeSpeed(speed);
    this.roomTimeState = { ...(this.roomTimeState || {}), speed, paused: speed === 0 };
    this.setRoomTimeSpeedActive(speed);
    this.updateRoomTimePauseButton();
    this.updateRoomTimeStatus();
  }

  applyRoomTimeState(state) {
    this.roomTimeState = state || null;
    this.setRoomTimeSeekPending(null, false);
    if (Number.isFinite(state?.speed) && state.speed > 0) this.lastRoomTimeSpeed = state.speed;
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

  noteSnapshotTick(tick) {
    if (!this.roomTime.available || !Number.isFinite(tick)) return;
    this.roomTimeState = { ...(this.roomTimeState || {}), currentTick: tick };
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
    surface.appendChild(pause);
  }

  buildReplayBranchControl() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".replay-branch-btn")) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;
    const resume = document.createElement("button");
    resume.type = "button";
    resume.className = "spd-btn replay-branch-btn";
    resume.textContent = "Resume play from here";
    resume.title = "Create a practice branch from the current replay tick.";
    resume.addEventListener("click", () => this.net.requestReplayBranch());
    surface.appendChild(resume);
  }

  buildReplayVisionControls() {
    if (!dom.roomTimeControls || dom.roomTimeControls.querySelector(".replay-vision-controls")) return;
    const surface = this.roomTimeControlSurface();
    if (!surface) return;

    const group = document.createElement("div");
    group.className = "replay-vision-controls";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", "Replay fog perspective");

    const all = document.createElement("button");
    all.type = "button";
    all.className = "spd-btn vision-btn active";
    all.dataset.vision = "all";
    all.textContent = "All vision";
    all.title = "Show the union of all players' replay vision.";
    group.appendChild(all);

    for (const player of this.state.players) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "spd-btn vision-btn";
      btn.dataset.playerId = String(player.id);
      btn.textContent = player.name || `P${player.id}`;
      btn.title = "Click for this player. Shift-click to combine players.";
      btn.style.setProperty("--player-color", player.color || "#aaa");
      group.appendChild(btn);
    }

    group.addEventListener("click", (ev) => this.onReplayVisionClick(ev));
    surface.appendChild(group);
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
    track.title = `Click to seek ${this.label.toLowerCase()}`;
    track.addEventListener("click", (ev) => this.onRoomTimeTimelineClick(ev));

    const progress = document.createElement("span");
    progress.className = "room-time-timeline-progress";
    track.appendChild(progress);

    const marks = document.createElement("span");
    marks.className = "room-time-timeline-marks";
    track.appendChild(marks);

    wrap.appendChild(track);
    surface.appendChild(wrap);
    this.updateRoomTimeTimeline();
  }

  onRoomTimeTimelineClick(ev) {
    if (!this.roomTime.timeline || !this.roomTime.seekAbsolute) return;
    const track = ev.currentTarget;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    if (!track || duration <= 0) return;
    const rect = track.getBoundingClientRect();
    if (!rect.width) return;
    const ratio = Math.max(0, Math.min(1, (ev.clientX - rect.left) / rect.width));
    const tick = Math.round(ratio * duration);
    this.setRoomTimeConcluded(false);
    this.setRoomTimeSeekPending(tick);
    this.net.seekRoomTimeTo(tick);
  }

  setRoomTimeSeekPending(targetTick, pending = true) {
    this.roomTimeSeekPending = !!pending;
    this.roomTimeSeekTargetTick = this.roomTimeSeekPending && Number.isFinite(targetTick) ? targetTick : null;
    this.updateRoomTimeStatus();
    this.updateRoomTimeTimeline();
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

  onReplayVisionClick(ev) {
    const btn = ev.target.closest(".vision-btn");
    if (!btn) return;
    if (btn.dataset.vision === "all") {
      this.replayVisionSelection.clear();
      this.net.setReplayVision({ mode: REPLAY_VISION.ALL });
      this.syncReplayVisionButtons();
      return;
    }

    const id = Number(btn.dataset.playerId);
    if (!Number.isFinite(id)) return;
    if (ev.shiftKey || ev.metaKey || ev.ctrlKey) {
      if (this.replayVisionSelection.has(id)) this.replayVisionSelection.delete(id);
      else this.replayVisionSelection.add(id);
    } else {
      this.replayVisionSelection.clear();
      this.replayVisionSelection.add(id);
    }
    if (this.replayVisionSelection.size === 0) {
      this.net.setReplayVision({ mode: REPLAY_VISION.ALL });
    } else if (this.replayVisionSelection.size === 1) {
      this.net.setReplayVision({
        mode: REPLAY_VISION.PLAYER,
        playerId: [...this.replayVisionSelection][0],
      });
    } else {
      this.net.setReplayVision({
        mode: REPLAY_VISION.PLAYERS,
        playerIds: [...this.replayVisionSelection].sort((a, b) => a - b),
      });
    }
    this.syncReplayVisionButtons();
  }

  syncReplayVisionButtons() {
    if (!dom.roomTimeControls) return;
    const allActive = this.replayVisionSelection.size === 0;
    for (const btn of dom.roomTimeControls.querySelectorAll(".vision-btn")) {
      if (btn.dataset.vision === "all") {
        btn.classList.toggle("active", allActive);
        continue;
      }
      const id = Number(btn.dataset.playerId);
      btn.classList.toggle("active", this.replayVisionSelection.has(id));
    }
  }

  updateRoomTimeStatus() {
    const status = dom.roomTimeControls?.querySelector(".room-time-tick-status");
    if (!status) return;
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const speed = Number.isFinite(this.roomTimeState?.speed) ? this.roomTimeState.speed : this.lastRoomTimeSpeed;
    const seeking = this.roomTimeSeekPending
      ? ` · Seeking${Number.isFinite(this.roomTimeSeekTargetTick) ? ` ${this.roomTimeSeekTargetTick}` : ""}...`
      : "";
    status.textContent = `${this.label} ${current} / ${duration} @ ${speed}x${seeking}`;
  }

  destroy() {
    if (!dom.roomTimeControls) return;
    if (this.roomTimeHandler) {
      dom.roomTimeControls.removeEventListener("click", this.roomTimeHandler);
      this.roomTimeHandler = null;
    }
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
    dom.roomTimeControls.classList.remove("replay-viewer-controls");
    dom.roomTimeControls.classList.remove("room-time-controls");
    dom.roomTimeControls.removeAttribute?.("aria-label");
    dom.roomTimeControls.querySelector(".replay-pause-btn")?.remove();
    dom.roomTimeControls.querySelector(".replay-branch-btn")?.remove();
    dom.roomTimeControls.querySelector(".replay-vision-controls")?.remove();
    dom.roomTimeControls.querySelector(".room-time-tick-status")?.remove();
    dom.roomTimeControls.querySelector(".room-time-timeline")?.remove();
    this.floatingPanel?.destroy();
    this.floatingPanel = null;
  }
}

export class ReplayControls extends RoomTimeControls {}
