const LATENCY_ISSUE_MS = 180;
const JITTER_ISSUE_MS = 20;
const JITTER_WINDOW = 8;
const FPS_WINDOW_MS = 60_000;

export class MatchHealth {
  constructor({ net, statusBadge, snapshotMs }) {
    this.net = net;
    this.statusBadge = statusBadge;
    this.snapshotMs = snapshotMs;
    this.lastLatencySampleAt = 0;
    this.snapshotJitterDeltas = [];
    this.lastSnapshotArrivedAt = null;
    this.reportStartedAt = performance.now();
    this.reportStats = this.createReportStats();
    this.frameSamples = [];
    this.frameWindowTotalMs = 0;
    this.health = {
      latencyMs: null,
      serverTickMs: null,
      serverLagMs: null,
      jitterMs: null,
      fps: null,
      fpsOneMinute: null,
      issues: {
        latency: { active: false, count: 0 },
        slowTick: { active: false, count: 0 },
        headOfLine: { active: false, count: 0 },
        jitter: { active: false, count: 0 },
      },
    };
  }

  createReportStats() {
    return {
      rttMaxMs: 0,
      badRttSamples: 0,
      snapshotGapMaxMs: 0,
      jitterSamples: 0,
      snapshots: 0,
      frameGapMaxMs: 0,
      frameCount: 0,
      frameTotalMs: 0,
    };
  }

  resetReportStats(now = performance.now()) {
    this.reportStartedAt = now;
    this.reportStats = this.createReportStats();
  }

  noteFrameGap(frameGapMs, now = performance.now()) {
    if (!Number.isFinite(frameGapMs) || frameGapMs < 0) return;
    this.reportStats.frameCount += 1;
    this.reportStats.frameTotalMs += frameGapMs;
    this.reportStats.frameGapMaxMs = Math.max(this.reportStats.frameGapMaxMs, frameGapMs);
    if (frameGapMs <= 0 || !Number.isFinite(now)) return;
    this.health.fps = 1000 / frameGapMs;
    this.frameSamples.push({ at: now, gapMs: frameGapMs });
    this.frameWindowTotalMs += frameGapMs;
    this.pruneFrameSamples(now);
    this.health.fpsOneMinute = this.frameWindowTotalMs > 0
      ? this.frameSamples.length * 1000 / this.frameWindowTotalMs
      : null;
  }

  pruneFrameSamples(now) {
    const cutoff = now - FPS_WINDOW_MS;
    let removeCount = 0;
    while (removeCount < this.frameSamples.length && this.frameSamples[removeCount].at < cutoff) {
      this.frameWindowTotalMs -= this.frameSamples[removeCount].gapMs;
      removeCount += 1;
    }
    if (removeCount > 0) this.frameSamples.splice(0, removeCount);
    if (this.frameSamples.length === 0) this.frameWindowTotalMs = 0;
  }

  noteSnapshotArrival(now, documentHidden) {
    this.reportStats.snapshots += 1;
    if (!documentHidden && this.lastSnapshotArrivedAt != null) {
      const gap = now - this.lastSnapshotArrivedAt;
      this.reportStats.snapshotGapMaxMs = Math.max(this.reportStats.snapshotGapMaxMs, gap);
      const delta = Math.abs(gap - this.snapshotMs);
      this.snapshotJitterDeltas.push(delta);
      if (this.snapshotJitterDeltas.length > JITTER_WINDOW) {
        this.snapshotJitterDeltas.splice(0, this.snapshotJitterDeltas.length - JITTER_WINDOW);
      }
      this.health.jitterMs = Math.max(...this.snapshotJitterDeltas, 0);
      const jitterActive = delta >= JITTER_ISSUE_MS;
      this.health.issues.jitter.active = jitterActive;
      if (jitterActive) {
        this.health.issues.jitter.count += 1;
        this.reportStats.jitterSamples += 1;
      }
    }
    this.lastSnapshotArrivedAt = now;
  }

  applyServerNetStatus(status) {
    if (!status) return;
    this.health.serverTickMs = status.tickMs;
    this.health.serverLagMs = status.serverLagMs;
    this.health.issues.slowTick.active = !!status.slowTick;
    this.health.issues.slowTick.count = status.slowTickCount || 0;
    this.health.issues.headOfLine.active = !!status.headOfLine;
    this.health.issues.headOfLine.count = status.headOfLineCount || 0;
  }

  refreshLatency() {
    if (this.net.latencyUpdatedAt === this.lastLatencySampleAt) return;
    this.lastLatencySampleAt = this.net.latencyUpdatedAt;
    this.health.latencyMs = this.net.latency;
    const latencyActive = Number.isFinite(this.net.latency) && this.net.latency >= LATENCY_ISSUE_MS;
    this.health.issues.latency.active = latencyActive;
    if (latencyActive) {
      this.health.issues.latency.count += 1;
      this.reportStats.badRttSamples += 1;
    }
    if (Number.isFinite(this.net.latency)) {
      this.reportStats.rttMaxMs = Math.max(this.reportStats.rttMaxMs, this.net.latency);
    }
  }

  publish() {
    this.statusBadge?.setMatchMetrics(this.metrics());
  }

  metrics() {
    return {
      latencyMs: this.health.latencyMs,
      serverTickMs: this.health.serverTickMs,
      serverLagMs: this.health.serverLagMs,
      jitterMs: this.health.jitterMs,
      fps: this.health.fps,
      fpsOneMinute: this.health.fpsOneMinute,
      issues: this.health.issues,
    };
  }
}
