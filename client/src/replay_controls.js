import { dom } from "./bootstrap.js";
import { REPLAY_VISION } from "./protocol.js";

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

    if (!dom.replaySpeed || !this.roomTime.available) return;

    dom.replaySpeed.hidden = false;
    dom.replaySpeed.classList.toggle("replay-viewer-controls", this.replayViewer);
    dom.replaySpeed.classList.add("room-time-controls");
    dom.replaySpeed.setAttribute("aria-label", `${this.label} controls`);
    for (const btn of dom.replaySpeed.querySelectorAll(".spd-btn")) {
      const speed = parseFloat(btn.dataset.speed);
      if (Number.isFinite(speed) && speed > 0) btn.hidden = !this.roomTime.setSpeed;
    }
    for (const btn of dom.replaySpeed.querySelectorAll(".seek-btn")) {
      btn.hidden = !this.roomTime.seekRelative;
    }
    for (const btn of dom.replaySpeed.querySelectorAll(".dev-pause-btn")) {
      btn.hidden = this.replayViewer || !this.roomTime.pause;
    }
    for (const btn of dom.replaySpeed.querySelectorAll(".dev-step-btn")) {
      btn.hidden = !this.roomTime.step;
    }
    this.roomTimeHandler = (e) => this.onRoomTimeControlClick(e);
    dom.replaySpeed.addEventListener("click", this.roomTimeHandler);
    this.setRoomTimeSpeedActive(this.replayViewer ? 2 : null);
    if (this.replayViewer && this.roomTime.pause) this.buildReplayPauseControl();
    if (this.replayViewer && this.actions.replayBranch) this.buildReplayBranchControl();
    if (this.visibility.replayVision) this.buildReplayVisionControls();
    this.buildRoomTimeStatus();
    if (this.roomTime.timeline && this.roomTime.seekAbsolute) this.buildRoomTimeTimeline();
    this.updateRoomTimePauseButton();
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
    if (btn.dataset.replayPauseToggle !== undefined || btn.classList.contains("dev-pause-btn")) {
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
    const status = dom.replaySpeed?.querySelector("#replay-concluded");
    if (!status) return;
    status.textContent = this.replayViewer ? "Replay Concluded" : "Room Time Ended";
    status.hidden = !concluded;
  }

  setRoomTimeSpeedActive(speed) {
    if (!dom.replaySpeed) return;
    for (const btn of dom.replaySpeed.querySelectorAll(".spd-btn:not(.seek-btn)")) {
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
    if (!dom.replaySpeed) return;
    const paused = this.isRoomTimePaused();
    for (const btn of dom.replaySpeed.querySelectorAll(".replay-pause-btn, .dev-pause-btn")) {
      btn.textContent = paused ? "Resume" : "Pause";
      btn.title = paused ? `Resume ${this.label.toLowerCase()} at ${this.lastRoomTimeSpeed}x.` : `Pause ${this.label.toLowerCase()}.`;
      btn.classList.toggle("active", paused);
    }
  }

  buildReplayPauseControl() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-pause-btn")) return;
    const pause = document.createElement("button");
    pause.type = "button";
    pause.className = "spd-btn replay-pause-btn";
    pause.dataset.replayPauseToggle = "1";
    pause.textContent = "Pause";
    pause.title = "Pause replay playback.";
    dom.replaySpeed.appendChild(pause);
  }

  buildReplayBranchControl() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-branch-btn")) return;
    const resume = document.createElement("button");
    resume.type = "button";
    resume.className = "spd-btn replay-branch-btn";
    resume.textContent = "Resume play from here";
    resume.title = "Create a practice branch from the current replay tick.";
    resume.addEventListener("click", () => this.net.requestReplayBranch());
    dom.replaySpeed.appendChild(resume);
  }

  buildReplayVisionControls() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-vision-controls")) return;

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
    dom.replaySpeed.appendChild(group);
  }

  buildRoomTimeStatus() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-tick-status")) return;
    const status = document.createElement("span");
    status.className = "replay-status replay-tick-status room-time-tick-status";
    status.textContent = `${this.label} 0 / 0`;
    dom.replaySpeed.appendChild(status);
  }

  buildRoomTimeTimeline() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-timeline")) return;
    if (!this.roomTime.timeline || !this.roomTime.seekAbsolute) return;

    const wrap = document.createElement("div");
    wrap.className = "replay-timeline room-time-timeline";

    const track = document.createElement("button");
    track.type = "button";
    track.className = "replay-timeline-track room-time-timeline-track";
    track.setAttribute("aria-label", `Seek ${this.label.toLowerCase()} timeline`);
    track.title = `Click to seek ${this.label.toLowerCase()}`;
    track.addEventListener("click", (ev) => this.onRoomTimeTimelineClick(ev));

    const progress = document.createElement("span");
    progress.className = "replay-timeline-progress room-time-timeline-progress";
    track.appendChild(progress);

    const marks = document.createElement("span");
    marks.className = "replay-timeline-marks room-time-timeline-marks";
    track.appendChild(marks);

    wrap.appendChild(track);
    dom.replaySpeed.appendChild(wrap);
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
    const timeline = dom.replaySpeed?.querySelector(".replay-timeline");
    if (!timeline) return;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const ratio = duration > 0 ? Math.max(0, Math.min(1, current / duration)) : 0;
    const progress = timeline.querySelector(".replay-timeline-progress");
    progress?.style.setProperty("--replay-progress", `${ratio * 100}%`);
    progress?.style.setProperty("--room-time-progress", `${ratio * 100}%`);

    const marks = timeline.querySelector(".replay-timeline-marks");
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
      mark.className = "replay-timeline-mark room-time-timeline-mark";
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
    if (!dom.replaySpeed) return;
    const allActive = this.replayVisionSelection.size === 0;
    for (const btn of dom.replaySpeed.querySelectorAll(".vision-btn")) {
      if (btn.dataset.vision === "all") {
        btn.classList.toggle("active", allActive);
        continue;
      }
      const id = Number(btn.dataset.playerId);
      btn.classList.toggle("active", this.replayVisionSelection.has(id));
    }
  }

  updateRoomTimeStatus() {
    const status = dom.replaySpeed?.querySelector(".replay-tick-status");
    if (!status) return;
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const speed = Number.isFinite(this.roomTimeState?.speed) ? this.roomTimeState.speed : this.lastRoomTimeSpeed;
    const seeking = this.roomTimeSeekPending
      ? ` · Seeking${Number.isFinite(this.roomTimeSeekTargetTick) ? ` ${this.roomTimeSeekTargetTick}` : ""}...`
      : "";
    status.textContent = `${this.label} ${current} / ${duration} @ ${speed}x${seeking}`;
  }

  onReplaySpeedClick(e) {
    return this.onRoomTimeControlClick(e);
  }

  setReplayConcluded(concluded) {
    return this.setRoomTimeConcluded(concluded);
  }

  isReplayPaused() {
    return this.isRoomTimePaused();
  }

  updateReplayPauseButton() {
    return this.updateRoomTimePauseButton();
  }

  buildReplayStatus() {
    return this.buildRoomTimeStatus();
  }

  buildReplayTimeline() {
    return this.buildRoomTimeTimeline();
  }

  onReplayTimelineClick(ev) {
    return this.onRoomTimeTimelineClick(ev);
  }

  setReplaySeekPending(targetTick, pending = true) {
    return this.setRoomTimeSeekPending(targetTick, pending);
  }

  updateReplayTimeline() {
    return this.updateRoomTimeTimeline();
  }

  updateReplayStatus() {
    return this.updateRoomTimeStatus();
  }

  destroy() {
    if (!dom.replaySpeed) return;
    if (this.roomTimeHandler) {
      dom.replaySpeed.removeEventListener("click", this.roomTimeHandler);
      this.roomTimeHandler = null;
    }
    dom.replaySpeed.hidden = true;
    this.setRoomTimeConcluded(false);
    for (const btn of dom.replaySpeed.querySelectorAll(".spd-btn")) {
      const speed = parseFloat(btn.dataset.speed);
      if (Number.isFinite(speed) && speed > 0) btn.hidden = false;
    }
    for (const btn of dom.replaySpeed.querySelectorAll(".seek-btn")) btn.hidden = false;
    for (const btn of dom.replaySpeed.querySelectorAll(".dev-pause-btn, .dev-step-btn")) {
      btn.hidden = true;
      if (btn.classList.contains("dev-pause-btn")) {
        btn.textContent = "Pause";
        btn.title = "Pause room time";
        btn.classList.remove("active");
      }
    }
    dom.replaySpeed.classList.remove("replay-viewer-controls");
    dom.replaySpeed.classList.remove("room-time-controls");
    dom.replaySpeed.removeAttribute?.("aria-label");
    dom.replaySpeed.querySelector(".replay-pause-btn")?.remove();
    dom.replaySpeed.querySelector(".replay-branch-btn")?.remove();
    dom.replaySpeed.querySelector(".replay-vision-controls")?.remove();
    dom.replaySpeed.querySelector(".replay-tick-status")?.remove();
    dom.replaySpeed.querySelector(".replay-timeline")?.remove();
  }
}

export class ReplayControls extends RoomTimeControls {}
