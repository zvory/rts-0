// Audio — client-side sound engine. See docs/sound/ASSETS.md and docs/sound/SOUND_NOTES.md.
//
// Owns one AudioContext (lazily created on first user gesture, per browser policy),
// a category gain bus, a buffer cache keyed by sound id, and a capped voice pool.
// Wired in main.js via DI; no module other than main.js constructs this class.
//
// Invariants:
//   - `play()` is a no-op until a user gesture has unlocked the AudioContext.
//   - One AudioBuffer per id; never decode per playback.
//   - Voice pool is capped (VOICE_CAP); eviction is priority-based, not FIFO.
//   - Pitch variance uses a seeded RNG so audio is reproducible for replays.
//   - Spatial play (opts.x/y present) goes through StereoPanner + lowpass per-voice.
//     `setListener()` must be called each frame from Match to keep distance math
//     in sync with the camera.
//   - Callers can tag voices with `key` and stop them early for sustained cues
//     whose authoritative game state has ended.

import {
  DEFAULT_AUDIO_REF_DIST,
  computeSpatialAudio,
} from "./audio_spatial.js";

const CATEGORIES = Object.freeze([
  "ui",
  "alert",
  "combat_self",
  "combat_other",
  "unit_voice",
  "ambient",
]);
const CATEGORY_SET = new Set(CATEGORIES);

/** Per-category default volumes when no localStorage value is present. */
const DEFAULT_MASTER = 0.7;
const DEFAULT_AMBIENT = 0.4;
const DEFAULT_COMBAT = 0.5;
const DEFAULT_OTHER = 1.0;

/** Maximum concurrent voices. Beyond this, eviction kicks in. */
const VOICE_CAP = 48;
/** Drop repeats of the same `id` within this window (ms). */
const DEDUP_MS = 60;
/** Spoken feedback should never stack faster than the line itself. */
const SPOKEN_MIN_COOLDOWN_MS = 1500;
/** Pitch jitter (±fraction) applied via playbackRate when caller does not override. */
const PITCH_VARIANCE = 0.06;
/** Master gain ramp time on tab show/hide (s). */
const VISIBILITY_RAMP_S = 0.1;
/** Spatial param ramp for in-flight voices when the listener moves (s).
 *  Short enough to feel instant on a minimap jump, long enough to avoid zipper noise. */
const SPATIAL_REFRESH_RAMP_S = 0.03;
/** Alert ducking ramps in fast and releases slower so spoken lines cut through battles. */
const DUCK_IN_S = 0.08;
const DUCK_OUT_S = 2.0;
const DB_ALERT_AMBIENT = -12;
const DB_ALERT_COMBAT = -10;

const BASE_PRIORITY = Object.freeze({
  alert: 100,
  ui: 90,
  unit_voice: 70,
  combat_self: 60,
  combat_other: 40,
  ambient: 10,
});
const STICKY_BONUS = Object.freeze({
  alert: 20,
  ui: 10,
});
const SPOKEN_CATEGORIES = new Set(["alert", "ui", "unit_voice"]);

export { CATEGORIES };

