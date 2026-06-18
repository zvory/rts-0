import { dom } from "./bootstrap.js";
import { REPLAY_VISION } from "./protocol.js";

export class ReplayControls {
  constructor({ net, state, replayViewer = false, isReplay = false, isScenario = false }) {
    this.net = net;
    this.state = state;
    this.replayViewer = !!replayViewer;
    this.isReplay = !!isReplay;
    this.isScenario = !!isScenario;
    this.replayVisionSelection = new Set();
    this.roomTimeState = null;
    this.replaySeekPending = false;
    this.replaySeekTargetTick = null;
    this.replaySpeedHandler = null;
    this.lastReplaySpeed = 2;

    if (!dom.replaySpeed || (!this.isReplay && !this.isScenario)) return;

    dom.replaySpeed.hidden = false;
    dom.replaySpeed.classList.toggle("replay-viewer-controls", this.replayViewer);
    for (const btn of dom.replaySpeed.querySelectorAll(".seek-btn")) {
      btn.hidden = this.isScenario;
    }
    for (const btn of dom.replaySpeed.querySelectorAll(".dev-pause-btn, .dev-step-btn")) {
      btn.hidden = !this.isScenario;
    }
    this.replaySpeedHandler = (e) => this.onReplaySpeedClick(e);
    dom.replaySpeed.addEventListener("click", this.replaySpeedHandler);
    this.setRoomTimeSpeedActive(this.replayViewer ? 2 : null);
    if (this.replayViewer) this.buildReplayVisionControls();
  }

  onReplaySpeedClick(e) {
    const btn = e.target.closest(".spd-btn");
    if (!btn) return;
    if (btn.dataset.stepRoomTime !== undefined) {
      if (!this.isScenario) return;
      this.net.stepRoomTime();
      return;
    }
    if (btn.dataset.seekBack !== undefined) {
      if (!this.isReplay) return;
      const ticksBack = parseInt(btn.dataset.seekBack, 10);
      if (!isFinite(ticksBack) || ticksBack <= 0) return;
      this.setReplayConcluded(false);
      const currentTick = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
      this.setReplaySeekPending(Math.max(0, currentTick - ticksBack));
      this.net.seekRoomTime(ticksBack);
      return;
    }
    if (btn.dataset.replayPauseToggle !== undefined) {
      if (!this.isReplay) return;
      const speed = this.isReplayPaused() ? this.lastReplaySpeed : 0;
      this.net.setRoomTimeSpeed(speed);
      this.roomTimeState = { ...(this.roomTimeState || {}), speed, paused: speed === 0 };
      this.setRoomTimeSpeedActive(speed);
      this.updateReplayPauseButton();
      this.updateReplayStatus();
      return;
    }
    const speed = parseFloat(btn.dataset.speed);
    if (!isFinite(speed)) return;
    if (speed === 0 && !this.isScenario) return;
    if (this.isReplay && speed > 0) this.lastReplaySpeed = speed;
    this.net.setRoomTimeSpeed(speed);
    if (this.isReplay) this.roomTimeState = { ...(this.roomTimeState || {}), speed, paused: speed === 0 };
    this.setRoomTimeSpeedActive(speed);
    this.updateReplayPauseButton();
    this.updateReplayStatus();
  }

  applyRoomTimeState(state) {
    this.roomTimeState = state || null;
    this.setReplaySeekPending(null, false);
    if (Number.isFinite(state?.speed) && state.speed > 0) this.lastReplaySpeed = state.speed;
    const ended =
      state?.ended === true ||
      (Number.isFinite(state?.currentTick) &&
        Number.isFinite(state?.durationTicks) &&
        state.durationTicks > 0 &&
        state.currentTick >= state.durationTicks);
    this.setReplayConcluded(ended);
    if (Number.isFinite(state?.speed)) this.setRoomTimeSpeedActive(state.speed);
    this.updateReplayPauseButton();
    this.updateReplayStatus();
    this.updateReplayTimeline();
  }

  noteSnapshotTick(tick) {
    if (!this.replayViewer || !Number.isFinite(tick)) return;
    this.roomTimeState = { ...(this.roomTimeState || {}), currentTick: tick };
    this.updateReplayStatus();
    this.updateReplayTimeline();
  }

