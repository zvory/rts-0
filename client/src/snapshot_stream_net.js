// Offline snapshot transport. It feeds pre-encoded server snapshot frames through Net's normal
// decoder and event emitter, but never opens a WebSocket or runs a simulation.

import { Net } from "./net.js";
import { S } from "./protocol.js";

const MAGIC = "RTSSTRM1";
const MAX_HEADER_BYTES = 1024 * 1024;
const MAX_FRAMES = 10_000;

export function snapshotStreamAssetUrl(id) {
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(id || "")) {
    throw new Error("Invalid snapshot stream id");
  }
  return `/assets/snapshot-streams/${id}.rtsstream`;
}

export function parseSnapshotStream(data) {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  if (bytes.byteLength < MAGIC.length + 4) throw new Error("Snapshot stream is truncated");
  const magic = new TextDecoder().decode(bytes.subarray(0, MAGIC.length));
  if (magic !== MAGIC) throw new Error("Snapshot stream has an invalid magic header");

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const headerLength = view.getUint32(MAGIC.length, true);
  if (headerLength === 0 || headerLength > MAX_HEADER_BYTES) {
    throw new Error("Snapshot stream header length is invalid");
  }
  let offset = MAGIC.length + 4;
  if (offset + headerLength > bytes.byteLength) throw new Error("Snapshot stream header is truncated");

  let header;
  try {
    header = JSON.parse(new TextDecoder().decode(bytes.subarray(offset, offset + headerLength)));
  } catch {
    throw new Error("Snapshot stream header is not valid JSON");
  }
  offset += headerLength;
  const frameCount = Number(header?.frameCount);
  if (
    header?.schemaVersion !== 1 ||
    !Number.isInteger(frameCount) ||
    frameCount < 1 ||
    frameCount > MAX_FRAMES ||
    !Number.isFinite(header?.tickRateHz) ||
    header.tickRateHz <= 0 ||
    !header?.start ||
    typeof header.start !== "object"
  ) {
    throw new Error("Snapshot stream header contract is invalid");
  }

  const frames = [];
  for (let index = 0; index < frameCount; index += 1) {
    if (offset + 4 > bytes.byteLength) throw new Error(`Snapshot stream frame ${index} is truncated`);
    const frameLength = view.getUint32(offset, true);
    offset += 4;
    if (frameLength === 0 || offset + frameLength > bytes.byteLength) {
      throw new Error(`Snapshot stream frame ${index} length is invalid`);
    }
    frames.push(bytes.slice(offset, offset + frameLength).buffer);
    offset += frameLength;
  }
  if (offset !== bytes.byteLength) throw new Error("Snapshot stream has trailing bytes");
  return { header, frames };
}

export class SnapshotStreamNet extends Net {
  constructor({
    id,
    diagnostics = null,
    fetchFn = (...args) => fetch(...args),
    now = () => performance.now(),
    setTimeoutFn = (...args) => setTimeout(...args),
    clearTimeoutFn = (timer) => clearTimeout(timer),
    autoStart = true,
  }) {
    super(snapshotStreamAssetUrl(id), diagnostics);
    this.id = id;
    this.offline = true;
    this.fetchFn = fetchFn;
    this.now = now;
    this.setTimeoutFn = setTimeoutFn;
    this.clearTimeoutFn = clearTimeoutFn;
    this.autoStart = autoStart;
    this.header = null;
    this.frames = [];
    this.frameIndex = 0;
    this.loopCount = 0;
    this.timer = null;
    this.startedAt = 0;
    this.closed = false;
    this.publicState = {
      id,
      source: "static-snapshot-stream",
      offline: true,
      websocket: false,
      serverSimulation: false,
      frameCount: 0,
      frameIndex: 0,
      loopCount: 0,
      tickRateHz: 0,
    };
  }

  async connect() {
    // Revalidate the stable asset URL so regenerating a stream cannot leave a benchmark silently
    // replaying an older cached artifact. A validated response may still be served from cache.
    const response = await this.fetchFn(this.url, { cache: "no-cache" });
    if (!response?.ok) {
      throw new Error(`Unable to load snapshot stream ${this.id} (${response?.status || "network error"})`);
    }
    const parsed = parseSnapshotStream(await response.arrayBuffer());
    if (parsed.header.id !== this.id) {
      throw new Error(`Snapshot stream id mismatch: expected ${this.id}`);
    }
    this.header = parsed.header;
    this.frames = parsed.frames;
    this._playerId = Number(this.header.start.playerId) || 1;
    this.publicState.frameCount = this.frames.length;
    this.publicState.tickRateHz = this.header.tickRateHz;
    if (typeof window !== "undefined") window.__rtsSnapshotStream = this.publicState;

    this.diagnostics?.mark("snapshotStream.open", {
      id: this.id,
      frames: this.frames.length,
      tickRateHz: this.header.tickRateHz,
    });
    this._emit("open");
    this._emit(S.START, this.header.start);
    if (this.autoStart) this._startFrames();
  }

  restartFromBeginning() {
    if (!this.header) throw new Error("Snapshot stream is not loaded");
    if (this.closed) throw new Error("Snapshot stream is closed");
    if (this.timer !== null) this.clearTimeoutFn(this.timer);
    this.timer = null;
    this.frameIndex = 0;
    this.loopCount = 0;
    this.publicState.frameIndex = 0;
    this.publicState.loopCount = 0;
    this._emit(S.START, this.header.start);
    this._startFrames();
  }

  close() {
    this.closed = true;
    if (this.timer !== null) this.clearTimeoutFn(this.timer);
    this.timer = null;
  }

  _schedule(delayMs) {
    if (this.closed) return;
    this.timer = this.setTimeoutFn(() => this._deliver(), Math.max(0, delayMs));
  }

  _startFrames() {
    this.startedAt = this.now();
    this._schedule(0);
  }

  _deliver() {
    this.timer = null;
    if (this.closed) return;
    if (this.frameIndex >= this.frames.length) {
      if (!this.header.loop) return;
      this.loopCount += 1;
      this.frameIndex = 0;
      this.publicState.loopCount = this.loopCount;
      this.publicState.frameIndex = 0;
      this._emit(S.START, this.header.start);
      const gap = Math.max(0, Number(this.header.loopGapMs) || 0);
      this.startedAt = this.now() + gap;
      this._schedule(gap);
      return;
    }

    this._onMessage({ data: this.frames[this.frameIndex] });
    this.frameIndex += 1;
    this.publicState.frameIndex = this.frameIndex;
    const intervalMs = 1000 / this.header.tickRateHz;
    const nextAt = this.startedAt + this.frameIndex * intervalMs;
    this._schedule(nextAt - this.now());
  }
}