function clamp01(v) {
  v = Number(v);
  if (!isFinite(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function dbToGain(db) {
  return Math.pow(10, db / 20);
}

function lsRead(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    if (v == null) return fallback;
    const n = parseFloat(v);
    return isFinite(n) ? clamp01(n) : fallback;
  } catch {
    return fallback;
  }
}

function lsWrite(key, v) {
  try {
    localStorage.setItem(key, String(v));
  } catch {
    /* private mode etc. — settings reset across reloads, no other harm. */
  }
}

// mulberry32: tiny deterministic PRNG. We never use Math.random in audio so the
// per-frame sequence of `play()` calls remains reproducible (phase 5).
function mulberry32(seed) {
  let s = seed >>> 0;
  return function () {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class Audio {
  constructor() {
    /** @type {AudioContext|null} */
    this.ctx = null;
    /** @type {GainNode|null} */
    this.master = null;
    /** @type {Record<string, GainNode>} */
    this.gains = {};
    /** @type {Map<string, AudioBuffer>} */
    this.buffers = new Map();
    /** @type {Map<string, Promise<AudioBuffer|null>>} */
    this.pending = new Map();
    /** Active voices, oldest first. */
    this.voices = [];
    /** Last-played timestamps (ms) for dedup. */
    this.lastPlay = new Map();
    /** Deterministic RNG for pitch variance. */
    this.rng = mulberry32(0xb7e15163);
    /** Manifest entries queued before the context was unlocked. */
    this._queuedManifests = [];
    /** @type {Set<() => void>} listeners notified when the context starts running. */
    this._unlockListeners = new Set();
    /** Whether queued manifests have already been handed to the decoder. */
    this._decodedQueued = false;
    /** Listener pose in world pixels + reference distance (1 screen-width at current zoom). */
    this.listener = { x: 0, y: 0, refDist: DEFAULT_AUDIO_REF_DIST };
    /** Number of active alert voices currently forcing lower-priority buses down. */
    this.alertDuckDepth = 0;

    this.volume = { master: lsRead("audio.master", DEFAULT_MASTER) };
    for (const c of CATEGORIES) {
      const fallback =
        c === "ambient"
          ? DEFAULT_AMBIENT
          : c === "combat_self" || c === "combat_other"
            ? DEFAULT_COMBAT
            : DEFAULT_OTHER;
      this.volume[c] = lsRead(`audio.cat.${c}`, fallback);
    }

    this._gesture = this._gesture.bind(this);
    this._visibility = this._visibility.bind(this);
    this._gestureEvents = [
      "pointerdown",
      "pointerup",
      "click",
      "mousedown",
      "touchstart",
      "keydown",
    ];
    for (const ev of this._gestureEvents) {
      window.addEventListener(ev, this._gesture, true);
    }
    document.addEventListener("visibilitychange", this._visibility);
  }

  /**
   * Preload a manifest of `{ id, url, category }`. Safe to call before the
   * audio context exists — entries are decoded once the first gesture unlocks it.
   * Re-preloading the same id is a no-op.
   * @returns {Promise<void>}
   */
  preload(manifest) {
    if (!Array.isArray(manifest) || manifest.length === 0) return Promise.resolve();
    if (!this.isUnlocked()) {
      this._queuedManifests.push(manifest);
      return Promise.resolve();
    }
    return this._decodeManifest(manifest);
  }

  /**
   * Attempt to unlock audio from a user gesture. Browser policy decides whether
   * the call succeeds; callers should invoke this only from real interaction
   * handlers so the gesture token is still live.
   * @param {Event} [ev]
   * @returns {Promise<boolean>} true once the context is running
   */
  async unlockFromGesture(ev) {
    if (ev && ev.isTrusted === false) return false;
    const ctx = this._ensureContext();
    if (!ctx) return false;
    if (ctx.state === "running") {
      this._markUnlocked();
      return true;
    }
    if (typeof ctx.resume === "function") {
      try {
        await ctx.resume();
      } catch {
        return false;
      }
    }
    if (ctx.state === "running") {
      this._markUnlocked();
      return true;
    }
    return false;
  }

  isUnlocked() {
    return !!this.ctx && this.ctx.state === "running";
  }

  /**
   * Subscribe to unlock state changes.
   * @param {() => void} fn
   * @returns {() => void}
   */
  onUnlockChange(fn) {
    if (typeof fn !== "function") return () => {};
    this._unlockListeners.add(fn);
    return () => this._unlockListeners.delete(fn);
  }

  /**
   * Schedule a one-shot.
   * @param {string} id manifest id
   * @param {object} [opts]
   * @param {number} [opts.x] world pixels - when present, sound is spatialized
   * @param {number} [opts.y] world pixels - when present, sound is spatialized
   * @param {number} [opts.priority] higher wins eviction (default 1)
   * @param {string} [opts.category] one of CATEGORIES (default "ui")
   * @param {number} [opts.pitchVariance] override default jitter (0 to disable)
   * @param {number} [opts.gain] linear gain multiplier applied before the category bus
   * @param {string} [opts.key] caller-owned voice key for early stopping
   * @param {boolean} [opts.loop] repeat the decoded buffer until stopped by key
   * @param {number} [opts.fadeInMs] ramp the per-voice gain up from silence
   * @param {string} [opts.dedupKey] override per-sound dedup bucket
   * @param {number} [opts.cooldownMs] override dedup cooldown
   * @param {boolean} [opts.duck] explicitly duck combat and ambient buses for this voice
   * @returns {boolean} true if scheduled, false if dropped
   */
  play(id, opts) {
    if (!this.isUnlocked()) return false;
    const buf = this.buffers.get(id);
    if (!buf) return false;
    opts = opts || {};

    const now = performance.now();
    const category = opts.category && CATEGORY_SET.has(opts.category) ? opts.category : "ui";
    const priorityBonus = typeof opts.priority === "number" ? opts.priority : 0;

    // Spatial gate: if a position is supplied, compute attenuation/pan/lpHz from
    // the current listener and the category's explicit spatial profile. Distant
    // sounds are dropped before voice allocation — the biggest perf win in a
    // 200-unit fight.
    let spatial = null;
    if (typeof opts.x === "number" && typeof opts.y === "number") {
      spatial = this._computeSpatial(opts.x, opts.y, category);
      if (!spatial) return false;
    }

    const dedupKey = this._dedupKey(id, category, opts, spatial);
    const cooldownMs = this._cooldownMs(id, category, opts, buf);
    const last = this.lastPlay.get(dedupKey);
    if (last != null && now - last < cooldownMs) return false;

    const distancePenalty =
      typeof opts.distancePenalty === "number"
        ? Math.max(0, opts.distancePenalty)
        : spatial?.distancePenalty || 0;
    const incomingScore = this._score(category, now, now, distancePenalty, priorityBonus);

    if (this.voices.length >= VOICE_CAP) {
      if (!this._evictLowest(incomingScore, now)) return false;
    }

    const src = this.ctx.createBufferSource();
    src.buffer = buf;
    src.loop = opts.loop === true;
    const variance = opts.pitchVariance == null ? PITCH_VARIANCE : Math.max(0, opts.pitchVariance);
    if (variance > 0) {
      src.playbackRate.value = 1 + (this.rng() * 2 - 1) * variance;
    }
    const bus = this.gains[category] || this.master;
    const gainValue = typeof opts.gain === "number" ? Math.max(0, opts.gain) : 1;

    const trail = [];
    const gainNode = this.ctx.createGain();
    const fadeInMs = Number(opts.fadeInMs);
    if (Number.isFinite(fadeInMs) && fadeInMs > 0) {
      const start = this.ctx.currentTime;
      gainNode.gain.value = 0;
      gainNode.gain.setValueAtTime(0, start);
      gainNode.gain.linearRampToValueAtTime(gainValue, start + fadeInMs / 1000);
    } else {
      gainNode.gain.value = gainValue;
    }
    let spatialNodes = null;
    if (spatial) {
      const panner = this.ctx.createStereoPanner();
      panner.pan.value = spatial.pan;
      const lp = this.ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.value = spatial.lpHz;
      const distGain = this.ctx.createGain();
      distGain.gain.value = spatial.gain;
      src.connect(panner);
      panner.connect(lp);
      lp.connect(distGain);
      distGain.connect(gainNode);
      gainNode.connect(bus);
      trail.push(panner, lp, distGain, gainNode);
      spatialNodes = { panner, lp, distGain, x: opts.x, y: opts.y };
    } else {
      src.connect(gainNode);
      gainNode.connect(bus);
      trail.push(gainNode);
    }
    try {
      src.start();
    } catch {
      return false;
    }
    const key = typeof opts.key === "string" && opts.key ? opts.key : null;
    const ducking = opts.duck === true || category === "alert";
    const voice = {
      node: src,
      priorityBonus,
      startedAt: now,
      category,
      id,
      key,
      trail,
      gainNode,
      distancePenalty,
      spatial: spatialNodes,
      ducking,
    };
    this.voices.push(voice);
    src.onended = () => this._finishVoice(voice);
    this.lastPlay.set(dedupKey, now);
    if (ducking) this._beginAlertDuck();
    return true;
  }

  /** Convenience: forces the ui category (non-spatial). */
  playUI(id, opts) {
    return this.play(id, { ...(opts || {}), category: "ui" });
  }

  /**
   * Stop all active voices tagged with a caller-owned key.
   * @param {string} key
   * @param {{fadeOutMs?:number}} [opts]
   * @returns {number} number of voices stopped or scheduled to stop
   */
  stopByKey(key, opts) {
    if (typeof key !== "string" || !key) return 0;
    const fadeOutMs = Number(opts?.fadeOutMs);
    const fade = Number.isFinite(fadeOutMs) && fadeOutMs > 0;
    let stopped = 0;
    for (const voice of [...this.voices]) {
      if (voice.key !== key || (voice.stopping && fade)) continue;
      if (fade) {
        this._fadeOutVoice(voice, fadeOutMs);
      } else {
        this._stopVoice(voice);
      }
      stopped += 1;
    }
    return stopped;
  }

  /** @param {string} key */
  hasVoiceKey(key) {
    return typeof key === "string" && key !== "" && this.voices.some((voice) => voice.key === key);
  }

  /**
   * Pick a variant id from a non-empty list using the seeded RNG. Returns null
   * if the list is empty. See PHASE_2.md §"Combat SFX wiring" — avoids the
   * "machine gun" feel of identical samples stacking.
   * @param {string[]} ids
   * @returns {string|null}
   */
  pickVariant(ids) {
    if (!Array.isArray(ids) || ids.length === 0) return null;
    if (ids.length === 1) return ids[0];
    const i = Math.floor(this.rng() * ids.length);
    return ids[i < ids.length ? i : ids.length - 1];
  }

  /**
   * Update the renderer-neutral listener pose and focus-plane reference distance.
   * @param {{x:number,y:number,referenceDistancePx:number}} listener
   */
  setListener(listener) {
    const x = Number(listener?.x);
    const y = Number(listener?.y);
    const referenceDistancePx = Number(listener?.referenceDistancePx);
    if (!Number.isFinite(x) || !Number.isFinite(y)) return;
    this.listener.x = x;
    this.listener.y = y;
    if (Number.isFinite(referenceDistancePx) && referenceDistancePx > 0) {
      this.listener.refDist = Math.max(1, referenceDistancePx);
    }
    this._refreshSpatialVoices();
  }

  /**
   * Re-evaluate pan/lowpass/distance gain for every in-flight spatial voice
   * against the current listener pose. Called from setListener so a minimap
   * jump (or any large camera move) updates dampening within ~30ms instead of
   * waiting for the next play() of the same sound.
   */
  _refreshSpatialVoices() {
    if (!this.ctx || this.voices.length === 0) return;
    const t = this.ctx.currentTime;
    const ramp = t + SPATIAL_REFRESH_RAMP_S;
    for (const voice of this.voices) {
      const s = voice.spatial;
      if (!s) continue;
      const next = this._computeSpatial(s.x, s.y, voice.category);
      if (!next) {
        // Beyond max distance — fade to zero quickly; let the voice finish naturally.
        s.distGain.gain.cancelScheduledValues(t);
        s.distGain.gain.setValueAtTime(s.distGain.gain.value, t);
        s.distGain.gain.linearRampToValueAtTime(0, ramp);
        voice.distancePenalty = 30;
        continue;
      }
      s.panner.pan.cancelScheduledValues(t);
      s.panner.pan.setValueAtTime(s.panner.pan.value, t);
      s.panner.pan.linearRampToValueAtTime(next.pan, ramp);
      s.lp.frequency.cancelScheduledValues(t);
      s.lp.frequency.setValueAtTime(s.lp.frequency.value, t);
      s.lp.frequency.linearRampToValueAtTime(next.lpHz, ramp);
      s.distGain.gain.cancelScheduledValues(t);
      s.distGain.gain.setValueAtTime(s.distGain.gain.value, t);
      s.distGain.gain.linearRampToValueAtTime(next.gain, ramp);
      voice.distancePenalty = next.distancePenalty;
    }
  }

  setMasterVolume(v) {
    v = clamp01(v);
    this.volume.master = v;
    lsWrite("audio.master", v);
    if (this.master && this.ctx && !document.hidden) {
      const now = this.ctx.currentTime;
      this.master.gain.cancelScheduledValues(now);
      this.master.gain.setValueAtTime(v, now);
    }
  }

  setCategoryVolume(cat, v) {
    if (!CATEGORY_SET.has(cat)) return;
    v = clamp01(v);
    this.volume[cat] = v;
    lsWrite(`audio.cat.${cat}`, v);
    const g = this.gains[cat];
    if (g && this.ctx) {
      const now = this.ctx.currentTime;
      g.gain.cancelScheduledValues(now);
      g.gain.setValueAtTime(this._categoryGainTarget(cat, this.alertDuckDepth > 0), now);
    }
  }

  getMasterVolume() {
    return this.volume.master;
  }

  getCategoryVolume(cat) {
    return this.volume[cat];
  }

  destroy() {
    document.removeEventListener("visibilitychange", this._visibility);
    for (const ev of this._gestureEvents) {
      window.removeEventListener(ev, this._gesture, true);
    }
    for (const v of [...this.voices]) this._stopVoice(v);
    this.buffers.clear();
    this.pending.clear();
    this.lastPlay.clear();
    this._unlockListeners.clear();
    this._queuedManifests = [];
    this._decodedQueued = false;
    if (this.ctx) {
      try {
        this.ctx.close();
      } catch {
        /* ignore */
      }
      this.ctx = null;
    }
    this.master = null;
    this.gains = {};
    this.alertDuckDepth = 0;
  }

  // --- Internals ------------------------------------------------------------

  _gesture(ev) {
    void this.unlockFromGesture(ev);
  }

  _ensureContext() {
    if (this.ctx) return this.ctx;
    const Ctor = window.AudioContext || window.webkitAudioContext;
    if (!Ctor) return null;
    let ctx;
    try {
      ctx = new Ctor();
    } catch {
      return null;
    }
    this.ctx = ctx;
    this.master = ctx.createGain();
    this.master.gain.value = this.volume.master;
    this.master.connect(ctx.destination);
    for (const c of CATEGORIES) {
      const g = ctx.createGain();
      g.gain.value = this.volume[c];
      g.connect(this.master);
      this.gains[c] = g;
    }
    return ctx;
  }

  _markUnlocked() {
    for (const ev of this._gestureEvents) {
      window.removeEventListener(ev, this._gesture, true);
    }
    if (this._decodedQueued) return;
    this._decodedQueued = true;
    const queued = this._queuedManifests;
    this._queuedManifests = [];
    for (const m of queued) {
      void this._decodeManifest(m);
    }
    for (const fn of [...this._unlockListeners]) {
      try {
        fn();
      } catch {
        /* listeners are UI niceties; never break audio startup */
      }
    }
  }

  _visibility() {
    if (!this.master || !this.ctx) return;
    const now = this.ctx.currentTime;
    const g = this.master.gain;
    g.cancelScheduledValues(now);
    g.setValueAtTime(g.value, now);
    const target = document.hidden ? 0 : this.volume.master;
    g.linearRampToValueAtTime(target, now + VISIBILITY_RAMP_S);
  }

  /**
   * Compute spatial parameters through the shared profile helper.
   * @param {number} x emitter world x
   * @param {number} y emitter world y
   * @param {string} [category] voice category
   * @returns {{gain:number, pan:number, lpHz:number,distance:number,distancePenalty:number}|null}
   */
  _computeSpatial(x, y, category) {
    return computeSpatialAudio(this.listener, x, y, category);
  }

  _dedupKey(id, category, opts, spatial) {
    if (typeof opts.dedupKey === "string" && opts.dedupKey) return `${id}:${opts.dedupKey}`;
    if (category === "combat_self" || category === "combat_other") {
      const refDist = Math.max(1, this.listener.refDist || DEFAULT_AUDIO_REF_DIST);
      const bucketPx = Math.max(160, refDist / 3);
      const bucket = spatial ? Math.floor(spatial.distance / bucketPx) : 0;
      return `${id}:${category}:d${bucket}`;
    }
    return `${id}:${category}`;
  }

  _cooldownMs(id, category, opts, buf) {
    if (typeof opts.cooldownMs === "number" && opts.cooldownMs >= 0) return opts.cooldownMs;
    if (SPOKEN_CATEGORIES.has(category)) {
      return Math.max(buf.duration * 1000 || 0, SPOKEN_MIN_COOLDOWN_MS);
    }
    return DEDUP_MS;
  }

  async _decodeManifest(manifest) {
    const jobs = [];
    for (const entry of manifest) {
      if (!entry || !entry.id || !entry.url) continue;
      if (this.buffers.has(entry.id) || this.pending.has(entry.id)) continue;
      const p = this._fetchAndDecode(entry.url).then(
        (buf) => {
          this.buffers.set(entry.id, buf);
          this.pending.delete(entry.id);
          return buf;
        },
        (err) => {
          this.pending.delete(entry.id);
          console.warn(`audio: decode failed for ${entry.id}`, err);
          return null;
        },
      );
      this.pending.set(entry.id, p);
      jobs.push(p);
    }
    await Promise.all(jobs);
  }

  async _fetchAndDecode(url) {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
    const data = await res.arrayBuffer();
    // Safari historically supported only the callback form of decodeAudioData.
    // The promise form has been the standard since Safari 14.1; both code paths
    // are kept so we degrade gracefully on older builds.
    return await new Promise((resolve, reject) => {
      const maybePromise = this.ctx.decodeAudioData(data, resolve, reject);
      if (maybePromise && typeof maybePromise.then === "function") {
        maybePromise.then(resolve, reject);
      }
    });
  }

  _stopVoice(voice) {
    try {
      voice.node.stop();
    } catch {
      /* already stopped */
    }
    this._finishVoice(voice);
  }

  _fadeOutVoice(voice, fadeOutMs) {
    if (!this.ctx || !voice.gainNode) {
      this._stopVoice(voice);
      return;
    }
    voice.stopping = true;
    const start = this.ctx.currentTime;
    const end = start + fadeOutMs / 1000;
    const gain = voice.gainNode.gain;
    gain.cancelScheduledValues(start);
    gain.setValueAtTime(gain.value, start);
    gain.linearRampToValueAtTime(0, end);
    try {
      voice.node.stop(end);
    } catch {
      this._finishVoice(voice);
    }
  }

  _finishVoice(voice) {
    const i = this.voices.indexOf(voice);
    if (i < 0) return;
    this.voices.splice(i, 1);
    voice.node.onended = null;
    for (const n of voice.trail || []) {
      try { n.disconnect(); } catch { /* already disconnected */ }
    }
    if (voice.ducking) this._releaseAlertDuck();
  }

  /**
   * Try to evict the worst-scoring voice. If even
   * the worst voice outranks the incoming call, the new sound is dropped.
   * @returns {boolean} true if a slot was freed
   */
  _evictLowest(incomingScore, now) {
    if (this.voices.length === 0) return true;
    let worstIdx = -1;
    let worstScore = Infinity;
    for (let i = 0; i < this.voices.length; i++) {
      const v = this.voices[i];
      const score = this._voiceScore(v, now);
      if (score < worstScore) {
        worstScore = score;
        worstIdx = i;
      }
    }
    if (worstIdx < 0) return false;
    if (worstScore >= incomingScore) return false;
    this._stopVoice(this.voices[worstIdx]);
    return true;
  }

  _voiceScore(voice, now) {
    return this._score(
      voice.category,
      voice.startedAt,
      now,
      voice.distancePenalty || 0,
      voice.priorityBonus || 0,
    );
  }

  _score(category, startedAt, now, distancePenalty, priorityBonus) {
    const ageS = Math.max(0, (now - startedAt) / 1000);
    return (
      (BASE_PRIORITY[category] || 0) +
      (STICKY_BONUS[category] || 0) +
      priorityBonus -
      ageS -
      distancePenalty
    );
  }

  _beginAlertDuck() {
    this.alertDuckDepth++;
    this._rampDuckedBuses(true);
  }

  _releaseAlertDuck() {
    if (this.alertDuckDepth > 0) this.alertDuckDepth--;
    if (this.alertDuckDepth === 0) this._rampDuckedBuses(false);
  }

  _rampDuckedBuses(duck) {
    if (!this.ctx) return;
    const now = this.ctx.currentTime;
    const ramp = duck ? DUCK_IN_S : DUCK_OUT_S;
    const targets = {
      ambient: this._categoryGainTarget("ambient", duck),
      combat_self: this._categoryGainTarget("combat_self", duck),
      combat_other: this._categoryGainTarget("combat_other", duck),
    };
    for (const [cat, target] of Object.entries(targets)) {
      const g = this.gains[cat];
      if (!g) continue;
      g.gain.cancelScheduledValues(now);
      g.gain.setValueAtTime(g.gain.value, now);
      g.gain.linearRampToValueAtTime(target, now + ramp);
    }
  }

  _categoryGainTarget(cat, duck) {
    const base = this.volume[cat];
    if (!duck) return base;
    if (cat === "ambient") return base * dbToGain(DB_ALERT_AMBIENT);
    if (cat === "combat_self" || cat === "combat_other") {
      return base * dbToGain(DB_ALERT_COMBAT);
    }
    return base;
  }
}

export { SOUND_MANIFEST } from "./sound_manifest.js";

/**
 * Pick a notice sound id from a server `Notice` message text, or null when
 * there is no matching voice line.
 */
export function noticeSoundId(msg) {
  const m = (msg || "").toLowerCase();
  if (m.includes("under_attack") || m.includes("under attack")) return "notice_under_attack";
  if (m.includes("supply") || m.includes("depot")) return "notice_supply";
  if (m.includes("steel")) return "notice_steel";
  if (m.includes("oil")) return "notice_oil";
  if (m.includes("cannot build") || m.includes("can't build") || m.includes("blocked")) {
    return "notice_cannot_build";
  }
  if (m.includes("out of range") || m.includes("too far")) return "notice_out_of_range";
  return null;
}