  setReplayConcluded(concluded) {
    const status = dom.replaySpeed?.querySelector("#replay-concluded");
    if (status) status.hidden = !concluded;
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

  isReplayPaused() {
    return this.roomTimeState?.paused === true || this.roomTimeState?.speed === 0;
  }

  updateReplayPauseButton() {
    const btn = dom.replaySpeed?.querySelector(".replay-pause-btn");
    if (!btn) return;
    const paused = this.isReplayPaused();
    btn.textContent = paused ? "Resume" : "Pause";
    btn.title = paused ? "Resume replay playback." : "Pause replay playback.";
    btn.classList.toggle("active", paused);
  }

  buildReplayVisionControls() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-vision-controls")) return;

    if (!dom.replaySpeed.querySelector(".replay-pause-btn")) {
      const pause = document.createElement("button");
      pause.type = "button";
      pause.className = "spd-btn replay-pause-btn";
      pause.dataset.replayPauseToggle = "1";
      pause.textContent = "Pause";
      pause.title = "Pause replay playback.";
      dom.replaySpeed.appendChild(pause);
    }

    if (!dom.replaySpeed.querySelector(".replay-branch-btn")) {
      const resume = document.createElement("button");
      resume.type = "button";
      resume.className = "spd-btn replay-branch-btn";
      resume.textContent = "Resume play from here";
      resume.title = "Create a practice branch from the current replay tick.";
      resume.addEventListener("click", () => this.net.requestReplayBranch());
      dom.replaySpeed.appendChild(resume);
    }

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

    const status = document.createElement("span");
    status.className = "replay-status replay-tick-status";
    status.textContent = "Replay 0 / 0";
    dom.replaySpeed.appendChild(status);

    this.buildReplayTimeline();
  }

  buildReplayTimeline() {
    if (!dom.replaySpeed || dom.replaySpeed.querySelector(".replay-timeline")) return;

    const wrap = document.createElement("div");
    wrap.className = "replay-timeline";

    const track = document.createElement("button");
    track.type = "button";
    track.className = "replay-timeline-track";
    track.setAttribute("aria-label", "Seek replay timeline");
    track.title = "Click to seek replay";
    track.addEventListener("click", (ev) => this.onReplayTimelineClick(ev));

    const progress = document.createElement("span");
    progress.className = "replay-timeline-progress";
    track.appendChild(progress);

    const marks = document.createElement("span");
    marks.className = "replay-timeline-marks";
    track.appendChild(marks);

    wrap.appendChild(track);
    dom.replaySpeed.appendChild(wrap);
    this.updateReplayTimeline();
  }

  onReplayTimelineClick(ev) {
    const track = ev.currentTarget;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    if (!track || duration <= 0) return;
    const rect = track.getBoundingClientRect();
    if (!rect.width) return;
    const ratio = Math.max(0, Math.min(1, (ev.clientX - rect.left) / rect.width));
    const tick = Math.round(ratio * duration);
    this.setReplayConcluded(false);
    this.setReplaySeekPending(tick);
    this.net.seekRoomTimeTo(tick);
  }

  setReplaySeekPending(targetTick, pending = true) {
    this.replaySeekPending = !!pending;
    this.replaySeekTargetTick = this.replaySeekPending && Number.isFinite(targetTick) ? targetTick : null;
    this.updateReplayStatus();
    this.updateReplayTimeline();
  }

  updateReplayTimeline() {
    const timeline = dom.replaySpeed?.querySelector(".replay-timeline");
    if (!timeline) return;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const ratio = duration > 0 ? Math.max(0, Math.min(1, current / duration)) : 0;
    timeline.querySelector(".replay-timeline-progress")?.style.setProperty("--replay-progress", `${ratio * 100}%`);

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
      mark.className = "replay-timeline-mark";
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

  updateReplayStatus() {
    const status = dom.replaySpeed?.querySelector(".replay-tick-status");
    if (!status) return;
    const current = Number.isFinite(this.roomTimeState?.currentTick) ? this.roomTimeState.currentTick : 0;
    const duration = Number.isFinite(this.roomTimeState?.durationTicks) ? this.roomTimeState.durationTicks : 0;
    const speed = Number.isFinite(this.roomTimeState?.speed) ? this.roomTimeState.speed : 2;
    const seeking = this.replaySeekPending
      ? ` · Seeking${Number.isFinite(this.replaySeekTargetTick) ? ` ${this.replaySeekTargetTick}` : ""}...`
      : "";
    status.textContent = `Replay ${current} / ${duration} @ ${speed}x${seeking}`;
  }

  destroy() {
    if (!dom.replaySpeed) return;
    if (this.replaySpeedHandler) {
      dom.replaySpeed.removeEventListener("click", this.replaySpeedHandler);
      this.replaySpeedHandler = null;
    }
    dom.replaySpeed.hidden = true;
    this.setReplayConcluded(false);
    for (const btn of dom.replaySpeed.querySelectorAll(".seek-btn")) btn.hidden = false;
    for (const btn of dom.replaySpeed.querySelectorAll(".dev-pause-btn, .dev-step-btn")) {
      btn.hidden = true;
    }
    dom.replaySpeed.classList.remove("replay-viewer-controls");
    dom.replaySpeed.querySelector(".replay-pause-btn")?.remove();
    dom.replaySpeed.querySelector(".replay-branch-btn")?.remove();
    dom.replaySpeed.querySelector(".replay-vision-controls")?.remove();
    dom.replaySpeed.querySelector(".replay-tick-status")?.remove();
    dom.replaySpeed.querySelector(".replay-timeline")?.remove();
  }
}
